//! D-Bus Projection Client — reads live plugin projections from the /org/opdbus/v1/plugins tree.
//!
//! Plugin paths are derived at runtime from the sealed blob catalog manifest
//! (`/dev/shm/opdbus/plugin-blobs/.manifest.json`). Every active blob maps to
//! /org/opdbus/v1/plugins/<plugin_id>. No hardcoded paths — a blob in the
//! catalog IS the plugin; if it's not sealed, it doesn't exist.

use anyhow::Result;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::{
    proxy::{Builder as ProxyBuilder, CacheProperties},
    Connection,
};

pub type ProjectionCache = Arc<RwLock<HashMap<String, Value>>>;

const DBUS_SERVICE: &str = op_core::config::OPDBUS_BUS_NAME;
const PLUGIN_ROOT: &str = op_core::config::PLUGIN_BASE_PATH;
const PROJECTED_OBJECT_IFACE: &str = "org.opdbus.ProjectedObjectV1";

fn plugin_ids_from_schema() -> Vec<String> {
    // The blob catalog manifest is the "which plugins exist" read: a blob in
    // the catalog IS the plugin.
    match op_blob::catalog::read_manifest_plugin_ids_shm() {
        Some(ids) => ids.into_iter().filter(|k| k != "schema_renderer").collect(),
        None => {
            warn!("Blob catalog manifest unavailable; no projections will be cached");
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
    let proxy = ProxyBuilder::<zbus::Proxy<'_>>::new(conn)
        .destination(DBUS_SERVICE)?
        .path(path)?
        .interface(PROJECTED_OBJECT_IFACE)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
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
