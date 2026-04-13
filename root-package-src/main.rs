//! OP-DBUS: Native, Deterministic Control Plane for Linux Systems
//!
//! Production entry point with all components wired together.

use simd_json::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(feature = "grpc")]
use std::sync::OnceLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Crate imports (Authoritative logic)
use op_blockchain::StreamingBlockchain;
use op_core::config::load_environment;
use op_core::types::BusType;
use op_introspection::projection::DbusProjection;
use op_plugins::plugin::PluginMetadata as PluginCore;
use op_state_store::StateStore;
use op_tools::{register_builtin_tools, ToolRegistry};
use op_workflows::orchestrator::{Orchestrator, OrchestratorConfig};

// Internal modules (Glue logic)
use op_dbus::{
    cache::BtrfsCache,
    chatbot::{Chatbot, ChatbotConfig},
    constants,
    dependency::DependencyManager,
    disaster_recovery::DisasterRecovery,
    error::Result,
    inspector_gadget::{InspectorConfig, InspectorGadget},
    json_rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse},
    mcp::{McpCompactDispatcher, McpRequest},
    mcp_live::McpLiveDispatcher,
    numa_cache::NumaOptimizer,
    policy::PolicyEngine,
    vectorization::FootprintGenerator,
};
use op_dbus_model;
use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::ovsdb::OvsdbClient;

#[cfg(feature = "grpc")]
use op_grpc_bridge::proto::PluginInfo;
#[cfg(feature = "grpc")]
use op_grpc_bridge::proto::{
    event_chain_service_server::EventChainServiceServer,
    plugin_service_server::PluginServiceServer, state_sync_server::StateSyncServer,
};
#[cfg(feature = "grpc")]
use op_grpc_bridge::{ChangeType, OperationGrpcServer, PluginSchemaProvider, SchemaEngine};
#[cfg(feature = "grpc")]
use op_mcp::grpc::proto::mcp_service_server::McpServiceServer;
#[cfg(feature = "grpc")]
use op_mcp::grpc::{GrpcInfrastructure, GrpcServerMode, McpGrpcService};
#[cfg(feature = "grpc")]
use op_state_store::ChainConfig;
use op_web::{routes, AppState};
#[cfg(feature = "grpc")]
static GLOBAL_SCHEMA_ENGINE: OnceLock<Arc<SchemaEngine>> = OnceLock::new();

#[cfg(feature = "dev-antigravity")]
use op_dbus::antigravity::{
    transport::{TransportConfig, TransportType, TunnelTransport},
    AntigravityConfig, AntigravityTunnel,
};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Clone)]
struct Config {
    database_url: String,
    cache_dir: String,
    enable_dbus: bool,
    dbus_connection: BusType,
    enable_web: bool,
    web_host: String,
    web_port: u16,
    listen: String,
    #[cfg(feature = "dev-antigravity")]
    enable_antigravity: bool,
    #[cfg(feature = "dev-antigravity")]
    antigravity_listen: String,
    #[cfg(feature = "dev-antigravity")]
    antigravity_transport: String,
}

