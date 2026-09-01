//! identity_sled method dispatch — the container is the sled is the identity.
//!
//! Executes the `identity_sled` plugin's method surface against the
//! MutationEngine's authoritative state cache. Every call is already
//! notarized in the immutable event chain by `dispatch_method_call` before it
//! reaches here, so this module only performs the domain effect.
//!
//! Durability: sleds and the session-event "snowball" ledger persist to the
//! Cozo relations `identity_sessions` / `session_events` (own sled-engine path —
//! never the users-cozo or cognitive-mcp store, which other processes hold
//! open). The event chain remains the durable proof; Cozo is the queryable
//! archive plus the restart-warm source for the state cache. All Cozo I/O is
//! synchronous sled disk work, so it runs under `spawn_blocking`.
//!
//! Provisioning: the sled inherits the Incus schema — `provision_container`
//! creates the Incus instance (named by the derived session_id) and writes
//! the sled in one mutation; a sled exists ⟺ its container exists.
//!
//! Authoritative store: the per-session record in this plugin's state cache
//! (projected to `/dev/shm/opdbus/state/identity_sled.json`, durable in Cozo)
//! IS the identity. The legacy global 152-byte sled at
//! The retired process-global raw identity file is not written or consulted.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use async_trait::async_trait;
use base64::Engine;
use op_cozo_store::{CozoGraphShuttle, GenesisInputsRecord, IdentitySledRecord};
use op_plugins::state_plugins::identity_sled::{
    ContainerIdentitySled, IdentitySledState, SessionEvent, SledBtrfsDevice, RECORD_FORMAT,
};
use op_plugins::state_plugins::incus::{IdentitySessionPowerState, IncusInstance, IncusPlugin};
use op_plugins::state_plugins::incus_device::Device;
use serde_json::Value as JsonValue;
use tokio::sync::OnceCell;

use crate::mutation_engine::{GenesisStamp, MutationEngine};

/// Session events kept in present-state per session. The event chain is the
/// durable ledger and Cozo `session_events` the queryable archive; the state
/// cache only carries a recent window.
const MAX_EVENTS_IN_STATE: usize = 256;

/// Default Cozo path for the identity sled store. Own path — the sled engine
/// is single-process, and `/var/lib/op-dbus/users-cozo` (op-web-server) and
/// the cognitive-mcp store are already held by other processes.
const DEFAULT_SLED_COZO_PATH: &str = "/var/lib/op-dbus/identity-cozo";

/// Narrow lifecycle seam for identity containers. Production uses Incus's
/// native Unix-socket API; tests inject a recorder and never touch Incus.
#[async_trait]
pub(crate) trait IdentityContainerLifecycle: Send + Sync {
    async fn start(&self, session_id: &str) -> anyhow::Result<()>;
    async fn stop(&self, session_id: &str) -> anyhow::Result<()>;
}

struct IncusIdentityContainerLifecycle;

#[async_trait]
impl IdentityContainerLifecycle for IncusIdentityContainerLifecycle {
    async fn start(&self, session_id: &str) -> anyhow::Result<()> {
        IncusPlugin::set_identity_session_power_state(
            session_id,
            IdentitySessionPowerState::Running,
        )
        .await
    }

    async fn stop(&self, session_id: &str) -> anyhow::Result<()> {
        IncusPlugin::set_identity_session_power_state(
            session_id,
            IdentitySessionPowerState::Stopped,
        )
        .await
    }
}

fn harden_identity_store_tree(path: &Path) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "identity store contains a symbolic link: {}",
            path.display()
        );
    }
    if metadata.is_dir() {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
        for entry in std::fs::read_dir(path)? {
            harden_identity_store_tree(&entry?.path())?;
        }
    } else if metadata.is_file() {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    } else {
        anyhow::bail!(
            "identity store contains an unsupported filesystem object: {}",
            path.display()
        );
    }
    Ok(())
}

fn prepare_identity_store(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)?;
    harden_identity_store_tree(path)
}

/// In-state snowball ledger key inside the plugin state cache entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct SledCacheState {
    #[serde(default)]
    sleds: Vec<ContainerIdentitySled>,
    #[serde(default)]
    events: Vec<SessionEvent>,
}

#[async_trait]
trait IdentityStatePublisher: Send + Sync {
    async fn publish(&self, engine: &MutationEngine, cache: &SledCacheState) -> anyhow::Result<()>;
}

struct MutationEngineStatePublisher;

#[async_trait]
impl IdentityStatePublisher for MutationEngineStatePublisher {
    async fn publish(&self, engine: &MutationEngine, cache: &SledCacheState) -> anyhow::Result<()> {
        write_cache(engine, cache).await
    }
}

/// Lazily opened durable store; `None` (with one warning) when the path can't
/// be opened — dispatch then runs cache-only, as before.
fn sled_cozo() -> Option<&'static CozoGraphShuttle> {
    static SLED_COZO: OnceLock<Option<CozoGraphShuttle>> = OnceLock::new();
    SLED_COZO
        .get_or_init(|| {
            let path = std::env::var("IDENTITY_SLED_COZO_DB_PATH")
                .unwrap_or_else(|_| DEFAULT_SLED_COZO_PATH.to_string());
            let path_buf = PathBuf::from(&path);
            if let Err(error) = prepare_identity_store(&path_buf) {
                tracing::error!(%path, %error, "identity sled store permissions rejected");
                return None;
            }
            match CozoGraphShuttle::new_persistent(path_buf.clone()) {
                Ok(store) => {
                    // Cozo creates its initial RocksDB files while opening.
                    // Re-apply the exact modes before exposing the handle;
                    // the runit service's 0077 umask governs later files.
                    if let Err(error) = harden_identity_store_tree(&path_buf) {
                        tracing::error!(%path, %error, "identity sled store hardening failed");
                        return None;
                    }
                    Some(store)
                }
                Err(e) => {
                    tracing::warn!(
                        %path,
                        error = %e,
                        "identity sled Cozo store unavailable; running cache-only"
                    );
                    None
                }
            }
        })
        .as_ref()
}

fn sled_to_record(sled: &ContainerIdentitySled) -> IdentitySledRecord {
    IdentitySledRecord {
        session_id: sled.session_id.clone(),
        wireguard_pubkey: sled.wireguard_pubkey.clone(),
        interface: sled.interface.clone(),
        peer_ip: sled.peer_ip.clone().unwrap_or_default(),
        mutation_index: sled.mutation_index as i64,
        session_genesis: sled.genesis.clone().unwrap_or_default(),
        trace_id: sled.trace_id.clone(),
        schema_version: sled.schema_version as i64,
        vector_id: sled.vector_id.clone(),
        sealed_id: sled.sealed_id.clone().unwrap_or_default(),
        btrfs_device_json: sled
            .btrfs_device
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok())
            .unwrap_or_default(),
        instance_json: sled
            .instance
            .as_ref()
            .and_then(|i| serde_json::to_string(i).ok())
            .unwrap_or_default(),
        session_started_at: sled.session_started_at,
        last_seen_at: sled.last_seen_at,
        active: sled.active,
        // 0 = lifelong (host/user/chatbot identities never expire on their
        // own); otherwise unix seconds a temporary/consumer identity
        // (e.g. Lovable) stops being valid.
        expires_at: sled.expires_at.unwrap_or(0),
    }
}

