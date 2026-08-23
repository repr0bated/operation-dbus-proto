//! Mutation Engine - The Authoritative Source for State and Schema DNA
//!
//! The Mutation Engine is the central coordinator that:
//! - Authoritatively routes all mutations (gRPC and D-Bus)
//! - Ensures all state changes are strictly recorded in the Event Chain (Audit Log)
//! - Broadcasts authoritative state changes to gRPC subscribers
//! - Directly manages authoritative RCP stores (OVSDB, NonNet, SQLite)

use anyhow::Context as _;
use async_trait::async_trait;
use serde_json;
use simd_json::prelude::{ValueAsContainer, ValueAsMutContainer, ValueAsScalar, ValueObjectAccess};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, OnceCell, RwLock, Semaphore};
use zbus::zvariant::OwnedValue as ZOwnedValue;
use zbus::{Connection, Proxy};

use base64::Engine;
use op_blockchain::{PluginFootprint, StreamingBlockchain};
use op_identity::{read_sled, write_sled_advance};
use op_llm::chat::ChatManager;
use op_network::rovs_proxy::OvsdbDbusClient;
use op_plugins::state_plugins::blockchain_plugin::{
    AuditEventRecord, QueryEventsInput, QueryEventsOutput, VerifyChainInput, VerifyChainOutput,
};
use op_state_store::{ChainEvent, Decision, EventChain, MemoryStore, OperationType, StateStore};

/// Default on-disk location of the streaming blockchain that backs the durable
/// audit trail, when `$OPDBUS_BLOCKCHAIN_PATH` is unset. Matches
/// `blockchain_plugin::DEFAULT_BASE_PATH` so both read the same chain.
const DEFAULT_BLOCKCHAIN_PATH: &str = "/var/lib/opdbus/blockchain";

/// A state change projected from the authoritative system bus
#[derive(Debug, Clone)]
pub struct StateChange {
    pub change_id: String,
    pub event_id: u64,
    pub plugin_id: String,
    pub object_path: String,
    pub change_type: ChangeType,
    pub member_name: Option<String>,
    pub old_value: Option<simd_json::OwnedValue>,
    pub new_value: simd_json::OwnedValue,
    pub tags_touched: Vec<String>,
    pub event_hash: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub actor_id: String,
    pub source: ChangeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    PropertySet,
    PropertyDelete,
    MethodCall,
    Signal,
    ObjectAdded,
    ObjectRemoved,
    SchemaMigration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeSource {
    DBus,
    Grpc,
    Internal,
}

pub struct MutationEngine {
    /// Authoritative Event Chain
    pub event_chain: Arc<RwLock<EventChain>>,
    /// Real-time change projection channel
    change_tx: broadcast::Sender<StateChange>,
    /// State cache for instant gRPC retrieval
    state_cache: Arc<RwLock<HashMap<String, simd_json::OwnedValue>>>,
    /// System D-Bus connection authority
    pub dbus_connection: Arc<OnceCell<Connection>>,
    /// Session bus connection for projection tree introspection
    session_bus: Arc<OnceCell<Connection>>,
    /// Session bus connection with registered PluginV1 interfaces for signal emission.
    /// Set after SchemaRouter registers objects (server.rs spawned task).
    signal_bus: Arc<OnceCell<Connection>>,
    /// Resource limiter for D-Bus operations
    #[allow(dead_code)]
    dbus_call_limiter: Arc<Semaphore>,

    /// Durable audit sink: the streaming blockchain's `timing_subvol` holds one
    /// JSON record per event chain event, so the trail survives a restart.
    /// Empty until [`MutationEngine::init_audit_durability`] runs; a missing
    /// sink degrades to RAM-only recording rather than failing dispatches.
    audit_sink: Arc<OnceCell<Arc<StreamingBlockchain>>>,

    /// Authoritative RCP stores
    pub ovsdb: Arc<OvsdbDbusClient>,
    /// In-process plugin handles for MethodCall dispatch (e.g. createunixsocket).
    pub unix_socket: Arc<op_plugins::state_plugins::UnixSocketPlugin>,
    /// Provider runtime used only after ZeroClaw resolves a schema-declared route.
    chat_manager: Arc<ChatManager>,
    /// Verified session identities, keyed by session_id. Projection of the
    /// session records for the mutation path — not a second store.
    sessions: Arc<RwLock<HashMap<String, SessionContext>>>,
}

impl std::fmt::Debug for MutationEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MutationEngine").finish()
    }
}

#[async_trait]
impl op_core::state_publisher::StatePublisher for MutationEngine {
    async fn publish_change(
        &self,
        plugin_id: String,
        path: String,
        change_type: op_core::state_publisher::ChangeType,
        property: Option<String>,
        old_value: Option<simd_json::OwnedValue>,
        new_value: simd_json::OwnedValue,
        tags: Vec<String>,
        source: String,
    ) -> anyhow::Result<()> {
        let internal_type = match change_type {
            op_core::state_publisher::ChangeType::PropertySet => ChangeType::PropertySet,
            op_core::state_publisher::ChangeType::Signal => ChangeType::Signal,
            op_core::state_publisher::ChangeType::Deleted => ChangeType::ObjectRemoved,
        };

        self.process_authoritative_change(
            plugin_id,
            path,
            internal_type,
            property,
            old_value,
            new_value,
            tags,
            source,
            None,
            ChangeSource::Internal,
        )
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!(e))
    }
}

/// Session genesis stamp used by identity_sled_dispatch.
#[derive(Debug, Clone, Default)]
pub struct GenesisStamp {
    pub session_id: String,
    pub wireguard_pubkey: String,
    pub genesis_hex: String,
    pub arrival_timestamp: i64,
    pub chain_head_at_arrival: String,
    pub catalog_hash_at_arrival: String,
    pub head_timestamp_at_arrival: i64,
}

/// Verified session identity stamped on the mutation path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionContext {
    pub genesis_hex: String,
    pub session_id: String,
    pub wireguard_pubkey: String,
}

const ANONYMOUS_ACTOR: &str = "anonymous";

impl MutationEngine {
    pub async fn session_context(&self, session_id: &str) -> Option<SessionContext> {
        self.sessions.read().await.get(session_id).cloned()
    }

    pub async fn session_context_for_actor(&self, actor_id: &str) -> Option<SessionContext> {
        if actor_id.is_empty() || actor_id == ANONYMOUS_ACTOR {
            return None;
        }
        {
            let sessions = self.sessions.read().await;
            if let Some(found) = sessions.get(actor_id) {
                return Some(found.clone());
            }
            if let Some(found) = sessions
                .values()
                .find(|ctx| ctx.wireguard_pubkey == actor_id || ctx.genesis_hex == actor_id)
            {
                return Some(found.clone());
            }
        }
        let record =
            crate::identity_sled_dispatch::session_record_for_actor(self, actor_id).await?;
        let genesis_hex = record.genesis.clone().filter(|g| !g.is_empty())?;
        let context = SessionContext {
            genesis_hex,
            session_id: record.session_id,
            wireguard_pubkey: record.wireguard_pubkey,
        };
        self.register_session_context(context.clone()).await;
        Some(context)
    }

    pub async fn ensure_session_context(&self, actor_id: &str) -> Option<SessionContext> {
        if actor_id.is_empty() || actor_id == ANONYMOUS_ACTOR {
            return None;
        }
        if let Some(found) = self.session_context_for_actor(actor_id).await {
            return Some(found);
        }
        let record =
            crate::identity_sled_dispatch::session_record_for_actor(self, actor_id).await?;
        match self
            .mint_and_store_genesis(&record.session_id, &record.wireguard_pubkey)
            .await
        {
            Ok(_) => self.session_context(&record.session_id).await,
            Err(error) => {
                tracing::warn!(
                    %error,
                    session_id = %record.session_id,
                    "session arrival could not be anchored"
                );
                None
            }
        }
    }

    pub async fn register_session_context(&self, context: SessionContext) {
        self.sessions
            .write()
            .await
            .insert(context.session_id.clone(), context);
    }

    pub async fn forget_session_context(&self, session_id: &str) {
        self.sessions.write().await.remove(session_id);
    }

    /// Mint this session's genesis once at arrival (FR-1).
    pub async fn mint_and_store_genesis(
        &self,
        session_id: &str,
        wireguard_pubkey: &str,
    ) -> anyhow::Result<String> {
        if session_id.is_empty() {
            anyhow::bail!("session genesis requires a session_id");
        }
        if let Some(existing) =
            crate::identity_sled_dispatch::stored_genesis(self, session_id).await
        {
            self.register_session_context(SessionContext {
                genesis_hex: existing.clone(),
                session_id: session_id.to_string(),
                wireguard_pubkey: wireguard_pubkey.to_string(),
            })
            .await;
            return Ok(existing);
        }

        let pubkey_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            wireguard_pubkey.trim(),
        )
        .ok()
        .and_then(|raw| <[u8; 32]>::try_from(raw).ok())
        .ok_or_else(|| {
            anyhow::anyhow!("session genesis requires a 32-byte base64 WireGuard pubkey")
        })?;

        let (chain_head_hex, head_timestamp) = {
            let chain = self.event_chain.read().await;
            let head_ts = chain
                .events()
                .last()
                .map(|event| event.timestamp.timestamp())
                .unwrap_or(0);
            (chain.last_hash().to_string(), head_ts)
        };
        let chain_head_bytes = hex::decode(&chain_head_hex)
            .ok()
            .and_then(|raw| <[u8; 32]>::try_from(raw).ok())
            .unwrap_or([0u8; 32]);
        let catalog_hash_bytes =
            op_identity::schema_bridge::schema_catalog_hash().unwrap_or_else(|| {
                tracing::warn!(
                    session_id,
                    "no published catalog hash at arrival; the anchor binds zeros"
                );
                [0u8; 32]
            });
        let arrival_timestamp = chrono::Utc::now().timestamp();

        let genesis = op_identity::session_genesis::mint_genesis(
            &pubkey_bytes,
            &chain_head_bytes,
            head_timestamp,
            &catalog_hash_bytes,
            arrival_timestamp,
        );

        let stamp = GenesisStamp {
            session_id: session_id.to_string(),
            wireguard_pubkey: wireguard_pubkey.to_string(),
            genesis_hex: hex::encode(genesis),
            arrival_timestamp,
            chain_head_at_arrival: chain_head_hex,
            catalog_hash_at_arrival: hex::encode(catalog_hash_bytes),
            head_timestamp_at_arrival: head_timestamp,
        };