fn start_privacy_router_bootstrap(state_manager: Arc<op_state::manager::StateManager>) {
    tokio::spawn(async move {
        let desired = match state_manager.get_plugin("privacy_router").await {
            Some(plugin) => match plugin.query_current_state().await {
                Ok(current) => current.get("config").cloned().unwrap_or_else(|| {
                    simd_json::json!(
                        op_plugins::state_plugins::privacy_router::PrivacyRouterConfig::default()
                    )
                }),
                Err(error) => {
                    tracing::warn!(
                        "privacy_router bootstrap could not query current config, using defaults: {}",
                        error
                    );
                    simd_json::json!(
                        op_plugins::state_plugins::privacy_router::PrivacyRouterConfig::default()
                    )
                }
            },
            None => {
                tracing::warn!("privacy_router bootstrap skipped because the plugin is not loaded");
                return;
            }
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match state_manager
                .apply_plugin_state("privacy_router", desired.clone())
                .await
            {
                Ok(result) if result.success => {
                    tracing::info!(
                        attempt,
                        changes = ?result.changes_applied,
                        "privacy_router bootstrap applied successfully"
                    );
                    break;
                }
                Ok(result) => {
                    tracing::warn!(
                        attempt,
                        errors = ?result.errors,
                        "privacy_router bootstrap reported errors"
                    );
                }
                Err(error) => {
                    tracing::warn!(attempt, "privacy_router bootstrap failed: {}", error);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });
}

fn privacy_router_bootstrap_enabled() -> bool {
    std::env::var("OP_DBUS_ENABLE_PRIVACY_ROUTER_BOOTSTRAP")
        .map(|v| {
            let value = v.trim().to_ascii_lowercase();
            !(value == "0" || value == "false" || value == "no" || value == "off")
        })
        .unwrap_or(true)
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            let value = v.trim().to_ascii_lowercase();
            value == "1" || value == "true" || value == "yes" || value == "on"
        })
        .unwrap_or(false)
}

fn dbus_publication_buses(primary: BusType) -> Vec<BusType> {
    let mut buses = vec![primary];

    if primary != BusType::System && env_flag("OP_DBUS_PUBLISH_SYSTEM_BUS") {
        buses.push(BusType::System);
    }

    if primary != BusType::Session && env_flag("OP_DBUS_PUBLISH_SESSION_BUS") {
        buses.push(BusType::Session);
    }

    buses
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: std::env::var("OP_DBUS_DATABASE_URL")
                .unwrap_or_else(|_| format!("sqlite://{}", constants::STATE_DB_PATH)),
            cache_dir: std::env::var("OP_DBUS_CACHE_DIR")
                .unwrap_or_else(|_| constants::BTRFS_CACHE_SUBVOL_PREFIX.to_string()),
            enable_dbus: std::env::var("OP_DBUS_ENABLE_DBUS")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(true),
            dbus_connection: if env_flag("OP_DBUS_SESSION_BUS") {
                BusType::Session
            } else {
                BusType::System
            },
            enable_web: std::env::var("OP_DBUS_ENABLE_WEB")
                .map(|v| v != "0" && v.to_lowercase() != "false")
                .unwrap_or(true),
            web_host: std::env::var("OP_DBUS_WEB_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            web_port: std::env::var("OP_DBUS_WEB_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(constants::WEB_DEFAULT_PORT),
            listen: std::env::var("OP_DBUS_LISTEN").unwrap_or_else(|_| "none".to_string()),
            #[cfg(feature = "dev-antigravity")]
            enable_antigravity: std::env::var("OP_DBUS_ENABLE_ANTIGRAVITY")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
            #[cfg(feature = "dev-antigravity")]
            antigravity_listen: std::env::var("OP_DBUS_ANTIGRAVITY_LISTEN")
                .unwrap_or_else(|_| format!("127.0.0.1:{}", constants::ANTIGRAVITY_DEFAULT_PORT)),
            #[cfg(feature = "dev-antigravity")]
            antigravity_transport: std::env::var("OP_DBUS_ANTIGRAVITY_TRANSPORT")
                .unwrap_or_else(|_| "tcp".to_string()),
        }
    }
}

#[cfg(feature = "grpc")]
struct OpdbusPluginProvider {
    ovsdb: Arc<op_network::ovsdb::OvsdbClient>,
    nonnet: Arc<op_jsonrpc::nonnet::NonNetDb>,
}

#[cfg(feature = "grpc")]
#[tonic::async_trait]
impl PluginSchemaProvider for OpdbusPluginProvider {
    async fn list_plugins(&self) -> Vec<PluginInfo> {
        let mut plugins = Vec::new();
        
        // 1. Add OVSDB (net)
        plugins.push(PluginInfo {
            id: "net".to_string(),
            name: "net".to_string(),
            version: "1.0.0".to_string(),
            description: "OVSDB Network Authority".to_string(),
            dbus_path: "/org/opdbus/v1/ovsdb".to_string(),
            interfaces: vec!["org.opdbus.OvsdbV1".to_string()],
            tags: vec!["network".to_string(), "ovsdb".to_string()],
        });

        // 2. Query NonNet schema for all other plugins
        let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new(
            "get_schema",
            simd_json::json!(["OpNonNet"]),
        );
        let resp = self.nonnet.handle_request(schema_req).await;
        if let Some(result) = resp.result {
            if let Some(tables) = result.get("tables").and_then(|t| t.as_object()) {
                for table_name in tables.keys() {
                    plugins.push(PluginInfo {
                        id: table_name.to_string(),
                        name: table_name.to_string(),
                        version: "1.0.0".to_string(),
                        description: format!("NonNet Plugin: {}", table_name),
                        dbus_path: format!("/org/opdbus/v1/nonnet/OpNonNet/{}", table_name),
                        interfaces: vec!["org.opdbus.NonNetV1".to_string()],
                        tags: vec!["nonnet".to_string()],
                    });
                }
            }
        }

        plugins.sort_by(|a, b| a.id.cmp(&b.id));
        plugins
    }

