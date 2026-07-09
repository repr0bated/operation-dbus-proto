//! identity_sled method dispatch — the container is the sled is the identity.
//!
//! Executes the `identity_sled` plugin's method surface against the
//! MutationEngine's authoritative state cache. Every call is already
//! notarized in the immutable event chain by `dispatch_method_call` before it
//! reaches here, so this module only performs the domain effect.
//!
//! Compatibility: when the written identity is the host's own ("container
//! zero"), the legacy raw sled at `/dev/shm/plugin_schema.dat` is also
//! refreshed via `op_identity::schema_bridge::write_sled_from_wg`, because
//! AnnaScribe still gates arrivals on that file. That projection goes away
//! when AnnaScribe reads the sealed blob directly.

use base64::Engine;
use op_plugins::state_plugins::identity_sled::{
    ContainerIdentitySled, IdentitySledState, SessionEvent, SledBtrfsDevice,
};
use serde_json::Value as JsonValue;

use crate::mutation_engine::MutationEngine;

/// Session events kept in present-state per session. The event chain is the
/// durable ledger; Cozo `session_memories` is the queryable archive.
const MAX_EVENTS_IN_STATE: usize = 256;

/// In-state snowball ledger key inside the plugin state cache entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct SledCacheState {
    #[serde(default)]
    sleds: Vec<ContainerIdentitySled>,
    #[serde(default)]
    events: Vec<SessionEvent>,
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

fn arg_str(args: &JsonValue, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

pub async fn dispatch_identity_sled_method(
    engine: &MutationEngine,
    method: &str,
    args: &JsonValue,
) -> anyhow::Result<JsonValue> {
    match method {
        "get_identity" => {
            let session_id = arg_str(args, "session_id");
            let cache = read_cache(engine).await;
            let sled = if session_id.is_empty() {
                // Container zero: the host's own identity.
                let host_pubkey = op_identity::wireguard::WireGuardIdentity::new()
                    .get_local_pubkey()
                    .unwrap_or_default();
                let host_id = op_identity::session::derive_session_id(&host_pubkey);
                cache.sleds.iter().find(|s| s.session_id == host_id).cloned()
            } else {
                cache.sleds.iter().find(|s| s.session_id == session_id).cloned()
            };
            match sled {
                Some(identity) => Ok(serde_json::json!({ "identity": identity })),
                None => Err(anyhow::anyhow!(
                    "no identity sled for session '{}'",
                    session_id
                )),
            }
        }

        "write_identity" => {
            let pubkey = arg_str(args, "wireguard_pubkey");
            if pubkey.is_empty() {
                anyhow::bail!("write_identity requires wireguard_pubkey");
            }
            // session_id is DERIVED from PSK + pubkey when PSK is supplied at provision time.
            let session_id = if let Some(psk_b64) = args.get("psk").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                let psk = base64::engine::general_purpose::STANDARD
                    .decode(psk_b64.trim())
                    .map_err(|e| anyhow::anyhow!("invalid psk base64: {e}"))?;
                op_identity::session::derive_session_id_from_psk(&pubkey, &psk)?
            } else {
                op_identity::session::derive_session_id(&pubkey)
            };
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
            let existing = cache
                .sleds
                .iter_mut()
                .find(|s| s.session_id == session_id);
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

        "touch_session" => {
            let session_id = arg_str(args, "session_id");
            let mut cache = read_cache(engine).await;
            let Some(sled) = cache
                .sleds
                .iter_mut()
                .find(|s| s.session_id == session_id)
            else {
                anyhow::bail!("no identity sled for session '{}'", session_id);
            };
            sled.last_seen_at = now();
            sled.active = true;
            write_cache(engine, &cache).await?;
            Ok(serde_json::json!({ "success": true }))
        }

        "record_session_event" => {
            let session_id = arg_str(args, "session_id");
            let kind = arg_str(args, "kind");
            if session_id.is_empty() || kind.is_empty() {
                anyhow::bail!("record_session_event requires session_id and kind");
            }
            let mut cache = read_cache(engine).await;
            let seq = cache
                .events
                .iter()
                .filter(|e| e.session_id == session_id)
                .map(|e| e.seq)
                .max()
                .map(|s| s + 1)
                .unwrap_or(0);
            cache.events.push(SessionEvent {
                session_id,
                seq,
                kind,
                subid: arg_str(args, "subid"),
                content: arg_str(args, "content"),
                created_at: now(),
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
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
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

        other => Err(anyhow::anyhow!(
            "unknown identity_sled method '{}'",
            other
        )),
    }
}

/// Expose the typed state for consumers that want the full sled set.
pub async fn current_state(engine: &MutationEngine) -> IdentitySledState {
    let cache = read_cache(engine).await;
    IdentitySledState { sleds: cache.sleds }
}