        let stored = crate::identity_sled_dispatch::store_genesis(self, &stamp).await?;
        self.register_session_context(SessionContext {
            genesis_hex: stored.clone(),
            session_id: session_id.to_string(),
            wireguard_pubkey: wireguard_pubkey.to_string(),
        })
        .await;
        self.record_session_arrival(session_id).await;
        Ok(stored)
    }

    async fn record_session_arrival(&self, session_id: &str) {
        let args = serde_json::json!({ "session_id": session_id }).to_string();
        let event = {
            let mut chain = self.event_chain.write().await;
            chain
                .record_method_call(
                    session_id.to_string(),
                    "identity_sled".to_string(),
                    "session_arrival".to_string(),
                    Some("identity_sled.write".to_string()),
                    &args,
                )
                .clone()
        };
        self.persist_audit_event(&event).await;
    }

    /// Create a new authoritative Mutation Engine
    pub fn new(event_chain: Arc<RwLock<EventChain>>, ovsdb: Arc<OvsdbDbusClient>) -> Self {
        let (change_tx, _) = broadcast::channel(1024);
        Self {
            event_chain,
            change_tx,
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            dbus_connection: Arc::new(OnceCell::new()),
            session_bus: Arc::new(OnceCell::new()),
            signal_bus: Arc::new(OnceCell::new()),
            dbus_call_limiter: Arc::new(Semaphore::new(32)),
            audit_sink: Arc::new(OnceCell::new()),
            ovsdb,
            unix_socket: Arc::new(op_plugins::state_plugins::UnixSocketPlugin::new()),
            chat_manager: Arc::new(ChatManager::new()),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Open the durable audit sink and rebuild the in-memory event chain from it.
    ///
    /// Called once at startup (before the gRPC/D-Bus surfaces accept traffic) so
    /// that a query issued immediately after a restart sees the pre-restart
    /// history. Idempotent: the sink is behind a `OnceCell`, and the rebuild is
    /// skipped when the chain already holds events.
    ///
    /// Returns the number of events replayed from disk. A sink that cannot be
    /// opened (no Btrfs, no permission, path absent) is not an error: the engine
    /// keeps recording to RAM and logs at `warn!`.
    ///
    /// OSCAL subid: `src.service.event-chain.rebuild@v1`
    pub async fn init_audit_durability(&self) -> usize {
        let base_path = std::env::var("OPDBUS_BLOCKCHAIN_PATH")
            .unwrap_or_else(|_| DEFAULT_BLOCKCHAIN_PATH.to_string());
        self.init_audit_durability_at(&base_path).await
    }

    /// [`init_audit_durability`](Self::init_audit_durability) against an explicit
    /// chain path, bypassing `$OPDBUS_BLOCKCHAIN_PATH`.
    pub async fn init_audit_durability_at(&self, base_path: &str) -> usize {
        if self.audit_sink.get().is_some() {
            return 0;
        }

        let chain_store = match StreamingBlockchain::new(base_path).await {
            Ok(chain) => Arc::new(chain),
            Err(error) => {
                tracing::warn!(
                    %error,
                    path = %base_path,
                    "durable audit sink unavailable; event chain stays in memory only"
                );
                return 0;
            }
        };

        let timing_dir = std::path::Path::new(base_path).join("timing");
        let replayed = self.rebuild_chain_from_disk(&timing_dir).await;
        let _ = self.audit_sink.set(chain_store);
        replayed
    }

    /// Replay persisted audit events from the `timing_subvol` into the chain.
    ///
    /// Records are sorted by their persisted `event_id` so hash linkage is
    /// restored in the original order. Malformed or non-audit files (the
    /// timing directory also holds footprints written by other producers) are
    /// skipped with a `warn!` rather than aborting the rebuild.
    async fn rebuild_chain_from_disk(&self, timing_dir: &std::path::Path) -> usize {
        {
            let chain = self.event_chain.read().await;
            if !chain.events().is_empty() {
                tracing::warn!(
                    events = chain.events().len(),
                    "event chain already populated; skipping disk rebuild"
                );
                return 0;
            }
        }

        let mut dir = match tokio::fs::read_dir(timing_dir).await {
            Ok(dir) => dir,
            Err(error) => {
                tracing::info!(
                    %error,
                    path = %timing_dir.display(),
                    "no audit trail on disk yet; starting with an empty chain"
                );
                return 0;
            }
        };

        let max_replay: usize = std::env::var("OPDBUS_AUDIT_REPLAY_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10_000);

        let mut candidate_paths = Vec::new();
        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let block_num = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(op_blockchain::parse_block_number);
            candidate_paths.push((block_num, path));
        }

        candidate_paths.sort_by(|(a_num, a_path), (b_num, b_path)| match (a_num, b_num) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (None, None) => a_path.cmp(b_path),
        });

        let total_candidates = candidate_paths.len();
        let target_paths = if total_candidates > max_replay {
            &candidate_paths[total_candidates - max_replay..]
        } else {
            &candidate_paths[..]
        };

        // (event_id, event_json) — sorted before replay so linkage is exact.
        let mut records: Vec<(u64, serde_json::Value)> = Vec::new();
        let mut skipped = 0usize;

        for (_, path) in target_paths {
            let bytes = match tokio::fs::read(path).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "unreadable audit record");
                    skipped += 1;
                    continue;
                }
            };
            let value: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "malformed audit record");
                    skipped += 1;
                    continue;
                }
            };
            // Only records carrying an embedded ChainEvent are replayable;
            // other footprints in the timing directory are ignored.
            let Some(event) = value
                .get("data")
                .and_then(|d| d.get("metadata"))
                .and_then(|m| m.get("audit_event"))
            else {
                continue;
            };
            let Some(event_id) = event.get("event_id").and_then(|v| v.as_u64()) else {
                tracing::warn!(path = %path.display(), "audit record has no event_id");
                skipped += 1;
                continue;
            };
            records.push((event_id, value));
        }

        records.sort_by_key(|(event_id, _)| *event_id);

        let mut replayed = 0usize;
        {
            let mut chain = self.event_chain.write().await;
            for (event_id, value) in &records {
                match chain.replay_from_footprint(value) {
                    Ok(_) => replayed += 1,
                    Err(error) => {
                        tracing::warn!(%error, event_id, "audit record rejected during replay");
                        skipped += 1;
                    }
                }
            }
        }

        if replayed > 0 || skipped > 0 {
            tracing::info!(
                replayed,
                skipped,
                "event chain rebuilt from durable audit trail"
            );
        }
        replayed
    }

    /// Persist one chain event to the durable audit sink.
    ///
    /// Runs inline on the dispatch path (NFR-4: not deferred to a background
    /// task). A write failure is logged and swallowed — the event is already in
    /// the in-memory chain, and losing durability must not fail the caller's
    /// method call.
    ///
    /// OSCAL subid: `evt.service.event-chain.persist@v1`
    async fn persist_audit_event(&self, event: &ChainEvent) {
        let Some(sink) = self.audit_sink.get() else {
            return;
        };
        if let Err(error) = sink.add_footprint(event_to_footprint(event)).await {
            tracing::warn!(
                %error,
                event_id = event.event_id,
                "audit durability write failed; event retained in memory only"
            );
        }
    }

    /// Store the session bus connection used for `Updated` signal emission.
    /// Called from server.rs after SchemaRouter registers its interfaces.
    pub fn set_signal_bus(&self, conn: Connection) {
        let _ = self.signal_bus.set(conn);
    }

    /// Top-level keys of a whole-state value, for batch `Updated` payloads.
    fn state_keys_owned(value: &simd_json::OwnedValue) -> Option<Vec<String>> {
        value
            .as_object()
            .map(|obj| obj.keys().map(|k| k.to_string()).collect())
    }

    /// Top-level keys of a serde_json whole-state value.
    fn state_keys_serde(value: &serde_json::Value) -> Option<Vec<String>> {
        value.as_object().map(|obj| obj.keys().cloned().collect())
    }

    /// Emit the `Updated` signal on `org.opdbus.v1.PluginV1` for a given plugin.
    ///
    /// `key` is the mutated member when a single key changed; `keys` lists the
    /// top-level subtrees affected by a whole-state write. The payload always
    /// identifies what changed (`{"plugin","key"}` or `{"plugin","keys"}`) so
    /// subscribers can act on it without a follow-up query (REQ-1.5).
    ///
    /// Best-effort: if the signal bus is not yet available (early boot) or the
    /// interface is not registered for this plugin, the write still succeeds.
    /// The signal is for reactivity; SHM is the source of truth.
    async fn emit_updated_signal(
        &self,
        plugin_id: &str,
        key: Option<&str>,
        keys: Option<Vec<String>>,
    ) {
        let conn = match self.signal_bus.get() {
            Some(c) => c,
            None => {
                tracing::debug!(
                    plugin_id,
                    "signal_bus not yet available; skipping Updated signal"
                );
                return;
            }
        };
        let path = format!("/org/opdbus/v1/plugins/{plugin_id}");
        let iface_ref = match conn
            .object_server()
            .interface::<_, crate::schema_router::SchemaBackedInterface>(&*path)
            .await
        {
            Ok(r) => r,
            Err(_) => {
                tracing::debug!(
                    plugin_id,
                    "PluginV1 interface not registered at path; skipping signal"
                );
                return;
            }
        };
        let payload = match (key, keys) {
            (Some(k), _) => serde_json::json!({"plugin": plugin_id, "key": k}),
            (None, Some(ks)) => serde_json::json!({"plugin": plugin_id, "keys": ks}),
            (None, None) => serde_json::json!({"plugin": plugin_id}),
        }
        .to_string();
        if let Err(e) = crate::schema_router::SchemaBackedInterface::updated(
            iface_ref.signal_emitter(),
            &payload,
        )
        .await
        {
            tracing::debug!(plugin_id, error = %e, "Failed to emit Updated signal");
        }
    }

    /// Seed missing plugin projection files and seal current schemas from the
    /// canonical PluginSchema catalog. This runs once during bridge startup.
    /// Existing state projections are preserved; stale schema blobs are
    /// replaced because their hash no longer describes the running code.
    ///
    /// A missing or schema-stale plugin in the SHM blob catalog is blobified
    /// and sealed here. The blob IS the reflected plugin contract.
    pub async fn seed_missing_plugin_projections(&self) -> anyhow::Result<usize> {
        let state_store: Arc<dyn StateStore> = Arc::new(MemoryStore::new());
        let registry = op_plugins::DefaultPluginRegistry::new(state_store);
        let plugins = registry.load_all_plugins().await?;
        let mut seeded = 0usize;
        let mut sealed = 0usize;
        let mut blob_store = match op_blob::BlobStore::open_default() {
            Ok(store) => Some(store),
            Err(error) => {
                tracing::warn!(%error, "cannot open SHM blob catalog; missing plugins will not be auto-sealed");
                None
            }
        };

        for plugin in plugins {
            let plugin_id = plugin.name().to_string();

            let Some(schema) = plugin.schema() else {
                tracing::debug!(
                    plugin_id = %plugin_id,
                    "Skipping projection seed for plugin without PluginSchema"
                );
                continue;
            };

            if schema.name != plugin_id {
                tracing::warn!(
                    plugin_id = %plugin_id,
                    schema_name = %schema.name,
                    "Skipping projection seed for schema owned by a different plugin"
                );
                continue;
            }

            // Auto-generate or refresh the sealed blob when the running
            // schema hash differs from the active SHM catalog.
            if let Some(store) = blob_store.as_mut() {
                let canonical_id = op_blob::canonical_plugin_id(&plugin_id);
                let blob = op_blob::blobify_plugin_schema(&plugin_id, schema.clone());
                let schema_changed = store
                    .manifest(&canonical_id)
                    .map(|manifest| manifest.schema_hash.as_str())
                    != Some(blob.manifest.schema_hash.as_str());
                if schema_changed {
                    match store.write(&blob) {
                        Ok(_) => {
                            tracing::info!(
                                plugin_id = %canonical_id,
                                schema_hash = %blob.manifest.schema_hash,
                                "sealed current plugin schema into SHM catalog"
                            );
                            sealed += 1;
                        }
                        Err(error) => {
                            tracing::warn!(plugin_id = %canonical_id, %error, "failed to seal current plugin schema");
                        }
                    }
                }
            }

            if let Some(mut bytes) = op_core::projection_shm::read_projection_bytes(&plugin_id) {
                match simd_json::to_owned_value(&mut bytes) {
                    Ok(projected_state) => {
                        self.update_state_cache(plugin_id.clone(), projected_state)
                            .await;
                    }
                    Err(error) => {
                        tracing::warn!(
                            plugin_id = %plugin_id,
                            %error,
                            "existing plugin projection is invalid JSON; leaving it authoritative but uncached"
                        );
                    }
                }
                continue;
            }

            let seed_state = op_plugins::projection_seed_state_from_schema(&schema);
            let json = simd_json::to_string(&seed_state)?;
            op_core::projection_shm::write_projection(&plugin_id, json.as_bytes())?;
            self.emit_updated_signal(&plugin_id, None, Self::state_keys_owned(&seed_state))
                .await;
            self.update_state_cache(plugin_id.clone(), seed_state).await;
            seeded += 1;
        }

        tracing::info!(
            projections = seeded,
            blobs = sealed,
            "Seeded missing plugin projections and sealed current plugin schemas"
        );
        Ok(seeded)
    }

    /// Authoritative D-Bus connection getter
    pub async fn dbus_connection(&self) -> anyhow::Result<Connection> {
        self.dbus_connection
            .get_or_try_init(|| async { Connection::system().await })
            .await
            .cloned()
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Get or create the session bus connection (for projection tree introspection)
    async fn session_bus(&self) -> anyhow::Result<Connection> {
        self.session_bus
            .get_or_try_init(|| async {
                let addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
                    .unwrap_or_else(|_| op_core::config::SESSION_BUS_ADDRESS.to_string());
                zbus::connection::Builder::address(addr.as_str())
                    .map_err(|e| anyhow::anyhow!("Builder::address: {}", e))?
                    .build()
                    .await
                    .map_err(|e| anyhow::anyhow!("Session bus connect: {}", e))
            })
            .await
            .cloned()
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Crawl the D-Bus projection tree for a plugin and return its full state
    /// as JSON. Introspects `/org/opdbus/v1/plugins/<plugin_id>`, recursively
    /// descends child nodes, and reads all properties from all interfaces.
    async fn crawl_plugin_dbus_tree(&self, plugin_id: &str) -> Option<simd_json::OwnedValue> {
        let conn = match self.session_bus().await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(error = %e, "session bus unavailable for crawl");
                return None;
            }
        };

        let root_path = format!("/org/opdbus/v1/plugins/{}", plugin_id);
        let bus_name = "org.opdbus.v1.plugins";

        let mut result = simd_json::value::owned::Object::new();
        Self::crawl_path(&conn, bus_name, &root_path, &mut result).await;
        if result.is_empty() {
            None
        } else {
            Some(simd_json::OwnedValue::Object(Box::new(result)))
        }
    }

    /// Recursively introspect a D-Bus path, reading properties and descending
    /// into child nodes.
    async fn crawl_path(
        conn: &Connection,
        bus_name: &str,
        path: &str,
        out: &mut simd_json::value::owned::Object,
    ) {
        // Introspect to discover interfaces and child nodes
        let introspect_xml: String = match conn
            .call_method(
                Some(bus_name),
                path,
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
        {
            Ok(msg) => msg.body().deserialize::<String>().unwrap_or_default(),
            Err(e) => {
                tracing::debug!(path = %path, error = %e, "introspect failed");
                return;
            }
        };

        // Parse the introspection XML for interfaces and child nodes
        let interfaces = Self::parse_interfaces(&introspect_xml);
        let child_nodes = Self::parse_child_nodes(&introspect_xml);

        // Read properties from each non-standard interface
        for iface in &interfaces {
            // Skip standard D-Bus interfaces
            if iface.starts_with("org.freedesktop.DBus.") {
                continue;
            }

            let props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> = match conn
                .call_method(
                    Some(bus_name),
                    path,
                    Some("org.freedesktop.DBus.Properties"),
                    "GetAll",
                    &iface,
                )
                .await
            {
                Ok(msg) => msg
                    .body()
                    .deserialize::<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>()
                    .unwrap_or_default(),
                Err(_) => std::collections::HashMap::new(),
            };

            let mut iface_obj = simd_json::value::owned::Object::new();
            for (prop_name, prop_val) in &props {
                // zvariant::OwnedValue serializes as {"signature":"s","value":"..."}
                // Extract just the value field for a clean JSON projection.
                let json_val: serde_json::Value =
                    serde_json::to_value(prop_val).unwrap_or(serde_json::Value::Null);
                let extracted = json_val.get("value").unwrap_or(&json_val);
                let json_str =
                    serde_json::to_string(extracted).unwrap_or_else(|_| "null".to_string());
                let mut bytes = json_str.into_bytes();
                if let Ok(v) = simd_json::to_owned_value(&mut bytes) {
                    iface_obj.insert(prop_name.clone(), v);
                }
            }
            if !iface_obj.is_empty() {
                out.insert(
                    iface.clone(),
                    simd_json::OwnedValue::Object(Box::new(iface_obj)),
                );
            }
        }

        // Recurse into child nodes
        for child in &child_nodes {
            let child_path = format!("{}/{}", path.trim_end_matches('/'), child);
            let mut child_obj = simd_json::value::owned::Object::new();
            Box::pin(Self::crawl_path(
                conn,
                bus_name,
                &child_path,
                &mut child_obj,
            ))
            .await;
            if !child_obj.is_empty() {
                out.insert(
                    child.clone(),
                    simd_json::OwnedValue::Object(Box::new(child_obj)),
                );
            }
        }
    }

    /// Parse interface names from D-Bus introspection XML
    fn parse_interfaces(xml: &str) -> Vec<String> {
        let mut interfaces = Vec::new();
        for line in xml.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("<interface name=\"") {
                if let Some(end) = rest.find("\">") {
                    interfaces.push(rest[..end].to_string());
                }
            }
        }
        interfaces
    }

    /// Parse child node names from D-Bus introspection XML
    fn parse_child_nodes(xml: &str) -> Vec<String> {
        let mut nodes = Vec::new();
        for line in xml.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("<node name=\"") {
                if let Some(end) = rest.find("\"") {
                    nodes.push(rest[..end].to_string());
                }
            }
        }
        nodes
    }

    fn compute_tags(&self, plugin_id: &str, object_path: &str) -> Vec<String> {
        let mut tags = Vec::new();
        if plugin_id == "net" || object_path.contains("/ovsdb/") {
            tags.push("network".to_string());
            tags.push("ovsdb".to_string());
        } else {
            tags.push("state".to_string());
            tags.push(plugin_id.to_string());
        }
        tags
    }

    /// Process a change that has already happened in an authoritative store.
    /// This records the change in the event chain and broadcasts it to gRPC.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_authoritative_change(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        old_value: Option<simd_json::OwnedValue>,
        new_value: simd_json::OwnedValue,
        mut tags: Vec<String>,
        actor_id: String,
        capability_id: Option<String>,
        source: ChangeSource,
    ) -> Result<StateChange, String> {
        if tags.is_empty() {
            tags = self.compute_tags(&plugin_id, &object_path);
        }

        let event = {
            let mut chain = self.event_chain.write().await;
            let op = match change_type {
                ChangeType::PropertySet => OperationType::PropertySet,
                ChangeType::ObjectRemoved => OperationType::Custom("delete".to_string()),
                _ => OperationType::EmitSignal,
            };
            if let Some(capability) = capability_id {
                let event = ChainEvent::new(
                    chain.next_event_id(),
                    chain.last_hash().to_string(),
                    actor_id.clone(),
                    plugin_id.clone(),
                    "1.0.0".to_string(),
                    op,
                    object_path.clone(),
                    tags.clone(),
                    Decision::Allow,
                    &new_value,
                )
                .with_capability(capability);
                chain.append(event).clone()
            } else {
                let event = chain.record(
                    actor_id.clone(),
                    plugin_id.clone(),
                    "1.0.0".to_string(),
                    op,
                    object_path.clone(),
                    tags.clone(),
                    Decision::Allow,
                    &new_value,
                );
                event.clone()
            }
        };

        // Durability: mirror the event into the timing_subvol so it survives a
        // restart. Inline with the change, never deferred.
        self.persist_audit_event(&event).await;

        self.update_cached_plugin_state(
            &plugin_id,
            &object_path,
            change_type,
            member_name.as_deref(),
            &new_value,
        )
        .await;

        // Write the plugin's present state verbatim to the shm static tree.
        //
        // The projection IS the state — `/dev/shm/opdbus/state/<plugin>.json`
        // is the single source of truth that the D-Bus tree (schema_router)
        // and op-web's state_tree read from. The `{"data","_introspection"}`
        // composite existed only for the deleted projection server's child-path
        // derivation; readers expect the raw state object.
        if change_type != ChangeType::ObjectRemoved {
            match simd_json::to_string(&new_value) {
                Ok(json) => {
                    if let Err(e) =
                        op_core::projection_shm::write_projection(&plugin_id, json.as_bytes())
                    {
                        tracing::warn!(plugin_id = %plugin_id, error = %e, "Failed to write shm projection");
                    } else {
                        let keys = if member_name.is_some() {
                            None
                        } else {
                            Self::state_keys_owned(&new_value)
                        };
                        self.emit_updated_signal(&plugin_id, member_name.as_deref(), keys)
                            .await;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        plugin_id = %plugin_id,
                        error = %e,
                        "Failed to serialize plugin state for shm projection"
                    );
                }
            }
        }

        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: event.event_id,
            plugin_id,
            object_path,
            change_type,
            member_name,
            old_value,
            new_value,
            tags_touched: tags,
            event_hash: event.event_hash.clone(),
            timestamp: event.timestamp,
            actor_id,
            source,
        };

        let _ = self.change_tx.send(change.clone());
        Ok(change)
    }

    /// Start the Mutation Engine background tasks.
    /// Subscribes to authoritative RCP stores and broadcasts changes.
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let me = self.clone();

        // Subscribe to OVSDB updates
        let ovsdb_self = me.clone();
        tokio::spawn(async move {
            if let Ok(mut rx) = ovsdb_self.ovsdb.monitor_db("Open_vSwitch").await {
                loop {
                    match rx.recv().await {
                        Ok(update) => {
                            if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
                                if params.len() >= 3 {
                                    if let Some(tables) = params[2].as_object() {
                                        for (table_name, table_update) in tables.iter() {
                                            let table_name_owned: String = table_name.to_string();
                                            // monitor_db returns serde_json::Value; convert to
                                            // simd_json::OwnedValue required by process_authoritative_change.
                                            let simd_val: simd_json::OwnedValue = {
                                                match serde_json::to_string(table_update)
                                                    .ok()
                                                    .and_then(|s| {
                                                        let mut b = s.into_bytes();
                                                        simd_json::to_owned_value(&mut b).ok()
                                                    }) {
                                                    Some(v) => v,
                                                    None => continue,
                                                }
                                            };
                                            let _ = ovsdb_self
                                                .process_authoritative_change(
                                                    "net".to_string(),
                                                    format!(
                                                        "/org/opdbus/v1/ovsdb/{}",
                                                        table_name_owned
                                                    ),
                                                    ChangeType::PropertySet,
                                                    Some(table_name_owned),
                                                    None,
                                                    simd_val,
                                                    vec![
                                                        "ovsdb".to_string(),
                                                        "network".to_string(),
                                                    ],
                                                    "ovsdb-monitor".to_string(),
                                                    None,
                                                    ChangeSource::DBus,
                                                )
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("OVSDB subscription lagged by {} events", n);
                            continue;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        });

        Ok(())
    }

    /// Unified mutation entry point. Writes to authoritative RCP stores and
    /// triggers the event recording/broadcast pipeline.
    #[allow(clippy::too_many_arguments)]
    pub async fn mutate(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        capability_id: Option<String>,
    ) -> anyhow::Result<MutationResult> {
        let mut old_value = None;
        let mut authoritative_value = value.clone();
        let mut caller_result = None;
        // Empty falls back to compute_tags in process_authoritative_change;
        // the human_principal arm sets this to the method's subid so the
        // event carries it in tags_touched (VAL-CROSS-020).
        let mut event_tags: Vec<String> = Vec::new();

        // Resolve the acting identity from the Sled (/dev/shm/plugin_schema.dat)
        // when the caller omitted it. The Sled carries the WireGuard footprint +
        // trace_id — that identity is authoritative for every mutation.
        let actor_id = if actor_id.is_empty() {
            sled_footprint().unwrap_or_else(|| "anonymous".to_string())
        } else {
            actor_id
        };

        // 1. Write to authoritative RCP store
        if plugin_id == "net" || object_path.contains("/ovsdb/") {
            // OVSDB Authoritative Path
            if change_type == ChangeType::MethodCall {
                if let Some(method) = &member_name {
                    match method.as_str() {
                        "create_bridge" => {
                            if let Some(name) = value.as_str() {
                                self.ovsdb.create_bridge(name).await?;
                            }
                        }
                        "add_port" => {
                            if let Some(args) = value.as_array() {
                                if args.len() >= 2 {
                                    if let (Some(br), Some(port)) =
                                        (args[0].as_str(), args[1].as_str())
                                    {
                                        self.ovsdb.add_port(br, port).await?;
                                    }
                                }
                            }
                        }
                        _ => {
                            // Fallback to generic D-Bus call if it's a known service
                            let _ = self
                                .call_dbus_method(
                                    &format!("org.opdbus.{}.v1", plugin_id),
                                    &object_path,
                                    "org.opdbus.OvsdbV1",
                                    method,
                                    vec![value.clone()],
                                    &actor_id,
                                    &capability_id,
                                )
                                .await?;
                        }
                    }
                }
            } else if change_type == ChangeType::PropertySet {
                if let Some(prop) = &member_name {
                    // Extract bridge name from path if possible
                    // Path format: /org/opdbus/v1/ovsdb/Bridge/bridge_name
                    let parts: Vec<&str> = object_path.split('/').collect();
                    if parts.len() >= 6 && parts[4] == "Bridge" {
                        let br_name = parts[5].replace('_', "-");
                        if let Some(val_str) = value.as_str() {
                            self.ovsdb
                                .set_bridge_property(&br_name, prop, val_str)
                                .await?;
                        }
                    }
                }
            }
        } else if plugin_id == "unix_socket" && change_type == ChangeType::MethodCall {
            // Schema method is `bind` (MMID BindService). `createunixsocket` is
            // the legacy CallMethod name. Both register (name, ports) against
            // the shared container.sock; Bind.path is accepted and ignored.
            if let Some(method) = &member_name {
                if is_unix_socket_bind_method(method) {
                    let (name, ports) = parse_socket_args(&value);
                    let result = self
                        .unix_socket
                        .create_unix_socket(name.clone(), ports.clone());
                    if !result.success {
                        anyhow::bail!(result.errors.join("; "));
                    }
                    authoritative_value = unix_socket_state_after_registration(&name, &ports);
                }
            }
        } else if plugin_id == "identity_sled" && change_type == ChangeType::MethodCall {
            // PluginService.CallMethod wraps object arguments in a one-element
            // array. The identity dispatcher owns provisioning, durable Cozo
            // persistence, and the native Btrfs attach operation.
            if let Some(method) = &member_name {
                let mut args_json = serde_json::to_value(&value)?;
                if let serde_json::Value::Array(items) = &args_json {
                    args_json = items
                        .first()
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
                }
                let domain = crate::identity_sled_dispatch::dispatch_identity_sled_method(
                    self, method, &args_json,
                )
                .await?;
                if let Some(state) = self.get_state("identity_sled").await {
                    authoritative_value = state;
                }
                caller_result = Some(simd_json::serde::to_owned_value(&domain)?);
            }
        } else if plugin_id == "human_principal" && change_type == ChangeType::MethodCall {
            // PluginService.CallMethod wraps object arguments in a one-element
            // array (identity_sled convention above). The dispatcher owns Cozo
            // durability and registry policy; the authoritative present state
            // is re-read from Cozo after the mutation, and the event carries
            // the method's subid in tags_touched (VAL-CROSS-020).
            if let Some(method) = &member_name {
                let mut args_json = serde_json::to_value(&value)?;
                if let serde_json::Value::Array(items) = &args_json {
                    args_json = items
                        .first()
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
                }
                let domain = crate::human_principal_dispatch::dispatch_human_principal_method(
                    method, &args_json,
                )
                .await?;
                let state = crate::human_principal_dispatch::current_state().await;
                authoritative_value = simd_json::serde::to_owned_value(&state)?;
                caller_result = Some(simd_json::serde::to_owned_value(&domain)?);
                event_tags = crate::human_principal_dispatch::method_subid(method)
                    .into_iter()
                    .collect();
            }
        } else if plugin_id == "xray" && change_type == ChangeType::MethodCall {
            // PluginService.CallMethod wraps object arguments in a
            // one-element array, matching identity_sled's convention above.
            if let Some(method) = &member_name {
                let mut args_json = serde_json::to_value(&value)?;
                if let serde_json::Value::Array(items) = &args_json {
                    args_json = items
                        .first()
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
                }
                let result = dispatch_xray_method(method, &args_json).await?;
                caller_result = Some(simd_json::serde::to_owned_value(&result)?);
            }
        } else if plugin_id == "netmaker" && change_type == ChangeType::MethodCall {
            if let Some(method) = &member_name {
                let mut args_json = serde_json::to_value(&value)?;
                if let serde_json::Value::Array(items) = &args_json {
                    args_json = items
                        .first()
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
                }
                let result = op_plugins::state_plugins::netmaker::dispatch_netmaker_method(
                    method, &args_json,
                )
                .await?;
                caller_result = Some(simd_json::serde::to_owned_value(&result)?);
            }
        } else {
            // NonNet / Generic Plugin Path
            if change_type == ChangeType::PropertySet {
                // Get old value for the footprint before update from cache
                old_value = self.get_state(&plugin_id).await.and_then(|v| {
                    if let Some(prop) = &member_name {
                        v.get(prop).cloned()
                    } else {
                        Some(v)
                    }
                });
            }
        }

        // 2. Record and broadcast change
        let change = self
            .process_authoritative_change(
                plugin_id,
                object_path,
                change_type,
                member_name,
                old_value,
                authoritative_value.clone(),
                event_tags, // Empty falls back to compute_tags in process_authoritative_change
                actor_id,
                capability_id,
                ChangeSource::Grpc,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Write the Identity Sled with the updated mutation index.
        {
            let (existing_pubkey_b64, existing_trace_hex) = if let Ok((ptr, _mmap)) = read_sled() {
                unsafe {
                    let sled = &*ptr;
                    (
                        base64::engine::general_purpose::STANDARD.encode(sled.wireguard_pubkey),
                        sled.trace_id_hex(),
                    )
                }
            } else {
                (String::new(), String::new())
            };
            if let Err(e) =
                write_sled_advance(
                    &existing_pubkey_b64,
                    change.event_id,
                    &existing_trace_hex,
                    0,
                )
            {
                tracing::warn!("sled write after mutation failed: {}", e);
            }
        }

        Ok(MutationResult {
            success: true,
            event_id: change.event_id,
            event_hash: change.event_hash,
            result: caller_result.or(Some(authoritative_value)),
            error: None,
        })
    }

    /// Backward-compatible wrapper for gRPC Mutations.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_grpc_mutation(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        capability_id: Option<String>,
    ) -> anyhow::Result<MutationResult> {
        self.mutate(
            plugin_id,
            object_path,
            change_type,
            member_name,
            value,
            actor_id,
            capability_id,
        )
        .await
    }

    /// Dispatch a method call from the schema-backed bridge interface.
    ///
    /// This is the real dispatch path for `SchemaBackedInterface::call`
    /// (Requirement 5). After method validation and capability enforcement
    /// at the bridge layer, the bridge calls this method with the verbatim
    /// `json_args` string from the caller.
    ///
    /// The method:
    /// 1. Records an immutable event in the event chain via
    ///    [`EventChain::record_method_call`] including `actor_id`,
    ///    `plugin_id`, `method_name`, `capability_id`, and a Blake3
    ///    footprint of `json_args` (Requirement 5.5 / VAL-DISPATCH-005).
    /// 2. The append occurs **before** the call returns success
    ///    (NFR-003 / VAL-NFR-003).
    /// 3. Broadcasts the change to subscribers.
    /// 4. Returns the result as a `serde_json::Value`.
    ///
    /// Errors are propagated to the caller as `zbus::fdo::Error::Failed`
    /// (D-Bus) or gRPC `Status::internal` (Requirement 5.3 /
    /// VAL-DISPATCH-003) by the calling bridge interface.
    pub async fn dispatch_method_call(
        &self,
        plugin_id: &str,
        method: &str,
        json_args: &str,
        capability_id: Option<&str>,
        actor_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        // Test-only: ForceDispatchError triggers an error for VAL-DISPATCH-003 testing.
        // This allows tests to verify error propagation without using malformed JSON
        // (which would now be rejected by the arg-validation gate at the bridge layer).
        if method == "ForceDispatchError" {
            return Err(anyhow::anyhow!(
                "dispatch error: method explicitly triggered failure"
            ));
        }

        // Parse the verbatim json_args string. If parsing fails, propagate
        // the error — the bridge converts it to fdo::Error::Failed.
        let parsed_value: simd_json::OwnedValue = {
            let mut bytes = json_args.as_bytes().to_vec();
            simd_json::to_owned_value(&mut bytes)
                .map_err(|e| anyhow::anyhow!("invalid json_args for method '{}': {}", method, e))?
        };

        // Record the immutable event with the full accountability surface.
        // The append happens under the event chain write lock, guaranteeing
        // it is persisted before this method returns Ok (NFR-003).
        let event_summary = {
            let mut chain = self.event_chain.write().await;
            let event = chain.record_method_call(
                actor_id.to_string(),
                plugin_id.to_string(),
                method.to_string(),
                capability_id.map(|s| s.to_string()),
                json_args, // verbatim string for Blake3 footprint
            );
            (
                event.event_id,
                event.event_hash.clone(),
                event.timestamp,
                event.clone(),
            )
        };

        // Durability: write the event to the timing_subvol before returning, so
        // the audit trail survives a restart (FR-6). A failure here is logged
        // and does not fail the dispatch (NFR-4).
        self.persist_audit_event(&event_summary.3).await;

        // Broadcast the method-call change to gRPC subscribers.
        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: event_summary.0,
            plugin_id: plugin_id.to_string(),
            object_path: format!("/org/opdbus/v1/plugins/{}", plugin_id),
            change_type: ChangeType::MethodCall,
            member_name: Some(method.to_string()),
            old_value: None,
            new_value: parsed_value.clone(),
            tags_touched: vec![],
            event_hash: event_summary.1.clone(),
            timestamp: event_summary.2,
            actor_id: actor_id.to_string(),
            source: ChangeSource::DBus,
        };
        let _ = self.change_tx.send(change.clone());

        // Dispatch to appropriate backend based on plugin_id
        let method_result: serde_json::Value = match plugin_id {
            "rovs_commands" => {
                dispatch_rovs_commands_method(&self.ovsdb, method, &parsed_value).await?
            }
            "ovsdb_bridge" => {
                dispatch_ovsdb_bridge_method(&self.ovsdb, method, &parsed_value).await?
            }
            "xray" => {
                let args = serde_json::to_value(&parsed_value)?;
                dispatch_xray_method(method, &args).await?
            }
            "identity_sled" => {
                let args = serde_json::to_value(&parsed_value)?;
                crate::identity_sled_dispatch::dispatch_identity_sled_method(self, method, &args)
                    .await?
            }
            "human_principal" => {
                let args = serde_json::to_value(&parsed_value)?;
                crate::human_principal_dispatch::dispatch_human_principal_method(method, &args)
                    .await?
            }
            "persona" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::persona::dispatch_persona_method(method, &args).await?
            }
            "gcloud_adc" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::gcloud_adc::dispatch_gcloud_adc_method(method, &args)
                    .await?
            }
            "compact_mcp" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::compact_mcp::dispatch_compact_mcp_method(method, &args)
                    .await?
            }
            "full_system" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::full_system::dispatch_full_system_method(method, &args)
                    .await?
            }
            "keyring" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::keyring::dispatch_keyring_method(method, &args).await?
            }
            "openflow" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::openflow::dispatch_openflow_method(method, &args).await?
            }
            "privacy_routes" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::privacy_routes::dispatch_privacy_routes_method(
                    method, &args,
                )
                .await?
            }
            "tched_router" => {
                let state = self
                    .projected_state::<op_plugins::state_plugins::tched_router::TchedRouterState>(
                        "tched_router",
                    )
                    .await
                    .unwrap_or_else(
                        op_plugins::state_plugins::tched_router::TchedRouterPlugin::current_state,
                    );
                if method == "Chat" {
                    let args = serde_json::from_value::<
                        op_plugins::state_plugins::tched_router::ChatInput,
                    >(serde_json::to_value(&parsed_value)?)
                    .context("invalid tched_router.Chat arguments")?;
                    // Selected model on tched-router (:8084). Tools are compact
                    // MCP on that agent — not deprecated op-llm.
                    serde_json::to_value(
                        crate::zeroclaw_runtime::ZeroclawRuntimeClient::from_env()
                            .chat(&state, args)
                            .await?,
                    )?
                } else {
                    match op_plugins::state_plugins::tched_router::dispatch_tched_router_method(
                        method, json_args, &state,
                    ) {
                        Ok(outcome) => {
                            if method.starts_with("Set") {
                                self.persist_zeroclaw_mutation(method, &outcome.result)
                                    .await?;
                            }
                            if let Some(sig) = &outcome.signal {
                                self.broadcast_method_signal(&change, &sig.name, &sig.payload);
                            }
                            outcome.result
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!(
                                "tched_router dispatch error for '{}': {}",
                                method,
                                e
                            ))
                        }
                    }
                }
            }
            "ghostbridge" => {
                let state = self
                    .projected_state::<op_plugins::state_plugins::ghostbridge::GhostbridgeState>(
                        "ghostbridge",
                    )
                    .await
                    .unwrap_or_else(
                        op_plugins::state_plugins::ghostbridge::GhostbridgePlugin::current_state,
                    );
                op_plugins::state_plugins::ghostbridge::dispatch_ghostbridge_method(method, &state)?
            }
            "cognitive_mcp" => {
                let args = serde_json::to_value(&parsed_value)?;
                dispatch_cognitive_mcp_method(method, &args).await?
            }
            // Audit-trail query surface. Deliberately scoped: only the two
            // audit methods are wired. The plugin's seven pre-existing methods
            // (snapshots, retention, rollback) fall through to the echo below,
            // exactly as before, until the schema-methods sweep wires them.
            "blockchain" => match method {
                "query_events" => {
                    dispatch_blockchain_query_events(&self.event_chain, &parsed_value).await?
                }
                "verify_chain" => {
                    dispatch_blockchain_verify_chain(&self.event_chain, &parsed_value).await?
                }
                _ => serde_json::to_value(&parsed_value).unwrap_or(serde_json::Value::Null),
            },
            _ => serde_json::to_value(&parsed_value).unwrap_or(serde_json::Value::Null),
        };

        // Return a JSON result carrying the event accountability proof and the
        // method's domain result. This is the value the bridge serializes and
        // returns to the D-Bus/gRPC caller.
        let result = serde_json::json!({
            "success": true,
            "event_id": change.event_id,
            "event_hash": change.event_hash,
            "plugin_id": plugin_id,
            "method": method,
            "result": method_result,
        });
        Ok(result)
    }

    async fn projected_state<T>(&self, plugin_id: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if let Some(value) = self.get_state(plugin_id).await {
            if let Ok(json) = serde_json::to_value(value) {
                if let Ok(state) = serde_json::from_value(json) {
                    return Some(state);
                }
            }
        }

        let mut bytes = op_core::projection_shm::read_projection_bytes(plugin_id)?;
        let value = simd_json::to_owned_value(&mut bytes).ok()?;
        let json = serde_json::to_value(&value).ok()?;
        let state = serde_json::from_value(json).ok()?;
        self.update_state_cache(plugin_id.to_string(), value).await;
        Some(state)
    }

    fn broadcast_method_signal(
        &self,
        method_change: &StateChange,
        signal_name: &str,
        payload: &serde_json::Value,
    ) {
        let mut bytes = serde_json::to_vec(payload).unwrap_or_default();
        let payload =
            simd_json::to_owned_value(&mut bytes).unwrap_or_else(|_| simd_json::json!(null));
        let signal = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: method_change.event_id,
            plugin_id: method_change.plugin_id.clone(),
            object_path: method_change.object_path.clone(),
            change_type: ChangeType::Signal,
            member_name: Some(signal_name.to_string()),
            old_value: None,
            new_value: payload,
            tags_touched: method_change.tags_touched.clone(),
            event_hash: method_change.event_hash.clone(),
            timestamp: method_change.timestamp,
            actor_id: method_change.actor_id.clone(),
            source: method_change.source,
        };
        let _ = self.change_tx.send(signal);
    }

    async fn persist_zeroclaw_mutation(
        &self,
        method: &str,
        result: &serde_json::Value,
    ) -> anyhow::Result<()> {
        match method {
            "SetProvider" | "SetModel" | "SetSelection" => {
                self.merge_into_state_cache("tched_router", result).await;
            }
            "SetOvsRoutingModel"
            | "SetObfuscationModel"
            | "SetVectorizationModel"
            | "SetQdrantRetrievalModel"
            | "SetCozoRetrievalModel" => {
                self.merge_nested_into_state_cache("tched_router", "model_assignments", result)
                    .await;
            }
            _ => {}
        }

        self.publish_plugin_projection_from_cache("tched_router", ChangeType::PropertySet)
            .await
    }

    /// Merge a flat JSON object of changed fields into the authoritative
    /// in-memory state cache for `plugin_id` (used to persist Zeroclaw `Set*`
    /// selection changes so readers observe the new effective state).
    async fn merge_into_state_cache(&self, plugin_id: &str, changes: &serde_json::Value) {
        let changes_obj = match changes.as_object() {
            Some(o) => o,
            None => return,
        };
        let mut current = self
            .get_state(plugin_id)
            .await
            .and_then(|v| serde_json::to_value(&v).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(cur_obj) = current.as_object_mut() {
            for (k, v) in changes_obj {
                cur_obj.insert(k.clone(), v.clone());
            }
        }
        let mut bytes = serde_json::to_vec(&current).unwrap_or_default();
        if let Ok(owned) = simd_json::to_owned_value(&mut bytes) {
            self.update_state_cache(plugin_id.to_string(), owned).await;
        }
    }

    async fn merge_nested_into_state_cache(
        &self,
        plugin_id: &str,
        field: &str,
        changes: &serde_json::Value,
    ) {
        let Some(changes_obj) = changes.as_object() else {
            return;
        };
        let mut current = self
            .get_state(plugin_id)
            .await
            .and_then(|value| serde_json::to_value(value).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let Some(current_obj) = current.as_object_mut() else {
            return;
        };
        let nested = current_obj
            .entry(field.to_string())
            .or_insert_with(|| serde_json::json!({}));
        let Some(nested_obj) = nested.as_object_mut() else {
            return;
        };
        for (key, value) in changes_obj {
            nested_obj.insert(key.clone(), value.clone());
        }

        let mut bytes = serde_json::to_vec(&current).unwrap_or_default();
        if let Ok(owned) = simd_json::to_owned_value(&mut bytes) {
            self.update_state_cache(plugin_id.to_string(), owned).await;
        }
    }

    /// Fetch current state for a specific plugin from authoritative cache
    pub async fn get_state(&self, plugin_id: &str) -> Option<simd_json::OwnedValue> {
        let cache = self.state_cache.read().await;
        cache.get(plugin_id).cloned()
    }

    /// Update the authoritative state cache
    pub async fn update_state_cache(&self, plugin_id: String, state: simd_json::OwnedValue) {
        let mut cache = self.state_cache.write().await;
        cache.insert(plugin_id, state);
    }

    /// Publish the current authoritative cache entry into the plugin's SHM
    /// projection after a domain dispatcher has completed its mutation.
    pub async fn publish_plugin_projection_from_cache(
        &self,
        plugin_id: &str,
        change_type: ChangeType,
    ) -> anyhow::Result<()> {
        let Some(state) = self.get_state(plugin_id).await else {
            return Ok(());
        };
        let new_value = serde_json::to_value(&state)?;
        let crawl = match tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.crawl_plugin_dbus_tree(plugin_id),
        )
        .await
        {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!(
                    plugin_id,
                    "plugin D-Bus crawl timed out; projecting cached state"
                );
                None
            }
        };
        // The shm state file holds present state verbatim; the composite is
        // only carried on the broadcast `StateChange` for event-stream consumers.
        let state_owned = simd_json::serde::to_owned_value(&new_value)?;
        let json = simd_json::to_string(&state_owned)?;
        op_core::projection_shm::write_projection(plugin_id, json.as_bytes())?;
        self.emit_updated_signal(plugin_id, None, Self::state_keys_serde(&new_value))
            .await;
        let projection = if let Some(crawl) = crawl {
            simd_json::json!({ "data": new_value, "_introspection": crawl })
        } else {
            state_owned
        };
        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: 0,
            plugin_id: plugin_id.to_string(),
            object_path: format!("/org/opdbus/v1/plugins/{plugin_id}"),
            change_type,
            member_name: None,
            old_value: None,
            new_value: projection,
            tags_touched: vec![],
            event_hash: String::new(),
            timestamp: chrono::Utc::now(),
            actor_id: "identity_sled_dispatch".to_string(),
            source: ChangeSource::Internal,
        };
        let _ = self.change_tx.send(change);
        Ok(())
    }

    async fn update_cached_plugin_state(
        &self,
        plugin_id: &str,
        object_path: &str,
        change_type: ChangeType,
        property: Option<&str>,
        new_value: &simd_json::OwnedValue,
    ) {
        if object_path.starts_with("schema/") {
            return;
        }

        let mut cache = self.state_cache.write().await;

        match change_type {
            ChangeType::ObjectRemoved => {
                cache.remove(plugin_id);
            }
            ChangeType::PropertySet => {
                if let Some(property) = property {
                    let entry = cache
                        .entry(plugin_id.to_string())
                        .or_insert_with(|| simd_json::json!({}));

                    if let Some(existing) = entry.as_object_mut() {
                        existing.insert(property.to_string(), new_value.clone());
                    } else {
                        let mut state = simd_json::value::owned::Object::new();
                        state.insert(property.to_string(), new_value.clone());
                        *entry = simd_json::OwnedValue::Object(Box::new(state));
                    }
                } else {
                    cache.insert(plugin_id.to_string(), new_value.clone());
                }
            }
            _ => {}
        }
    }

    /// Route a D-Bus method call through the authoritative bridge
    #[allow(clippy::too_many_arguments)]
    pub async fn call_dbus_method(
        &self,
        bus_name: &str,
        path: &str,
        interface: &str,
        method: &str,
        _args: Vec<simd_json::OwnedValue>,
        _actor_id: &str,
        capability_id: &Option<String>,
    ) -> anyhow::Result<simd_json::OwnedValue> {
        tracing::debug!(
            bus_name,
            path,
            interface,
            method,
            capability_id = ?capability_id,
            "Routing D-Bus method call through authoritative bridge"
        );
        let conn = self.dbus_connection().await?;
        let proxy = Proxy::new(&conn, bus_name, path, interface).await?;
        let result: ZOwnedValue = proxy.call(method, &()).await?;
        simd_json::serde::to_owned_value(&result).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn change_tx(&self) -> broadcast::Sender<StateChange> {
        self.change_tx.clone()
    }
}