    async fn get_schema(&self, plugin_id: &str) -> Option<(String, String, String)> {
        if plugin_id == "net" {
            let schema = self.ovsdb.get_schema().await.ok()?;
            return Some((
                simd_json::to_string(&schema).ok()?,
                "ovsdb".to_string(),
                "1.0.0".to_string(),
            ));
        }

        // Query NonNet for plugin table schema
        let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new(
            "get_schema",
            simd_json::json!(["OpNonNet"]),
        );
        let resp = self.nonnet.handle_request(schema_req).await;
        let result = resp.result?;
        let tables = result.get("tables")?;
        let table_schema = tables.get(plugin_id)?;
        
        Some((
            simd_json::to_string(table_schema).ok()?,
            "json-schema-2026".to_string(),
            "1.0.0".to_string(),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load canonical environment file before reading configuration.
    let _ = load_environment();

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "op_dbus=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::default();

    tracing::info!("======================================");
    tracing::info!("OP-DBUS: Native Deterministic Control Plane");
    tracing::info!("======================================");
    tracing::info!("Authoritative Stores: OVSDB + NonNet (Direct RCP)");
    tracing::info!("Cache: {}", config.cache_dir);
    tracing::info!("Web: {}:{}", config.web_host, config.web_port);

    #[cfg(feature = "dev-antigravity")]
    if config.enable_antigravity {
        tracing::warn!("============================================");
        tracing::warn!("DEVELOPMENT BUILD: Antigravity tunnel enabled");
        tracing::warn!("This feature is REMOVED in production builds");
        tracing::warn!("============================================");
    }

    // Initialize authoritative RCP stores
    let authoritative_ovsdb = Arc::new(op_network::ovsdb::OvsdbClient::new());
    let authoritative_nonnet = Arc::new(NonNetDb::new());

    // Initialize transient state store (No SQL used for live state)
    let state_store: Arc<dyn StateStore> = Arc::new(op_state_store::MemoryStore::new());

    // Initialize tool registry and register built-in tools
    let tool_registry = Arc::new(ToolRegistry::new());
    register_builtin_tools(&tool_registry).await?;

    // Create orchestrator (direct tool execution, no catalog needed at runtime)
    let orchestrator = Arc::new(Orchestrator::new(
        OrchestratorConfig::default(),
        tool_registry.clone(),
    ));

    // Create policy engine
    let policy_engine = Arc::new(PolicyEngine::new(state_store.clone()));
    policy_engine.load_policies().await?;

    // Create Inspector Gadget (one-shot discovery, direct RCP backends)
    let inspector = Arc::new(InspectorGadget::new(
        InspectorConfig::default(),
        state_store.clone(),
        tool_registry.clone(),
    ));

    let state_manager = Arc::new(op_state::manager::StateManager::new());
    // (Rest of the plugin loading and D-Bus mirror setup...)

    // Initialize Schema Engine (Single Authoritative Pipeline)
    #[cfg(feature = "grpc")]
    let schema_engine = {
        let chain = Arc::new(tokio::sync::RwLock::new(op_state_store::EventChain::new(
            ChainConfig::default(),
        )));
        let engine = Arc::new(SchemaEngine::new(
            chain,
            authoritative_ovsdb.clone(),
            authoritative_nonnet.clone(),
        ));

        let engine_clone = engine.clone();
        tokio::spawn(async move {
            if let Err(e) = engine_clone.start().await {
                tracing::error!(
                    "Schema Engine authoritative pipeline failed to start: {}",
                    e
                );
            }
        });

        let _ = GLOBAL_SCHEMA_ENGINE.set(engine.clone());
        engine
    };

    // Initialize blockchain (StreamingBlockchain)
    let blockchain_path = PathBuf::from(&config.cache_dir).join("blockchain");
    let blockchain_stream = StreamingBlockchain::new(blockchain_path).await?;
    // DbusProjection expects Arc<tokio::sync::RwLock<StreamingBlockchain>>
    let blockchain = Arc::new(tokio::sync::RwLock::new(blockchain_stream));

    // Initialize cache
    let cache = Arc::new(BtrfsCache::new(PathBuf::from(&config.cache_dir)).await?);

    // Create orchestrator
    let orchestrator = Arc::new(Orchestrator::new(
        OrchestratorConfig::default(),
        tool_registry.clone(),
    ));

    // Create MCP dispatchers
    let mcp_compact = Arc::new(McpCompactDispatcher::new(tool_registry.clone()));

    let mcp_live = Arc::new(McpLiveDispatcher::new(tool_registry.clone()));

    // Create policy engine
    let policy_engine = Arc::new(PolicyEngine::new(state_store.clone()));
    policy_engine.load_policies().await?;
    tracing::info!("Policy engine initialized");

    // Create Inspector Gadget (one-shot only)
    let inspector = Arc::new(InspectorGadget::new(
        InspectorConfig::default(),
        state_store.clone(),
        tool_registry.clone(),
    ));
    tracing::info!("Inspector Gadget ready (one-shot discovery only)");

    // Create disaster recovery
    let disaster_recovery = Arc::new(DisasterRecovery::new(state_store.clone()));
    tracing::info!("Disaster recovery ready");

    // Create dependency manager
    let mut dependency_manager = DependencyManager::new(state_store.clone());
    dependency_manager.init().await?;
    let dependency_manager = Arc::new(dependency_manager);
    tracing::info!("Dependency manager initialized");

    let state_manager = Arc::new(op_state::manager::StateManager::new());
    let default_plugin_loader = op_plugins::DefaultPluginRegistry::new(state_store.clone());
    let mut initial_plugin_states = std::collections::HashMap::new();
    match default_plugin_loader.load_default_plugins().await {
        Ok(plugins) => {
            let mut join_set = tokio::task::JoinSet::new();
            for plugin in plugins {
                let plugin_name = plugin.name().to_string();
                let plugin_clone = plugin.clone();
                state_manager.register_plugin(plugin_name.clone(), plugin.clone()).await;
                
                // Fetch initial state concurrently with a timeout
                join_set.spawn(async move {
                    let res = tokio::time::timeout(std::time::Duration::from_secs(5), plugin_clone.query_current_state()).await;
                    (plugin_name, res)
                });
            }

            while let Some(res) = join_set.join_next().await {
                if let Ok((plugin_name, timeout_res)) = res {
                    match timeout_res {
                        Ok(Ok(state)) => {
                            initial_plugin_states.insert(plugin_name, state);
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("Failed to query initial state for plugin {}: {}", plugin_name, e);
                        }
                        Err(_) => {
                            tracing::warn!("Timed out querying initial state for plugin {}", plugin_name);
                        }
                    }
                }
            }
            if privacy_router_bootstrap_enabled() {
                start_privacy_router_bootstrap(state_manager.clone());
            } else {
                tracing::info!("privacy_router bootstrap disabled");
            }
        }
        Err(e) => {
            tracing::warn!("Failed to load state plugins: {}", e);
        }
    }

    // Seed authoritative NonNet from both schema definitions AND initial live state.
    // This ensures both the hierarchy (base paths) and the objects (rows) are published.
    let schema_defs = op_dbus::plugins::plugin_schema_defs();
    
    // Merge schema definitions with live state
    let schema_defs_simd: std::collections::HashMap<String, simd_json::OwnedValue> = schema_defs
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    authoritative_nonnet
        .load_from_plugins(&initial_plugin_states, Some(&schema_defs_simd))
        .await;

    tracing::info!(
        "Seeded authoritative NonNet from {} plugins ({} with schema definitions)",
        initial_plugin_states.len(),
        schema_defs_simd.len()
    );

    #[cfg(feature = "grpc")]
    {
        // Seed schema engine cache from schema defs (no OVSDB dependency)
        for (plugin_id, schema) in &schema_defs {
            schema_engine
                .update_state_cache(plugin_id.clone(), schema.clone())
                .await;
        }
    }

    // Create chatbot (cognitive brain - reasons but never executes directly)
    let _chatbot = Arc::new(
        Chatbot::new(
            ChatbotConfig::default(),
            mcp_compact.clone(),
            mcp_live.clone(),
            state_store.clone(),
        )
        .with_policy_engine(policy_engine.clone())
        .with_inspector(inspector.clone())
        .with_disaster_recovery(disaster_recovery.clone())
        .with_dependency_manager(dependency_manager.clone()),
    );
    tracing::info!("Chatbot initialized (reasoning only, no direct execution)");

    // D-Bus projection for mirrored state persistence paths.
    // Runtime discovery/introspection is migration-only and not executed here.
    // NonNet seeding already handled by the spawned task above (line 486).
    if config.enable_dbus {
        for dbus_bus in dbus_publication_buses(config.dbus_connection) {
            let mirror = match op_dbus_mirror::DbusMirror::new(
                    dbus_bus,
                    authoritative_ovsdb.clone(),
                    authoritative_nonnet.clone(),
                    #[cfg(feature = "grpc")]
                    Some(schema_engine.clone()),
                    #[cfg(not(feature = "grpc"))]
                    None,
                )
                .await
            {
                Ok(m) => Arc::new(m),
                Err(e) => {
                    tracing::error!(bus = %dbus_bus, "Failed to connect D-Bus mirror, skipping: {}", e);
                    continue;
                }
            };

            let mirror_clone = mirror.clone();
            tokio::spawn(async move {
                if let Err(e) = mirror_clone.start().await {
                    tracing::error!(bus = %dbus_bus, "D-Bus publication service failed: {}", e);
                }
            });

            tracing::info!(
                bus = %dbus_bus,
                "1:1 D-Bus publication service initialized and started"
            );
        }
    }

    let _dbus_projection = if config.enable_dbus {
        let projection = DbusProjection::new().with_blockchain(blockchain.clone());
        tracing::info!("Runtime D-Bus introspection disabled; using mirrored state/catalog");

        Some(projection)
    } else {
        None
    };

    // Start Antigravity tunnel if enabled (DEVELOPMENT ONLY)
    #[cfg(feature = "dev-antigravity")]
    let _antigravity_handle = if config.enable_antigravity {
        let transport_type = match config.antigravity_transport.to_lowercase().as_str() {
            "stdio" => TransportType::Stdio,
            "tcp" => TransportType::Tcp,
            "websocket" | "ws" => TransportType::WebSocket,
            _ => TransportType::Tcp,
        };

        let antigravity_config = AntigravityConfig {
            enabled: true,
            transport: TransportConfig {
                transport_type,
                listen_addr: config.antigravity_listen.clone(),
                tls: false,
            },
            session_timeout_secs: constants::ANTIGRAVITY_SESSION_TIMEOUT_SECS,
            track_billing: true,
            allowed_ides: vec![],
            max_sessions: 100,
        };

        let tunnel = Arc::new(AntigravityTunnel::new(
            antigravity_config,
            mcp_compact.clone(),
            orchestrator.clone(),
        ));

        let transport = TunnelTransport::new(TransportConfig {
            transport_type,
            listen_addr: config.antigravity_listen.clone(),
            tls: false,
        });

        let tunnel_clone = tunnel.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = transport.start(tunnel_clone).await {
                tracing::error!("Antigravity tunnel error: {}", e);
            }
        });

        tracing::info!(
            "Antigravity tunnel started at {}",
            config.antigravity_listen
        );
        Some(handle)
    } else {
        None
    };

    // Start web server if enabled
    if config.enable_web {
        tracing::info!(
            "Starting web interface at http://{}:{}",
            config.web_host,
            config.web_port
        );
        let addr: SocketAddr = format!("{}:{}", config.web_host, config.web_port)
            .parse()
            .map_err(|e| {
                op_dbus::error::OpDbusError::ConfigError(format!(
                    "Invalid OP_DBUS_WEB_HOST/PORT: {}",
                    e
                ))
            })?;

        // Share the tool_registry with web server (avoids duplicating 16k+ D-Bus tools)
        let web_state = Arc::new(AppState::new_with_registry(Some(tool_registry.clone())).await?);
        let app = routes::create_router(web_state);

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::error!("Web server bind error: {}", e);
                    return;
                }
            };
            if let Err(e) = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            {
                tracing::error!("Web server error: {}", e);
            }
        });
    }

    // Start gRPC server
    #[cfg(feature = "grpc")]
    if std::env::var("OP_DBUS_ENABLE_GRPC")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
    {
        let addr =
            std::env::var("OP_DBUS_GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string());
        let socket_addr: std::net::SocketAddr = addr.parse().map_err(|e| {
            op_dbus::error::OpDbusError::ConfigError(format!("Invalid OP_DBUS_GRPC_ADDR: {}", e))
        })?;

        let plugin_provider = Arc::new(OpdbusPluginProvider {
            ovsdb: authoritative_ovsdb.clone(),
            nonnet: authoritative_nonnet.clone(),
        });
        let op_grpc_server =
            OperationGrpcServer::with_plugin_provider(schema_engine, plugin_provider);

        // Initialize MCP gRPC service with shared tool registry
        let mcp_infra = GrpcInfrastructure::new().with_tool_registry(tool_registry.clone());
        let mcp_grpc_service =
            McpGrpcService::with_infrastructure(GrpcServerMode::Compact, mcp_infra);

        tokio::spawn(async move {
            tracing::info!("Starting unified gRPC server at {}", socket_addr);

            let reflection_service = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(op_grpc_bridge::proto::FILE_DESCRIPTOR_SET)
                .build_v1()
                .unwrap();

            if let Err(e) = tonic::transport::Server::builder()
                .add_service(reflection_service)
                .add_service(StateSyncServer::new(op_grpc_server.clone()))
                .add_service(PluginServiceServer::new(op_grpc_server.clone()))
                .add_service(EventChainServiceServer::new(op_grpc_server))
                .add_service(McpServiceServer::new(mcp_grpc_service))
                .serve(socket_addr)
                .await
            {
                tracing::error!("gRPC server error: {}", e);
            }
        });
    }

    // Run JSON-RPC server (blocking)
    match config.listen.as_str() {
        "stdio" => run_stdio_server(mcp_compact).await?,
        listen if listen.starts_with("tcp:") => {
            let addr = listen.strip_prefix("tcp:").unwrap();
            run_tcp_server(addr, mcp_compact).await?;
        }
        listen if listen.starts_with("unix:") => {
            let path = listen.strip_prefix("unix:").unwrap();
            run_unix_server(path, mcp_compact).await?;
        }
        "none" => {
            tracing::info!("Running in web-only mode. Press Ctrl+C to stop.");
            tokio::signal::ctrl_c().await?;
        }
        _ => {
            tracing::error!("Unknown listen address: {}", config.listen);
        }
    }

    tracing::info!("OP-DBUS shutdown complete");
    Ok(())
}

