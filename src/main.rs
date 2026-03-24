//! OP-DBUS: Native, Deterministic Control Plane for Linux Systems
//!
//! Production entry point with all components wired together.

use parking_lot::RwLock;
use simd_json::prelude::*;
use std::collections::HashMap;
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
use op_plugins::plugin::{PluginMetadata as PluginCore, PluginTunables};
use op_plugins::registry::PluginRegistry;
use op_state_store::{SqliteStore, StateStore};
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
    mcp::{McpCompactDispatcher, McpError, McpRequest, McpResponse},
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
use op_grpc_bridge::sync_engine::ChangeType;
#[cfg(feature = "grpc")]
use op_grpc_bridge::{
    DbusWatcher, OperationGrpcServer, PluginSchemaProvider, SyncEngine, WatchConfig,
};
#[cfg(feature = "grpc")]
use op_mcp::grpc::proto::mcp_service_server::McpServiceServer;
#[cfg(feature = "grpc")]
use op_mcp::grpc::{GrpcInfrastructure, GrpcServerMode, McpGrpcService};
#[cfg(feature = "grpc")]
use op_state_store::ChainConfig;
use op_web::{routes, AppState};
#[cfg(feature = "grpc")]
use serde_json::Value as JsonValue;
#[cfg(feature = "grpc")]
static GLOBAL_SYNC_ENGINE: OnceLock<Arc<SyncEngine>> = OnceLock::new();

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
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;

            let desired_config = match simd_json::serde::to_owned_value(
                op_plugins::state_plugins::privacy_router::PrivacyRouterConfig::default(),
            ) {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!("Failed to encode default privacy_router config: {}", e);
                    continue;
                }
            };

            let desired = op_state::manager::DesiredState {
                version: 1,
                plugins: HashMap::from([("privacy_router".to_string(), desired_config)]),
            };

            match state_manager
                .apply_state_single_plugin(desired, "privacy_router")
                .await
            {
                Ok(report) => {
                    tracing::debug!(
                        "privacy_router bootstrap applied: success={}, results={}",
                        report.success,
                        report.results.len()
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to bootstrap privacy_router from op-dbus: {}", e);
                }
            }
        }
    });
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
            dbus_connection: if std::env::var("OP_DBUS_SESSION_BUS").is_ok() {
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
struct OpdbusPluginProvider;

#[cfg(feature = "grpc")]
impl PluginSchemaProvider for OpdbusPluginProvider {
    fn list_plugins(&self) -> Vec<PluginInfo> {
        let mut plugins = Vec::new();
        for plugin in op_dbus::plugins::plugin_definitions() {
            let mut description = String::new();
            let mut dbus_path = String::new();
            let mut interfaces = Vec::new();

            if let Ok(value) = serde_json::from_str::<JsonValue>(plugin.schema_json) {
                if let Some(desc) = value.get("description").and_then(|v| v.as_str()) {
                    description = desc.to_string();
                }
                if let Some(object_types) = value.get("object_types").and_then(|v| v.as_object()) {
                    for (_name, entry) in object_types {
                        if let Some(path) = entry.get("base_path").and_then(|v| v.as_str()) {
                            if dbus_path.is_empty() {
                                dbus_path = path.to_string();
                            }
                        }
                        if let Some(interface) = entry.get("interface").and_then(|v| v.as_str()) {
                            interfaces.push(interface.to_string());
                        }
                    }
                }
            }

            plugins.push(PluginInfo {
                id: plugin.name.to_string(),
                name: plugin.name.to_string(),
                version: "v1".to_string(),
                description,
                dbus_path,
                interfaces,
                tags: Vec::new(),
            });
        }
        plugins
    }

    fn get_schema(&self, plugin_id: &str) -> Option<(String, String, String)> {
        let schema = op_dbus::plugins::get_plugin_schema_json(plugin_id)?;
        Some((
            schema.to_string(),
            "json-schema-2020-12".to_string(),
            "v1".to_string(),
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
    tracing::info!("Database: {}", config.database_url);
    tracing::info!("Cache: {}", config.cache_dir);
    tracing::info!("Web: {}:{}", config.web_host, config.web_port);

    #[cfg(feature = "dev-antigravity")]
    if config.enable_antigravity {
        tracing::warn!("============================================");
        tracing::warn!("DEVELOPMENT BUILD: Antigravity tunnel enabled");
        tracing::warn!("This feature is REMOVED in production builds");
        tracing::warn!("============================================");
    }

    // Initialize state store (authoritative database)
    if let Some(db_path) = config.database_url.strip_prefix("sqlite://") {
        if db_path != ":memory:" {
            if let Some(parent) = std::path::Path::new(db_path).parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    tracing::warn!(
                        "Failed to create database directory {}: {}",
                        parent.display(),
                        e
                    );
                }
            }
        }
    }

    let sqlite_store = match SqliteStore::new(&config.database_url).await {
        Ok(store) => store,
        Err(e) => {
            tracing::warn!(
                "Failed to initialize state store at {}: {}, using in-memory",
                config.database_url,
                e
            );
            SqliteStore::new(":memory:").await?
        }
    };
    let pool = sqlite_store.pool().clone();
    let state_store: Arc<dyn StateStore> = Arc::new(sqlite_store);

    op_dbus_model::create_schema(&pool).await?;
    op_dbus::plugins::insert_plugins(&pool).await?;
    op_dbus::pre_canned::create_pre_canned_schemas(&pool).await?;
    op_dbus::plugins::validate_plugin_schemas_from_repo()?;

    // Initialize NUMA optimizer
    let numa_optimizer = NumaOptimizer::from_env();
    if numa_optimizer.is_available() {
        tracing::info!("NUMA optimization enabled");
    }

    // Initialize vectorization
    let footprint_generator = FootprintGenerator::from_env();
    tracing::info!("Vectorization level: {:?}", footprint_generator);

    // Initialize tool registry and register built-in tools
    let tool_registry = Arc::new(ToolRegistry::new());
    register_builtin_tools(&tool_registry).await?;

    // Discover D-Bus tools to reach the full 16k+ toolset
    let introspection = Arc::new(op_introspection::IntrospectionService::new());
    let projection = op_tools::discovery::projection_engine::ProjectionEngine::new(introspection);
    tracing::info!("Discovering D-Bus tools (System bus)...");
    if let Ok(count) = projection
        .discover_all(&tool_registry, BusType::System)
        .await
    {
        tracing::info!("Registered {} tools from D-Bus projection", count);
    }

    tracing::info!("Total tools in registry: {}", tool_registry.len().await);

    // Initialize plugin registry
    let plugin_dir = PathBuf::from(&config.cache_dir).join("plugins");
    let plugin_registry = Arc::new(PluginRegistry::new(&plugin_dir));

    // Register system plugin metadata (placeholder until full plugin loading is restored)
    let _system_plugin = PluginCore {
        name: "system".to_string(),
        version: "1.0.0".to_string(),
        description: "Core system plugin".to_string(),
        ..Default::default()
    };

    // The new registry requires a BoxedPlugin trait object, not just metadata.
    // For now, we skip manual registration of "system" plugin as tools are registered directly.
    // plugin_registry.register_core(system_plugin);
    // plugin_registry.register_tunables("system", TunableScope::Global, PluginTunables::default());

    // Initialize blockchain (StreamingBlockchain)
    let blockchain_path = PathBuf::from(&config.cache_dir).join("blockchain");
    let blockchain_stream = StreamingBlockchain::new(blockchain_path).await?;
    // DbusProjection expects Arc<parking_lot::RwLock<StreamingBlockchain>>
    let blockchain = Arc::new(parking_lot::RwLock::new(blockchain_stream));

    // Initialize cache
    let cache = Arc::new(BtrfsCache::new(PathBuf::from(&config.cache_dir)).await?);

    // Create orchestrator
    let orchestrator = Arc::new(Orchestrator::new(
        OrchestratorConfig::default(),
        tool_registry.clone(),
        plugin_registry.clone(),
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
        plugin_registry.clone(),
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
    if config.enable_dbus {
        let mirror_ovsdb = Arc::new(OvsdbClient::new());
        let mirror_nonnet = Arc::new(NonNetDb::new());
        let mirror = Arc::new(
            op_dbus_mirror::DbusMirror::new(config.dbus_connection, mirror_ovsdb, mirror_nonnet)
                .await?,
        );

        // Start StateManager + OvsdbV1 D-Bus service on org.opdbus
        let state_manager = Arc::new(op_state::manager::StateManager::new());

        // Load and register plugins via DefaultPluginRegistry
        let plugin_registry_state = op_plugins::DefaultPluginRegistry::new(state_store.clone());
        match plugin_registry_state.load_default_plugins().await {
            Ok(plugins) => {
                for plugin in plugins {
                    state_manager.register_plugin(plugin).await;
                }
                start_privacy_router_bootstrap(state_manager.clone());
            }
            Err(e) => {
                tracing::warn!("Failed to load state plugins: {}", e);
            }
        }

        match state_manager.query_current_state().await {
            Ok(current_state) => {
                mirror.load_plugin_state(&current_state.plugins).await;
                tracing::info!(
                    "Seeded NonNet mirror state from {} plugins",
                    current_state.plugins.len()
                );
            }
            Err(e) => {
                tracing::warn!("Failed to seed NonNet mirror state: {}", e);
            }
        }

        let mirror_clone = mirror.clone();
        tokio::spawn(async move {
            if let Err(e) = mirror_clone.start().await {
                tracing::error!("D-Bus publication service failed: {}", e);
            }
        });

        tracing::info!("1:1 D-Bus publication service initialized and started");

        let dbus_ovsdb = Arc::new(OvsdbClient::new());
        let sm = state_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = op_state::dbus_server::start_system_bus(sm, dbus_ovsdb).await {
                tracing::error!("D-Bus StateManager service failed: {}", e);
            }
        });

        tracing::info!("StateManager + OvsdbV1 + NonNetV1 D-Bus service started on org.opdbus");
    }

    let _dbus_projection = if config.enable_dbus {
        let projection = DbusProjection::new().with_blockchain(blockchain.clone());
        tracing::info!("Runtime D-Bus introspection disabled; using mirrored state/registry");

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

        let chain = Arc::new(tokio::sync::RwLock::new(op_state_store::EventChain::new(
            ChainConfig::default(),
        )));
        let sync_engine = Arc::new(SyncEngine::new(chain));
        let _ = GLOBAL_SYNC_ENGINE.set(sync_engine.clone());

        // Start D-Bus watcher to push property changes into the sync engine.
        let mut watcher = DbusWatcher::new(WatchConfig::default(), sync_engine.clone());
        if let Err(e) = watcher.connect().await {
            tracing::warn!("D-Bus watcher connect failed: {}", e);
        } else if let Err(e) = watcher.start().await {
            tracing::warn!("D-Bus watcher start failed: {}", e);
        } else {
            let watcher = Arc::new(watcher);
            // Register plugin base paths for routing.
            for plugin in op_dbus::plugins::plugin_definitions() {
                let mut schema = plugin.schema_json.to_string();
                if let Ok(schema_value) =
                    unsafe { simd_json::from_str::<simd_json::OwnedValue>(&mut schema) }
                {
                    if let Some(object_types) = schema_value
                        .as_object()
                        .and_then(|o| o.get("object_types"))
                        .and_then(|v| v.as_object())
                    {
                        for (_name, entry_value) in object_types {
                            if let Some(path) = entry_value
                                .as_object()
                                .and_then(|o| o.get("base_path"))
                                .and_then(|v| v.as_str())
                            {
                                watcher
                                    .register_path(path.to_string(), plugin.name.to_string())
                                    .await;
                            }
                        }
                    }
                }
            }
            watcher.spawn();
        }

        let plugin_provider = Arc::new(OpdbusPluginProvider);
        let op_grpc_server =
            OperationGrpcServer::with_plugin_provider(sync_engine, plugin_provider);

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

    let Some(sync_engine) = GLOBAL_SYNC_ENGINE.get() else {
        return JsonRpcResponse::error_with_code(
            id,
            error_codes::INTERNAL_ERROR,
            "sync engine unavailable; enable grpc bridge first",
        );
    };

    match sync_engine
        .process_jsonrpc_mutation(
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
