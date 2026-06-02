//! D-Bus Projection Client — reads live schema-validated projections from the tree.
//!
//! All projection data is published by org.opdbus.v1 under the unified
//! `/opdbus/v1/plugins/<plugin>` tree. The interface is
//! `org.opdbus.ProjectedObjectV1` with a `JsonData` property.
//!
//! This client reads ONLY verified plugin projections — explicit paths for each
//! plugin registered through the schema registry. No dynamic tree walking.

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

/// Verified plugin projections to read. Each path is a plugin object derived
/// from a registered plugin schema (procfs, zeroclaw, etc.).
const PROJECTION_PATHS: &[(&str, &str)] = &[
    // procfs plugin children — defined in the procfs plugin schema
    ("system.memory", "/opdbus/v1/plugins/procfs/memory"),
    ("system.cpu", "/opdbus/v1/plugins/procfs/cpuinfo"),
    ("system.load", "/opdbus/v1/plugins/procfs/loadavg"),
    ("system.network", "/opdbus/v1/plugins/procfs/net_dev"),
    ("system.processes", "/opdbus/v1/plugins/procfs/stat"),
    ("system.filesystems", "/opdbus/v1/plugins/procfs/mounts"),
    // zeroclaw plugin — single object with full state
    ("zeroclaw", "/opdbus/v1/plugins/zeroclaw"),
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
    for (entity_type, path) in PROJECTION_PATHS {
        if let Err(e) = refresh_projection(&conn, cache.clone(), entity_type, path).await {
            debug!(path = %path, error = %e, "Projection not yet available");
        }
    }

    // Periodic refresh every 3 seconds
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
    loop {
        interval.tick().await;
        for (entity_type, path) in PROJECTION_PATHS {
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
