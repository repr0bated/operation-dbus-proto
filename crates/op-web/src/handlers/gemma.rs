//! Gemma UI gallery handlers.
//!
//! Reads/writes `/dev/shm/gemma-ui-specs.json` (produced by the `op-gemma` binary).
//! The lovable SPA gallery page polls these endpoints to display 200 json-render.dev
//! specs, delete individual specs, and trigger regeneration.

use axum::{
    extract::Path,
    http::StatusCode,
    response::Json,
};
use serde_json::{json, Value};
use std::fs;
use tracing::warn;

const GEMMA_SPECS_PATH: &str = "/dev/shm/gemma-ui-specs.json";

/// GET /api/gemma/specs — return the full gallery JSON.
pub async fn specs_handler() -> Result<Json<Value>, (StatusCode, String)> {
    match fs::read_to_string(GEMMA_SPECS_PATH) {
        Ok(raw) => {
            let parsed: Value = serde_json::from_str(&raw)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse error: {e}")))?;
            Ok(Json(parsed))
        }
        Err(_) => {
            // File doesn't exist yet — return empty gallery
            Ok(Json(json!({"version": 1, "specs": []})))
        }
    }
}

/// DELETE /api/gemma/specs/:id — delete a spec by ID.
/// The op-gemma binary will regenerate a replacement on its next run.
/// For immediate replacement, we remove the spec and rewrite the file.
pub async fn delete_spec_handler(Path(spec_id): Path<String>) -> Result<Json<Value>, (StatusCode, String)> {
    let raw = fs::read_to_string(GEMMA_SPECS_PATH)
        .map_err(|e| (StatusCode::NOT_FOUND, format!("gallery not found: {e}")))?;
    let mut gallery: Value = serde_json::from_str(&raw)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse error: {e}")))?;

    if let Some(specs) = gallery.get_mut("specs").and_then(|s| s.as_array_mut()) {
        let before = specs.len();
        specs.retain(|s| {
            s.get("id").and_then(|v| v.as_str()).map_or(true, |id| id != spec_id)
        });
        let after = specs.len();
        if before == after {
            return Err((StatusCode::NOT_FOUND, format!("spec {spec_id} not found")));
        }
    }

    // Atomic write
    let tmp = format!("{GEMMA_SPECS_PATH}.tmp");
    let json_str = serde_json::to_string(&gallery)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize error: {e}")))?;
    fs::write(&tmp, &json_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write error: {e}")))?;
    fs::rename(&tmp, GEMMA_SPECS_PATH)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("rename error: {e}")))?;

    Ok(Json(json!({"deleted": spec_id, "remaining": gallery.get("specs").and_then(|s| s.as_array()).map(|a| a.len()).unwrap_or(0)})))
}

/// POST /api/gemma/regenerate — trigger op-gemma to regenerate the gallery.
/// Runs the op-gemma binary which tops up the gallery back to 200 specs.
pub async fn regenerate_handler() -> Result<Json<Value>, (StatusCode, String)> {
    let gemma_bin = std::env::var("GEMMA_BINARY_PATH")
        .unwrap_or_else(|_| "/usr/local/bin/op-gemma".to_string());

    match tokio::process::Command::new(&gemma_bin)
        .env("RUST_LOG", "info")
        .output()
        .await
    {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Gemma regeneration failed: {stderr}");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("gemma exited with {}", output.status)));
            }
            // Read the updated gallery
            let raw = fs::read_to_string(GEMMA_SPECS_PATH)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("gallery read error: {e}")))?;
            let gallery: Value = serde_json::from_str(&raw)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse error: {e}")))?;
            let count = gallery.get("specs").and_then(|s| s.as_array()).map(|a| a.len()).unwrap_or(0);
            Ok(Json(json!({"regenerated": true, "total": count})))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("failed to run gemma: {e}")))
        }
    }
}

/// GET /api/gemma/catalog — return the plugin schema catalog (for reference).
pub async fn catalog_handler() -> Result<Json<Value>, (StatusCode, String)> {
    let schema_path = "/dev/shm/live-schema.json";
    match fs::read_to_string(schema_path) {
        Ok(raw) => {
            let parsed: Value = serde_json::from_str(&raw)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse error: {e}")))?;
            Ok(Json(parsed))
        }
        Err(_) => Ok(Json(json!({}))),
    }
}