/// Result of an authoritative mutation
#[derive(Debug, Clone)]
pub struct MutationResult {
    pub success: bool,
    pub event_id: u64,
    pub event_hash: String,
    pub result: Option<simd_json::OwnedValue>,
    pub error: Option<MutationError>,
}

#[derive(Debug, Clone)]
pub struct MutationError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    NotFound,
    PermissionDenied,
    ValidationFailed,
    ReadOnly,
    Internal,
}

/// Read the WireGuard footprint from the Sled (1:1 shared memory identity at
/// `/dev/shm/plugin_schema.dat`). Returns the hex footprint used as the default
/// `actor_id` when a caller omits identity.
fn sled_footprint() -> Option<String> {
    // SAFETY: read_sled returns a valid pointer to IdentitySled in shared memory
    // for the lifetime of the process; we copy the footprint out and drop the ptr.
    let (ptr, _mmap) = read_sled().ok()?;
    unsafe {
        let sled = &*ptr;
        if sled.hashed_footprint != [0u8; 32] {
            Some(hex::encode(sled.hashed_footprint))
        } else {
            None
        }
    }
}

/// Build the authoritative `unix_socket` projection state after a successful
/// `createunixsocket` mutation. The mutation argument is not state; the state is
/// the registered service list under the single shared container socket.
fn unix_socket_state_after_registration(name: &str, ports: &[u16]) -> simd_json::OwnedValue {
    let mut sockets = existing_unix_socket_sockets();
    sockets.retain(|socket| {
        socket
            .get("name")
            .and_then(|value| value.as_str())
            .map(|existing_name| existing_name != name)
            .unwrap_or(true)
    });

    sockets.push(simd_json::json!({
        "name": name,
        "path": op_plugins::state_plugins::unix_socket::SHARED_CONTAINER_SOCKET,
        "ports": ports,
        "protocol": "grpc",
        "label": "",
    }));

    simd_json::json!({ "sockets": sockets })
}

