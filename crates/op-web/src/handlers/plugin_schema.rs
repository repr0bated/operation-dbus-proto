//! Handler to serve plugin schemas from the sealed SHM blob catalog
//! GET /api/plugin-schema/:plugin_id

use axum::{extract::Path, Json};
use op_blob::catalog;
use serde_json::Value;

pub async fn plugin_schema_handler(
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, String> {
    match catalog::read_plugin_schema_shm(&plugin_id) {
        Some(schema) => Ok(Json(
            serde_json::to_value(schema).map_err(|e| e.to_string())?,
        )),
        None => Err(format!("Plugin schema not found: {plugin_id}")),
    }
}
