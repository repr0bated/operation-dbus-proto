//! Projection Server: Main entry point for the projection system.
//!
//! This binary wires together all components of the projection system:
//! SchemaEngine, ProjectionEngine, EventMaterializer, and SourceReaders.

use anyhow::{Context, Result};
use op_projection::*;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Operation-DBus Projection Server");

    // 1. Initialize Schema Engine
    let mut schema_engine = SchemaEngine::new();

    let plugin_reader = match SystemPluginReader::new().await {
        Ok(reader) => reader,
        Err(error) => {
            warn!(
                error = %error,
                "Failed to initialize plugin projection reader; continuing with non-plugin sources"
            );
            SystemPluginReader::empty()
        }
    };

    for schema in plugin_reader.projection_schemas() {
        register_schema_if_missing(&mut schema_engine, schema)?;
    }

    info!(
        "Registered initial schemas ({} total)",
        schema_engine.list_schemas().len()
    );

    // 2. Initialize Projection Store and Engine
    let store = ProjectionStore::new();
    let validator = SchemaValidator::new(schema_engine.clone());
    let engine = Arc::new(Mutex::new(ProjectionSystemEngine::new(
        store.clone(),
        validator,
    )));

    // 3. Plugin projections are the only D-Bus projection source. System,
    // procfs, identity, and network state must enter through state plugins.
    info!("Initialized plugin projection source");

    // 4. Initialize JSON-stream Server
    let mut stream_server = ProjectionStreamServer::new();
    stream_server.start(8082)?;
    let mut dbus_server = ProjectionDbusServer::new()
        .await
        .context("failed to start projection D-Bus server")?;

    // 5. Initial Scan and Projection
    {
        let mut initial_entities = Vec::new();

        info!("Performing initial scan...");

        if plugin_reader.is_available() {
            match plugin_reader.read_all_async().await {
                Ok(entities) => initial_entities.extend(entities),
                Err(error) => warn!(error = %error, "Failed to read plugin projection entities"),
            }
        }

        for entity in initial_entities {
            let projection = {
                let mut engine_lock = engine.lock();
                engine_lock.create_projection(entity)?
            };
            dbus_server.upsert(&projection).await?;
            stream_server.broadcast(&ProjectionUpdate {
                update_type: UpdateType::Created,
                projection,
                timestamp: chrono::Utc::now(),
            });
        }

        info!("Initial scan complete");
    }

    // 6. Initialize Access Controller
    let mut access_controller = ProjectionAccessController::new();
    access_controller.add_policy(AccessPolicy {
        id: "allow-all-read".to_string(),
        resource_pattern: ".*".to_string(),
        required_permissions: vec![],
        action: "read".to_string(),
        redact_sensitive: false,
    });

    info!("Projection Server is ready");

    // 7. Keep-alive loop (in production, this would be the event loop)
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        let mut refresh_entities = Vec::new();

        if plugin_reader.is_available() {
            match plugin_reader.read_all_async().await {
                Ok(entities) => refresh_entities.extend(entities),
                Err(error) => warn!(
                    error = %error,
                    "Failed to refresh plugin projection entities"
                ),
            }
        }

        for entity in refresh_entities {
            let update = {
                let mut engine_lock = engine.lock();
                engine_lock.create_projection(entity)?
            };
            dbus_server.upsert(&update).await?;

            stream_server.broadcast(&ProjectionUpdate {
                update_type: UpdateType::Updated,
                projection: update,
                timestamp: chrono::Utc::now(),
            });
        }

        info!("Periodic refresh complete");
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
