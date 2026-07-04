use crate::state::AppState;
use axum::{extract::Extension, response::Json};
use simd_json::json;
use simd_json::prelude::ValueAsContainer;
use simd_json::OwnedValue as Value;
use std::sync::Arc;

const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

pub async fn schema_catalog_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    match read_schema_catalog_bytes() {
        Ok(mut bytes) => match simd_json::to_owned_value(&mut bytes) {
            Ok(value) => {
                let count = if let Some(obj) = value.as_object() {
                    obj.len()
                } else {
                    0
                };
                Json(json!({
                    "success": true,
                    "catalog": value,
                    "total": count
                }))
            }
            Err(e) => Json(json!({
                "success": false,
                "error": format!("Failed to parse schema: {}", e)
            })),
        },
        Err(e) => Json(json!({
            "success": false,
            "error": format!("Schema catalog not available: {}", e)
        })),
    }
}

pub async fn schema_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    match read_schema_catalog_bytes() {
        Ok(mut bytes) => match simd_json::to_owned_value(&mut bytes) {
            Ok(value) => Json(value),
            Err(_) => Json(json!({})),
        },
        Err(_) => Json(json!({})),
    }
}

fn read_schema_catalog_bytes() -> std::io::Result<Vec<u8>> {
    op_identity::read_schema_blob().or_else(|_| std::fs::read(SHM_SCHEMA_PATH))
}
