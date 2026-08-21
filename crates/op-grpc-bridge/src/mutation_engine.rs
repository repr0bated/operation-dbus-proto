//! Mutation Engine - The Authoritative Source for State and Schema DNA
//!
//! The Mutation Engine is the central coordinator that:
//! - Authoritatively routes all mutations (gRPC and D-Bus)
//! - Ensures all state changes are strictly recorded in the Event Chain (Audit Log)
//! - Broadcasts authoritative state changes to gRPC subscribers
//! - Directly manages authoritative RCP stores (OVSDB, NonNet, SQLite)

use anyhow::Context;
use async_trait::async_trait;
use op_cognitive_mcp::{CognitiveMemoryStore, ContextAwarenessEngine, SessionManager};
use op_mcp::tool_registry::ToolRegistry;
use serde_json;
use simd_json::prelude::{ValueAsContainer, ValueAsMutContainer, ValueAsScalar, ValueObjectAccess};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, OnceCell, RwLock, Semaphore};
use zbus::zvariant::OwnedValue as ZOwnedValue;
use zbus::{Connection, Proxy};

use op_blockchain::{PluginFootprint, StreamingBlockchain};
use op_identity::session_genesis::mint_genesis;
use op_network::rovs_proxy::OvsdbDbusClient;
use op_plugins::state_plugins::blockchain_plugin::{
    AuditEventRecord, QueryEventsInput, QueryEventsOutput, VerifyChainInput, VerifyChainOutput,
};
use op_state_store::{ChainEvent, Decision, EventChain, MemoryStore, OperationType, StateStore};

/// Default on-disk location of the streaming blockchain that backs the durable
/// audit trail, when `$OPDBUS_BLOCKCHAIN_PATH` is unset. Matches
/// `blockchain_plugin::DEFAULT_BASE_PATH` so both read the same chain.
const DEFAULT_BLOCKCHAIN_PATH: &str = "/var/lib/opdbus/blockchain";

/// Context-awareness state extracted from [`CognitiveMcpServer`] for Task 3 routes.
pub type CognitiveContextState = (
    Arc<ContextAwarenessEngine>,
    Arc<CognitiveMemoryStore>,
    Arc<SessionManager>,
);

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

/// One plugin's sealed contract, held for stream hydration.
///
/// Both fields come out of the same [`op_blob::blobify_plugin_schema`] call that
/// produced the sealed blob, so `schema_json` is byte-identical to the catalog's
/// SCHEMA_JSON section and `schema_hash` is the hash covering exactly those
/// bytes. Nothing here re-serializes or re-hashes the schema.
#[derive(Debug, Clone)]
pub struct SchemaSnapshot {
    pub schema_hash: String,
    pub schema_json: simd_json::OwnedValue,
}

/// Actor label for a mutation that arrived with no identity at all.
///
/// It names no session, so it anchors none: the mutation is still notarized in
/// the chain, but its session stamp is empty rather than borrowed from whoever
/// wrote a shared file last.
pub const ANONYMOUS_ACTOR: &str = "anonymous";

/// The verified identity of the session a mutation belongs to.
///
/// Assembled once, at arrival, by [`MutationEngine::mint_and_store_genesis`]
/// and read from there by every consumer. Nothing recomputes the genesis: the
/// interceptor compares this value, the chain stamper embeds it, the UDS
/// injector presents it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionContext {
    /// Hex blake3 session genesis — the immutable anchor.
    pub genesis_hex: String,
    /// Session identifier (container name / derived session id).
    pub session_id: String,
    /// WireGuard public key (base64) that owns this session.
    pub wireguard_pubkey: String,
}

/// The genesis and the inputs it was minted from, written to the session
/// record exactly once at arrival.
///
/// The inputs are stored because the genesis is irreproducible without them —
/// `arrival_timestamp` cannot be recovered after the fact — which is what makes
/// offline re-verification possible (FR-3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenesisStamp {
    pub session_id: String,
    pub wireguard_pubkey: String,
    pub genesis_hex: String,
    pub arrival_timestamp: i64,
    pub chain_head_at_arrival: String,
    pub catalog_hash_at_arrival: String,
    pub head_timestamp_at_arrival: i64,
}

pub struct MutationEngine {
    /// Authoritative Event Chain
    pub event_chain: Arc<RwLock<EventChain>>,
    /// Real-time change projection channel
    change_tx: broadcast::Sender<StateChange>,
    /// Real-time audit-chain projection channel.
    ///
    /// Fed from the same pipeline that appends to `event_chain`, so the mutation
    /// door remains the only producer of chain events. Sent after the projection
    /// write for the same reason `change_tx` is: a subscriber must never learn of
    /// an event before the state it describes is readable.
    chain_tx: broadcast::Sender<ChainEvent>,
    /// State cache for instant gRPC retrieval
    state_cache: Arc<RwLock<HashMap<String, simd_json::OwnedValue>>>,
    /// Sealed plugin contracts, keyed by canonical plugin id.
    ///
    /// Populated by [`MutationEngine::seed_missing_plugin_projections`] at
    /// startup — the same pass that seals the blobs — so a subscriber can be
    /// handed the running contract at hydration without touching the catalog.
    /// Ordered so hydration frames arrive in a stable sequence.
    schema_cache: Arc<RwLock<BTreeMap<String, SchemaSnapshot>>>,
    /// Identity of the published catalog, read from `op_blob`'s single
    /// implementation after sealing — never recomputed here. Stamped on every
    /// outgoing frame so a subscriber can detect drift without being told.
    catalog_hash: Arc<RwLock<String>>,
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
    /// Loopback adapter to the real ZeroClaw runtime authority.
    tched_router_runtime: Arc<crate::tched_router_runtime::TchedRouterRuntimeClient>,
    /// In-process cognitive MCP tool registry (Phase 2). `None` when init failed.
    cognitive_tool_registry: std::sync::RwLock<Option<Arc<ToolRegistry>>>,
    /// Context engine tuple for Task 3 (context-awareness routes).
    cognitive_context_engine: std::sync::RwLock<Option<CognitiveContextState>>,
    /// Verified session identities, keyed by session_id.
    ///
    /// Populated at arrival by [`MutationEngine::mint_and_store_genesis`] and
    /// read (never recomputed) by the chain stamper and the per-session record
    /// writer. This is a projection of the session records for the mutation
    /// path, not a second store of the genesis: the record is authoritative and
    /// every entry here was written from it.
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

/// How many of the newest audit records to replay into the in-memory chain at
/// startup. `OP_EVENT_CHAIN_REPLAY_LIMIT=0` restores the old unbounded rebuild.
///
/// The default is a window, not a cap on history: the durable trail on disk is
/// untouched and complete. It exists because the boot path must not scale with
/// total recorded history.
const DEFAULT_REPLAY_LIMIT: usize = 50_000;

fn replay_limit() -> usize {
    std::env::var("OP_EVENT_CHAIN_REPLAY_LIMIT")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_REPLAY_LIMIT)
}

/// Extract the block number from a `block-000000000042.json` timing record.
///
/// Selecting on the filename is what makes a bounded replay cheap: the newest
/// records can be chosen without opening, reading, or parsing any of them.
fn block_number_from_name(path: &std::path::Path) -> Option<u64> {
    let stem = path.file_stem()?.to_str()?;
    let digits = stem.strip_prefix("block-")?;
    digits.parse::<u64>().ok()
}

impl MutationEngine {
    /// Create a new authoritative Mutation Engine
    pub fn new(event_chain: Arc<RwLock<EventChain>>, ovsdb: Arc<OvsdbDbusClient>) -> Self {
        Self::new_with_tched_router_client(
            event_chain,
            ovsdb,
            crate::tched_router_runtime::TchedRouterRuntimeClient::from_env(),
        )
    }

    /// Create an engine using an explicit ZeroClaw endpoint.
    ///
    /// This is primarily useful for integration tests, where the runtime API is
    /// provided by an ephemeral local server instead of the host daemon.
    pub fn new_with_tched_router_runtime(
        event_chain: Arc<RwLock<EventChain>>,
        ovsdb: Arc<OvsdbDbusClient>,
        endpoint: String,
        agent_alias: String,
        token: Option<String>,
    ) -> Self {
        Self::new_with_tched_router_client(
            event_chain,
            ovsdb,
            crate::tched_router_runtime::TchedRouterRuntimeClient::new(
                endpoint,
                agent_alias,
                token,
            ),
        )
    }

