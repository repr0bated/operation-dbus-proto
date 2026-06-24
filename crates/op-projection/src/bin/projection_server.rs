//! Projection Server: mutation-driven D-Bus projection of the plugin tree.
//!
//! Mounts the projected plugin tree at `org.opdbus.v1.plugins` and serves
//! each plugin's state as a `ProjectedObject` that reads 1:1 from the shm
//! projection layer (`/dev/shm/opdbus/projections/<plugin>.json`).
//!
//! The MutationEngine (the single write door) writes each plugin's full state
//! to shm on every mutation, THEN broadcasts a `StateChange`. This server
//! subscribes to that stream and syncs the D-Bus object tree (mount new
//! nested objects, unmount removed ones, emit `updated` on changed ones).
//! No poll loop, no held cache, no StateManager — the projection is a pure
//! read-through of the mutation fold.

use anyhow::{Context, Result};
use op_grpc_bridge::{GrpcClientPool, RemoteOperationClient};
use op_projection::*;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Operation-DBus Projection Server (mutation-driven)");

    // 1. Initialize Schema Engine + discover plugin schemas (writes to shm).
    //    The SchemaEngine is the schema/catalog authority; it publishes
    //    `/dev/shm/opdbus/schemas/` so all consumers share one schema source.
    let mut schema_engine = SchemaEngine::new();

    let plugin_reader = match SystemPluginReader::new().await {
        Ok(reader) => reader,
        Err(error) => {
            warn!(
                error = %error,
                "Failed to initialize plugin reader; continuing with empty schema catalog"
            );
            SystemPluginReader::empty()
        }
    };

    for schema in plugin_reader.projection_schemas() {
        register_schema_if_missing(&mut schema_engine, schema)?;
    }

    info!(
        schemas = schema_engine.list_schemas().len(),
        "Schema catalog published to shm"
    );

    // 2. Initialize D-Bus server (owns org.opdbus.v1.plugins on the bus).
    let mut dbus_server = ProjectionDbusServer::new()
        .await
        .context("failed to start projection D-Bus server")?;

    // 3. Initial mount: read all shm projection files and mount D-Bus objects.
    //    The MutationEngine writes these files on every mutation; at startup
    //    we mount whatever exists. New mutations sync the tree going forward.
    let plugins = op_core::projection_shm::list_projected_plugins();
    info!(plugin_count = plugins.len(), "Mounting projections from shm");

    for plugin_id in &plugins {
        if let Some(paths) = read_and_derive_paths(plugin_id) {
            if let Err(e) = dbus_server.sync_plugin(plugin_id, &paths).await {
                warn!(plugin_id, error = %e, "Failed to sync plugin projection");
            }
        }
    }
    info!(objects = dbus_server.object_count(), "Initial mount complete");

    // 4. Initialize JSON-stream server (for WebSocket clients).
    let mut stream_server = ProjectionStreamServer::new();
    stream_server.start(8082)?;

    // 5. Subscribe to the StateChange stream from the MutationEngine.
    //    On each StateChange, the engine has ALREADY written the plugin's
    //    full state to shm. We read it, derive all nested object paths, and
    //    sync the D-Bus tree (mount/unmount/update).
    let grpc_addr = std::env::var("OP_DBUS_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string());
    let pool = Arc::new(GrpcClientPool::new());
    let grpc_client = RemoteOperationClient::new(pool, &grpc_addr, "op-projection");

    info!(grpc_addr, "Subscribing to StateChange stream");

    match grpc_client.subscribe(vec![], vec![], vec![]).await {
        Ok(stream) => {
            tokio::pin!(stream);
            while let Some(result) = stream.next().await {
                match result {
                    Ok(update) => {
                        let plugin_id = &update.plugin_id;
                        if let Some(paths) = read_and_derive_paths(plugin_id) {
                            if let Err(e) = dbus_server.sync_plugin(plugin_id, &paths).await {
                                warn!(plugin_id, error = %e, "Failed to sync plugin projection");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "StateChange stream error");
                    }
                }
            }
            warn!("StateChange stream ended");
        }
        Err(e) => {
            warn!(
                error = %e,
                "Failed to subscribe to StateChange stream; serving in degraded mode (shm reads only, no push updates). s6 will restart."
            );
            tokio::signal::ctrl_c().await?;
        }
    }

    Ok(())
}

/// Read a plugin's projection from shm and derive all D-Bus object paths.
fn read_and_derive_paths(plugin_id: &str) -> Option<Vec<Vec<String>>> {
    let bytes = op_core::projection_shm::read_projection_bytes(plugin_id)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    Some(derive_paths_from_state(&value))
}

/// Derive all D-Bus object paths from a plugin's state JSON.
///
/// The root (empty path) is always included. Nested objects and arrays
/// produce path segments mirroring the JSON structure, using "id"/"name"/
/// "label" etc. for array elements when available (matching the derivation
/// in `plugin_reader.rs::collect_nested_entities`).
fn derive_paths_from_state(value: &serde_json::Value) -> Vec<Vec<String>> {
    let mut paths = vec![Vec::new()]; // root (plugin root)
    derive_paths_recursive(value, &mut Vec::new(), &mut paths);
    paths
}

fn derive_paths_recursive(
    value: &serde_json::Value,
    current: &mut Vec<String>,
    paths: &mut Vec<Vec<String>>,
) {
    match value {
        serde_json::Value::Object(map) => {
            if !current.is_empty() {
                paths.push(current.clone());
            }
            for (key, child) in map {
                if child.is_object() || child.is_array() {
                    current.push(key.clone());
                    derive_paths_recursive(child, current, paths);
                    current.pop();
                }
            }
        }
        serde_json::Value::Array(arr) => {
            if !current.is_empty() {
                paths.push(current.clone());
            }
            for (idx, child) in arr.iter().enumerate() {
                if child.is_object() || child.is_array() {
                    let segment = ["id", "name", "label", "key", "path", "domain", "host"]
                        .iter()
                        .find_map(|&field| child.get(field).and_then(|v| v.as_str()))
                        .map(|s| s.replace(['/', ' ', ':'], "_"))
                        .unwrap_or_else(|| idx.to_string());
                    current.push(segment);
                    derive_paths_recursive(child, current, paths);
                    current.pop();
                }
            }
        }
        _ => {}
    }
}

fn register_schema_if_missing(
    schema_engine: &mut SchemaEngine,
    schema: PluginSchema,
) -> Result<()> {
    if schema_engine.has_valid_schema(&schema.name) {
        return Ok(());
    }

    let schema_name = schema.name.clone();
    schema_engine
        .register_schema(schema)
        .with_context(|| format!("failed to register projection schema '{}'", schema_name))?;
    Ok(())
}