fn record_to_sled(rec: &IdentitySledRecord) -> ContainerIdentitySled {
    let parse_json = |label: &str, json: &str| -> Option<serde_json::Value> {
        if json.is_empty() {
            return None;
        }
        match serde_json::from_str(json) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(
                    session_id = %rec.session_id,
                    error = %e,
                    "dropping corrupt {label} on identity sled row"
                );
                None
            }
        }
    };
    ContainerIdentitySled {
        session_id: rec.session_id.clone(),
        wireguard_pubkey: rec.wireguard_pubkey.clone(),
        interface: rec.interface.clone(),
        peer_ip: (!rec.peer_ip.is_empty()).then(|| rec.peer_ip.clone()),
        mutation_index: rec.mutation_index.max(0) as u64,
        genesis: (!rec.session_genesis.is_empty()).then(|| rec.session_genesis.clone()),
        trace_id: rec.trace_id.clone(),
        schema_version: rec.schema_version.max(0) as u32,
        vector_id: rec.vector_id.clone(),
        sealed_id: (!rec.sealed_id.is_empty()).then(|| rec.sealed_id.clone()),
        btrfs_device: parse_json("btrfs_device", &rec.btrfs_device_json)
            .and_then(|v| serde_json::from_value(v).ok()),
        instance: parse_json("instance", &rec.instance_json)
            .and_then(|v| serde_json::from_value(v).ok()),
        session_started_at: rec.session_started_at,
        last_seen_at: rec.last_seen_at,
        active: rec.active,
        expires_at: (rec.expires_at != 0).then_some(rec.expires_at),
        arrival_timestamp: 0,
        chain_head_at_arrival: String::new(),
        catalog_hash_at_arrival: String::new(),
        head_timestamp_at_arrival: 0,
    }
}

/// One-time cache warm from Cozo: after an opdbus restart the state cache is
/// empty; the persisted sleds are its restart-warm source. One read at first
/// dispatch — not a watcher, not a poll.
async fn ensure_hydrated(engine: &MutationEngine) {
    static HYDRATED: OnceCell<()> = OnceCell::const_new();
    HYDRATED
        .get_or_init(|| async {
            let Some(cozo) = sled_cozo() else { return };
            let cozo = cozo.clone();
            let rows = tokio::task::spawn_blocking(move || {
                let sleds = cozo.list_identity_sessions()?;
                let genesis = cozo.list_identity_genesis()?;
                Ok::<_, op_cozo_store::CozoError>((sleds, genesis))
            })
            .await;
            let (mut rows, genesis_rows) = match rows {
                Ok(Ok(rows)) => rows,
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "identity sled hydration read failed");
                    return;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "identity sled hydration task failed");
                    return;
                }
            };
            if rows.is_empty() {
                return;
            }
            let mut cache = read_cache(engine).await;
            if !cache.sleds.is_empty() {
                return;
            }
            cache.sleds = rows.iter().map(record_to_sled).collect();
            join_genesis_inputs(&mut cache.sleds, &genesis_rows);
            cache.sleds.sort_by(|a, b| a.session_id.cmp(&b.session_id));
            if let Err(e) = write_cache(engine, &cache).await {
                tracing::warn!(error = %e, "identity sled hydration cache write failed");
            }
        })
        .await;
}

fn sled_to_genesis_inputs(sled: &ContainerIdentitySled) -> GenesisInputsRecord {
    GenesisInputsRecord {
        session_id: sled.session_id.clone(),
        arrival_timestamp: sled.arrival_timestamp,
        chain_head_at_arrival: sled.chain_head_at_arrival.clone(),
        catalog_hash_at_arrival: sled.catalog_hash_at_arrival.clone(),
        head_timestamp_at_arrival: sled.head_timestamp_at_arrival,
        schema_content_hash: op_plugins::state_plugins::identity_sled::SCHEMA_CONTENT_HASH
            .trim()
            .to_string(),
    }
}

fn join_genesis_inputs(sleds: &mut [ContainerIdentitySled], inputs: &[GenesisInputsRecord]) {
    let expected_shape = op_plugins::state_plugins::identity_sled::SCHEMA_CONTENT_HASH.trim();
    for sled in sleds.iter_mut() {
        let Some(found) = inputs.iter().find(|row| row.session_id == sled.session_id) else {
            continue;
        };
        if !found.schema_content_hash.is_empty() && found.schema_content_hash != expected_shape {
            tracing::error!(
                session_id = %sled.session_id,
                stored = %found.schema_content_hash,
                expected = %expected_shape,
                "identity record shape drift; genesis inputs skipped"
            );
            continue;
        }
        sled.arrival_timestamp = found.arrival_timestamp;
        sled.chain_head_at_arrival = found.chain_head_at_arrival.clone();
        sled.catalog_hash_at_arrival = found.catalog_hash_at_arrival.clone();
        sled.head_timestamp_at_arrival = found.head_timestamp_at_arrival;
    }
}

/// Persist one sled row to Cozo; failure is logged, not fatal — the event
/// chain already notarized the mutation.
async fn persist_sled(sled: &ContainerIdentitySled) {
    let Some(cozo) = sled_cozo() else { return };
    let cozo = cozo.clone();
    let rec = sled_to_record(sled);
    let inputs = (sled.arrival_timestamp != 0).then(|| sled_to_genesis_inputs(sled));
    match tokio::task::spawn_blocking(move || {
        cozo.put_identity_sled(&rec)?;
        if let Some(inputs) = inputs {
            cozo.put_identity_genesis(&inputs)?;
        }
        Ok::<_, op_cozo_store::CozoError>(())
    })
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "identity sled Cozo persist failed"),
        Err(e) => tracing::warn!(error = %e, "identity sled Cozo persist task failed"),
    }
}

/// The genesis already stored for `session_id`, if the session has arrived.
///
/// One read of the authoritative store (the in-process state cache behind the
/// SHM projection) — no hashing, no Cozo query, no second store consulted.
pub(crate) async fn stored_genesis(engine: &MutationEngine, session_id: &str) -> Option<String> {
    ensure_hydrated(engine).await;
    read_cache(engine)
        .await
        .sleds
        .iter()
        .find(|sled| sled.session_id == session_id)
        .filter(|sled| sled.is_anchored())
        .and_then(|sled| sled.genesis.clone())
        .filter(|genesis| !genesis.is_empty())
}

/// The whole session record for `session_id`, for callers that need the term
/// (`expires_at`, `active`) beside the anchor.
pub(crate) async fn stored_session(
    engine: &MutationEngine,
    session_id: &str,
) -> Option<ContainerIdentitySled> {
    ensure_hydrated(engine).await;
    read_cache(engine)
        .await
        .sleds
        .into_iter()
        .find(|sled| sled.session_id == session_id)
}

/// The session record a caller's handle names, whichever handle it holds.
///
/// A session is named by its id, by the WireGuard pubkey that owns it, by the
/// trace it propagates, or by the anchor it presents — all four are fields of
/// the same record, so all four resolve through this one lookup rather than
/// four call sites each deriving their own.
pub(crate) async fn session_record_for_actor(
    engine: &MutationEngine,
    actor_id: &str,
) -> Option<ContainerIdentitySled> {
    if actor_id.is_empty() {
        return None;
    }
    ensure_hydrated(engine).await;
    read_cache(engine).await.sleds.into_iter().find(|sled| {
        sled.session_id == actor_id
            || sled.wireguard_pubkey == actor_id
            || (!sled.trace_id.is_empty() && sled.trace_id == actor_id)
            || sled.genesis.as_deref() == Some(actor_id)
    })
}