    fn new_with_tched_router_client(
        event_chain: Arc<RwLock<EventChain>>,
        ovsdb: Arc<OvsdbDbusClient>,
        tched_router_runtime: crate::tched_router_runtime::TchedRouterRuntimeClient,
    ) -> Self {
        let (change_tx, _) = broadcast::channel(1024);
        let (chain_tx, _) = broadcast::channel(1024);
        Self {
            event_chain,
            change_tx,
            chain_tx,
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            schema_cache: Arc::new(RwLock::new(BTreeMap::new())),
            catalog_hash: Arc::new(RwLock::new(String::new())),
            dbus_connection: Arc::new(OnceCell::new()),
            session_bus: Arc::new(OnceCell::new()),
            signal_bus: Arc::new(OnceCell::new()),
            dbus_call_limiter: Arc::new(Semaphore::new(32)),
            audit_sink: Arc::new(OnceCell::new()),
            ovsdb,
            unix_socket: Arc::new(op_plugins::state_plugins::UnixSocketPlugin::new()),
            tched_router_runtime: Arc::new(tched_router_runtime),
            cognitive_tool_registry: std::sync::RwLock::new(None),
            cognitive_context_engine: std::sync::RwLock::new(None),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Attach the in-process cognitive MCP registry constructed at bridge startup.
    pub fn attach_cognitive_mcp(
        &self,
        registry: Option<Arc<ToolRegistry>>,
        context: Option<CognitiveContextState>,
    ) {
        if let Ok(mut guard) = self.cognitive_tool_registry.write() {
            *guard = registry;
        }
        if let Ok(mut guard) = self.cognitive_context_engine.write() {
            *guard = context;
        }
    }

    fn cognitive_tool_registry(&self) -> Option<Arc<ToolRegistry>> {
        self.cognitive_tool_registry
            .read()
            .ok()
            .and_then(|guard| guard.clone())
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
    ///
    /// Only the newest [`replay_limit`] records are replayed. The boot path
    /// must not rebuild the entire chain: the trail is append-only and the full
    /// scan cost minutes of startup and tens of GB of RSS. Records older than
    /// the window stay on disk and are still served from there, so nothing is
    /// lost — but a `verify_range` starting at genesis must read the durable
    /// trail rather than the in-memory chain.
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

        // Select the newest records by filename *before* reading any of them.
        //
        // The trail is append-only and grows without bound (over 1.7M records
        // on the primary host). Reading and parsing all of it held every record
        // in memory at once — tens of GB — and delayed the listener by minutes
        // on every boot. Only the tail is needed to answer live queries; the
        // full trail stays on disk, which remains the authority for
        // verification from genesis.
        let limit = replay_limit();
        let mut newest: std::collections::BinaryHeap<(std::cmp::Reverse<u64>, std::path::PathBuf)> =
            std::collections::BinaryHeap::new();
        let mut candidates = 0usize;

        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(block) = block_number_from_name(&path) else {
                continue;
            };
            candidates += 1;
            newest.push((std::cmp::Reverse(block), path));
            if limit > 0 && newest.len() > limit {
                // Heap is ordered by Reverse(block), so the root is the OLDEST
                // retained record — exactly the one to drop.
                newest.pop();
            }
        }

        let mut selected: Vec<std::path::PathBuf> = newest
            .into_sorted_vec()
            .into_iter()
            .map(|(_, path)| path)
            .collect();
        // `into_sorted_vec` on Reverse(block) yields newest-first; replay must
        // run oldest-first so `replay_event`'s ordering check passes.
        selected.reverse();

        if limit > 0 && candidates > selected.len() {
            tracing::info!(
                candidates,
                replaying = selected.len(),
                limit,
                "audit trail truncated for replay; older records remain on disk"
            );
        }

        // (event_id, event_json) — sorted before replay so linkage is exact.
        let mut records: Vec<(u64, serde_json::Value)> = Vec::new();
        let mut skipped = 0usize;

        for path in selected {
            let bytes = match tokio::fs::read(&path).await {
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
        let session = self.session_context_for_actor(&event.actor_id).await;
        if let Err(error) = sink
            .add_footprint(event_to_footprint(event, session.as_ref()))
            .await
        {
            tracing::warn!(
                %error,
                event_id = event.event_id,
                "audit durability write failed; event retained in memory only"
            );
        }
    }

    /// Advance the session's own record to the chain position this mutation
    /// just took.
    ///
    /// The record answers "given this session, which slice of the chain is it,
    /// and against which contract" — so it has to move for every mutation.
    ///
    /// This is the mutation-path writer of FR-6: it owns `mutation_index` and
    /// `genesis` and touches nothing else, so it can never overwrite the
    /// stream path's liveness fields. `mutation_index` advances only. A
    /// mutation that belongs to no known session advances nothing — a chain
    /// position belongs to a session or to no record at all; it is never
    /// written to a shared last-write-wins file.
    async fn advance_session_record(&self, event_id: u64, actor_id: &str) {
        let Some(session) = self.ensure_session_context(actor_id).await else {
            return;
        };
        if let Err(error) = crate::identity_sled_dispatch::advance_mutation_index(
            self,
            &session.session_id,
            event_id,
        )
        .await
        {
            tracing::warn!(%error, event_id, "session record advance failed");
        }
    }

    /// The verified session identity for `session_id`, if one has arrived.
    pub async fn session_context(&self, session_id: &str) -> Option<SessionContext> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// Resolve the session a mutation belongs to from its actor.
    ///
    /// Accepts whichever handle the caller happens to hold — the session id,
    /// the WireGuard pubkey, the trace, or the genesis itself — because all of
    /// them are fields of the same record. Nothing is derived here: the record
    /// is the author and this is a read of it, warmed into the in-process map
    /// so the chain stamper does not re-read per event.
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
        // Restart-warm: after a cold start the map is empty but the records
        // hydrated from Cozo already carry their anchors.
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

    /// The session identity for `actor_id`, minting the anchor when this is the
    /// session's arrival (FR-1: arrival is mutation one).
    ///
    /// A record that exists with no genesis is a session that has not arrived
    /// yet — either its first mutation is happening now, or the process
    /// restarted before the mint reached Cozo (§5.2). Both are handled the
    /// same way: mint inline, once, before the mutation proceeds. An actor with
    /// no record at all mints nothing; a chain position belongs to a session or
    /// to no record at all.
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
                    "session arrival could not be anchored; the mutation is \
                     recorded without a session stamp"
                );
                None
            }
        }
    }

    /// Publish a session identity for the mutation path to stamp.
    pub async fn register_session_context(&self, context: SessionContext) {
        self.sessions
            .write()
            .await
            .insert(context.session_id.clone(), context);
    }

    /// Drop the in-process projection of one session's identity.
    ///
    /// The record stays authoritative; this only discards the mutation path's
    /// warm copy, so the next lookup re-reads the record. Session teardown and
    /// crash-recovery re-mint both go through here rather than mutating the map
    /// in place.
    pub async fn forget_session_context(&self, session_id: &str) {
        self.sessions.write().await.remove(session_id);
    }

    /// Mint this session's genesis, store it, and return it (FR-1).
    ///
    /// Called once per session, at arrival — the first authenticated mutation.
    /// A session that already carries a genesis gets that stored value back;
    /// the formula is never evaluated twice for one session.
    ///
    /// The sequence is the durability contract of §5.2: mint, write the record
    /// (cache + SHM projection + inline Cozo persist), record the arrival event
    /// in the chain, then return. Arrival is mutation one, so every login is
    /// durable, auditable, and sliceable like any other mutation.
    ///
    /// OSCAL subid: `mut.service.session-genesis.mint@v1`
    pub async fn mint_and_store_genesis(
        &self,
        session_id: &str,
        wireguard_pubkey: &str,
    ) -> anyhow::Result<String> {
        if session_id.is_empty() {
            anyhow::bail!("session genesis requires a session_id");
        }
        // An already-minted session is never re-minted, not even here.
        if let Some(existing) =
            crate::identity_sled_dispatch::stored_genesis(self, session_id).await
        {
            let context = SessionContext {
                genesis_hex: existing.clone(),
                session_id: session_id.to_string(),
                wireguard_pubkey: wireguard_pubkey.to_string(),
            };
            self.register_session_context(context).await;
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
                    "no published catalog hash at arrival; the anchor binds zeros \
                 and cannot attest which contract this session operated against"
                );
                [0u8; 32]
            });
        let arrival_timestamp = chrono::Utc::now().timestamp();

        let genesis = mint_genesis(
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

        // Inline: the record is durable (or logged as not) before the caller is
        // told the session exists.
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

    /// Record the session's first chain entry — arrival is mutation one.
    ///
    /// The stated consequence of FR-6: every login writes to the chain, so a
    /// session that only ever reads still has a real first element and a
    /// bounded span. The genesis is NOT in the event payload; it reaches the
    /// durable record through `event_to_footprint`'s session stamp, which reads
    /// the session context this arrival just registered.
    ///
    /// OSCAL subid: `evt.service.session-genesis.arrival@v1`
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
        let _ = self.chain_tx.send(event);
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

            // One blobify call per plugin: it yields both the canonical
            // SCHEMA_JSON bytes and the hash that covers them. The catalog
            // write, the hydration cache and the broadcast frame all read from
            // this single value, so no consumer can see a schema and a hash
            // that describe different contracts.
            let canonical_id = op_blob::canonical_plugin_id(&plugin_id);
            let blob = op_blob::blobify_plugin_schema(&plugin_id, schema.clone());

            // Auto-generate or refresh the sealed blob when the running
            // schema hash differs from the active SHM catalog.
            let mut resealed = false;
            if let Some(store) = blob_store.as_mut() {
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
                            resealed = true;
                        }
                        Err(error) => {
                            tracing::warn!(plugin_id = %canonical_id, %error, "failed to seal current plugin schema");
                        }
                    }
                }
            }

            // Cache the running contract for stream hydration. This happens
            // whether or not the blob moved: a subscriber connecting later
            // needs the contract regardless of when it was last sealed.
            self.cache_plugin_schema(&canonical_id, &blob).await;

            // A reseal during a live process is the only moment the contract
            // actually changes under an existing subscriber. At startup there
            // are none, so this send is a no-op by design — hydration is what
            // carries schema to the UI. It exists for resealing paths that run
            // after the bridge is serving.
            if resealed {
                self.broadcast_schema_change(&canonical_id).await;
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

        // Take the catalog identity after all sealing is done, from op_blob's
        // single implementation. Recomputing it here would be a second
        // derivation of a value that is only allowed to have one.
        if let Some(store) = blob_store.as_ref() {
            let hash = store.catalog_hash();
            tracing::info!(catalog_hash = %hash, "published catalog identity");
            *self.catalog_hash.write().await = hash;
        }

        tracing::info!(
            projections = seeded,
            blobs = sealed,
            "Seeded missing plugin projections and sealed current plugin schemas"
        );
        if let Err(error) = self.refresh_tched_router_projection().await {
            tracing::warn!(
                %error,
                "ZeroClaw runtime was unavailable during startup projection refresh"
            );
        }
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
        // Fan the recorded event out to EventChainService.SubscribeEvents from
        // the same pipeline that appended it, after the projection write above.
        let _ = self.chain_tx.send(event);
        Ok(change)
    }

    /// Start the Mutation Engine background tasks.
    /// Subscribes to authoritative RCP stores and broadcasts changes.
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let me = self.clone();

        // Subscribe to OVSDB updates (native Idl monitor → process_authoritative_change).
        let ovsdb_self = me.clone();
        tokio::spawn(async move {
            match ovsdb_self.ovsdb.monitor_db("Open_vSwitch").await {
                Ok(mut rx) => {
                    tracing::info!("MutationEngine: OVSDB monitor_db subscribed");
                    loop {
                        match rx.recv().await {
                            Ok(update) => {
                                let Some(tables) = ovsdb_monitor_tables(&update) else {
                                    tracing::debug!(
                                        "OVSDB monitor update had no table map; skipping"
                                    );
                                    continue;
                                };
                                for (table_name, table_update) in tables.iter() {
                                    let table_name_owned = table_name.to_string();
                                    // monitor_db returns serde_json::Value; convert to
                                    // simd_json::OwnedValue required by process_authoritative_change.
                                    let simd_val: simd_json::OwnedValue = {
                                        match serde_json::to_string(table_update).ok().and_then(
                                            |s| {
                                                let mut b = s.into_bytes();
                                                simd_json::to_owned_value(&mut b).ok()
                                            },
                                        ) {
                                            Some(v) => v,
                                            None => continue,
                                        }
                                    };
                                    if let Err(e) = ovsdb_self
                                        .process_authoritative_change(
                                            "net".to_string(),
                                            format!("/org/opdbus/v1/ovsdb/{}", table_name_owned),
                                            ChangeType::PropertySet,
                                            Some(table_name_owned),
                                            None,
                                            simd_val,
                                            vec!["ovsdb".to_string(), "network".to_string()],
                                            "ovsdb-monitor".to_string(),
                                            None,
                                            ChangeSource::DBus,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            error = %e,
                                            "OVSDB monitor → process_authoritative_change failed"
                                        );
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
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "MutationEngine: OVSDB monitor_db unavailable; continuing without it"
                    );
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

        // A caller that names no actor is anonymous. This used to read the
        // global 152-byte sled at `/dev/shm/plugin_schema.dat` and label the
        // mutation with whatever session wrote that file last — the shared
        // last-write-wins store FR-6 retires. Identity arrives with the
        // request or not at all.
        let actor_id = if actor_id.is_empty() {
            ANONYMOUS_ACTOR.to_string()
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
            // unix_socket plugin dispatch: createunixsocket <name> <ports csv/vec>.
            // The method registers the name+ports routing tag against the
            // shared container.sock transport; the transport owner is not
            // replaced during registration.
            if let Some(method) = &member_name {
                if method == "createunixsocket" || method == "bind" {
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
        } else if plugin_id == "emqx" && change_type == ChangeType::MethodCall {
            if let Some(method) = &member_name {
                let mut args_json = serde_json::to_value(&value)?;
                if let serde_json::Value::Array(items) = &args_json {
                    args_json = items
                        .first()
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
                }
                let result =
                    op_plugins::state_plugins::emqx::dispatch_emqx_method(method, &args_json)
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
                actor_id.clone(),
                capability_id,
                ChangeSource::Grpc,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Advance this session's own record to the new chain position.
        self.advance_session_record(change.event_id, &actor_id)
            .await;

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
        // Same fan-out as process_authoritative_change: the event is already in
        // the chain and durable, so audit subscribers can see it now.
        let _ = self.chain_tx.send(event_summary.3);

        // Same position record as the property-set path — a method call is a
        // mutation and occupies a chain position like any other.
        self.advance_session_record(event_summary.0, actor_id).await;

        // Dispatch to appropriate backend based on plugin_id
        let method_result: serde_json::Value = match plugin_id {
            "rovs_commands" => {
                dispatch_rovs_commands_method(&self.ovsdb, method, &parsed_value).await?
            }
            "ovsdb_bridge" => {
                dispatch_ovsdb_bridge_method(&self.ovsdb, method, &parsed_value).await?
            }
            "rtnetlink" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::rtnetlink::dispatch_rtnetlink_method(method, &args)
                    .await?
            }
            "xray" => {
                let args = serde_json::to_value(&parsed_value)?;
                dispatch_xray_method(method, &args).await?
            }
            "unix_socket" if method == "bind" || method == "createunixsocket" => {
                // The schema router calls dispatch_method_call directly.  Keep
                // socket registration on this path so bind is not reduced to
                // the generic echo result and the shared transport metadata is
                // visible to subsequent readers.
                let (name, ports) = parse_socket_args(&parsed_value);
                let applied = self
                    .unix_socket
                    .create_unix_socket(name.clone(), ports.clone());
                if !applied.success {
                    return Err(anyhow::anyhow!(applied.errors.join("; ")));
                }
                let state = unix_socket_state_after_registration(&name, &ports);
                self.update_state_cache("unix_socket".to_string(), state.clone())
                    .await;
                let state_json = serde_json::to_vec(&state)?;
                op_core::projection_shm::write_projection("unix_socket", &state_json)
                    .map_err(|e| anyhow::anyhow!("persist unix_socket bind state: {e}"))?;
                serde_json::json!({
                    "name": name,
                    "path": op_plugins::state_plugins::unix_socket::SHARED_CONTAINER_SOCKET,
                    "ports": ports,
                    "protocol": parsed_value
                        .get("protocol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("grpc"),
                })
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
            "procfs" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::procfs::dispatch_procfs_method(method, &args).await?
            }
            "host_runtime" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::host_runtime::dispatch_host_runtime_method(method, &args)
                    .await?
            }
            "service" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::service::dispatch_service_method(method, &args).await?
            }
            "privacy_routes" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::privacy_routes::dispatch_privacy_routes_method(
                    method, &args,
                )
                .await?
            }
            "btrfs" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::btrfs_plugin::dispatch_btrfs_method(method, &args)
                    .await?
            }
            "mail_server" | "mail" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::mail_server::dispatch_mail_server_method(method, &args)
                    .await?
            }
            "incus" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::incus::dispatch_incus_method(method, &args).await?
            }
            "emqx" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::emqx::dispatch_emqx_method(method, &args).await?
            }
            "antigravity" => {
                let state = self
                    .projected_state::<op_plugins::state_plugins::antigravity::AntigravityState>(
                        "antigravity",
                    )
                    .await
                    .unwrap_or_else(
                        op_plugins::state_plugins::antigravity::AntigravityPlugin::current_state,
                    );
                op_plugins::state_plugins::antigravity::dispatch_antigravity_method(
                    method, json_args, &state,
                )?
            }
            "tched_router" => {
                let declared_state = self
                    .projected_state::<op_plugins::state_plugins::tched_router::TchedRouterState>(
                        "tched_router",
                    )
                    .await
                    .unwrap_or_else(
                        op_plugins::state_plugins::tched_router::TchedRouterPlugin::current_state,
                    );
                let mut state = self
                    .tched_router_runtime
                    .project_state(declared_state)
                    .await
                    .context("ZeroClaw runtime state projection failed")?;
                self.cache_tched_router_state(&state).await?;
                // SetSelection publishes only after both fields have been
                // validated and installed in the same cached projection.
                if method != "SetSelection" {
                    if let Err(error) = self
                        .publish_plugin_projection_from_cache(
                            "tched_router",
                            ChangeType::PropertySet,
                        )
                        .await
                    {
                        tracing::warn!(
                            %error,
                            "unable to publish refreshed ZeroClaw state projection"
                        );
                    }
                }
                if method == "Chat" {
                    let args = serde_json::from_value::<
                        op_plugins::state_plugins::tched_router::ChatInput,
                    >(serde_json::to_value(&parsed_value)?)
                    .context("invalid tched_router.Chat arguments")?;
                    serde_json::to_value(self.tched_router_runtime.chat(&state, args).await?)?
                } else if op_plugins::state_plugins::tched_router::RUNTIME_EXECUTED_METHODS
                    .contains(&method)
                {
                    // Daemon-backed methods: the plugin declares them, this
                    // crate executes them and remains the audit boundary.
                    let result = self
                        .tched_router_runtime
                        .dispatch_runtime_method(method, json_args)
                        .await?;
                    if op_plugins::state_plugins::tched_router::tched_router_method_mutates(method)
                    {
                        self.persist_tched_router_mutation(method, &result).await?;
                        // Config changed underneath the projection, so re-read
                        // rather than serving the pre-mutation snapshot.
                        if let Err(error) = self.refresh_tched_router_projection().await {
                            tracing::warn!(
                                %error,
                                "unable to refresh ZeroClaw projection after {method}"
                            );
                        }
                    }
                    result
                } else {
                    match op_plugins::state_plugins::tched_router::dispatch_tched_router_method(
                        method, json_args, &state,
                    ) {
                        Ok(outcome) => {
                            match method {
                                "SetProvider" => {
                                    let provider_id = outcome
                                        .result
                                        .get("selected_provider")
                                        .and_then(serde_json::Value::as_str)
                                        .context("SetProvider returned no selected_provider")?;
                                    let selection =
                                        self.tched_router_runtime.set_provider(provider_id).await?;
                                    state.selected_provider = selection.provider;
                                    state.selected_model = selection.model;
                                    state.projection.router.provider =
                                        state.selected_provider.clone();
                                    state.projection.router.model = state.selected_model.clone();
                                    self.cache_tched_router_state(&state).await?;
                                }
                                "SetModel" => {
                                    let model_id = outcome
                                        .result
                                        .get("selected_model")
                                        .and_then(serde_json::Value::as_str)
                                        .context("SetModel returned no selected_model")?;
                                    let selection = self
                                        .tched_router_runtime
                                        .set_model(&state.selected_provider, model_id)
                                        .await?;
                                    state.selected_provider = selection.provider;
                                    state.selected_model = selection.model;
                                    state.projection.router.provider =
                                        state.selected_provider.clone();
                                    state.projection.router.model = state.selected_model.clone();
                                    self.cache_tched_router_state(&state).await?;
                                }
                                "SetSelection" => {
                                    let provider_id = outcome
                                        .result
                                        .get("selected_provider")
                                        .and_then(serde_json::Value::as_str)
                                        .context("SetSelection returned no selected_provider")?;
                                    let model_id = outcome
                                        .result
                                        .get("selected_model")
                                        .and_then(serde_json::Value::as_str)
                                        .context("SetSelection returned no selected_model")?;
                                    let selection = self
                                        .tched_router_runtime
                                        .set_selection(provider_id, model_id)
                                        .await?;
                                    state.selected_provider = selection.provider;
                                    state.selected_model = selection.model;
                                    state.projection.router.provider =
                                        state.selected_provider.clone();
                                    state.projection.router.model = state.selected_model.clone();
                                    self.cache_tched_router_state(&state).await?;
                                }
                                _ => {}
                            }
                            if method.starts_with("Set") {
                                self.persist_tched_router_mutation(method, &outcome.result)
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
                dispatch_cognitive_mcp_method(&self.cognitive_tool_registry(), method, json_args)
                    .await?
            }
            "large_language_model" => {
                let args = serde_json::to_value(&parsed_value)?;
                op_plugins::state_plugins::large_language_model::dispatch_large_language_model_method(
                    method, &args,
                )
                .await?
            }
            "json_render" => {
                let state = self
                    .projected_state::<op_plugins::state_plugins::json_render::JsonRenderState>(
                        "json_render",
                    )
                    .await
                    .unwrap_or_else(
                        op_plugins::state_plugins::json_render::JsonRenderPlugin::current_state,
                    );
                op_plugins::state_plugins::json_render::dispatch_json_render_method(method, &state)?
            }
            "agent_config" => {
                let state = self
                    .projected_state::<op_plugins::state_plugins::agent_config::AgentConfigState>(
                        "agent_config",
                    )
                    .await
                    .unwrap_or_else(
                        op_plugins::state_plugins::agent_config::AgentConfigPlugin::current_state,
                    );
                op_plugins::state_plugins::agent_config::dispatch_agent_config_method(
                    method, &state,
                )?
            }
            "wireguard" => {
                let state = self
                    .projected_state::<op_plugins::state_plugins::wireguard::WireGuardState>(
                        "wireguard",
                    )
                    .await
                    .unwrap_or_else(
                        op_plugins::state_plugins::wireguard::WireGuardPlugin::current_state,
                    );
                op_plugins::state_plugins::wireguard::dispatch_wireguard_method(method, &state)?
            }
            "qdrant" => {
                let args = serde_json::to_value(&parsed_value)?;
                let method_result =
                    op_plugins::state_plugins::qdrant::dispatch_qdrant_method(method, &args)
                        .await?;
                // Publish observed collections into the mutation cache so
                // StateSync / GetState (json-render $state) see live data.
                if method == "list_collections" {
                    if let Some(names) = method_result
                        .get("collections")
                        .and_then(serde_json::Value::as_array)
                    {
                        let template =
                            op_plugins::state_plugins::qdrant::QdrantPlugin::exemplar_state();
                        let template_collection = template.collections.first().cloned();
                        let mut state = template;
                        state.collections = names
                            .iter()
                            .filter_map(|n| n.as_str())
                            .filter_map(|name| {
                                let mut c = template_collection.clone()?;
                                c.name = name.to_string();
                                Some(c)
                            })
                            .collect();
                        if let Ok(owned) = simd_json::serde::to_owned_value(&state) {
                            self.update_state_cache("qdrant".to_string(), owned.clone())
                                .await;
                            let bytes = serde_json::to_vec(&owned).unwrap_or_default();
                            let _ = op_core::projection_shm::write_projection("qdrant", &bytes);
                        }
                    }
                }
                method_result
            }
            // gemma_brain is semi-deprecated for inference; UI uses large_language_model.
            "gemma_brain" => {
                return Err(anyhow::anyhow!(
                    "gemma_brain.{method} is not on the UI mutation path; use large_language_model"
                ));
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
            // UI data-plane plugins must never silently echo empty args.
            ui_plugin
                if matches!(
                    ui_plugin,
                    "large_language_model"
                        | "json_render"
                        | "agent_config"
                        | "wireguard"
                        | "host_runtime"
                        | "gemma_brain"
                        | "s6_systemctl"
                        | "qdrant"
                ) =>
            {
                return Err(anyhow::anyhow!(
                    "plugin '{ui_plugin}' method '{method}' has no mutation dispatch arm; refuse echo"
                ));
            }
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

    async fn persist_tched_router_mutation(
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

    async fn cache_tched_router_state(
        &self,
        state: &op_plugins::state_plugins::tched_router::TchedRouterState,
    ) -> anyhow::Result<()> {
        let owned = simd_json::serde::to_owned_value(state)?;
        self.update_state_cache("tched_router".to_string(), owned)
            .await;
        Ok(())
    }

    async fn refresh_tched_router_projection(&self) -> anyhow::Result<()> {
        let declared_state = self
            .projected_state::<op_plugins::state_plugins::tched_router::TchedRouterState>(
                "tched_router",
            )
            .await
            .unwrap_or_else(
                op_plugins::state_plugins::tched_router::TchedRouterPlugin::current_state,
            );
        let state = self
            .tched_router_runtime
            .project_state(declared_state)
            .await?;
        self.cache_tched_router_state(&state).await?;
        self.publish_plugin_projection_from_cache("tched_router", ChangeType::PropertySet)
            .await
    }

    /// Merge a flat JSON object of changed fields into the authoritative
    /// in-memory state cache for `plugin_id` (used to persist TchedRouter `Set*`
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

    /// Take one coherent copy of the present-state cache for a new stream
    /// subscriber. The cache is hydrated from the SHM static tree before the
    /// server starts accepting requests, so this is not a D-Bus fan-out.
    pub async fn state_snapshot(&self) -> Vec<(String, simd_json::OwnedValue)> {
        let cache = self.state_cache.read().await;
        let mut snapshot: Vec<_> = cache
            .iter()
            .map(|(plugin_id, state)| (plugin_id.clone(), state.clone()))
            .collect();
        snapshot.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        snapshot
    }

    /// Update the authoritative state cache
    pub async fn update_state_cache(&self, plugin_id: String, state: simd_json::OwnedValue) {
        let mut cache = self.state_cache.write().await;
        cache.insert(plugin_id, state);
    }

    /// Record one plugin's sealed contract for stream hydration.
    ///
    /// Takes the blob rather than a schema so the cached bytes are the sealed
    /// bytes; parsing is the only transformation applied.
    async fn cache_plugin_schema(&self, canonical_id: &str, blob: &op_blob::PluginObjectBlob) {
        let mut bytes = blob.schema_json.clone().into_bytes();
        let schema_json = match simd_json::to_owned_value(&mut bytes) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(
                    plugin_id = %canonical_id,
                    %error,
                    "sealed schema JSON did not parse; plugin will hydrate without a contract"
                );
                return;
            }
        };
        // Mirror into the in-process catalog the cognitive MCP tools read, so a
        // model calling `blob_catalog` sees the same contract this engine puts
        // on the stream instead of independently re-reading SHM. Parsed from
        // the same sealed string, so the two views cannot describe different
        // contracts. Startup-only work, once per plugin.
        match serde_json::from_str::<serde_json::Value>(&blob.schema_json) {
            Ok(value) => op_cognitive_mcp::live_schema::global().publish(
                canonical_id,
                blob.manifest.schema_hash.clone(),
                value,
            ),
            Err(error) => tracing::warn!(
                plugin_id = %canonical_id,
                %error,
                "sealed schema JSON did not parse for the MCP mirror; tools will read SHM"
            ),
        }

        self.schema_cache.write().await.insert(
            canonical_id.to_string(),
            SchemaSnapshot {
                schema_hash: blob.manifest.schema_hash.clone(),
                schema_json,
            },
        );
    }

    /// Take a coherent copy of every known plugin contract, for a new stream
    /// subscriber. Ordered by canonical plugin id.
    pub async fn schema_snapshot(&self) -> Vec<(String, SchemaSnapshot)> {
        self.schema_cache
            .read()
            .await
            .iter()
            .map(|(plugin_id, snapshot)| (plugin_id.clone(), snapshot.clone()))
            .collect()
    }

    /// The contract currently published for one plugin, if any.
    pub async fn plugin_schema(&self, canonical_id: &str) -> Option<SchemaSnapshot> {
        self.schema_cache.read().await.get(canonical_id).cloned()
    }

    /// Hash of the contract one plugin is published under, or empty when the
    /// plugin has no sealed contract in this process.
    pub async fn plugin_schema_hash(&self, canonical_id: &str) -> String {
        self.schema_cache
            .read()
            .await
            .get(canonical_id)
            .map(|snapshot| snapshot.schema_hash.clone())
            .unwrap_or_default()
    }

    /// Identity of the published catalog. Empty before the seal pass has run.
    pub async fn catalog_hash(&self) -> String {
        self.catalog_hash.read().await.clone()
    }

    /// Announce a contract change on the state stream.
    ///
    /// Carries the sealed schema itself, not a pointer to it: the frame is the
    /// delivery, so a consumer never has to reach back into the catalog. The
    /// hash rides in `member_name` so a consumer can tell two contracts apart
    /// without diffing them.
    async fn broadcast_schema_change(&self, canonical_id: &str) {
        let Some(snapshot) = self.plugin_schema(canonical_id).await else {
            return;
        };
        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: 0,
            plugin_id: canonical_id.to_string(),
            object_path: format!("/org/opdbus/v1/plugins/{canonical_id}"),
            change_type: ChangeType::SchemaMigration,
            member_name: Some(snapshot.schema_hash),
            old_value: None,
            new_value: snapshot.schema_json,
            tags_touched: vec![],
            event_hash: String::new(),
            timestamp: chrono::Utc::now(),
            actor_id: "schema_seal".to_string(),
            source: ChangeSource::Internal,
        };
        let _ = self.change_tx.send(change);
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

    /// Audit-chain fan-out backing `EventChainService.SubscribeEvents`.
    ///
    /// Every event appended to the chain by this engine is published here; there
    /// is no other producer.
    pub fn chain_tx(&self) -> broadcast::Sender<ChainEvent> {
        self.chain_tx.clone()
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
/// The session stamp (FR-3) makes the durable chain sliceable by session and
/// re-verifiable offline: `session_genesis` / `session_id` /
/// `wireguard_pubkey` are copied from the session context the arrival minted.
/// A mutation that belongs to no session carries the keys with empty values —
/// present and honest, never absent.
///
/// OSCAL subid: `evt.service.event-chain.persist@v1`, `evt.service.event-chain.session-stamp@v1`
fn event_to_footprint(event: &ChainEvent, session: Option<&SessionContext>) -> PluginFootprint {
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
    // Session identity stamp (FR-3).
    let empty = SessionContext::default();
    let session = session.unwrap_or(&empty);
    metadata.insert(
        "session_genesis".to_string(),
        simd_json::OwnedValue::from(session.genesis_hex.as_str()),
    );
    metadata.insert(
        "session_id".to_string(),
        simd_json::OwnedValue::from(session.session_id.as_str()),
    );
    metadata.insert(
        "wireguard_pubkey".to_string(),
        simd_json::OwnedValue::from(session.wireguard_pubkey.as_str()),
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
fn parse_rovs_command_input<T: serde::de::DeserializeOwned>(
    args: &simd_json::OwnedValue,
) -> anyhow::Result<T> {
    serde_json::from_value(serde_json::to_value(args)?).map_err(Into::into)
}

async fn dispatch_rovs_commands_method(
    ovsdb: &op_network::rovs_proxy::OvsdbDbusClient,
    method: &str,
    args: &simd_json::OwnedValue,
) -> anyhow::Result<serde_json::Value> {
    match method {
        "create_bridge" => {
            let input: op_plugins::state_plugins::rovs_commands::CreateBridgeInput =
                parse_rovs_command_input(args)?;
            ovsdb.create_bridge(&input.bridge_name).await?;
            Ok(serde_json::to_value(
                op_plugins::state_plugins::rovs_commands::CreateBridgeOutput {
                    bridge_name: input.bridge_name,
                },
            )?)
        }
        "delete_bridge" => {
            let input: op_plugins::state_plugins::rovs_commands::DeleteBridgeInput =
                parse_rovs_command_input(args)?;
            ovsdb.delete_bridge(&input.bridge_name).await?;
            Ok(serde_json::to_value(
                op_plugins::state_plugins::rovs_commands::DeleteBridgeOutput {
                    bridge_name: input.bridge_name,
                },
            )?)
        }
        "add_port" => {
            let input: op_plugins::state_plugins::rovs_commands::AddPortInput =
                parse_rovs_command_input(args)?;
            ovsdb.add_port(&input.bridge_name, &input.port_name).await?;
            Ok(serde_json::to_value(
                op_plugins::state_plugins::rovs_commands::AddPortOutput {
                    bridge_name: input.bridge_name,
                    port_name: input.port_name,
                },
            )?)
        }
        "remove_port" => {
            let input: op_plugins::state_plugins::rovs_commands::RemovePortInput =
                parse_rovs_command_input(args)?;
            ovsdb
                .delete_port(&input.bridge_name, &input.port_name)
                .await?;
            Ok(serde_json::to_value(
                op_plugins::state_plugins::rovs_commands::RemovePortOutput {
                    bridge_name: input.bridge_name,
                    port_name: input.port_name,
                },
            )?)
        }
        "list_bridges" => {
            let bridges = ovsdb.list_bridges().await?;
            Ok(serde_json::to_value(
                op_plugins::state_plugins::rovs_commands::ListBridgesOutput { bridges },
            )?)
        }
        "list_ports" => {
            let input: op_plugins::state_plugins::rovs_commands::ListPortsInput =
                parse_rovs_command_input(args)?;
            let ports = ovsdb.list_bridge_ports(&input.bridge_name).await?;
            Ok(serde_json::to_value(
                op_plugins::state_plugins::rovs_commands::ListPortsOutput {
                    bridge_name: input.bridge_name,
                    ports,
                },
            )?)
        }
        "list_dbs" => {
            let databases = ovsdb.list_dbs().await?;
            Ok(serde_json::to_value(
                op_plugins::state_plugins::rovs_commands::ListDbsOutput { databases },
            )?)
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
            let port_name = args
                .get("port")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("port_name required"))?;
            ovsdb
                .delete_port(
                    args.get("bridge")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default(),
                    port_name,
                )
                .await?;
            Ok(serde_json::json!({"removed": port_name}))
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
            // subprocess — signals the real PID directly via /proc + libc::kill.
            let pids = find_xray_pids();
            if pids.is_empty() {
                return Err(anyhow::anyhow!("no running xray process found"));
            }
            let mut reloaded = Vec::new();
            for pid in pids {
                signal_process(pid, libc::SIGHUP)
                    .with_context(|| format!("sending SIGHUP to xray pid {pid}"))?;
                reloaded.push(pid);
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
fn find_xray_pids() -> Vec<i32> {
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
                pids.push(pid);
            }
        }
    }
    pids
}

fn signal_process(pid: i32, sig: i32) -> Result<(), std::io::Error> {
    let rc = unsafe { libc::kill(pid, sig) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Dispatch a `cognitive_mcp` schema method to the cognitive tool registry.
///
/// OSCAL subid: mut.service.cognitive-mcp.dispatch@v1
///
/// Phase 2 transport is an in-process `ToolRegistry::execute` / `list` call. The
/// bridge remains the only authorization door: the method-existence gate, arg
/// validation, capability check and event-chain append have all already happened in
/// `dispatch_method_call` before this function is reached.
async fn dispatch_cognitive_mcp_method(
    tool_registry: &Option<Arc<ToolRegistry>>,
    method: &str,
    json_args: &str,
) -> anyhow::Result<serde_json::Value> {
    // Plugin-local methods: unchanged from Phase 1
    match method {
        "get_config" | "get_health" => {
            let bytes = op_core::projection_shm::read_projection_bytes("cognitive_mcp")
                .ok_or_else(|| anyhow::anyhow!("cognitive_mcp projection not available"))?;
            let val: serde_json::Value = serde_json::from_slice(&bytes)?;
            return Ok(val);
        }
        "set_config" | "restart_service" => {
            return Ok(serde_json::json!({"acknowledged": true, "method": method}));
        }
        _ => {}
    }

    let registry = tool_registry
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cognitive_mcp tool registry unavailable (init failed)"))?;

    if method == "list_tools" {
        let reg = registry.clone();
        let timeout = Duration::from_secs(
            std::env::var("COGNITIVE_TOOL_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        );
        return tokio::time::timeout(timeout, async move {
            let defs = reg.list(0, usize::MAX, None).await;
            cognitive_tool_catalog_response(defs)
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "tool execution timed out after {}s: list_tools",
                timeout.as_secs()
            )
        })?
        .map_err(|e| anyhow::anyhow!("tool execution failed: {}", e));
    }

    let args: serde_json::Value = if json_args.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(json_args)?
    };
    let (tool_name, tool_args) = map_schema_method_to_tool(method, &args)?;

    // Execute with timeout on a spawned task to avoid blocking D-Bus loop
    let reg = registry.clone();
    let name = tool_name.clone();
    let timeout = Duration::from_secs(
        std::env::var("COGNITIVE_TOOL_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
    );

    let result = tokio::time::timeout(timeout, async move {
        let mut payload = tool_args.to_string().into_bytes();
        let tool_input =
            simd_json::to_owned_value(&mut payload).context("invalid cognitive tool arguments")?;
        reg.execute(&name, tool_input)
            .await
            .map(|v| serde_json::to_value(&v).unwrap_or(serde_json::Value::Null))
    })
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "tool execution timed out after {}s: {}",
            timeout.as_secs(),
            tool_name
        )
    })?
    .map_err(|e| anyhow::anyhow!("tool execution failed: {}", e))?;

    // `invoke_tool` is the one schema-typed door to the runtime registry. Its
    // domain response is intentionally stable even though each invoked tool
    // returns its own payload shape; callers can inspect `result` using the
    // input contract published by `list_tools`.
    if method == "invoke_tool" {
        return serde_json::to_value(op_plugins::state_plugins::cognitive_mcp::InvokeToolOutput {
            success: true,
            tool_name,
            result: Some(result),
            error: None,
        })
        .context("serialize cognitive invoke_tool response");
    }

    Ok(result)
}

fn cognitive_tool_catalog_response(
    definitions: Vec<op_core::ToolDefinition>,
) -> anyhow::Result<serde_json::Value> {
    let mut tools = definitions
        .into_iter()
        .map(|tool| {
            Ok(serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "category": tool.category,
                "tags": tool.tags,
                "namespace": tool.namespace,
                "input_schema": serde_json::to_value(tool.input_schema)
                    .context("serialize cognitive tool input schema")?,
                "schema_version": tool.schema_version,
            }))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    // ToolRegistry storage is a HashMap.  A stable order keeps catalog views,
    // generated specs, and tests deterministic across process restarts.
    tools.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));

    Ok(serde_json::json!({ "tools": tools }))
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
        "development_upsert" => Ok((
            "cognitive_development".into(),
            with_field("operation", "upsert"),
        )),
        "development_list" => Ok((
            "cognitive_development".into(),
            with_field("operation", "list"),
        )),
        "development_categories" => Ok((
            "cognitive_development".into(),
            with_field("operation", "categories"),
        )),
        "development_summary" => Ok((
            "cognitive_development".into(),
            with_field("operation", "summary"),
        )),
        "development_history" => Ok((
            "cognitive_development".into(),
            with_field("operation", "history"),
        )),
        "development_record_verification" => Ok((
            "cognitive_development".into(),
            with_field("operation", "record_verification"),
        )),
        // Code retrieval / indexing.
        // Code retrieval / indexing (native 1-to-1 mapping now that tools are restored).
        "code_search" => Ok(("code_search".into(), base)),
        "code_index" => Ok(("code_index".into(), base)),
        "code_context" => Ok(("code_context".into(), base)),
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

/// Parse createunixsocket arguments. Accepts either a JSON array
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

/// Extract the table-update map from an OVSDB monitor notification.
///
/// Accepts:
/// - IDL snapshots from `OvsdbDbusClient::monitor_db`: `{ "Bridge": [...], ... }`
/// - RFC 7047 JSON-RPC updates: `{ "params": [<monitor-id>, <table-updates>] }`
/// - Legacy 3-param drafts: `{ "params": [?, ?, <table-updates>] }`
pub(crate) fn ovsdb_monitor_tables(
    update: &serde_json::Value,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
        if let Some(obj) = params.get(1).and_then(|v| v.as_object()) {
            return Some(obj);
        }
        if let Some(obj) = params.get(2).and_then(|v| v.as_object()) {
            return Some(obj);
        }
        return None;
    }
    update.as_object()
}

#[cfg(test)]
mod cognitive_development_dispatch_tests {
    use super::map_schema_method_to_tool;
    use serde_json::json;

    #[test]
    fn development_schema_methods_share_one_tool_with_distinct_operations() {
        for (method, operation) in [
            ("development_upsert", "upsert"),
            ("development_list", "list"),
            ("development_categories", "categories"),
            ("development_record_verification", "record_verification"),
        ] {
            let (tool, args) = map_schema_method_to_tool(method, &json!({"status": "planned"}))
                .expect("development method should map");
            assert_eq!(tool, "cognitive_development");
            assert_eq!(args["operation"], operation);
            assert_eq!(args["status"], "planned");
        }
        assert!(map_schema_method_to_tool("development_missing", &json!({})).is_err());
    }
}

#[cfg(test)]
mod cognitive_tool_catalog_tests {
    use super::{cognitive_tool_catalog_response, dispatch_cognitive_mcp_method};
    use anyhow::Result;
    use async_trait::async_trait;
    use op_core::ToolDefinition;
    use op_mcp::tool_registry::{Tool, ToolRegistry};
    use serde_json::json;
    use simd_json::OwnedValue;
    use std::sync::Arc;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn description(&self) -> &str {
            "Return the supplied arguments for contract testing."
        }

        fn input_schema(&self) -> OwnedValue {
            simd_json::json!({
                "type": "object",
                "properties": { "message": { "type": "string" } },
                "required": ["message"],
            })
        }

        async fn execute(&self, input: OwnedValue) -> Result<OwnedValue> {
            Ok(input)
        }
    }

    fn definition(
        name: &str,
        category: &str,
        tags: &[&str],
        namespace: &str,
        schema_version: &str,
    ) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: format!("{name} description"),
            input_schema: simd_json::json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
            }),
            schema_version: schema_version.to_string(),
            category: category.to_string(),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            namespace: namespace.to_string(),
        }
    }

    #[test]
    fn list_tools_is_a_stable_catalog_with_invocation_contracts() {
        let catalog = cognitive_tool_catalog_response(vec![
            definition("zeta", "operations", &["write"], "system", "v2"),
            definition("alpha", "retrieval", &["read", "rag"], "cognitive", "v1"),
        ])
        .expect("catalog serialization");

        assert_eq!(
            catalog,
            json!({
                "tools": [
                    {
                        "name": "alpha",
                        "description": "alpha description",
                        "category": "retrieval",
                        "tags": ["read", "rag"],
                        "namespace": "cognitive",
                        "input_schema": {
                            "type": "object",
                            "properties": { "query": { "type": "string" } },
                        },
                        "schema_version": "v1",
                    },
                    {
                        "name": "zeta",
                        "description": "zeta description",
                        "category": "operations",
                        "tags": ["write"],
                        "namespace": "system",
                        "input_schema": {
                            "type": "object",
                            "properties": { "query": { "type": "string" } },
                        },
                        "schema_version": "v2",
                    },
                ],
            })
        );
    }

    #[tokio::test]
    async fn invoke_tool_wraps_the_runtime_payload_in_the_declared_contract() {
        let registry = Arc::new(ToolRegistry::new());
        registry
            .register(Arc::new(EchoTool))
            .await
            .expect("register echo tool");

        let result = dispatch_cognitive_mcp_method(
            &Some(registry),
            "invoke_tool",
            r#"{"tool_name":"echo","arguments":{"message":"hello"}}"#,
        )
        .await
        .expect("invoke echo tool");

        assert_eq!(
            result,
            json!({
                "success": true,
                "tool_name": "echo",
                "result": { "message": "hello" },
            })
        );
    }
}

#[cfg(test)]
mod replay_window_tests {
    use super::{block_number_from_name, DEFAULT_REPLAY_LIMIT};
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    use std::path::{Path, PathBuf};

    #[test]
    fn block_number_is_read_from_the_filename() {
        assert_eq!(
            block_number_from_name(Path::new("/t/block-000000000042.json")),
            Some(42)
        );
        assert_eq!(
            block_number_from_name(Path::new("/t/block-001709617.json")),
            Some(1_709_617)
        );
        // Non-block records share the timing directory and must be ignored.
        assert_eq!(block_number_from_name(Path::new("/t/vec-000001.bin")), None);
        assert_eq!(block_number_from_name(Path::new("/t/unrelated.json")), None);
        assert_eq!(block_number_from_name(Path::new("/t/block-abc.json")), None);
    }

    /// The bounded selection must keep the *newest* records and hand them back
    /// oldest-first — replay rejects out-of-order events, so a reversed window
    /// would drop everything after the first record.
    #[test]
    fn window_keeps_newest_and_replays_oldest_first() {
        let limit = 3usize;
        let mut newest: BinaryHeap<(Reverse<u64>, PathBuf)> = BinaryHeap::new();

        for block in [10u64, 1, 7, 4, 9, 2] {
            newest.push((
                Reverse(block),
                PathBuf::from(format!("/t/block-{block:012}.json")),
            ));
            if newest.len() > limit {
                newest.pop();
            }
        }

        let mut selected: Vec<PathBuf> = newest
            .into_sorted_vec()
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        selected.reverse();

        let blocks: Vec<u64> = selected
            .iter()
            .map(|p| block_number_from_name(p).unwrap())
            .collect();
        assert_eq!(blocks, vec![7, 9, 10], "newest three, ascending");
    }

    #[test]
    fn default_limit_is_bounded() {
        assert!(
            DEFAULT_REPLAY_LIMIT > 0,
            "an unbounded default would restore the full-history rebuild this replaces"
        );
    }
}

#[cfg(test)]
mod ovsdb_monitor_tables_tests {
    use super::{ovsdb_monitor_tables, parse_rovs_command_input};
    use serde_json::json;

    #[test]
    fn idl_snapshot_tables() {
        let update = json!({
            "Bridge": [{"name": "br0"}],
            "Port": []
        });
        let tables = ovsdb_monitor_tables(&update).expect("tables");
        assert_eq!(tables["Bridge"][0]["name"], "br0");
    }

    #[test]
    fn rfc7047_params_tables() {
        let update = json!({
            "params": ["mon", {"Interface": {"u": {"new": {"name": "eth0"}}}}]
        });
        let tables = ovsdb_monitor_tables(&update).expect("tables");
        assert!(tables.contains_key("Interface"));
    }

    #[test]
    fn legacy_three_param_tables() {
        let update = json!({
            "params": [null, "mon", {"Bridge": {"u": {"old": {}, "new": {"name": "br1"}}}}]
        });
        let tables = ovsdb_monitor_tables(&update).expect("tables");
        assert!(tables.contains_key("Bridge"));
    }

    #[test]
    fn rovs_add_port_input_is_bound_to_the_typed_plugin_contract() {
        let args = simd_json::json!({
            "bridge_name": "ovsbr0",
            "port_name": "netmaker"
        });
        let input: op_plugins::state_plugins::rovs_commands::AddPortInput =
            parse_rovs_command_input(&args).expect("typed rovs add_port input");
        assert_eq!(input.bridge_name, "ovsbr0");
        assert_eq!(input.port_name, "netmaker");
    }
}

#[cfg(test)]
mod session_genesis_tests {
    use super::*;
    use crate::human_principal_dispatch::tests::pk;
    use crate::identity_sled_dispatch::tests::{sled_engine, write_identity};
    use op_state_store::ChainConfig;

    /// One real chain event per actor, appended to a throwaway chain so the
    /// event carries genuine `prev_hash` linkage rather than hand-built fields.
    fn chain_events(actors: &[&str]) -> Vec<ChainEvent> {
        let mut chain = EventChain::new(ChainConfig::default());
        actors
            .iter()
            .map(|actor| {
                chain
                    .record_method_call(
                        actor.to_string(),
                        "identity_sled".to_string(),
                        "write_identity".to_string(),
                        Some("identity_sled.write".to_string()),
                        "{}",
                    )
                    .clone()
            })
            .collect()
    }

    fn chain_event(actor_id: &str) -> ChainEvent {
        chain_events(&[actor_id]).remove(0)
    }

    fn metadata_str(footprint: &PluginFootprint, key: &str) -> String {
        footprint
            .metadata
            .get(key)
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| panic!("footprint metadata missing '{key}'"))
    }

    /// VAL-GENESIS-010 (`mint_and_store_genesis`): the first call mints, every
    /// call after it returns the stored value — the formula is never evaluated
    /// twice for one session.
    #[tokio::test(flavor = "multi_thread")]
    async fn mint_and_store_genesis_mints_exactly_once() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x60);
        let session_id = op_identity::session::derive_session_id(&pubkey);

        let first = engine
            .mint_and_store_genesis(&session_id, &pubkey)
            .await
            .expect("mint");
        assert_eq!(first.len(), 64);
        assert_ne!(first, "0".repeat(64));

        let second = engine
            .mint_and_store_genesis(&session_id, &pubkey)
            .await
            .expect("second call reads the stored anchor");
        assert_eq!(first, second, "genesis must never be re-minted");

        let context = engine.session_context(&session_id).await.expect("context");
        assert_eq!(context.genesis_hex, first);
        assert_eq!(context.wireguard_pubkey, pubkey);
    }

    /// VAL-GENESIS-011: minting needs a session id and a real 32-byte pubkey;
    /// neither is invented on the caller's behalf.
    #[tokio::test(flavor = "multi_thread")]
    async fn mint_and_store_genesis_refuses_incomplete_input() {
        let (engine, _shm) = sled_engine();
        assert!(engine.mint_and_store_genesis("", &pk(0x61)).await.is_err());
        assert!(engine
            .mint_and_store_genesis("session-no-key", "not-base64!!")
            .await
            .is_err());
    }

    /// VAL-GENESIS-012 (`genesis_not_reminted`): a session's second mutation
    /// reads the stored anchor rather than minting a fresh one.
    #[tokio::test(flavor = "multi_thread")]
    async fn second_mutation_reads_stored_genesis() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x62);
        let identity = write_identity(engine.as_ref(), &pubkey)
            .await
            .expect("write");
        let minted = identity.genesis.clone().expect("genesis");

        // Both handles the mutation path may hold resolve the same anchor.
        for handle in [identity.session_id.as_str(), pubkey.as_str()] {
            let context = engine
                .ensure_session_context(handle)
                .await
                .unwrap_or_else(|| panic!("handle '{handle}' resolves a session"));
            assert_eq!(context.genesis_hex, minted, "stored anchor reused");
        }
    }

    /// VAL-GENESIS-013 (`genesis_none_triggers_remint`): a record hydrated with
    /// no anchor (crash between mint and persist, §5.2) is treated as an
    /// arrival — the next request mints a fresh genesis inline.
    #[tokio::test(flavor = "multi_thread")]
    async fn genesis_none_triggers_remint() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x63);
        let identity = write_identity(engine.as_ref(), &pubkey)
            .await
            .expect("write");
        let session_id = identity.session_id.clone();
        let original = identity.genesis.clone().expect("genesis");

        crate::identity_sled_dispatch::tests::clear_genesis(engine.as_ref(), &session_id).await;
        engine.forget_session_context(&session_id).await;
        assert!(
            engine
                .session_context_for_actor(&session_id)
                .await
                .is_none(),
            "a record with no anchor must not resolve a session context"
        );

        let context = engine
            .ensure_session_context(&session_id)
            .await
            .expect("arrival re-mints");
        assert_eq!(context.genesis_hex.len(), 64);
        assert_ne!(
            context.genesis_hex, original,
            "a re-mint is a new arrival: new chain head, new arrival timestamp"
        );
    }

    /// VAL-GENESIS-014: an actor that names no session anchors nothing — the
    /// mutation is still notarized, it just carries no session stamp.
    #[tokio::test(flavor = "multi_thread")]
    async fn anonymous_actor_anchors_nothing() {
        let (engine, _shm) = sled_engine();
        assert!(engine
            .ensure_session_context(ANONYMOUS_ACTOR)
            .await
            .is_none());
        assert!(engine.ensure_session_context("").await.is_none());
        assert!(engine.ensure_session_context("who-dis").await.is_none());
    }

    /// VAL-GENESIS-020 (`chain_carries_session_identity`): the durable
    /// footprint carries all three session fields (FR-3).
    #[test]
    fn chain_carries_session_identity() {
        let session = SessionContext {
            genesis_hex: "ab".repeat(32),
            session_id: "session-one".to_string(),
            wireguard_pubkey: "cGs=".to_string(),
        };
        let footprint = event_to_footprint(&chain_event("session-one"), Some(&session));
        assert_eq!(
            metadata_str(&footprint, "session_genesis"),
            session.genesis_hex
        );
        assert_eq!(metadata_str(&footprint, "session_id"), session.session_id);
        assert_eq!(
            metadata_str(&footprint, "wireguard_pubkey"),
            session.wireguard_pubkey
        );
    }

    /// VAL-GENESIS-021: an unanchored event carries the stamp keys with empty
    /// values — present and honest, never absent, so a slicing query cannot
    /// mistake "no session" for "field not written yet".
    #[test]
    fn event_to_footprint_stamps_empty_session_when_unanchored() {
        let footprint = event_to_footprint(&chain_event(ANONYMOUS_ACTOR), None);
        for key in ["session_genesis", "session_id", "wireguard_pubkey"] {
            assert_eq!(metadata_str(&footprint, key), "", "{key} must be present");
        }
    }

    /// VAL-GENESIS-022 (`chain_sliceable_by_session`): two sessions' events
    /// interleaved in one chain are recovered by filtering on session_genesis.
    #[test]
    fn chain_sliceable_by_session() {
        let one = SessionContext {
            genesis_hex: "11".repeat(32),
            session_id: "session-one".to_string(),
            wireguard_pubkey: "b25l".to_string(),
        };
        let two = SessionContext {
            genesis_hex: "22".repeat(32),
            session_id: "session-two".to_string(),
            wireguard_pubkey: "dHdv".to_string(),
        };

        let actors: Vec<&str> = (0..6)
            .map(|i| {
                if i % 2 == 0 {
                    one.session_id.as_str()
                } else {
                    two.session_id.as_str()
                }
            })
            .collect();
        let chain: Vec<PluginFootprint> = chain_events(&actors)
            .iter()
            .map(|event| {
                let session = if event.actor_id == one.session_id {
                    &one
                } else {
                    &two
                };
                event_to_footprint(event, Some(session))
            })
            .collect();

        for session in [&one, &two] {
            let slice: Vec<&PluginFootprint> = chain
                .iter()
                .filter(|fp| {
                    fp.metadata
                        .get("session_genesis")
                        .and_then(|v| v.as_str())
                        .map(|g| g == session.genesis_hex)
                        .unwrap_or(false)
                })
                .collect();
            assert_eq!(slice.len(), 3, "each session recovers its own segment");
            for fp in slice {
                assert_eq!(metadata_str(fp, "session_id"), session.session_id);
            }
        }
    }

    /// VAL-GENESIS-030 (`offline_reverification`): stored mint inputs are
    /// sufficient to re-derive each session anchor after extracting
    /// interleaved events. The first session deliberately arrives one event
    /// after its recorded chain head, exercising the ancestor-not-parent rule.
    #[test]
    fn offline_reverification() {
        fn hash32(value: &str) -> [u8; 32] {
            hex::decode(value)
                .ok()
                .and_then(|raw| <[u8; 32]>::try_from(raw).ok())
                .unwrap_or([0; 32])
        }

        fn is_ancestor(events: &[ChainEvent], ancestor: &str, descendant: &str) -> bool {
            let by_hash: HashMap<&str, &ChainEvent> = events
                .iter()
                .map(|event| (event.event_hash.as_str(), event))
                .collect();
            let mut cursor = descendant;
            while cursor != ancestor {
                let Some(event) = by_hash.get(cursor) else {
                    return false;
                };
                if event.prev_hash == cursor {
                    return false;
                }
                cursor = &event.prev_hash;
            }
            true
        }

        let events = chain_events(&[
            "bootstrap",
            "session-two-gap",
            "session-one",
            "session-two",
            "session-one",
            "session-two",
        ]);
        let head = &events[0];
        let catalog = [0x77; 32];
        let one_key = [0x11; 32];
        let two_key = [0x22; 32];
        let head_hash = hash32(&head.event_hash);
        let head_timestamp = head.timestamp.timestamp();

        let one_stamp = GenesisStamp {
            session_id: "session-one".into(),
            wireguard_pubkey: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                one_key,
            ),
            genesis_hex: hex::encode(mint_genesis(
                &one_key,
                &head_hash,
                head_timestamp,
                &catalog,
                1_700_000_101,
            )),
            arrival_timestamp: 1_700_000_101,
            chain_head_at_arrival: head.event_hash.clone(),
            catalog_hash_at_arrival: hex::encode(catalog),
            head_timestamp_at_arrival: head_timestamp,
        };
        let two_stamp = GenesisStamp {
            session_id: "session-two".into(),
            wireguard_pubkey: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                two_key,
            ),
            genesis_hex: hex::encode(mint_genesis(
                &two_key,
                &head_hash,
                head_timestamp,
                &catalog,
                1_700_000_102,
            )),
            arrival_timestamp: 1_700_000_102,
            chain_head_at_arrival: head.event_hash.clone(),
            catalog_hash_at_arrival: hex::encode(catalog),
            head_timestamp_at_arrival: head_timestamp,
        };
        let one = SessionContext {
            genesis_hex: one_stamp.genesis_hex.clone(),
            session_id: one_stamp.session_id.clone(),
            wireguard_pubkey: one_stamp.wireguard_pubkey.clone(),
        };
        let two = SessionContext {
            genesis_hex: two_stamp.genesis_hex.clone(),
            session_id: two_stamp.session_id.clone(),
            wireguard_pubkey: two_stamp.wireguard_pubkey.clone(),
        };

        let footprints: Vec<_> = events[1..]
            .iter()
            .map(|event| {
                let session = if event.actor_id.contains("one") {
                    &one
                } else {
                    &two
                };
                event_to_footprint(event, Some(session))
            })
            .collect();

        for (stamp, key) in [(&one_stamp, one_key), (&two_stamp, two_key)] {
            let recomputed = mint_genesis(
                &key,
                &hash32(&stamp.chain_head_at_arrival),
                stamp.head_timestamp_at_arrival,
                &hash32(&stamp.catalog_hash_at_arrival),
                stamp.arrival_timestamp,
            );
            assert_eq!(hex::encode(recomputed), stamp.genesis_hex);

            let slice: Vec<_> = footprints
                .iter()
                .filter(|fp| metadata_str(fp, "session_genesis") == stamp.genesis_hex)
                .collect();
            assert!(!slice.is_empty());
            let first_hash = slice[0].content_hash.as_str();
            assert!(is_ancestor(
                &events,
                &stamp.chain_head_at_arrival,
                first_hash
            ));
        }

        for pair in events.windows(2) {
            assert_eq!(pair[1].prev_hash, pair[0].event_hash);
        }
        assert_ne!(
            events[2].prev_hash, one_stamp.chain_head_at_arrival,
            "session-one arrival includes a deliberate intervening mutation"
        );
    }
}
