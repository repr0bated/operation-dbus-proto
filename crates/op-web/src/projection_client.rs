//! D-Bus Projection Client — reads live plugin projections from the /opdbus/v1/plugins tree.
//!
//! Plugin paths are derived at runtime from /dev/shm/live-schema.json.
//! Every key in the schema catalog maps to /opdbus/v1/plugins/<plugin_id>.
//! No hardcoded paths — if it's not in the schema, it doesn't exist.

use anyhow::Result;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::Connection;

pub type ProjectionCache = Arc<RwLock<HashMap<String, Value>>>;

const DBUS_SERVICE: &str = "org.opdbus.v1";
const PLUGIN_ROOT: &str = "/opdbus/v1/plugins";
const PROJECTED_OBJECT_IFACE: &str = "org.opdbus.ProjectedObjectV1";
const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

fn plugin_ids_from_schema() -> Vec<String> {
    let bytes = match std::fs::read(SHM_SCHEMA_PATH) {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "Could not read live schema; no projections will be cached");
            return vec![];
        }
    };
    let mut bytes = bytes;
    match simd_json::to_owned_value(&mut bytes) {
        Ok(Value::Object(map)) => map
            .keys()
            .filter(|k| *k != "schema_renderer")
            .map(|k| k.to_string())
            .collect(),
        _ => {
            warn!("Live schema is not a JSON object; no projections will be cached");
            vec![]
        }
    }
}

pub async fn start_projection_monitor(cache: ProjectionCache) {
    let conn = match Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to connect to system D-Bus; projection monitor disabled");
            return;
        }
    };

    info!(
        "Connected to system D-Bus; starting projection monitor for {}",
        PLUGIN_ROOT
    );

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        interval.tick().await;
        let plugin_ids = plugin_ids_from_schema();
        for id in &plugin_ids {
            let path = format!("{}/{}", PLUGIN_ROOT, id);
            if let Err(e) = refresh_projection(&conn, cache.clone(), id, &path).await {
                debug!(path = %path, error = %e, "Projection not available");
            }
        }
    }
}

async fn refresh_projection(
    conn: &Connection,
    cache: ProjectionCache,
    plugin_id: &str,
    path: &str,
) -> Result<()> {
    let proxy = zbus::Proxy::new(conn, DBUS_SERVICE, path, PROJECTED_OBJECT_IFACE).await?;
    let data_json: String = proxy.get_property("JsonData").await?;
    let mut bytes = data_json.into_bytes();
    let value = simd_json::to_owned_value(&mut bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse projection JSON: {}", e))?;
    cache.write().await.insert(plugin_id.to_string(), value);
    debug!(plugin_id = %plugin_id, path = %path, "Projection cached");
    Ok(())
}

pub async fn get_projection(cache: &ProjectionCache, entity_type: &str) -> Option<Value> {
    cache.read().await.get(entity_type).cloned()
}

pub async fn get_all_projections(cache: &ProjectionCache) -> HashMap<String, Value> {
    cache.read().await.clone()
}
