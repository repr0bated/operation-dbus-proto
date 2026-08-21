//! Identity Sled Handler — Read the live identity sled from shared memory.
//!
//! GET /api/identity/sled — returns the current sled contents as JSON.
//! Readable by any WireGuard-connected device without root access.
//!
//! Primary source: the Cozo-backed sled state projected to
//! `/dev/shm/opdbus/state/identity_sled.json` by the identity_sled plugin
//! (the current identity system). Falls back to the legacy 152-byte
//! `/dev/shm/plugin_schema.dat` mmap for backward compatibility.

use axum::{
    body::Body,
    extract::Extension,
    http::{header, StatusCode},
    response::Response,
};
use op_identity::schema_bridge::{read_sled, IdentitySled};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

use crate::state::AppState;

/// Minimal projection of the Cozo-backed sled state JSON.
#[derive(Deserialize)]
struct SledStateJson {
    #[serde(default)]
    sleds: Vec<SledEntryJson>,
}

#[derive(Deserialize)]
struct SledEntryJson {
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    genesis: Option<String>,
    #[serde(default)]
    trace_id: Option<String>,
    #[serde(default)]
    mutation_index: Option<u64>,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    wireguard_pubkey: Option<String>,
}

/// Read the active sled from the Cozo-backed state JSON.
fn read_sled_json() -> Result<SledEntryJson, Box<dyn std::error::Error + Send + Sync>> {
    let path = "/dev/shm/opdbus/state/identity_sled.json";
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read {path}: {e}"))?;
    let state: SledStateJson = serde_json::from_str(&content)
        .map_err(|e| format!("parse {path}: {e}"))?;
    state
        .sleds
        .into_iter()
        .find(|s| s.active == Some(true))
        .ok_or("no active sled in state JSON".into())
}

/// GET /api/identity/sled
pub async fn identity_sled_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    // Prefer the Cozo-backed sled state (the current identity system).
    if let Ok(entry) = read_sled_json() {
        let footprint = entry.genesis.unwrap_or_default();
        let trace_id = entry.trace_id.unwrap_or_default();
        let is_valid = !footprint.is_empty() && !trace_id.is_empty();

        let body = json!({
            "path": "/dev/shm/opdbus/state/identity_sled.json",
            "is_valid": is_valid,
            "hashed_footprint": footprint,
            "trace_id": trace_id,
            "mutation_index": entry.mutation_index.unwrap_or(0),
            "schema_version": entry.schema_version.unwrap_or(0),
            "session_id": entry.session_id,
            "wireguard_pubkey": entry.wireguard_pubkey,
            "backend": "cozo",
        });

        info!("Served identity sled from Cozo state JSON");
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&body).expect("identity body serialization should not fail"),
            ))
            .expect("response with valid body should not fail");
    }

    // Fall back to the legacy 152-byte sled mmap (deprecated, may be absent).
    match read_sled() {
        Ok((ptr, _mmap)) => {
            // SAFETY: `ptr` is derived from a live `memmap2::Mmap` (`_mmap`). The mapping
            // outlives this borrow because `_mmap` is dropped after `sled` goes out of
            // scope. `IdentitySled` is a plain-data struct (no padding gaps, no references)
            // so a bitwise read from the mmap is valid as long as the backing file was
            // written with a canonical `IdentitySled` layout.
            let sled: &IdentitySled = unsafe { &*ptr };

            let schema_catalog_hash = op_identity::schema_bridge::schema_catalog_hash()
                .map(hex::encode)
                .unwrap_or_else(|| "(missing)".to_string());

            let is_valid = sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16];

            let body = json!({
                "path": op_identity::schema_bridge::SHM_SLED_PATH,
                "is_valid": is_valid,
                "wireguard_pubkey": encode_b64(&sled.wireguard_pubkey),
                "wireguard_pubkey_hex": hex::encode(sled.wireguard_pubkey),
                "mutation_index": sled.mutation_index,
                "hashed_footprint": hex::encode(sled.hashed_footprint),
                "trace_id": sled.trace_id_hex(),
                "schema_version": sled.schema_version,
                "schema_catalog_hash": schema_catalog_hash,
                "backend": "shm",
            });

            info!("Served identity sled from shared memory (legacy)");
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&body).expect("identity body serialization should not fail"),
                ))
                .expect("response with valid body should not fail")
        }
        Err(e) => {
            warn!(error = %e, "Identity sled not available (both Cozo JSON and legacy SHM failed)");
            Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"error":"Identity sled not available","detail":"{}"}}"#,
                    e
                )))
                .expect("response with valid body should not fail")
        }
    }
}

fn encode_b64(bytes: &[u8; 32]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
