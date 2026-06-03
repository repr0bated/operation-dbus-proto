use axum::{extract::Extension, response::Json};
use serde::Serialize;
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use crate::state::AppState;

const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

pub async fn schema_catalog_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<Value> {
    match std::fs::read_to_string(SHM_SCHEMA_PATH) {
        Ok(content) => {
            let mut bytes = content.into_bytes();
            match simd_json::to_owned_value(&mut bytes) {
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
            }
        }
        Err(e) => Json(json!({
            "success": false,
            "error": format!("Schema catalog not available: {}", e)
        })),
    }
}

pub async fn schema_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<Value> {
    match std::fs::read_to_string(SHM_SCHEMA_PATH) {
        Ok(content) => {
            let mut bytes = content.into_bytes();
            match simd_json::to_owned_value(&mut bytes) {
                Ok(value) => Json(value),
                Err(_) => Json(json!({})),
            }
        }
        Err(_) => Json(json!({})),
    }
}