fn existing_unix_socket_sockets() -> Vec<simd_json::OwnedValue> {
    let Some(mut bytes) = op_core::projection_shm::read_projection_bytes("unix_socket") else {
        return Vec::new();
    };
    let Ok(state) = simd_json::to_owned_value(&mut bytes) else {
        return Vec::new();
    };

    let sockets = state
        .get("sockets")
        .and_then(|value| value.as_array())
        .or_else(|| {
            state
                .get("data")
                .and_then(|data| data.get("sockets"))
                .and_then(|value| value.as_array())
        });

    sockets
        .into_iter()
        .flatten()
        .filter(|socket| {
            let has_name = socket
                .get("name")
                .and_then(|value| value.as_str())
                .map(|name| !name.is_empty())
                .unwrap_or(false);
            let has_path = socket
                .get("path")
                .and_then(|value| value.as_str())
                .is_some();
            has_name && has_path
        })
        .cloned()
        .collect()
}

// ── Audit trail: durable persistence + query dispatch ────────────────────────

/// Convert a chain event into a durable footprint for the `timing_subvol`.
///
/// The full event is embedded under `metadata.audit_event` so the startup
/// rebuild can restore it losslessly, including the stored `event_hash` and
/// `prev_hash` that make the chain tamper-evident. The flat metadata keys
/// beside it exist for operators and tools grepping the timing directory
/// directly.
///
/// OSCAL subid: `evt.service.event-chain.persist@v1`
fn event_to_footprint(event: &ChainEvent) -> PluginFootprint {
    let mut metadata: HashMap<String, simd_json::OwnedValue> = HashMap::new();
    metadata.insert(
        "actor_id".to_string(),
        simd_json::OwnedValue::from(event.actor_id.as_str()),
    );
    metadata.insert(
        "capability_id".to_string(),
        simd_json::OwnedValue::from(event.capability_id.clone().unwrap_or_default()),
    );
    metadata.insert(
        "method_name".to_string(),
        simd_json::OwnedValue::from(event.method_name.clone().unwrap_or_default()),
    );
    metadata.insert(
        "event_id".to_string(),
        simd_json::OwnedValue::from(event.event_id),
    );
    metadata.insert(
        "event_hash".to_string(),
        simd_json::OwnedValue::from(event.event_hash.as_str()),
    );
    metadata.insert(
        "decision".to_string(),
        simd_json::OwnedValue::from(format!("{:?}", event.decision)),
    );
    // Lossless copy for replay. Serialized through serde_json (ChainEvent's
    // derive target) and re-parsed into simd_json for the footprint map.
    match serde_json::to_vec(event) {
        Ok(mut bytes) => match simd_json::to_owned_value(&mut bytes) {
            Ok(value) => {
                metadata.insert("audit_event".to_string(), value);
            }
            Err(error) => {
                tracing::warn!(%error, event_id = event.event_id, "audit event not replayable");
            }
        },
        Err(error) => {
            tracing::warn!(%error, event_id = event.event_id, "audit event not serializable");
        }
    }

    PluginFootprint {
        plugin_id: event.plugin_id.clone(),
        operation: event
            .method_name
            .clone()
            .unwrap_or_else(|| format!("{:?}", event.op)),
        timestamp: event.timestamp.timestamp_millis() as u64,
        data_hash: event.input_patch_hash.clone(),
        // The event hash is the timing file name, so one file per event.
        content_hash: event.event_hash.clone(),
        metadata,
        vector_features: vec![],
    }
}

