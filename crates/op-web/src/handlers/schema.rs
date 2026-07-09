//! Schema catalog HTTP handlers — blob-catalog only.
//!
//! Absolute Base: PluginSchema lives in sealed blobs under
//! `/dev/shm/opdbus/plugin-blobs/`. The legacy `/dev/shm/live-schema.json`
//! monolith is not consulted.

use crate::state::AppState;
use axum::{extract::Extension, response::Json};
use simd_json::json;
use simd_json::prelude::ValueAsContainer;
use simd_json::OwnedValue as Value;
use std::sync::Arc;

/// Build a catalog map `{ plugin_id: PluginSchema }` from the sealed blob
/// manifest. Missing catalog → empty object (caller surfaces as error/empty).
fn read_blob_catalog_map() -> Option<Value> {
    let ids = op_blob::catalog::read_manifest_plugin_ids_shm()?;
    let mut map = simd_json::owned::Object::new();
    for id in ids {
        if let Some(schema) = op_blob::catalog::read_plugin_schema_shm(&id) {
            if let Ok(v) = simd_json::serde::to_owned_value(&schema) {
                map.insert(id, v);
            }
        }
    }
    Some(Value::Object(map))
}

pub async fn schema_catalog_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    match read_blob_catalog_map() {
        Some(value) => {
            let count = value.as_object().map(|o| o.len()).unwrap_or(0);
            Json(json!({
                "success": true,
                "catalog": value,
                "total": count,
                "source": "plugin-blobs"
            }))
        }
        None => Json(json!({
            "success": false,
            "error": "Blob catalog manifest unavailable at /dev/shm/opdbus/plugin-blobs/.manifest.json"
        })),
    }
}

pub async fn schema_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    match read_blob_catalog_map() {
        Some(value) => Json(value),
        None => Json(json!({})),
    }
}
