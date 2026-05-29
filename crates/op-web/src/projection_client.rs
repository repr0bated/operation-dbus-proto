//! D-Bus Projection Client — reads live schema-validated projections from the tree.
//!
//! Live procfs data is published by org.opdbus.v1 under /org/opdbus/v1/plugins/procfs/*.
//! The interface is org.opdbus.ProjectedObjectV1 with a JsonData property.

use anyhow::Result;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::Connection;

/// Cached projection data keyed by entity type.
pub type ProjectionCache = Arc<RwLock<HashMap<String, Value>>>;

const DBUS_SERVICE: &str = "org.opdbus.v1";
const PROJECTED_OBJECT_IFACE: &str = "org.opdbus.ProjectedObjectV1";
const PLUGINS_IFACE: &str = "org.opdbus.PluginsV1";

/// Well-known procfs projection paths under /org/opdbus/v1/plugins/procfs.
const PROCFS_PATHS: &[(&str, &str)] = &[
    ("system.memory", "/org/opdbus/v1/plugins/procfs/memory"),
    ("system.cpu", "/org/opdbus/v1/plugins/procfs/cpuinfo"),
    ("system.load", "/org/opdbus/v1/plugins/procfs/loadavg"),
    ("system.network", "/org/opdbus/v1/plugins/procfs/net_dev"),
    ("system.processes", "/org/opdbus/v1/plugins/procfs/stat"),
    ("system.filesystems", "/org/opdbus/v1/plugins/procfs/mounts"),
];

/// Start a background task that polls D-Bus projections and caches them.
pub async fn start_projection_monitor(cache: ProjectionCache) {
    let conn = match Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to connect to system D-Bus; projection monitor disabled");
            return;
        }
    };

    info!("Connected to system D-Bus; starting projection monitor for org.opdbus.v1");

    // Initial scan
    for (entity_type, path) in PROCFS_PATHS {
        if let Err(e) = refresh_projection(&conn, cache.clone(), entity_type, path).await {
            debug!(path = %path, error = %e, "Projection not yet available");
        }
    }

    // Periodic refresh every 3 seconds
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
    loop {
        interval.tick().await;
        for (entity_type, path) in PROCFS_PATHS {
            if let Err(e) = refresh_projection(&conn, cache.clone(), entity_type, path).await {
                debug!(path = %path, error = %e, "Projection refresh failed");
            }
        }
    }
}

async fn refresh_projection(
    conn: &Connection,
    cache: ProjectionCache,
    entity_type: &str,
    path: &str,
) -> Result<()> {
    // Read JsonData property from org.opdbus.ProjectedObjectV1 interface.
    let proxy = zbus::Proxy::new(conn, DBUS_SERVICE, path, PROJECTED_OBJECT_IFACE).await?;

    let data_json: String = proxy.get_property("JsonData").await?;

    let mut bytes = data_json.into_bytes();
    let value = simd_json::to_owned_value(&mut bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse projection JSON: {}", e))?;

    let mut cache_lock = cache.write().await;
    cache_lock.insert(entity_type.to_string(), value);
    drop(cache_lock);

    debug!(entity_type = %entity_type, path = %path, "Projection cached from D-Bus");
    Ok(())
}

/// Get a cached projection by entity type.
pub async fn get_projection(cache: &ProjectionCache, entity_type: &str) -> Option<Value> {
    let cache_lock = cache.read().await;
    cache_lock.get(entity_type).cloned()
}

/// Get all cached projections.
pub async fn get_all_projections(cache: &ProjectionCache) -> HashMap<String, Value> {
    let cache_lock = cache.read().await;
    cache_lock.clone()
}
