//! Projection Server: pure read-through D-Bus projection of the plugin tree.
//!
//! Mounts the projected plugin tree at `org.opdbus.v1.plugins` and serves
//! each plugin's state as a `ProjectedObject` that reads 1:1 from the shm
//! projection layer (`/dev/shm/opdbus/projections/<plugin>.json`).
//!
//! The MutationEngine (the single write door) writes each plugin's full state
//! to shm on every mutation. This server mounts the tree from shm on startup
//! and exposes a `refresh()` D-Bus method on the root interface that re-reads
//! shm and syncs the tree (mount/unmount). No gRPC subscription, no polling,
//! no inotify — the tree is derived on demand.

use anyhow::{Context, Result};
use op_projection::*;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Operation-DBus Projection Server (read-through)");

    // 1. Initialize Schema Engine + discover plugin schemas (writes to shm).
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

    // 2. Initialize D-Bus server (owns org.opdbus.v1.plugins, root interface
    //    at /org/opdbus/v1/plugins is always present for introspection).
    let dbus_server = ProjectionDbusServer::new()
        .await
        .context("failed to start projection D-Bus server")?;

    // 3. Initial tree sync: mount from shm (mutations) then seed all known
    //    plugin roots from the schema catalog so the tree is never empty.
    dbus_server.sync_from_shm().await?;

    let known_plugins: Vec<String> = plugin_reader
        .projection_schemas()
        .iter()
        .map(|s| s.name.clone())
        .collect();
    dbus_server.seed_plugin_roots(&known_plugins).await?;

    let count = dbus_server.object_count().await;
    info!(objects = count, plugins = known_plugins.len(), "Initial tree sync complete");

    // 4. Initialize JSON-stream server (for WebSocket clients).
    let mut stream_server = ProjectionStreamServer::new();
    stream_server.start(8082)?;

    // 5. Run forever. The tree is derived on-demand via the `refresh()` D-Bus
    //    method on the ProjectionRoot interface. No gRPC, no polling.
    info!("Projection server ready — tree syncs on refresh() D-Bus call or restart");
    tokio::signal::ctrl_c().await?;

    Ok(())
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
