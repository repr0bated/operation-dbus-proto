//! Schema Handler — Single Source of Truth from Shared Memory
//!
//! Serves the canonical PluginSchema catalog from `/dev/shm/live-schema.json`.
//! All UI, blockchain, and downstream consumers read from this endpoint.

use axum::{
    body::Body,
    extract::Extension,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{info, warn};

use crate::state::AppState;

const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

/// GET /api/schema — Return the canonical schema catalog from shared memory.
///
/// The file is already valid JSON; we return it as raw bytes to avoid any
/// serialization round-trip. If the schema catalog is missing or unreadable,
/// returns 503 Service Unavailable to enforce the Absolute Base rule:
/// without a valid schema, the entity does not exist.
pub async fn schema_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    match std::fs::read(SHM_SCHEMA_PATH) {
        Ok(bytes) => {
            info!(
                bytes = bytes.len(),
                "Served schema catalog from shared memory"
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(e) => {
            warn!(error = %e, "Schema SHM not available");
            Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"error":"Schema catalog not available","detail":"{}"}}"#,
                    e
                )))
                .unwrap()
        }
    }
}