/// Flatten a chain event into the plugin-surface record shape.
fn chain_event_to_record(event: &ChainEvent) -> AuditEventRecord {
    AuditEventRecord {
        event_id: event.event_id,
        event_hash: event.event_hash.clone(),
        prev_hash: event.prev_hash.clone(),
        timestamp: event.timestamp.to_rfc3339(),
        actor_id: event.actor_id.clone(),
        capability_id: event.capability_id.clone().unwrap_or_default(),
        plugin_id: event.plugin_id.clone(),
        method_name: event.method_name.clone().unwrap_or_default(),
        operation_type: format!("{:?}", event.op),
        target: event.target.clone(),
        tags_touched: event.tags_touched.clone(),
        decision: format!("{:?}", event.decision),
        input_patch_hash: event.input_patch_hash.clone(),
        result_effective_hash: event.result_effective_hash.clone().unwrap_or_default(),
    }
}

/// `blockchain.query_events` — paginated, filtered read of the audit trail.
///
/// Reads the same `EventChain` the gRPC `EventChainService.GetEvents` serves, so
/// the D-Bus/MCP path and the GUI path can never disagree.
///
/// OSCAL subid: `obs.service.blockchain.events.query@v1`
async fn dispatch_blockchain_query_events(
    event_chain: &Arc<RwLock<EventChain>>,
    args: &simd_json::OwnedValue,
) -> anyhow::Result<serde_json::Value> {
    let input: QueryEventsInput =
        serde_json::from_value(serde_json::to_value(args)?).unwrap_or_default();

    // Default 50, hard ceiling 100 — clamped silently, never unbounded (FR-4).
    let limit = input.limit.unwrap_or(50).clamp(1, 100) as usize;

    let chain = event_chain.read().await;
    let total_in_chain = chain.events().len() as u64;

    let decision_filter = input.decision.as_deref().map(str::to_ascii_lowercase);

    let mut matched: Vec<AuditEventRecord> = chain
        .events()
        .iter()
        .filter(|e| input.from_event_id.is_none_or(|id| e.event_id >= id))
        .filter(|e| input.to_event_id.is_none_or(|id| e.event_id <= id))
        .filter(|e| {
            input
                .plugin_id
                .as_ref()
                .is_none_or(|p| p.is_empty() || e.plugin_id == *p)
        })
        .filter(|e| match decision_filter.as_deref() {
            Some("allow") => e.decision == Decision::Allow,
            Some("deny") => e.decision == Decision::Deny,
            _ => true,
        })
        // One extra row reveals whether another page exists.
        .take(limit + 1)
        .map(chain_event_to_record)
        .collect();

    let has_more = matched.len() > limit;
    matched.truncate(limit);

    Ok(serde_json::to_value(QueryEventsOutput {
        events: matched,
        has_more,
        total_in_chain,
    })?)
}

