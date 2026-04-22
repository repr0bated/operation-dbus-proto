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

    // Register some initial schemas (in production, load from files)
    let memory_schema = PluginSchema {
        name: "system.memory".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![
            FieldSchema {
                name: "total_kb".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Total system memory in KB".to_string()),
                constraints: vec![Constraint::MinValue(0)],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "free_kb".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Free system memory in KB".to_string()),
                constraints: vec![Constraint::MinValue(0)],
                example: None,
                read_only: true,
            },
        ],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };

    schema_engine.register_schema(memory_schema)?;

    let cpu_schema = PluginSchema {
        name: "system.cpu".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![
            FieldSchema {
                name: "cores".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Number of CPU cores".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "model".to_string(),
                field_type: FieldType::String,
                required: true,
                description: Some("CPU model name".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
        ],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(cpu_schema)?;

    let network_schema = PluginSchema {
        name: "system.network".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![FieldSchema {
            name: "interfaces".to_string(),
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: true,
            description: Some("List of network interfaces".to_string()),
            constraints: vec![],
            example: None,
            read_only: true,
        }],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(network_schema)?;

    let sled_schema = PluginSchema {
        name: "identity.sled".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![
            FieldSchema {
                name: "mutation_index".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Current mutation index".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "hashed_footprint".to_string(),
                field_type: FieldType::String,
                required: true,
                description: Some("Blake3 hashed footprint".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "wireguard_pubkey".to_string(),
                field_type: FieldType::String,
                required: true,
                description: Some("WireGuard public key".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
        ],
        category: Some("identity".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(sled_schema)?;

    let process_schema = PluginSchema {
        name: "system.process".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![FieldSchema {
            name: "name".to_string(),
            field_type: FieldType::String,
            required: true,
            description: Some("Process name".to_string()),
            constraints: vec![],
            example: None,
            read_only: true,
        }],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(process_schema)?;

    let filesystems_schema = PluginSchema {
        name: "system.filesystems".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![FieldSchema {
            name: "types".to_string(),
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: true,
            description: Some("Filesystem types listed by /proc/filesystems".to_string()),
            constraints: vec![],
            example: None,
            read_only: true,
        }],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
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

    info!("Registered initial schemas");

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
        }

        let mut engine_lock = engine.lock();
        for entity in initial_entities {
            let projection = engine_lock.create_projection(entity)?;
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
        }

        let mut engine_lock = engine.lock();
        for entity in refresh_entities {
            let update = engine_lock.create_projection(entity)?;

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
