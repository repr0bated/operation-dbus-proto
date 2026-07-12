//! identity_sled method dispatch — the container is the sled is the identity.
//!
//! Executes the `identity_sled` plugin's method surface against the
//! MutationEngine's authoritative state cache. Every call is already
//! notarized in the immutable event chain by `dispatch_method_call` before it
//! reaches here, so this module only performs the domain effect.
//!
//! Durability: sleds and the session-event "snowball" ledger persist to the
//! Cozo relations `identity_sleds` / `session_events` (own sled-engine path —
//! never the users-cozo or cognitive-mcp store, which other processes hold
//! open). The event chain remains the durable proof; Cozo is the queryable
//! archive plus the restart-warm source for the state cache. All Cozo I/O is
//! synchronous sled disk work, so it runs under `spawn_blocking`.
//!
//! Provisioning: the sled inherits the Incus schema — `provision_container`
//! creates the Incus instance (named by the derived session_id) and writes
//! the sled in one mutation; a sled exists ⟺ its container exists.
//!
//! Compatibility: when the written identity is the host's own ("container
//! zero"), the legacy raw sled at `/dev/shm/plugin_schema.dat` is also
//! refreshed via `op_identity::schema_bridge::write_sled_from_wg`, because
//! AnnaScribe still gates arrivals on that file. That projection goes away
//! when AnnaScribe reads the sealed blob directly.

use std::path::PathBuf;
use std::sync::OnceLock;

use base64::Engine;
use op_cozo_store::{CozoGraphShuttle, IdentitySledRecord};
use op_plugins::state_plugins::identity_sled::{
    ContainerIdentitySled, IdentitySledState, SessionEvent, SledBtrfsDevice,
};
use op_plugins::state_plugins::incus::{IncusInstance, IncusPlugin};
use op_plugins::state_plugins::incus_device::Device;
use serde_json::Value as JsonValue;
use tokio::sync::OnceCell;

use crate::mutation_engine::MutationEngine;

/// Session events kept in present-state per session. The event chain is the
/// durable ledger and Cozo `session_events` the queryable archive; the state
/// cache only carries a recent window.
const MAX_EVENTS_IN_STATE: usize = 256;

/// Default Cozo path for the identity sled store. Own path — the sled engine
/// is single-process, and `/var/lib/op-dbus/users-cozo` (op-web-server) and
/// the cognitive-mcp store are already held by other processes.
const DEFAULT_SLED_COZO_PATH: &str = "/var/lib/op-dbus/identity-cozo";

/// In-state snowball ledger key inside the plugin state cache entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct SledCacheState {
    #[serde(default)]
    sleds: Vec<ContainerIdentitySled>,
    #[serde(default)]
    events: Vec<SessionEvent>,
}