/// `blockchain.verify_chain` — hash-chain integrity check over a range.
///
/// OSCAL subid: `obs.service.blockchain.chain.verify@v1`
async fn dispatch_blockchain_verify_chain(
    event_chain: &Arc<RwLock<EventChain>>,
    args: &simd_json::OwnedValue,
) -> anyhow::Result<serde_json::Value> {
    let input: VerifyChainInput =
        serde_json::from_value(serde_json::to_value(args)?).unwrap_or_default();

    let chain = event_chain.read().await;
    let result = chain.verify_range(
        input.from_event_id.unwrap_or(0),
        input.to_event_id.unwrap_or(0),
    );

    Ok(serde_json::to_value(VerifyChainOutput {
        valid: result.valid,
        events_verified: result.events_verified as u64,
        errors: result.errors,
    })?)
}

/// Execute rovs_commands methods via OVSDB proxy
async fn dispatch_rovs_commands_method(
    ovsdb: &op_network::rovs_proxy::OvsdbDbusClient,
    method: &str,
    args: &simd_json::OwnedValue,
) -> anyhow::Result<serde_json::Value> {
    match method {
        "create_bridge" => {
            let bridge_name = args
                .get("bridge_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("bridge_name required"))?;
            ovsdb.create_bridge(bridge_name).await?;
            Ok(serde_json::json!({"created": bridge_name}))
        }
        "delete_bridge" => {
            let bridge_name = args
                .get("bridge_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("bridge_name required"))?;
            ovsdb.delete_bridge(bridge_name).await?;
            Ok(serde_json::json!({"deleted": bridge_name}))
        }
        "add_port" => {
            let bridge_name = args
                .get("bridge_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("bridge_name required"))?;
            let port_name = args
                .get("port_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("port_name required"))?;
            ovsdb.add_port(bridge_name, port_name).await?;
            Ok(serde_json::json!({"added": port_name, "to": bridge_name}))
        }
        "remove_port" => {
            let bridge_name = args
                .get("bridge_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("bridge_name required"))?;
            let port_name = args
                .get("port_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("port_name required"))?;
            ovsdb.delete_port(bridge_name, port_name).await?;
            Ok(serde_json::json!({"removed": port_name, "from": bridge_name}))
        }
        "list_bridges" => {
            let bridges = ovsdb.list_bridges().await?;
            Ok(serde_json::json!(bridges))
        }
        "list_ports" => {
            let bridge_name = args
                .get("bridge_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("bridge_name required"))?;
            let ports = ovsdb.list_bridge_ports(bridge_name).await?;
            Ok(serde_json::json!(ports))
        }
        "list_dbs" => {
            let dbs = ovsdb.list_dbs().await?;
            Ok(serde_json::json!(dbs))
        }
        _ => Err(anyhow::anyhow!("unknown rovs_commands method: {}", method)),
    }
}

/// Execute the schema-declared OVSDB bridge methods used by canonical
/// `org.opdbus.v1.plugins` compatibility clients.
async fn dispatch_ovsdb_bridge_method(
    ovsdb: &op_network::rovs_proxy::OvsdbDbusClient,
    method: &str,
    args: &simd_json::OwnedValue,
) -> anyhow::Result<serde_json::Value> {
    match method {
        "list_dbs" => Ok(serde_json::json!(ovsdb.list_dbs().await?)),
        "get_schema" => Ok(ovsdb.get_schema().await?),
        "transact" => {
            let operations = args
                .get("operations")
                .ok_or_else(|| anyhow::anyhow!("operations required"))?
                .clone();
            Ok(ovsdb.transact_simd(operations).await?)
        }
        _ => Err(anyhow::anyhow!(
            "ovsdb_bridge.{method} is schema-declared but has no runtime dispatcher"
        )),
    }
}

/// Dispatch for the `xray` plugin's schema-declared methods. Only `get_stats`
/// is backed by real behavior so far — it calls xray-core's own StatsService
/// over the commander UDS (see `op_xray_daemon::commander_client`). The
/// other declared methods (`add_user`, `remove_user`, `add_inbound`,
/// `restart`, `start_trace`/`end_trace`/`record_span`) are schema-declared
/// but not yet implemented; they fail closed rather than silently
/// succeeding or falling through to the generic echo.
async fn dispatch_xray_method(
    method: &str,
    args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    match method {
        "get_stats" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("name required"))?;
            let mut client = op_xray_daemon::commander_client::stats_client(
                op_xray_daemon::commander_client::DEFAULT_API_SOCKET,
            )
            .await
            .context("connecting to xray commander API socket")?;
            let resp = client
                .get_stats(op_xray_daemon::commander_client::stats::GetStatsRequest {
                    name: name.to_string(),
                    reset: false,
                })
                .await
                .context("xray StatsService.GetStats")?
                .into_inner();
            let stat = resp
                .stat
                .ok_or_else(|| anyhow::anyhow!("no such stat counter: {name}"))?;
            Ok(serde_json::json!({ "name": stat.name, "value": stat.value }))
        }
        "restart" => {
            // Reload (SIGHUP), not a hard kill+respawn: the `xray` container's
            // own service manager already supervises the process; a config
            // reload is the safe, sanctioned action here (see op-plugins::xray's
            // apply_state, which uses the same pattern). No Command::new
            // subprocess — signals the real PID directly via /proc + nix.
            let pids = find_xray_pids();
            if pids.is_empty() {
                return Err(anyhow::anyhow!("no running xray process found"));
            }
            let mut reloaded = Vec::new();
            for pid in pids {
                nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGHUP)
                    .with_context(|| format!("sending SIGHUP to xray pid {pid}"))?;
                reloaded.push(pid.as_raw());
            }
            Ok(serde_json::json!({ "reloaded_pids": reloaded }))
        }
        "add_user" | "remove_user" | "add_inbound" | "start_trace" | "end_trace"
        | "record_span" => Err(anyhow::anyhow!(
            "xray.{method} is schema-declared but not yet implemented"
        )),
        _ => Err(anyhow::anyhow!("unknown xray method: {}", method)),
    }
}

/// Find running `xray` process PIDs via `/proc` — no `pgrep`/`pkill`
/// subprocess spawning. Mirrors `op_plugins::state_plugins::xray`'s private
/// helper of the same shape; kept local since it's a few lines and the two
/// crates don't otherwise share process-inspection utilities.
fn find_xray_pids() -> Vec<nix::unistd::Pid> {
    let mut pids = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return pids;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        if let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) {
            if comm.trim() == "xray" {
                pids.push(nix::unistd::Pid::from_raw(pid));
            }
        }
    }
    pids
}

/// Dispatch a `cognitive_mcp` schema method to the cognitive tool registry.
///
/// OSCAL subid: mut.service.cognitive-mcp.dispatch@v1
///
/// Phase 1 transport is an HTTP loopback to `op-cognitive-mcp`'s MCP endpoint. The
/// bridge remains the only authorization door: the method-existence gate, arg
/// validation, capability check and event-chain append have all already happened in
/// `dispatch_method_call` before this function is reached.
///
/// Phase 2 replaces the loopback with an in-process `ToolRegistry::execute` call and
/// retires the `:3003` listener. See
/// `.kiro/specs/cognitive-mcp-only-door-phase2/`.
async fn dispatch_cognitive_mcp_method(
    method: &str,
    args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Plugin-local methods answer from published state; no executor hop needed.
    match method {
        "get_config" | "get_health" => {
            let mut bytes = op_core::projection_shm::read_projection_bytes("cognitive_mcp")
                .ok_or_else(|| anyhow::anyhow!("cognitive_mcp projection not available"))?;
            let value = simd_json::to_owned_value(&mut bytes)
                .map_err(|e| anyhow::anyhow!("cognitive_mcp projection is not valid JSON: {e}"))?;
            return Ok(serde_json::to_value(&value)?);
        }
        "set_config" | "restart_service" => {
            // These belong to the plugin's own apply_state path, which is reached
            // through StatePlugin rather than the tool registry. Acknowledge without
            // fabricating a result so callers can distinguish "accepted" from "done".
            return Ok(serde_json::json!({
                "acknowledged": true,
                "method": method,
                "note": "handled by cognitive_mcp apply_state; not a tool-registry call",
            }));
        }
        _ => {}
    }

    // `list_tools` is an MCP *protocol* method (`tools/list`), not an entry in the
    // tool registry, so it cannot go through `tools/call`.
    if method == "list_tools" {
        return cognitive_tools_list().await;
    }

    let (tool_name, tool_args) = map_schema_method_to_tool(method, args)?;
    let result = call_cognitive_tool(&tool_name, &tool_args).await?;
    Ok(result)
}

/// Translate a `cognitive_mcp` schema method name into a live tool-registry call.
///
/// The schema method names do not map one-to-one onto registry tool names: the
/// registry exposes a single `cognitive_memory` tool selected by an `operation`
/// field, so the memory methods inject that field rather than existing as separate
/// tools. `invoke_tool` bypasses the table entirely and names its tool directly.
fn map_schema_method_to_tool(
    method: &str,
    args: &serde_json::Value,
) -> anyhow::Result<(String, serde_json::Value)> {
    let base = if args.is_object() {
        args.clone()
    } else {
        serde_json::json!({})
    };

    let with_field = |key: &str, value: &str| -> serde_json::Value {
        let mut v = base.clone();
        if let Some(obj) = v.as_object_mut() {
            obj.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        v
    };

    match method {
        // Memory methods all resolve to the one `cognitive_memory` tool.
        "memory_store" => Ok(("cognitive_memory".into(), with_field("operation", "store"))),
        "memory_query" => Ok(("cognitive_memory".into(), with_field("operation", "query"))),
        "memory_retrieve" => Ok((
            "cognitive_memory".into(),
            with_field("operation", "retrieve"),
        )),
        "memory_delete" => Ok(("cognitive_memory".into(), with_field("operation", "delete"))),
        "memory_list_namespaces" => Ok((
            "cognitive_memory".into(),
            with_field("operation", "list_namespaces"),
        )),
        // Code retrieval / indexing.
        "code_search" => Ok(("search_blob_vectors".into(), base)),
        "code_index" => Ok(("refresh_blob_vectors".into(), base)),
        "code_context" => Ok((
            "search_blob_vectors".into(),
            with_field("activity_type", "query"),
        )),
        // Gemini question answering: the tool names the field `question`.
        "gemini_query" => {
            let mut v = base.clone();
            if let Some(obj) = v.as_object_mut() {
                if let Some(q) = obj.remove("query") {
                    obj.insert("question".to_string(), q);
                }
            }
            Ok(("ask_question".into(), v))
        }
        "register_tool" => Ok(("register_tool".into(), base)),
        // Generic door: the caller names the tool.
        "invoke_tool" => {
            let tool_name = args
                .get("tool_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("invoke_tool: missing required field 'tool_name'"))?
                .to_string();
            if tool_name.is_empty() {
                return Err(anyhow::anyhow!(
                    "invoke_tool: 'tool_name' must not be empty"
                ));
            }
            let tool_args = args
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            Ok((tool_name, tool_args))
        }
        other => Err(anyhow::anyhow!(
            "cognitive_mcp method '{other}' has no tool-registry mapping"
        )),
    }
}

/// Shared HTTP client for the cognitive MCP loopback.
///
/// Built once: a fresh `Client` per call would leak a connection pool and defeat
/// keep-alive on what is a hot path.
static COGNITIVE_MCP_HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn cognitive_mcp_http() -> &'static reqwest::Client {
    COGNITIVE_MCP_HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// MCP endpoint of the local cognitive service.
///
/// Overridable so the bridge can follow a relocated listener without a rebuild.
fn cognitive_mcp_endpoint() -> String {
    std::env::var("COGNITIVE_MCP_MCP_URL")
        .unwrap_or_else(|_| "http://10.200.0.2:3003/mcp".to_string())
}

/// Send the MCP `initialize` handshake.
///
/// The cognitive MCP server rejects `tools/call` with "Server not initialized" until
/// this has been sent. Initialization is server-side state, so it survives across
/// requests but is lost whenever `op-cognitive-mcp` restarts — which is why callers
/// treat it as a recoverable condition rather than doing it once at startup.
async fn cognitive_mcp_initialize() -> anyhow::Result<()> {
    let endpoint = cognitive_mcp_endpoint();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {},
    });

    let response = cognitive_mcp_http()
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("X-Ghostbridge-Footprint", "op-grpc-bridge")
        .header("X-Ghostbridge-Trace-ID", "bridge-dispatch")
        .json(&request)
        .send()
        .await
        .with_context(|| format!("cognitive_mcp initialize POST {endpoint}"))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "cognitive_mcp initialize returned HTTP {}",
            response.status()
        ));
    }
    Ok(())
}