async fn run_stdio_server(dispatcher: Arc<McpCompactDispatcher>) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    tracing::info!("JSON-RPC server listening on stdio");

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let response = process_request(&dispatcher, &line).await;
        let response_json = simd_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Serialization error"}}"#.to_string()
        });

        stdout.write_all(response_json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

async fn run_tcp_server(addr: &str, dispatcher: Arc<McpCompactDispatcher>) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("JSON-RPC server listening on tcp://{}", addr);

    loop {
        let (socket, peer) = listener.accept().await?;
        let dispatcher = dispatcher.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_tcp_connection(socket, dispatcher).await {
                tracing::error!("Connection error from {}: {}", peer, e);
            }
        });
    }
}

async fn handle_tcp_connection(
    socket: tokio::net::TcpStream,
    dispatcher: Arc<McpCompactDispatcher>,
) -> Result<()> {
    let (reader, mut writer) = socket.into_split();
    let reader = BufReader::new(reader);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let response = process_request(&dispatcher, &line).await;
        let response_json = simd_json::to_string(&response).unwrap_or_default();

        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}

#[cfg(unix)]
async fn run_unix_server(path: &str, dispatcher: Arc<McpCompactDispatcher>) -> Result<()> {
    let _ = std::fs::remove_file(path);
    let listener = tokio::net::UnixListener::bind(path)?;
    tracing::info!("JSON-RPC server listening on unix://{}", path);

    loop {
        let (socket, _) = listener.accept().await?;
        let dispatcher = dispatcher.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.into_split();
            let reader = BufReader::new(reader);
            let mut lines = reader.lines();

            while let Some(line) = lines.next_line().await.unwrap_or(None) {
                if line.trim().is_empty() {
                    continue;
                }

                let response = process_request(&dispatcher, &line).await;
                let response_json = simd_json::to_string(&response).unwrap_or_default();

                let _ = writer.write_all(response_json.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
            }
        });
    }
}

