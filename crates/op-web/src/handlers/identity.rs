//! Identity Sled Handler — Read the live identity sled from shared memory.
//!
//! GET /api/identity/sled — returns the current sled contents as JSON.
//! Readable by any WireGuard-connected device without root access.

use axum::{
    body::Body,
    extract::Extension,
    http::{header, StatusCode},
    response::Response,
};
use op_identity::schema_bridge::{read_sled, IdentitySled};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

use crate::state::AppState;

/// GET /api/identity/sled
pub async fn identity_sled_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    match read_sled() {
        Ok((ptr, _mmap)) => {
            let sled: &IdentitySled = unsafe { &*ptr };

            let schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
                .map(|bytes| hex::encode(blake3::hash(&bytes).as_bytes()))
                .unwrap_or_else(|_| "(missing)".to_string());

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
                "backend": false,
            });

            info!("Served identity sled from shared memory");
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        }
        Err(e) => {
            warn!(error = %e, "Identity sled not available");
            Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"error":"Identity sled not available","detail":"{}"}}"#,
                    e
                )))
                .unwrap()
        }
    }
}

fn encode_b64(bytes: &[u8; 32]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