/// Write one session's genesis and the inputs it was minted from.
///
/// The mutation path owns these fields (FR-6) and writes them exactly once: a
/// record that already carries a genesis keeps it, and the stored value is
/// returned instead. Liveness fields (`last_seen_at`, `active`, `peer_ip`,
/// `session_started_at`) are never touched here — they belong to the stream
/// path, so the two writers cannot disagree.
pub(crate) async fn store_genesis(
    engine: &MutationEngine,
    stamp: &GenesisStamp,
) -> anyhow::Result<String> {
    let claims = op_identity::sealed_id::SealedId::from_inline_ref(&stamp.sealed_id)
        .map_err(|error| anyhow::anyhow!("invalid MutationEngine sealed ID: {error}"))?;
    if claims.principal_id != op_identity::session::derive_principal_id(&stamp.wireguard_pubkey)
        || claims.session_id != stamp.session_id
        || claims.wireguard_pubkey != stamp.wireguard_pubkey
        || claims.session_genesis != stamp.genesis_hex
        || claims.trace_id != stamp.trace_id
        || claims.arrival_timestamp != stamp.arrival_timestamp
        || claims.chain_head_at_arrival != stamp.chain_head_at_arrival
        || claims.catalog_hash_at_arrival != stamp.catalog_hash_at_arrival
        || claims.head_timestamp_at_arrival != stamp.head_timestamp_at_arrival
    {
        anyhow::bail!("MutationEngine sealed ID does not match its genesis stamp");
    }
    ensure_hydrated(engine).await;
    let mut cache = read_cache(engine).await;
    let ts = now();
    let record = match cache
        .sleds
        .iter_mut()
        .find(|sled| sled.session_id == stamp.session_id)
    {
        Some(sled) => {
            // An anchored account is never re-anchored. A record that merely
            // carries a v2 `session_genesis` in `genesis` is NOT anchored —
            // see `ContainerIdentitySled::is_anchored` — so it gets minted
            // properly here rather than keeping a value nothing can verify.
            if sled.is_anchored() {
                let stored_genesis = sled.genesis.clone().unwrap_or_default();
                if stored_genesis != stamp.genesis_hex
                    || sled.wireguard_pubkey != stamp.wireguard_pubkey
                    || sled.trace_id != stamp.trace_id
                    || sled.arrival_timestamp != stamp.arrival_timestamp
                    || sled.chain_head_at_arrival != stamp.chain_head_at_arrival
                    || sled.catalog_hash_at_arrival != stamp.catalog_hash_at_arrival
                    || sled.head_timestamp_at_arrival != stamp.head_timestamp_at_arrival
                    || claims.schema_version != sled.schema_version
                    || claims.expires_at != sled.expires_at.unwrap_or(0)
                {
                    anyhow::bail!(
                        "refusing to replace the sealed ID for mismatched anchored session '{}'",
                        stamp.session_id
                    );
                }
                match sled
                    .sealed_id
                    .as_deref()
                    .filter(|sealed| !sealed.is_empty())
                {
                    Some(sealed) if sealed != stamp.sealed_id => {
                        anyhow::bail!(
                            "refusing to replace the authored sealed ID for anchored session '{}'",
                            stamp.session_id
                        );
                    }
                    Some(_) => {}
                    None => sled.sealed_id = Some(stamp.sealed_id.clone()),
                }
                sled.clone()
            } else {
                sled.genesis = Some(stamp.genesis_hex.clone());
                sled.trace_id = stamp.trace_id.clone();
                sled.arrival_timestamp = stamp.arrival_timestamp;
                sled.chain_head_at_arrival = stamp.chain_head_at_arrival.clone();
                sled.catalog_hash_at_arrival = stamp.catalog_hash_at_arrival.clone();
                sled.head_timestamp_at_arrival = stamp.head_timestamp_at_arrival;
                sled.schema_version = RECORD_FORMAT;
                sled.sealed_id = Some(stamp.sealed_id.clone());
                if sled.wireguard_pubkey.is_empty() {
                    sled.wireguard_pubkey = stamp.wireguard_pubkey.clone();
                }
                sled.clone()
            }
        }
        None => {
            // Arrival of a session that has no record yet: the arrival IS the
            // record's creation, so the stream path has not run and the
            // liveness fields start from this moment.
            let sled = ContainerIdentitySled {
                session_id: stamp.session_id.clone(),
                wireguard_pubkey: stamp.wireguard_pubkey.clone(),
                interface: String::new(),
                peer_ip: None,
                mutation_index: 0,
                genesis: Some(stamp.genesis_hex.clone()),
                trace_id: stamp.trace_id.clone(),
                schema_version: RECORD_FORMAT,
                vector_id: String::new(),
                sealed_id: Some(stamp.sealed_id.clone()),
                btrfs_device: None,
                instance: None,
                session_started_at: ts,
                last_seen_at: ts,
                active: true,
                expires_at: None,
                arrival_timestamp: stamp.arrival_timestamp,
                chain_head_at_arrival: stamp.chain_head_at_arrival.clone(),
                catalog_hash_at_arrival: stamp.catalog_hash_at_arrival.clone(),
                head_timestamp_at_arrival: stamp.head_timestamp_at_arrival,
            };
            cache.sleds.push(sled.clone());
            cache.sleds.sort_by(|a, b| a.session_id.cmp(&b.session_id));
            sled
        }
    };
    write_cache(engine, &cache).await?;
    // Inline, awaited: `arrival_timestamp` is irreproducible, so a session
    // whose genesis never reached Cozo is permanently unverifiable after a
    // restart. A failed write is a warning, not a rejected first request.
    persist_sled(&record).await;
    Ok(stamp.genesis_hex.clone())
}

/// Advance one session's `mutation_index` to the chain position it just took.
///
/// Advance-only and field-disjoint: a stale writer cannot roll the account
/// backwards, and nothing the stream path owns is written here.
pub(crate) async fn advance_mutation_index(
    engine: &MutationEngine,
    session_id: &str,
    event_id: u64,
) -> anyhow::Result<()> {
    let mut cache = read_cache(engine).await;
    let Some(sled) = cache
        .sleds
        .iter_mut()
        .find(|sled| sled.session_id == session_id)
    else {
        return Ok(());
    };
    if event_id <= sled.mutation_index {
        return Ok(());
    }
    sled.mutation_index = event_id;
    let record = sled.clone();
    write_cache(engine, &cache).await?;
    persist_sled(&record).await;
    Ok(())
}

async fn read_cache(engine: &MutationEngine) -> SledCacheState {
    let Some(state) = engine.get_state("identity_sled").await else {
        return SledCacheState::default();
    };
    simd_json::serde::from_owned_value(state).unwrap_or_default()
}