#[cfg(not(unix))]
async fn run_unix_server(_path: &str, _dispatcher: Arc<McpCompactDispatcher>) -> Result<()> {
    Err(op_dbus::error::OpDbusError::ConfigError(
        "Unix sockets not supported on this platform".into(),
    ))
}

async fn process_request(dispatcher: &McpCompactDispatcher, input: &str) -> JsonRpcResponse {
    let mut input_mut = input.to_string();
    let request: JsonRpcRequest = match unsafe { simd_json::from_str(&mut input_mut) } {
        Ok(req) => req,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                simd_json::OwnedValue::from(()),
                op_dbus::json_rpc::error_codes::PARSE_ERROR,
                format!("Parse error: {}", e),
            );
        }
    };

    let id = request.id.clone();

    #[cfg(feature = "grpc")]
    if request.method == "state.mutate" {
        return handle_state_mutate_request(request).await;
    }

    let mcp_request = McpRequest::from(request);

    let mcp_response = dispatcher.handle_request(mcp_request).await;

    // Convert McpResponse back to JsonRpcResponse
    if let Some(error) = mcp_response.error {
        JsonRpcResponse::error(
            id,
            JsonRpcError {
                code: error.code,
                message: error.message,
                data: error.data,
            },
        )
    } else {
        JsonRpcResponse::success(
            id,
            mcp_response
                .result
                .unwrap_or(simd_json::OwnedValue::from(())),
        )
    }
}