/// Issue one `tools/call` and return the raw JSON-RPC envelope.
async fn cognitive_tools_call_raw(
    tool_name: &str,
    tool_args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let endpoint = cognitive_mcp_endpoint();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": tool_name, "arguments": tool_args },
    });

    let response = cognitive_mcp_http()
        .post(&endpoint)
        .header("Content-Type", "application/json")
        // The interceptor on the direct listener expects these; the authoritative
        // authorization decision was already made by the bridge before dispatch.
        .header("X-Ghostbridge-Footprint", "op-grpc-bridge")
        .header("X-Ghostbridge-Trace-ID", "bridge-dispatch")
        .json(&request)
        .send()
        .await
        .with_context(|| format!("cognitive_mcp loopback POST {endpoint}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .context("reading cognitive_mcp loopback response body")?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "cognitive_mcp loopback returned HTTP {status}: {body}"
        ));
    }

    serde_json::from_str(&body)
        .with_context(|| format!("cognitive_mcp returned non-JSON body: {body}"))
}

/// Invoke one tool over the MCP `tools/call` JSON-RPC method and unwrap the result.
///
/// Performs the `initialize` handshake lazily: rather than initializing once at bridge
/// startup (which would break the first call after any `op-cognitive-mcp` restart), a
/// "not initialized" error triggers one initialize and a single retry.
async fn call_cognitive_tool(
    tool_name: &str,
    tool_args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let mut envelope = cognitive_tools_call_raw(tool_name, tool_args).await?;

    if mcp_needs_initialize(&envelope) {
        cognitive_mcp_initialize().await?;
        envelope = cognitive_tools_call_raw(tool_name, tool_args).await?;
    }

    if let Some(error) = envelope.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown MCP error");
        return Err(anyhow::anyhow!("tool '{tool_name}' failed: {message}"));
    }

    envelope.get("result").cloned().ok_or_else(|| {
        anyhow::anyhow!("cognitive_mcp response had neither 'result' nor 'error': {envelope}")
    })
}