async fn write_cache(engine: &MutationEngine, cache: &SledCacheState) -> anyhow::Result<()> {
    let owned = simd_json::serde::to_owned_value(cache)?;
    engine
        .update_state_cache("identity_sled".to_string(), owned)
        .await;
    engine
        .publish_plugin_projection_from_cache(
            "identity_sled",
            crate::mutation_engine::ChangeType::PropertySet,
        )
        .await?;
    Ok(())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The host's own session id — "container zero" under its own record.
///
/// Read from the environment, never from the global 152-byte sled and never
/// from a subprocess: `IDENTITY_SLED_HOST_SESSION_ID` when the operator names
/// it outright, otherwise derived from `WG_PUBKEY` the same way every other
/// session id is derived. `None` means the host has not been registered as a
/// session yet, which is a missing record — not a licence to read a shared
/// last-write-wins file and call whatever is in it the host's identity.
pub(crate) fn host_session_id() -> Option<String> {
    if let Ok(session_id) = std::env::var("IDENTITY_SLED_HOST_SESSION_ID") {
        let session_id = session_id.trim().to_string();
        if !session_id.is_empty() {
            return Some(session_id);
        }
    }
    let pubkey = std::env::var("WG_PUBKEY").ok()?;
    let pubkey = pubkey.trim();
    (!pubkey.is_empty()).then(|| op_identity::session::derive_session_id(pubkey))
}

fn arg_str(args: &JsonValue, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Integer args that round-trip through gRPC's `PluginService.CallMethod`
/// arrive as a JSON float: `google.protobuf.Struct`'s `Value` only has a
/// `number_value: double` variant (see `grpc_client.rs`'s
/// `prost_value_to_serde`/`simd_to_prost_value`), so a plain `as_i64()` on
/// values from that path silently returns `None` even for whole numbers.
/// D-Bus-sourced args don't have this problem, but this helper is safe for
/// both since `as_i64()` is tried first.
fn arg_i64(args: &JsonValue, key: &str) -> Option<i64> {
    args.get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
}

/// Derive the session id (= container name) from the supplied pubkey, using
/// the provision-time PSK when present. Never trusts a supplied session_id.
fn derive_session_id(args: &JsonValue, pubkey: &str) -> anyhow::Result<String> {
    if let Some(psk_b64) = args
        .get("psk")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let psk = base64::engine::general_purpose::STANDARD
            .decode(psk_b64.trim())
            .map_err(|e| anyhow::anyhow!("invalid psk base64: {e}"))?;
        op_identity::session::derive_session_id_from_psk(pubkey, &psk)
    } else {
        Ok(op_identity::session::derive_session_id(pubkey))
    }
}

/// Apply the non-negotiable identity-container lifecycle policy to a
/// provision-time Incus definition. `incus init` creates the instance parked;
/// only authenticated session activation may start it.
fn prepare_parked_instance(
    session_id: &str,
    mut instance: IncusInstance,
) -> anyhow::Result<IncusInstance> {
    instance.name = session_id.to_string();
    instance.instance_type = "container".to_string();
    instance.status = "Stopped".to_string();
    if instance.profiles.is_empty() {
        // The identity profile owns the root disk plus the shared fabric UDS
        // and intentionally has no NIC. The host's Incus default profile has
        // no devices and cannot boot an identity container.
        instance.profiles = vec!["identity".to_string()];
    }
    instance
        .config
        .get_or_insert_with(Default::default)
        .insert("boot.autostart".to_string(), "false".to_string());

    if instance
        .devices
        .iter()
        .chain(instance.expanded_devices.iter())
        .any(|device| matches!(device.device, Device::Nic(_)))
    {
        anyhow::bail!(
            "nic devices are not allowed: containers get no NIC or IP \
             (all container I/O goes over the UDS socket surface)"
        );
    }
    Ok(instance)
}

async fn set_session_active_with(
    engine: &MutationEngine,
    session_id: &str,
    active: bool,
    lifecycle: &dyn IdentityContainerLifecycle,
) -> anyhow::Result<ContainerIdentitySled> {
    set_session_active_with_publisher(
        engine,
        session_id,
        active,
        lifecycle,
        &MutationEngineStatePublisher,
    )
    .await
}

async fn set_session_active_with_publisher(
    engine: &MutationEngine,
    session_id: &str,
    active: bool,
    lifecycle: &dyn IdentityContainerLifecycle,
    publisher: &dyn IdentityStatePublisher,
) -> anyhow::Result<ContainerIdentitySled> {
    if session_id.is_empty() {
        anyhow::bail!("session lifecycle transition requires session_id");
    }
    ensure_hydrated(engine).await;
    let mut cache = read_cache(engine).await;
    let index = cache
        .sleds
        .iter()
        .position(|sled| sled.session_id == session_id)
        .ok_or_else(|| anyhow::anyhow!("no identity sled for session '{session_id}'"))?;

    let was_active = cache.sleds[index].active;
    let previous_record = cache.sleds[index].clone();
    let has_instance = match cache.sleds[index].instance.as_ref() {
        Some(instance) if instance.name == session_id => true,
        Some(instance) => anyhow::bail!(
            "identity sled '{}' names foreign Incus instance '{}'",
            session_id,
            instance.name
        ),
        None => false,
    };

    // Activation is start-first: never publish an active credential for a
    // container that failed to start. Deactivation is the inverse security
    // order below: invalidate the sled first, then attempt the physical stop.
    let newly_active = active && !was_active;
    let started = newly_active && has_instance;
    if started {
        lifecycle.start(session_id).await?;
    }

    let sled = &mut cache.sleds[index];
    if active {
        sled.last_seen_at = now();
    }
    sled.active = active;
    if active {
        if let Some(instance) = sled.instance.as_mut() {
            instance.status = "Running".to_string();
        }
    }
    let record = sled.clone();
    let projection_result = publisher.publish(engine, &cache).await;

    if active {
        if let Err(projection_error) = projection_result {
            if newly_active {
                // A start succeeded but activation could not be projected.
                // Restore inactive authority before rolling the container
                // back; never return an activation error with a newly-active
                // OIB left in cache or durable storage.
                cache.sleds[index] = previous_record.clone();
                if let Err(rollback_error) = publisher.publish(engine, &cache).await {
                    tracing::error!(
                        %rollback_error,
                        %session_id,
                        "failed to republish inactive session after activation rollback"
                    );
                }
                persist_sled(&previous_record).await;
                if started {
                    if let Err(stop_error) = lifecycle.stop(session_id).await {
                        tracing::error!(
                            %stop_error,
                            %session_id,
                            "failed to stop container after activation rollback"
                        );
                        // Authority stays inactive, but retain a non-stopped
                        // physical-state marker so startup reconciliation
                        // retries the rollback stop.
                        let mut retry_record = previous_record.clone();
                        if let Some(instance) = retry_record.instance.as_mut() {
                            instance.status = "Running".to_string();
                        }
                        cache.sleds[index] = retry_record.clone();
                        if let Err(retry_projection_error) = publisher.publish(engine, &cache).await
                        {
                            tracing::error!(
                                %retry_projection_error,
                                %session_id,
                                "failed to project retryable activation rollback state"
                            );
                        }
                        persist_sled(&retry_record).await;
                    }
                }
            } else {
                // This session was already active for another binding; keep
                // its durable liveness unchanged while reporting projection
                // failure to the new registration attempt.
                persist_sled(&previous_record).await;
            }
            return Err(projection_error);
        }
        persist_sled(&record).await;
        return Ok(record);
    }

    // Cozo receives inactive authority before the physical stop, even when
    // SHM projection fails. Keep the prior non-stopped status until Incus
    // confirms success so startup reconciliation can retry a failed stop.
    persist_sled(&record).await;
    let needs_stop = has_instance
        && (was_active
            || record
                .instance
                .as_ref()
                .is_some_and(|instance| instance.status != "Stopped"));
    let stop_result = if needs_stop {
        lifecycle.stop(session_id).await
    } else {
        Ok(())
    };

    let mut stopped_record = record.clone();
    let stopped_projection = if stop_result.is_ok() && has_instance {
        if let Some(instance) = cache.sleds[index].instance.as_mut() {
            instance.status = "Stopped".to_string();
        }
        stopped_record = cache.sleds[index].clone();
        let result = publisher.publish(engine, &cache).await;
        persist_sled(&stopped_record).await;
        result
    } else {
        Ok(())
    };

    if let Err(projection_error) = projection_result {
        if let Err(stop_error) = &stop_result {
            tracing::error!(%stop_error, %session_id, "container stop also failed after identity projection failure");
        }
        if let Err(stopped_projection_error) = &stopped_projection {
            tracing::error!(%stopped_projection_error, %session_id, "stopped status projection also failed");
        }
        return Err(projection_error);
    }
    stop_result?;
    stopped_projection?;
    Ok(stopped_record)
}

/// Activate a parked session. Container-backed sleds start through Incus;
/// host identities have no instance and only change their liveness record.
pub(crate) async fn activate_session(
    engine: &MutationEngine,
    session_id: &str,
) -> anyhow::Result<ContainerIdentitySled> {
    set_session_active_with(engine, session_id, true, &IncusIdentityContainerLifecycle).await
}

/// Park a session after its last authenticated binding is gone.
pub(crate) async fn deactivate_session(
    engine: &MutationEngine,
    session_id: &str,
) -> anyhow::Result<ContainerIdentitySled> {
    set_session_active_with(engine, session_id, false, &IncusIdentityContainerLifecycle).await
}

async fn park_orphaned_container_sessions_with(
    engine: &MutationEngine,
    lifecycle: &dyn IdentityContainerLifecycle,
) -> anyhow::Result<usize> {
    ensure_hydrated(engine).await;
    let session_ids: Vec<String> = read_cache(engine)
        .await
        .sleds
        .into_iter()
        .filter(|sled| {
            sled.instance
                .as_ref()
                .is_some_and(|instance| sled.active || instance.status != "Stopped")
        })
        .map(|sled| sled.session_id)
        .collect();
    let mut errors = Vec::new();
    for session_id in &session_ids {
        if let Err(error) = set_session_active_with(engine, session_id, false, lifecycle).await {
            errors.push(format!("{session_id}: {error:#}"));
        }
        // Contexts are process-local and no binding survived the restart.
        engine.forget_session_context(session_id).await;
    }
    if errors.is_empty() {
        Ok(session_ids.len())
    } else {
        anyhow::bail!(
            "failed to park {} orphaned identity container(s): {}",
            errors.len(),
            errors.join("; ")
        )
    }
}

/// Reconcile durable liveness before any post-restart transport is exposed.
/// D-Bus bindings are process-local, so every active instance-backed sled is
/// orphaned after startup; host/no-instance identities are intentionally left
/// untouched.
pub(crate) async fn park_orphaned_container_sessions(
    engine: &MutationEngine,
) -> anyhow::Result<usize> {
    park_orphaned_container_sessions_with(engine, &IncusIdentityContainerLifecycle).await
}

pub async fn dispatch_identity_sled_method(
    engine: &MutationEngine,
    method: &str,
    args: &JsonValue,
) -> anyhow::Result<JsonValue> {
    match method {
        "get_identity" => {
            let mut session_id = arg_str(args, "session_id");
            let trace_id = arg_str(args, "trace_id");
            if session_id.is_empty() && trace_id.is_empty() {
                // Empty = the host ("container zero"). It is the same kind of
                // object as every other session, so it is read the same way:
                // its own record out of the authoritative state cache.
                session_id = host_session_id().ok_or_else(|| {
                    anyhow::anyhow!(
                        "no host session id: set IDENTITY_SLED_HOST_SESSION_ID or WG_PUBKEY, \
                         or name a session_id / trace_id"
                    )
                })?;
            }

            ensure_hydrated(engine).await;
            let cache = read_cache(engine).await;
            // Vendor/partner identities (e.g. a browser frontend that has no
            // WireGuard identity of its own) are looked up by the trace_id
            // they present directly, rather than a pubkey-derived session_id.
            let sled = cache
                .sleds
                .iter()
                .find(|s| {
                    (!session_id.is_empty() && s.session_id == session_id)
                        || (!trace_id.is_empty() && s.trace_id == trace_id)
                })
                .cloned();
            match sled {
                Some(identity) => Ok(serde_json::json!({ "identity": identity })),
                None => Err(anyhow::anyhow!(
                    "no identity sled for session '{}' / trace '{}'",
                    session_id,
                    trace_id
                )),
            }
        }

        "write_identity" => {
            ensure_hydrated(engine).await;
            if args.get("sealed_id").is_some() {
                anyhow::bail!("sealed_id is MutationEngine-authored and cannot be supplied");
            }
            let pubkey = arg_str(args, "wireguard_pubkey");
            if pubkey.is_empty() {
                anyhow::bail!("write_identity requires wireguard_pubkey");
            }
            // session_id is DERIVED from PSK + pubkey when PSK is supplied at provision time.
            let session_id = derive_session_id(args, &pubkey)?;
            let interface = arg_str(args, "interface");
            let peer_ip = args
                .get("peer_ip")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let btrfs_device: Option<SledBtrfsDevice> = args
                .get("btrfs_device")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok());
            // Explicit term length for this call, when given. Omitted (the
            // common case for host/user/chatbot re-registration) means
            // "don't touch whatever term is already set" — a generic
            // write_identity call must never silently extend or shorten a
            // temporary identity's term as a side effect.
            let ttl_seconds = arg_i64(args, "ttl_seconds");

            let mut cache = read_cache(engine).await;
            let ts = now();
            let existing = cache.sleds.iter_mut().find(|s| s.session_id == session_id);
            let identity = match existing {
                Some(sled) => {
                    sled.wireguard_pubkey = pubkey.clone();
                    if !interface.is_empty() {
                        sled.interface = interface;
                    }
                    if peer_ip.is_some() {
                        sled.peer_ip = peer_ip;
                    }
                    if btrfs_device.is_some() {
                        sled.btrfs_device = btrfs_device;
                    }
                    sled.mutation_index += 1;
                    sled.last_seen_at = ts;
                    sled.active = true;
                    // Only an explicit ttl_seconds renews the term. A lifelong
                    // account's caller passes this on every heartbeat to keep
                    // renewing it; a bare re-registration with no ttl leaves
                    // an already-set expiry (temporary identity) untouched.
                    if let Some(ttl) = ttl_seconds {
                        sled.expires_at = Some(ts + ttl);
                    }
                    sled.clone()
                }
                None => {
                    // A brand-new record gets a trace_id now, at creation,
                    // rather than staying blank forever. The identity anchor
                    // is NOT minted here: the genesis belongs to a session
                    // arrival, which needs the chain head, so it is minted
                    // below (once the record exists) by the mutation engine.
                    let trace_id = hex::encode(uuid::Uuid::new_v4().as_bytes());

                    let sled = ContainerIdentitySled {
                        session_id: session_id.clone(),
                        wireguard_pubkey: pubkey.clone(),
                        interface,
                        peer_ip,
                        mutation_index: 0,
                        genesis: None,
                        trace_id,
                        schema_version: RECORD_FORMAT,
                        vector_id: String::new(),
                        sealed_id: None,
                        btrfs_device,
                        instance: None,
                        session_started_at: ts,
                        last_seen_at: ts,
                        active: true,
                        expires_at: ttl_seconds.map(|ttl| ts + ttl),
                        arrival_timestamp: 0,
                        chain_head_at_arrival: String::new(),
                        catalog_hash_at_arrival: String::new(),
                        head_timestamp_at_arrival: 0,
                    };
                    cache.sleds.push(sled.clone());
                    cache.sleds.sort_by(|a, b| a.session_id.cmp(&b.session_id));
                    sled
                }
            };
            write_cache(engine, &cache).await?;
            persist_sled(&identity).await;

            // Provisioning IS arrival for this session: mint the genesis
            // against the real chain head now, so the record the caller is
            // handed already carries the anchor it must present. One author
            // (`mint_genesis`, reached through the engine), one write.
            let identity = match engine
                .mint_and_store_genesis(&identity.session_id, &pubkey)
                .await
            {
                Ok(_) => stored_session(engine, &identity.session_id)
                    .await
                    .unwrap_or(identity),
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        session_id = %identity.session_id,
                        "session anchor not minted at provisioning; the next \
                         authenticated request will mint it"
                    );
                    identity
                }
            };

            Ok(serde_json::json!({ "identity": identity }))
        }

        "provision_container" => {
            if args.get("sealed_id").is_some() {
                anyhow::bail!("sealed_id is MutationEngine-authored and cannot be supplied");
            }
            let pubkey = arg_str(args, "wireguard_pubkey");
            if pubkey.is_empty() {
                anyhow::bail!("provision_container requires wireguard_pubkey");
            }
            let session_id = derive_session_id(args, &pubkey)?;

            let mut instance: IncusInstance = match args.get("instance") {
                Some(v) => {
                    // `name`/`status`/`type` are mandatory on IncusInstance
                    // (it also models read-back instances, where they always
                    // exist) but irrelevant to a provision-time override:
                    // `name` is always forced to the derived session_id below
                    // and `status`/`type` default to their obvious values —
                    // inject placeholders so a partial override (e.g.
                    // image-only) still parses.
                    let mut v = v.clone();
                    if let Some(obj) = v.as_object_mut() {
                        obj.entry("name").or_insert_with(|| serde_json::json!(""));
                        obj.entry("status").or_insert_with(|| serde_json::json!(""));
                        obj.entry("type").or_insert_with(|| serde_json::json!(""));
                    }
                    serde_json::from_value(v)
                        .map_err(|e| anyhow::anyhow!("invalid instance definition: {e}"))?
                }
                None => IncusInstance::default(),
            };
            // The container name IS the identity. It is provisioned parked,
            // with boot autostart forced off; authenticated login owns start.
            instance = prepare_parked_instance(&session_id, instance)?;

            let btrfs_device: Option<SledBtrfsDevice> = args
                .get("btrfs_device")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok());

            let cache = read_cache(engine).await;
            if cache
                .sleds
                .iter()
                .any(|sled| sled.session_id == session_id && sled.instance.is_some())
            {
                anyhow::bail!(
                    "identity sled '{}' already has a provisioned container",
                    session_id
                );
            }
            drop(cache);

            // `incus init` creates the container stopped. A sled exists ⟺ its
            // container exists, so an Incus failure means no sled row.
            IncusPlugin::apply_create(&instance)
                .await
                .map_err(|e| anyhow::anyhow!("incus create for '{session_id}' failed: {e:#}"))?;

            let ts = now();
            let ttl_seconds = arg_i64(args, "ttl_seconds");
            let sled = ContainerIdentitySled {
                session_id: session_id.clone(),
                wireguard_pubkey: pubkey.clone(),
                interface: String::new(),
                peer_ip: None,
                mutation_index: 0,
                genesis: None,
                trace_id: String::new(),
                schema_version: RECORD_FORMAT,
                vector_id: String::new(),
                sealed_id: None,
                btrfs_device,
                instance: Some(instance),
                session_started_at: ts,
                last_seen_at: ts,
                active: false,
                expires_at: ttl_seconds.map(|ttl| ts + ttl),
                arrival_timestamp: 0,
                chain_head_at_arrival: String::new(),
                catalog_hash_at_arrival: String::new(),
                head_timestamp_at_arrival: 0,
            };
            let mut cache = read_cache(engine).await;
            cache.sleds.retain(|s| s.session_id != session_id);
            cache.sleds.push(sled.clone());
            cache.sleds.sort_by(|a, b| a.session_id.cmp(&b.session_id));
            write_cache(engine, &cache).await?;
            persist_sled(&sled).await;

            // Provisioning IS arrival, here as in `write_identity`: the
            // container the caller was just handed carries the anchor it must
            // present, minted against the real chain head by the one author.
            let sled = match engine.mint_and_store_genesis(&session_id, &pubkey).await {
                Ok(_) => stored_session(engine, &session_id).await.unwrap_or(sled),
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        %session_id,
                        "session anchor not minted at provisioning; the next \
                         authenticated request will mint it"
                    );
                    sled
                }
            };

            Ok(serde_json::json!({ "identity": sled }))
        }

        "attach_btrfs_device" => {
            let session_id = arg_str(args, "session_id");
            if session_id.is_empty() {
                anyhow::bail!("attach_btrfs_device requires session_id");
            }
            let mut cache = read_cache(engine).await;
            let Some(sled) = cache.sleds.iter_mut().find(|s| s.session_id == session_id) else {
                anyhow::bail!("no identity sled for session '{}'", session_id);
            };
            let Some(device) = sled.btrfs_device.clone() else {
                anyhow::bail!(
                    "identity sled '{}' has no registered btrfs device",
                    session_id
                );
            };
            if device.attached {
                anyhow::bail!("btrfs device '{}' is already attached", device.device_path);
            }
            if device.device_path.is_empty() || device.mount_point.is_empty() {
                anyhow::bail!("btrfs device needs both device_path and mount_point");
            }

            let dev = PathBuf::from(&device.device_path);
            let mount = PathBuf::from(&device.mount_point);
            tokio::task::spawn_blocking(move || op_network::btrfs::device_add(&dev, &mount))
                .await
                .map_err(|e| anyhow::anyhow!("btrfs device add task failed: {e}"))??;

            if let Some(d) = sled.btrfs_device.as_mut() {
                d.attached = true;
            }
            sled.mutation_index += 1;
            sled.last_seen_at = now();
            let identity = sled.clone();
            write_cache(engine, &cache).await?;
            persist_sled(&identity).await;
            Ok(serde_json::json!({ "identity": identity }))
        }

        "touch_session" => {
            let session_id = arg_str(args, "session_id");
            activate_session(engine, &session_id).await?;
            Ok(serde_json::json!({ "success": true }))
        }

        "record_session_event" => {
            let session_id = arg_str(args, "session_id");
            let kind = arg_str(args, "kind");
            if session_id.is_empty() || kind.is_empty() {
                anyhow::bail!("record_session_event requires session_id and kind");
            }
            let subid = arg_str(args, "subid");
            let content = arg_str(args, "content");
            let created_at = now();

            // Durable append first: Cozo allocates the seq (monotonic across
            // restarts, unlike the capped in-state window).
            let mut cozo_seq: Option<u64> = None;
            if let Some(cozo) = sled_cozo() {
                let cozo = cozo.clone();
                let (sid, k, sb, c) = (
                    session_id.clone(),
                    kind.clone(),
                    subid.clone(),
                    content.clone(),
                );
                match tokio::task::spawn_blocking(move || {
                    cozo.append_session_event(&sid, &k, &sb, &c, created_at)
                })
                .await
                {
                    Ok(Ok(seq)) => cozo_seq = Some(seq.max(0) as u64),
                    Ok(Err(e)) => tracing::warn!(error = %e, "session event persist failed"),
                    Err(e) => tracing::warn!(error = %e, "session event persist task failed"),
                }
            }

            let mut cache = read_cache(engine).await;
            let seq = cozo_seq.unwrap_or_else(|| {
                cache
                    .events
                    .iter()
                    .filter(|e| e.session_id == session_id)
                    .map(|e| e.seq)
                    .max()
                    .map(|s| s + 1)
                    .unwrap_or(0)
            });
            cache.events.push(SessionEvent {
                session_id,
                seq,
                kind,
                subid,
                content,
                created_at,
            });
            if cache.events.len() > MAX_EVENTS_IN_STATE {
                let excess = cache.events.len() - MAX_EVENTS_IN_STATE;
                cache.events.drain(..excess);
            }
            write_cache(engine, &cache).await?;
            Ok(serde_json::json!({ "success": true }))
        }

        "get_session_history" => {
            let session_id = arg_str(args, "session_id");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            // Prefer the durable archive; fall back to the in-state window.
            if let Some(cozo) = sled_cozo() {
                let cozo = cozo.clone();
                let sid = session_id.clone();
                match tokio::task::spawn_blocking(move || cozo.list_session_events(&sid, limit))
                    .await
                {
                    Ok(Ok(rows)) => {
                        let events: Vec<SessionEvent> = rows
                            .iter()
                            .map(|r| SessionEvent {
                                session_id: r.session_id.clone(),
                                seq: r.seq.max(0) as u64,
                                kind: r.kind.clone(),
                                subid: r.subid.clone(),
                                content: r.content.clone(),
                                created_at: r.created_at,
                            })
                            .collect();
                        return Ok(serde_json::json!({ "events": events }));
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "session history read failed; using cache")
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "session history task failed; using cache")
                    }
                }
            }

            let cache = read_cache(engine).await;
            let mut events: Vec<&SessionEvent> = cache
                .events
                .iter()
                .filter(|e| e.session_id == session_id)
                .collect();
            events.sort_by_key(|event| std::cmp::Reverse(event.seq));
            if limit > 0 {
                events.truncate(limit);
            }
            Ok(serde_json::json!({ "events": events }))
        }

        other => Err(anyhow::anyhow!("unknown identity_sled method '{}'", other)),
    }
}

