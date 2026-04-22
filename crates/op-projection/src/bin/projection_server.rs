//! Projection Server: Main entry point for the projection system.
//!
//! This binary wires together all components of the projection system:
//! SchemaEngine, ProjectionEngine, EventMaterializer, and SourceReaders.

use anyhow::Result;
use op_projection::*;
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{info, Level};
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
        fields: vec![
            FieldSchema {
                name: "interfaces".to_string(),
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: Some("List of network interfaces".to_string()),
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
        fields: vec![
            FieldSchema {
                name: "name".to_string(),
                field_type: FieldType::String,
                required: true,
                description: Some("Process name".to_string()),
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
    schema_engine.register_schema(process_schema)?;

    info!("Registered initial schemas");

    // 2. Initialize Projection Store and Engine
    let store = ProjectionStore::new();
    let validator = SchemaValidator::new(schema_engine.clone());
    let engine = Arc::new(Mutex::new(ProjectionSystemEngine::new(store.clone(), validator)));

    // 3. Initialize Source Readers
    let procfs_reader = SystemProcfsReader::new();
    let sled_reader = IdentitySledReader::new();
    let _dbus_reader = SystemDbusReader::new();
    let _grpc_reader = SystemGrpcReader::new();
    let _plugin_reader = SystemPluginReader::new();
    
    info!("Initialized source readers");

    // 4. Initialize JSON-stream Server
    let mut stream_server = ProjectionStreamServer::new();
    stream_server.start(8082)?;

    // 5. Initial Scan and Projection
    {
        let mut engine_lock = engine.lock();
        
        info!("Performing initial scan...");
        
        // Scan procfs
        if procfs_reader.is_available() {
            let entities = procfs_reader.read_all()?;
            for entity in entities {
                let projection = engine_lock.create_projection(entity)?;
                
                // Broadcast initial results
                stream_server.broadcast(&ProjectionUpdate {
                    update_type: UpdateType::Created,
                    projection,
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        // Scan Sled
        if sled_reader.is_available() {
            if let Ok(entities) = sled_reader.read_all() {
                for entity in entities {
                    let projection = engine_lock.create_projection(entity)?;
                    stream_server.broadcast(&ProjectionUpdate {
                        update_type: UpdateType::Created,
                        projection,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
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
        
        let mut engine_lock = engine.lock();

        // Periodic refresh from procfs
        if let Ok(entities) = procfs_reader.read_all() {
            for entity in entities {
                let update = engine_lock.create_projection(entity)?;
                
                // Broadcast update to UI
                stream_server.broadcast(&ProjectionUpdate {
                    update_type: UpdateType::Updated,
                    projection: update,
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        // Periodic refresh from Sled
        if let Ok(entities) = sled_reader.read_all() {
            for entity in entities {
                let update = engine_lock.create_projection(entity)?;
                
                // Broadcast update to UI
                stream_server.broadcast(&ProjectionUpdate {
                    update_type: UpdateType::Updated,
                    projection: update,
                    timestamp: chrono::Utc::now(),
                });
            }
        }
        
        info!("Periodic refresh complete");
    }
}