#[cfg(feature = "grpc")]
async fn handle_state_mutate_request(request: JsonRpcRequest) -> JsonRpcResponse {
    use op_dbus::json_rpc::error_codes;

    let id = request.id.clone();
    let params = match request.params.as_object() {
        Some(p) => p,
        None => {
            return JsonRpcResponse::error_with_code(
                id,
                error_codes::INVALID_PARAMS,
                "params must be an object",
            )
        }
    };

    let plugin_id = match params.get("plugin_id").and_then(|v| v.as_str()) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => {
            return JsonRpcResponse::error_with_code(
                id,
                error_codes::INVALID_PARAMS,
                "missing required params.plugin_id",
            )
        }
    };

    let object_path = match params.get("object_path").and_then(|v| v.as_str()) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => {
            return JsonRpcResponse::error_with_code(
                id,
                error_codes::INVALID_PARAMS,
                "missing required params.object_path",
            )
        }
    };

    let change_type = match params
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("set_property")
    {
        "set_property" => ChangeType::PropertySet,
        "call_method" => ChangeType::MethodCall,
        "apply_patch" | "object_added" => ChangeType::ObjectAdded,
        "object_removed" => ChangeType::ObjectRemoved,
        other => {
            return JsonRpcResponse::error_with_code(
                id,
                error_codes::INVALID_PARAMS,
                format!("unsupported params.operation '{}'", other),
            )
        }
    };

    let value = match params.get("value") {
        Some(v) => v.clone(),
        None => {
            return JsonRpcResponse::error_with_code(
                id,
                error_codes::INVALID_PARAMS,
                "missing required params.value",
            )
        }
    };

    let member_name = params
        .get("member_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let actor_id = params
        .get("actor_id")
        .and_then(|v| v.as_str())
        .unwrap_or("jsonrpc-client")
        .to_string();
    let capability_id = params
        .get("capability_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let Some(schema_engine) = GLOBAL_SCHEMA_ENGINE.get() else {
        return JsonRpcResponse::error_with_code(
            id,
            error_codes::INTERNAL_ERROR,
            "schema engine unavailable; enable grpc bridge first",
        );
    };

    match schema_engine
        .mutate(
            plugin_id,
            object_path,
            change_type,
            member_name,
            value,
            actor_id,
            capability_id,
        )
        .await
    {
        Ok(result) => JsonRpcResponse::success(
            id,
            simd_json::json!({
                "success": result.success,
                "event_id": result.event_id,
                "event_hash": result.event_hash,
                "result": result.result
            }),
        ),
        Err(e) => JsonRpcResponse::error_with_code(id, error_codes::INVALID_PARAMS, e.to_string()),
    }
}