/// Expose the typed state for consumers that want the full sled set.
pub async fn current_state(engine: &MutationEngine) -> IdentitySledState {
    let cache = read_cache(engine).await;
    IdentitySledState { sleds: cache.sleds }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::human_principal_dispatch::tests::{pk, temp_shm, test_engine, TempShm};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingLifecycle {
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl IdentityContainerLifecycle for RecordingLifecycle {
        async fn start(&self, session_id: &str) -> anyhow::Result<()> {
            self.calls
                .lock()
                .expect("recording lifecycle lock")
                .push(format!("start:{session_id}"));
            Ok(())
        }

        async fn stop(&self, session_id: &str) -> anyhow::Result<()> {
            self.calls
                .lock()
                .expect("recording lifecycle lock")
                .push(format!("stop:{session_id}"));
            Ok(())
        }
    }

    #[derive(Default)]
    struct FailingStopLifecycle {
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl IdentityContainerLifecycle for FailingStopLifecycle {
        async fn start(&self, session_id: &str) -> anyhow::Result<()> {
            self.calls
                .lock()
                .expect("failing lifecycle lock")
                .push(format!("start:{session_id}"));
            Ok(())
        }

        async fn stop(&self, session_id: &str) -> anyhow::Result<()> {
            self.calls
                .lock()
                .expect("failing lifecycle lock")
                .push(format!("stop:{session_id}"));
            anyhow::bail!("injected Incus stop failure")
        }
    }

    #[derive(Default)]
    struct FailFirstProjection {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl IdentityStatePublisher for FailFirstProjection {
        async fn publish(
            &self,
            engine: &MutationEngine,
            cache: &SledCacheState,
        ) -> anyhow::Result<()> {
            // Match write_cache's ordering: authoritative memory changes
            // before projection publication can fail.
            engine
                .update_state_cache(
                    "identity_sled".to_string(),
                    simd_json::serde::to_owned_value(cache)?,
                )
                .await;
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                anyhow::bail!("injected projection failure");
            }
            Ok(())
        }
    }

    /// One temp Cozo store for the whole test process.
    ///
    /// `sled_cozo()` resolves its path exactly once (a `OnceLock`), so the
    /// first test to touch it fixes the path for every test after it. Pointing
    /// it at a tempdir here keeps the suite off the live
    /// `/var/lib/op-dbus/identity-cozo`.
    pub(crate) fn sled_cozo_sandbox() {
        static SANDBOX: OnceLock<tempfile::TempDir> = OnceLock::new();
        SANDBOX.get_or_init(|| {
            let dir = tempfile::tempdir().expect("tempdir");
            std::env::set_var(
                "IDENTITY_SLED_COZO_DB_PATH",
                dir.path().join("identity-cozo"),
            );
            dir
        });
    }

    /// An engine with the projection tree and the durable store both sandboxed.
    pub(crate) fn sled_engine() -> (Arc<MutationEngine>, TempShm) {
        sled_cozo_sandbox();
        let shm = temp_shm();
        (test_engine(), shm)
    }

    /// Simulate the §5.2 crash window: the record survives but its anchor
    /// never reached Cozo, so hydration produced `genesis: None`.
    pub(crate) async fn clear_genesis(engine: &MutationEngine, session_id: &str) {
        let mut cache = read_cache(engine).await;
        if let Some(sled) = cache
            .sleds
            .iter_mut()
            .find(|sled| sled.session_id == session_id)
        {
            sled.genesis = None;
            sled.arrival_timestamp = 0;
            sled.chain_head_at_arrival = String::new();
            sled.catalog_hash_at_arrival = String::new();
            sled.head_timestamp_at_arrival = 0;
        }
        write_cache(engine, &cache).await.expect("cache write");
    }

    pub(crate) async fn write_identity(
        engine: &MutationEngine,
        pubkey: &str,
    ) -> anyhow::Result<ContainerIdentitySled> {
        let out = dispatch_identity_sled_method(
            engine,
            "write_identity",
            &serde_json::json!({ "wireguard_pubkey": pubkey }),
        )
        .await?;
        Ok(serde_json::from_value(
            out.get("identity").cloned().expect("identity in output"),
        )?)
    }

    #[test]
    fn provision_policy_forces_parked_non_autostart_container() {
        let session_id = op_identity::session::derive_session_id(&pk(0x60));
        let instance = IncusInstance {
            name: "caller-selected".to_string(),
            status: "Running".to_string(),
            instance_type: "virtual-machine".to_string(),
            config: Some(std::collections::HashMap::from([(
                "boot.autostart".to_string(),
                "true".to_string(),
            )])),
            ..Default::default()
        };

        let parked = prepare_parked_instance(&session_id, instance).expect("parked instance");
        assert_eq!(parked.name, session_id);
        assert_eq!(parked.status, "Stopped");
        assert_eq!(parked.instance_type, "container");
        assert_eq!(parked.profiles, ["identity"]);
        assert_eq!(
            parked
                .config
                .as_ref()
                .and_then(|config| config.get("boot.autostart"))
                .map(String::as_str),
            Some("false")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parked_container_starts_once_and_stops_once_per_active_term() {
        let (engine, _shm) = sled_engine();
        let identity = write_identity(&engine, &pk(0x61)).await.expect("write");
        let session_id = identity.session_id.clone();
        let mut cache = read_cache(engine.as_ref()).await;
        let sled = cache
            .sleds
            .iter_mut()
            .find(|sled| sled.session_id == session_id)
            .expect("identity sled");
        sled.active = false;
        sled.instance = Some(
            prepare_parked_instance(&session_id, IncusInstance::default())
                .expect("parked instance"),
        );
        write_cache(engine.as_ref(), &cache)
            .await
            .expect("park cache");

        let lifecycle = RecordingLifecycle::default();
        let activated = set_session_active_with(engine.as_ref(), &session_id, true, &lifecycle)
            .await
            .expect("activate");
        assert!(activated.active);
        assert_eq!(
            activated
                .instance
                .as_ref()
                .map(|instance| instance.status.as_str()),
            Some("Running")
        );

        // A second binding/touch for the same live term does not start twice.
        set_session_active_with(engine.as_ref(), &session_id, true, &lifecycle)
            .await
            .expect("repeat activation");
        let parked = set_session_active_with(engine.as_ref(), &session_id, false, &lifecycle)
            .await
            .expect("deactivate");
        assert!(!parked.active);
        assert_eq!(
            parked
                .instance
                .as_ref()
                .map(|instance| instance.status.as_str()),
            Some("Stopped")
        );

        // Repeated cleanup/disconnect remains idempotent.
        set_session_active_with(engine.as_ref(), &session_id, false, &lifecycle)
            .await
            .expect("repeat deactivation");
        assert_eq!(
            *lifecycle.calls.lock().expect("recorded calls"),
            vec![format!("start:{session_id}"), format!("stop:{session_id}")]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stop_failure_still_invalidates_and_persists_session_first() {
        let (engine, _shm) = sled_engine();
        let identity = write_identity(&engine, &pk(0x62)).await.expect("write");
        let session_id = identity.session_id.clone();
        let mut cache = read_cache(engine.as_ref()).await;
        let sled = cache
            .sleds
            .iter_mut()
            .find(|sled| sled.session_id == session_id)
            .expect("identity sled");
        let mut instance =
            prepare_parked_instance(&session_id, IncusInstance::default()).expect("instance");
        instance.status = "Running".to_string();
        sled.instance = Some(instance);
        sled.active = true;
        write_cache(engine.as_ref(), &cache)
            .await
            .expect("active cache");

        let lifecycle = FailingStopLifecycle::default();
        let result = set_session_active_with(engine.as_ref(), &session_id, false, &lifecycle).await;
        assert!(result.is_err(), "physical stop failure must be reported");
        let parked = stored_session(engine.as_ref(), &session_id)
            .await
            .expect("parked sled retained");
        assert!(!parked.active, "credential must fail closed before stop");
        assert_eq!(
            parked
                .instance
                .as_ref()
                .map(|instance| instance.status.as_str()),
            Some("Running"),
            "failed physical stop remains retryable while authority is inactive"
        );
        assert_eq!(
            *lifecycle.calls.lock().expect("recorded stop"),
            vec![format!("stop:{session_id}")]
        );

        let retry = RecordingLifecycle::default();
        assert_eq!(
            park_orphaned_container_sessions_with(engine.as_ref(), &retry)
                .await
                .expect("startup retry"),
            1
        );
        let stopped = stored_session(engine.as_ref(), &session_id)
            .await
            .expect("stopped sled");
        assert!(!stopped.active);
        assert_eq!(
            stopped
                .instance
                .as_ref()
                .map(|instance| instance.status.as_str()),
            Some("Stopped")
        );
        assert_eq!(
            *retry.calls.lock().expect("recorded retry"),
            vec![format!("stop:{session_id}")]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn activation_projection_failure_rolls_back_container_and_authority() {
        let (engine, _shm) = sled_engine();
        let identity = write_identity(&engine, &pk(0x65)).await.expect("write");
        let session_id = identity.session_id.clone();
        let mut cache = read_cache(engine.as_ref()).await;
        let sled = cache
            .sleds
            .iter_mut()
            .find(|sled| sled.session_id == session_id)
            .expect("identity sled");
        sled.instance = Some(
            prepare_parked_instance(&session_id, IncusInstance::default())
                .expect("parked instance"),
        );
        sled.active = false;
        write_cache(engine.as_ref(), &cache)
            .await
            .expect("parked cache");
        let parked_before = stored_session(engine.as_ref(), &session_id)
            .await
            .expect("parked identity");
        persist_sled(&parked_before).await;

        let lifecycle = RecordingLifecycle::default();
        let publisher = FailFirstProjection::default();
        let activation = set_session_active_with_publisher(
            engine.as_ref(),
            &session_id,
            true,
            &lifecycle,
            &publisher,
        )
        .await;
        assert!(activation.is_err(), "projection failure must be returned");
        let after = stored_session(engine.as_ref(), &session_id)
            .await
            .expect("rolled-back identity");
        assert!(
            !after.active,
            "failed activation must not leave an active OIB"
        );
        assert_eq!(
            after
                .instance
                .as_ref()
                .map(|instance| instance.status.as_str()),
            Some("Stopped")
        );
        assert_eq!(
            *lifecycle.calls.lock().expect("activation rollback calls"),
            vec![format!("start:{session_id}"), format!("stop:{session_id}")]
        );
        let durable = sled_cozo()
            .expect("test Cozo")
            .get_identity_sled(&session_id)
            .expect("durable read")
            .expect("durable row");
        assert!(!durable.active, "rollback must persist inactive authority");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn startup_parks_orphaned_containers_but_leaves_host_identity_active() {
        let (engine, _shm) = sled_engine();
        let container = write_identity(&engine, &pk(0x63))
            .await
            .expect("container identity");
        let host = write_identity(&engine, &pk(0x64))
            .await
            .expect("host identity");
        let mut cache = read_cache(engine.as_ref()).await;
        let container_sled = cache
            .sleds
            .iter_mut()
            .find(|sled| sled.session_id == container.session_id)
            .expect("container sled");
        let mut instance = prepare_parked_instance(&container.session_id, IncusInstance::default())
            .expect("instance");
        instance.status = "Running".to_string();
        container_sled.instance = Some(instance);
        container_sled.active = true;
        write_cache(engine.as_ref(), &cache)
            .await
            .expect("startup fixture cache");

        let lifecycle = RecordingLifecycle::default();
        let parked = park_orphaned_container_sessions_with(engine.as_ref(), &lifecycle)
            .await
            .expect("startup reconciliation");
        assert_eq!(parked, 1);
        assert!(
            !stored_session(engine.as_ref(), &container.session_id)
                .await
                .expect("container retained")
                .active
        );
        assert!(
            stored_session(engine.as_ref(), &host.session_id)
                .await
                .expect("host retained")
                .active,
            "host/no-instance identity must not be parked at bridge startup"
        );
        assert_eq!(
            *lifecycle.calls.lock().expect("recorded orphan stop"),
            vec![format!("stop:{}", container.session_id)]
        );
    }

    /// VAL-SLED-GEN-001 (`write_identity_mints_genesis`): provisioning a new
    /// session produces a non-empty genesis, minted through `mint_genesis`
    /// against the real chain head — never `etch_footprint`.
    #[tokio::test(flavor = "multi_thread")]
    async fn write_identity_mints_genesis() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x51);
        let identity = write_identity(&engine, &pubkey).await.expect("write");

        let genesis = identity.genesis.clone().expect("genesis minted");
        assert_eq!(genesis.len(), 64, "genesis is hex blake3: {genesis}");
        assert!(genesis.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(genesis, "0".repeat(64), "genesis must not be the sentinel");
        assert_eq!(identity.schema_version, RECORD_FORMAT);
        assert_ne!(
            identity.arrival_timestamp, 0,
            "arrival_timestamp is stored for offline re-verification"
        );
        assert!(
            !identity.chain_head_at_arrival.is_empty(),
            "the anchor binds the chain head it was minted against"
        );
    }

    /// VAL-SLED-GEN-002: re-registering the same pubkey does not re-mint — the
    /// anchor is immutable for the life of the session.
    #[tokio::test(flavor = "multi_thread")]
    async fn write_identity_does_not_remint_existing_genesis() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x52);
        let first = write_identity(&engine, &pubkey).await.expect("write");
        let second = write_identity(&engine, &pubkey).await.expect("rewrite");
        assert_eq!(
            first.genesis, second.genesis,
            "genesis must not be recomputed for an existing session"
        );
        assert_eq!(first.arrival_timestamp, second.arrival_timestamp);
        assert_eq!(
            first.sealed_id, second.sealed_id,
            "the exact authored SID1 bytes must be preserved"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn callers_cannot_write_mutation_engine_sealed_id() {
        let (engine, _shm) = sled_engine();
        let args = serde_json::json!({
            "wireguard_pubkey": pk(0x5a),
            "sealed_id": "sid1:caller-controlled"
        });
        let write = dispatch_identity_sled_method(&engine, "write_identity", &args).await;
        assert!(write.is_err());
        let provision = dispatch_identity_sled_method(&engine, "provision_container", &args).await;
        assert!(provision.is_err());
    }

    /// VAL-SLED-GEN-003 (`mutation_does_not_overwrite_liveness`): the mutation
    /// path owns `mutation_index` + `genesis` and touches nothing the stream
    /// path owns (FR-6 field disjointness).
    #[tokio::test(flavor = "multi_thread")]
    async fn mutation_does_not_overwrite_liveness() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x53);
        let identity = write_identity(&engine, &pubkey).await.expect("write");
        let session_id = identity.session_id.clone();

        // Stream path: set the liveness fields the mutation path must not touch.
        {
            let mut cache = read_cache(engine.as_ref()).await;
            let sled = cache
                .sleds
                .iter_mut()
                .find(|s| s.session_id == session_id)
                .expect("record");
            sled.last_seen_at = 4_242;
            sled.active = true;
            sled.peer_ip = Some("10.0.0.9".to_string());
            sled.session_started_at = 1_111;
            write_cache(engine.as_ref(), &cache).await.expect("cache");
        }

        advance_mutation_index(engine.as_ref(), &session_id, 9_001)
            .await
            .expect("advance");

        let after = stored_session(engine.as_ref(), &session_id)
            .await
            .expect("record");
        assert_eq!(after.mutation_index, 9_001);
        assert_eq!(after.last_seen_at, 4_242, "liveness field preserved");
        assert!(after.active);
        assert_eq!(after.peer_ip.as_deref(), Some("10.0.0.9"));
        assert_eq!(after.session_started_at, 1_111);
        assert_eq!(after.genesis, identity.genesis, "anchor preserved");
    }

    /// VAL-SLED-GEN-004 (`stream_does_not_overwrite_genesis`): a liveness
    /// touch preserves the anchor and the chain position.
    #[tokio::test(flavor = "multi_thread")]
    async fn stream_does_not_overwrite_genesis() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x54);
        let identity = write_identity(&engine, &pubkey).await.expect("write");
        let session_id = identity.session_id.clone();
        advance_mutation_index(engine.as_ref(), &session_id, 77)
            .await
            .expect("advance");

        dispatch_identity_sled_method(
            engine.as_ref(),
            "touch_session",
            &serde_json::json!({ "session_id": session_id }),
        )
        .await
        .expect("touch");

        let after = stored_session(engine.as_ref(), &session_id)
            .await
            .expect("record");
        assert_eq!(after.genesis, identity.genesis);
        assert_eq!(after.mutation_index, 77);
    }

    /// VAL-SLED-GEN-005: `mutation_index` never regresses.
    #[tokio::test(flavor = "multi_thread")]
    async fn mutation_index_advances_only() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x55);
        let identity = write_identity(&engine, &pubkey).await.expect("write");
        let session_id = identity.session_id.clone();

        advance_mutation_index(engine.as_ref(), &session_id, 500)
            .await
            .expect("advance");
        advance_mutation_index(engine.as_ref(), &session_id, 10)
            .await
            .expect("stale write ignored");

        let after = stored_session(engine.as_ref(), &session_id)
            .await
            .expect("record");
        assert_eq!(after.mutation_index, 500);
    }

    /// VAL-SLED-GEN-006: every handle on the record resolves the same session.
    #[tokio::test(flavor = "multi_thread")]
    async fn session_record_resolves_by_any_handle() {
        let (engine, _shm) = sled_engine();
        let pubkey = pk(0x56);
        let identity = write_identity(&engine, &pubkey).await.expect("write");
        let genesis = identity.genesis.clone().expect("genesis");

        for handle in [
            identity.session_id.as_str(),
            pubkey.as_str(),
            identity.trace_id.as_str(),
            genesis.as_str(),
        ] {
            let found = session_record_for_actor(engine.as_ref(), handle)
                .await
                .unwrap_or_else(|| panic!("handle '{handle}' must resolve the session"));
            assert_eq!(found.session_id, identity.session_id);
        }
        assert!(session_record_for_actor(engine.as_ref(), "")
            .await
            .is_none());
        assert!(session_record_for_actor(engine.as_ref(), "nobody")
            .await
            .is_none());
    }
}
