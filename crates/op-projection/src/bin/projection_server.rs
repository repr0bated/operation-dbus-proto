//! Projection Server: Main entry point for the projection system.
//!
//! This binary wires together all components of the projection system:
//! SchemaEngine, ProjectionEngine, EventMaterializer, and SourceReaders.

use anyhow::{Context, Result};
use op_projection::*;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, warn, Level};

use tracing_subscriber::FmtSubscriber;

// builtin_plugin_schemas is used from op_state_store
use op_state_store::builtin_plugin_schemas;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Operation-DBus Projection Server");

    // 1. Initialize Schema Engine
    let mut schema_engine = SchemaEngine::new();

    // Register some initial schemas (in production, load from files).
    // Built with the canonical PluginSchema builder — the plugin is the schema.
    let ro_field =
        |field_type: FieldType, description: &str, constraints: Vec<Constraint>| FieldSchema {
            field_type,
            required: true,
            description: description.to_string(),
            default: None,
            example: None,
            constraints,
            read_only: true,
            read_only_when: None,
        };

    let memory_schema = PluginSchema::builder("system.memory")
        .version("1.0.0")
        .category("system")
        .field(
            "total_kb",
            ro_field(
                FieldType::Integer,
                "Total system memory in KB",
                vec![Constraint::Min { value: 0.0 }],
            ),
        )
        .field(
            "free_kb",
            ro_field(
                FieldType::Integer,
                "Free system memory in KB",
                vec![Constraint::Min { value: 0.0 }],
            ),
        )
        .build();
    schema_engine.register_schema(memory_schema)?;

    let cpu_schema = PluginSchema::builder("system.cpu")
        .version("1.0.0")
        .category("system")
        .field(
            "cores",
            ro_field(FieldType::Integer, "Number of CPU cores", vec![]),
        )
        .field(
            "model",
            ro_field(FieldType::String, "CPU model name", vec![]),
        )
        .build();
    schema_engine.register_schema(cpu_schema)?;

    let network_schema = PluginSchema::builder("system.network")
        .version("1.0.0")
        .category("system")
        .field(
            "interfaces",
            ro_field(
                FieldType::Array(Box::new(FieldType::String)),
                "List of network interfaces",
                vec![],
            ),
        )
        .build();
    schema_engine.register_schema(network_schema)?;

    let sled_schema = PluginSchema::builder("identity.sled")
        .version("1.0.0")
        .category("identity")
        .field(
            "mutation_index",
            ro_field(FieldType::Integer, "Current mutation index", vec![]),
        )
        .field(
            "hashed_footprint",
            ro_field(FieldType::String, "Blake3 hashed footprint", vec![]),
        )
        .field(
            "wireguard_pubkey",
            ro_field(FieldType::String, "WireGuard public key", vec![]),
        )
        .build();
    schema_engine.register_schema(sled_schema)?;

    let process_schema = PluginSchema::builder("system.process")
        .version("1.0.0")
        .category("system")
        .field("name", ro_field(FieldType::String, "Process name", vec![]))
        .build();
    schema_engine.register_schema(process_schema)?;

    let filesystems_schema = PluginSchema::builder("system.filesystems")
        .version("1.0.0")
        .category("system")
        .field(
            "types",
            ro_field(
                FieldType::Array(Box::new(FieldType::String)),
                "Filesystem types listed by /proc/filesystems",
                vec![],
            ),
        )
        .build();
    schema_engine.register_schema(filesystems_schema)?;

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

    // Register all builtin schemas from op-state-store so the shm catalog
    // is the single source of truth for UI, blockchain, everything.
    // These include web_ui, mcp, wireguard, incus, openflow, etc.
    for schema in builtin_plugin_schemas() {
        register_schema_if_missing(&mut schema_engine, schema)?;
    }

    info!(
        "Registered initial schemas ({} total)",
        schema_engine.list_schemas().len()
    );

    // 1b. Write plugin capability schemas to SHM.
    // The producer is the sole writer of per-plugin schema files, the combined
    // monolith, the catalog hash manifest, and present-state snapshots.
    // Plugins returning None from schema() are skipped (VAL-PROD-001).
    let shm_writer = ShmWriter::default();
    if let Err(e) = shm_writer.ensure_dirs() {
        warn!(error = %e, "Failed to create SHM directory hierarchy; continuing");
    }
    let plugin_schemas = plugin_reader.plugin_schemas_with_ids();
    if let Err(e) = shm_writer.write_all_schemas(&plugin_schemas) {
        warn!(error = %e, "Failed to write plugin schemas to SHM; continuing");
    } else {
        info!(
            plugin_count = plugin_schemas.iter().filter(|(_, s)| s.is_some()).count(),
            "Wrote plugin capability schemas to SHM"
        );
    }

    // 2. Initialize Projection Store and Engine
    let store = ProjectionStore::new();
    let validator = SchemaValidator::new(schema_engine.clone());
    let engine = Arc::new(Mutex::new(ProjectionSystemEngine::new(
        store.clone(),
        validator,
    )));

    // 3. Initialize Source Readers
    let procfs_reader = SystemProcfsReader::new();
    let sled_reader = IdentitySledReader::new();
    let _dbus_reader = SystemDbusReader::new();
    let _grpc_reader = SystemGrpcReader::new();

    info!("Initialized source readers");

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

        if procfs_reader.is_available() {
            initial_entities.extend(procfs_reader.read_all()?);
        }

        if sled_reader.is_available() {
            if let Ok(entities) = sled_reader.read_all() {
                initial_entities.extend(entities);
            }
        }

        if plugin_reader.is_available() {
            match plugin_reader.read_all_async().await {
                Ok(entities) => initial_entities.extend(entities),
                Err(error) => warn!(error = %error, "Failed to read plugin projection entities"),
            }

            // Write present-state snapshots to SHM for every plugin that
            // successfully reports its state (VAL-PROD-004).
            let present_states = plugin_reader.plugin_present_states().await;
            if let Err(e) = shm_writer.write_all_present_states(&present_states) {
                warn!(error = %e, "Failed to write present-states to SHM");
            } else {
                info!(
                    count = present_states.len(),
                    "Wrote present-state snapshots to SHM"
                );
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

        // Periodic refresh from procfs
        if let Ok(entities) = procfs_reader.read_all() {
            refresh_entities.extend(entities);
        }

        // Periodic refresh from Sled
        if let Ok(entities) = sled_reader.read_all() {
            refresh_entities.extend(entities);
        }

        if plugin_reader.is_available() {
            match plugin_reader.read_all_async().await {
                Ok(entities) => refresh_entities.extend(entities),
                Err(error) => warn!(
                    error = %error,
                    "Failed to refresh plugin projection entities"
                ),
            }

            // Refresh present-state snapshots in SHM and update manifest hash
            // so the bridge detects the change on its next inbound connection.
            let present_states = plugin_reader.plugin_present_states().await;
            if let Err(e) = shm_writer.write_all_present_states(&present_states) {
                warn!(error = %e, "Failed to refresh present-states in SHM");
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