/// Lazily opened durable store; `None` (with one warning) when the path can't
/// be opened — dispatch then runs cache-only, as before.
fn sled_cozo() -> Option<&'static CozoGraphShuttle> {
    static SLED_COZO: OnceLock<Option<CozoGraphShuttle>> = OnceLock::new();
    SLED_COZO
        .get_or_init(|| {
            let path = std::env::var("IDENTITY_SLED_COZO_DB_PATH")
                .unwrap_or_else(|_| DEFAULT_SLED_COZO_PATH.to_string());
            match CozoGraphShuttle::new_persistent(PathBuf::from(&path)) {
                Ok(store) => Some(store),
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
        hashed_footprint: sled.hashed_footprint.clone(),
        trace_id: sled.trace_id.clone(),
        schema_version: sled.schema_version as i64,
        vector_id: sled.vector_id.clone(),
        blob_ref: sled.blob_ref.clone().unwrap_or_default(),
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
        hashed_footprint: rec.hashed_footprint.clone(),
        trace_id: rec.trace_id.clone(),
        schema_version: rec.schema_version.max(0) as u32,
        vector_id: rec.vector_id.clone(),
        blob_ref: (!rec.blob_ref.is_empty()).then(|| rec.blob_ref.clone()),
        btrfs_device: parse_json("btrfs_device", &rec.btrfs_device_json)
            .and_then(|v| serde_json::from_value(v).ok()),
        instance: parse_json("instance", &rec.instance_json)
            .and_then(|v| serde_json::from_value(v).ok()),
        session_started_at: rec.session_started_at,
        last_seen_at: rec.last_seen_at,
        active: rec.active,
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
            let rows = tokio::task::spawn_blocking(move || cozo.list_identity_sleds()).await;
            let rows = match rows {
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
            cache.sleds.sort_by(|a, b| a.session_id.cmp(&b.session_id));
            if let Err(e) = write_cache(engine, &cache).await {
                tracing::warn!(error = %e, "identity sled hydration cache write failed");
            }
        })
        .await;
}

/// Persist one sled row to Cozo; failure is logged, not fatal — the event
/// chain already notarized the mutation.
async fn persist_sled(sled: &ContainerIdentitySled) {
    let Some(cozo) = sled_cozo() else { return };
    let cozo = cozo.clone();
    let rec = sled_to_record(sled);
    match tokio::task::spawn_blocking(move || cozo.put_identity_sled(&rec)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "identity sled Cozo persist failed"),
        Err(e) => tracing::warn!(error = %e, "identity sled Cozo persist task failed"),
    }
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

fn host_identity_from_raw_sled() -> anyhow::Result<ContainerIdentitySled> {
    let (ptr, _mmap) = op_identity::schema_bridge::read_sled()?;
    let sled = unsafe { &*ptr };
    let wireguard_pubkey = base64::engine::general_purpose::STANDARD.encode(sled.wireguard_pubkey);
    let ts = now();

    Ok(ContainerIdentitySled {
        session_id: op_identity::session::derive_session_id(&wireguard_pubkey),
        wireguard_pubkey,
        interface: std::env::var("IDENTITY_SLED_HOST_INTERFACE")
            .unwrap_or_else(|_| "opdbus".to_string()),
        peer_ip: std::env::var("IDENTITY_SLED_HOST_PEER_IP").ok(),
        mutation_index: sled.mutation_index,
        hashed_footprint: hex::encode(sled.hashed_footprint),
        trace_id: sled.trace_id_hex(),
        schema_version: sled.schema_version,
        vector_id: sled.vector_id_hex(),
        blob_ref: Some(op_identity::schema_bridge::SHM_SLED_PATH.to_string()),
        btrfs_device: None,
        instance: None,
        session_started_at: ts,
        last_seen_at: ts,
        active: sled.is_sled_valid(),
    })
}

fn arg_str(args: &JsonValue, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
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

pub async fn dispatch_identity_sled_method(
    engine: &MutationEngine,
    method: &str,
    args: &JsonValue,
) -> anyhow::Result<JsonValue> {
    match method {
        "get_identity" => {
            let session_id = arg_str(args, "session_id");
            if session_id.is_empty() {
                let identity = host_identity_from_raw_sled()?;
                return Ok(serde_json::json!({ "identity": identity }));
            }

            ensure_hydrated(engine).await;
            let cache = read_cache(engine).await;
            let sled = cache
                .sleds
                .iter()
                .find(|s| s.session_id == session_id)
                .cloned();
            match sled {
                Some(identity) => Ok(serde_json::json!({ "identity": identity })),
                None => Err(anyhow::anyhow!(
                    "no identity sled for session '{}'",
                    session_id
                )),
            }
        }

        "write_identity" => {
            ensure_hydrated(engine).await;
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
            let blob_ref = args
                .get("blob_ref")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let btrfs_device: Option<SledBtrfsDevice> = args
                .get("btrfs_device")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok());

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
                    if blob_ref.is_some() {
                        sled.blob_ref = blob_ref;
                    }
                    if btrfs_device.is_some() {
                        sled.btrfs_device = btrfs_device;
                    }
                    sled.mutation_index += 1;
                    sled.last_seen_at = ts;
                    sled.active = true;
                    sled.clone()
                }
                None => {
                    let sled = ContainerIdentitySled {
                        session_id: session_id.clone(),
                        wireguard_pubkey: pubkey.clone(),
                        interface,
                        peer_ip,
                        mutation_index: 0,
                        hashed_footprint: String::new(),
                        trace_id: String::new(),
                        schema_version: 0,
                        vector_id: String::new(),
                        blob_ref,
                        btrfs_device,
                        instance: None,
                        session_started_at: ts,
                        last_seen_at: ts,
                        active: true,
                    };
                    cache.sleds.push(sled.clone());
                    cache.sleds.sort_by(|a, b| a.session_id.cmp(&b.session_id));
                    sled
                }
            };
            write_cache(engine, &cache).await?;
            persist_sled(&identity).await;

            // Container zero: refresh the legacy raw sled AnnaScribe gates on.
            let host_pubkey = op_identity::wireguard::WireGuardIdentity::new()
                .get_local_pubkey()
                .unwrap_or_default();
            if !host_pubkey.is_empty() && host_pubkey == pubkey {
                if let Err(e) = op_identity::schema_bridge::write_sled_from_wg(&pubkey) {
                    tracing::warn!(error = %e, "legacy sled projection write failed");
                }
            }

            Ok(serde_json::json!({ "identity": identity }))
        }

        "provision_container" => {
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
            // The container name IS the identity — always the derived id.
            instance.name = session_id.clone();
            if instance.instance_type.is_empty() {
                instance.instance_type = "container".to_string();
            }
            if instance.status.is_empty() {
                instance.status = "Running".to_string();
            }
            // No profile override = the Incus "default" profile, which on
            // this host supplies only the root disk device (no nic) — an
            // empty profile list would make apply_create pass --no-profiles,
            // and Incus then fails instance creation outright with no root
            // device to mount.
            if instance.profiles.is_empty() {
                instance.profiles = vec!["default".to_string()];
            }
            // Hard rule: no container NIC or IP; all I/O over the UDS surface.
            if instance
                .devices
                .iter()
                .chain(instance.expanded_devices.iter())
                .any(|d| matches!(d.device, Device::Nic(_)))
            {
                anyhow::bail!(
                    "nic devices are not allowed: containers get no NIC or IP \
                     (all container I/O goes over the UDS socket surface)"
                );
            }

            let blob_ref = args
                .get("blob_ref")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let btrfs_device: Option<SledBtrfsDevice> = args
                .get("btrfs_device")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok());

            let cache = read_cache(engine).await;
            if cache
                .sleds
                .iter()
                .any(|s| s.session_id == session_id && s.active)
            {
                anyhow::bail!(
                    "identity sled '{}' is already provisioned and active",
                    session_id
                );
            }
            drop(cache);

            // Create the container first: a sled exists ⟺ its container
            // exists, so an incus failure means no sled row.
            IncusPlugin::apply_create(&instance)
                .await
                .map_err(|e| anyhow::anyhow!("incus create for '{session_id}' failed: {e:#}"))?;

            let ts = now();
            let sled = ContainerIdentitySled {
                session_id: session_id.clone(),
                wireguard_pubkey: pubkey,
                interface: String::new(),
                peer_ip: None,
                mutation_index: 0,
                hashed_footprint: String::new(),
                trace_id: String::new(),
                schema_version: 0,
                vector_id: String::new(),
                blob_ref,
                btrfs_device,
                instance: Some(instance),
                session_started_at: ts,
                last_seen_at: ts,
                active: true,
            };
            let mut cache = read_cache(engine).await;
            cache.sleds.retain(|s| s.session_id != session_id);
            cache.sleds.push(sled.clone());
            cache.sleds.sort_by(|a, b| a.session_id.cmp(&b.session_id));
            write_cache(engine, &cache).await?;
            persist_sled(&sled).await;

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
            let mut cache = read_cache(engine).await;
            let Some(sled) = cache.sleds.iter_mut().find(|s| s.session_id == session_id) else {
                anyhow::bail!("no identity sled for session '{}'", session_id);
            };
            let ts = now();
            sled.last_seen_at = ts;
            sled.active = true;
            write_cache(engine, &cache).await?;
            if let Some(cozo) = sled_cozo() {
                let cozo = cozo.clone();
                let sid = session_id.clone();
                match tokio::task::spawn_blocking(move || cozo.touch_identity_sled(&sid, ts, true))
                    .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => tracing::warn!(error = %e, "identity sled touch persist failed"),
                    Err(e) => tracing::warn!(error = %e, "identity sled touch task failed"),
                }
            }
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
            events.sort_by(|a, b| b.seq.cmp(&a.seq));
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