/// Enumerate the live tool registry via the MCP `tools/list` protocol method.
///
/// Distinct from `call_cognitive_tool`: `tools/list` is a protocol method rather than a
/// registry entry, so routing the `list_tools` schema method through `tools/call` would
/// (and did) fail with "Tool not found: list_tools".
async fn cognitive_tools_list() -> anyhow::Result<serde_json::Value> {
    let endpoint = cognitive_mcp_endpoint();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {},
    });

    let post = || async {
        let response = cognitive_mcp_http()
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header("X-Ghostbridge-Footprint", "op-grpc-bridge")
            .header("X-Ghostbridge-Trace-ID", "bridge-dispatch")
            .json(&request)
            .send()
            .await
            .with_context(|| format!("cognitive_mcp tools/list POST {endpoint}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("reading cognitive_mcp tools/list response body")?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "cognitive_mcp tools/list returned HTTP {status}: {body}"
            ));
        }
        serde_json::from_str::<serde_json::Value>(&body)
            .with_context(|| format!("cognitive_mcp tools/list returned non-JSON body: {body}"))
    };

    let mut envelope = post().await?;
    if mcp_needs_initialize(&envelope) {
        cognitive_mcp_initialize().await?;
        envelope = post().await?;
    }

    if let Some(error) = envelope.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown MCP error");
        return Err(anyhow::anyhow!("tools/list failed: {message}"));
    }

    envelope
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("cognitive_mcp tools/list response had no 'result'"))
}

/// True when the envelope is the server's "call initialize first" rejection.
fn mcp_needs_initialize(envelope: &serde_json::Value) -> bool {
    envelope
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .is_some_and(|m| m.contains("not initialized"))
}

/// True for the live `unix_socket` registration method and its aliases.
fn is_unix_socket_bind_method(method: &str) -> bool {
    matches!(
        method,
        "bind" | "Bind" | "createunixsocket" | "create_unix_socket"
    )
}

/// Parse createunixsocket / bind arguments. Accepts either a JSON array
/// `[name, [ports...]]` or an object `{ "name": "...", "ports": [...] }`.
/// `ports` may be a JSON array of numbers or a CSV string ("6334" / "6334,8080").
fn parse_socket_args(value: &simd_json::OwnedValue) -> (String, Vec<u16>) {
    if let Some(obj) = value.as_object() {
        return parse_socket_arg_object(obj);
    }
    if let Some(arr) = value.as_array() {
        if arr.len() == 1 {
            if let Some(obj) = arr.first().and_then(|v| v.as_object()) {
                return parse_socket_arg_object(obj);
            }
        }
        let name = arr
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let ports = parse_ports_value(arr.get(1));
        return (name, ports);
    }
    (String::new(), Vec::new())
}

fn parse_socket_arg_object(obj: &simd_json::value::owned::Object) -> (String, Vec<u16>) {
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let ports = parse_ports_value(obj.get("ports"));
    (name, ports)
}

fn parse_ports_value(value: Option<&simd_json::OwnedValue>) -> Vec<u16> {
    let Some(v) = value else {
        return Vec::new();
    };
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(parse_port_number)
            .map(|n| n as u16)
            .collect();
    }
    if let Some(s) = v.as_str() {
        return s
            .split(',')
            .filter_map(|p| p.trim().parse::<u16>().ok())
            .collect();
    }
    if let Some(n) = parse_port_number(v) {
        return vec![n as u16];
    }
    Vec::new()
}

fn parse_port_number(value: &simd_json::OwnedValue) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| (n >= 0).then_some(n as u64)))
        .or_else(|| {
            value.as_f64().and_then(|n| {
                (n.is_finite() && n >= 0.0 && n <= u16::MAX as f64).then_some(n as u64)
            })
        })
}
