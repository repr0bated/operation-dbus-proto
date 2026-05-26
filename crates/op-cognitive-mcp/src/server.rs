//! Cognitive MCP Server — Dual Transport (HTTP/SSE + gRPC)
//!
//! Single persistent backend (CozoDB) hosts every relation:
//! memory namespaces/entries, users, sessions, compliance graph, subid registry, audit log.

use crate::cognitive_tools::CognitiveToolRegistry;
use crate::cozo_shuttle::CozoGraphShuttle;
use crate::gemini_fallback::GeminiFallback;
use crate::grpc_service::CognitiveGrpcService;
use crate::memory_store::CognitiveMemoryStore;
use crate::proto::cognitive_tool_service_server::CognitiveToolServiceServer;
use crate::qdrant_shuttle::QdrantSemanticShuttle;
use crate::quota::QuotaManager;
use crate::session::SessionManager;
use crate::typed_tools;
use op_mcp::tool_registry::{RegistryExecutor, ToolRegistry};
use std::path::PathBuf;
use std::sync::Arc;

pub struct CognitiveMcpServer {
    memory_store: Arc<CognitiveMemoryStore>,
    cozo_shuttle: Arc<CozoGraphShuttle>,
    qdrant_shuttle: Option<Arc<QdrantSemanticShuttle>>,
    tool_registry: Arc<ToolRegistry>,
    session_manager: Arc<SessionManager>,
    quota_manager: Arc<QuotaManager>,
    gemini_fallback: Arc<GeminiFallback>,
}

impl CognitiveMcpServer {
    /// `db_path` is the CozoDB directory backing every persistent relation
    /// (memory namespaces/entries, users, sessions, compliance graph, audit log).
    pub async fn new(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let cozo_shuttle = Arc::new(CozoGraphShuttle::new_persistent(PathBuf::from(db_path))?);
        let memory_store = Arc::new(CognitiveMemoryStore::new(cozo_shuttle.clone()).await?);

        let tool_registry = Arc::new(ToolRegistry::new());

        let qdrant_shuttle = match QdrantSemanticShuttle::new().await {
            Ok(shuttle) => Some(Arc::new(shuttle)),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Qdrant Semantic Shuttle unavailable; cognitive MCP will continue without vector retrieval"
                );
                None
            }
        };

        let session_manager = Arc::new(SessionManager::with_defaults());
        let quota_manager = Arc::new(QuotaManager::with_defaults());
        let gemini_fallback = Arc::new(GeminiFallback::new());

        CognitiveToolRegistry::register_all(&tool_registry, memory_store.clone()).await?;

        typed_tools::register_typed_tools(
            &tool_registry,
            memory_store.clone(),
            session_manager.clone(),
            quota_manager.clone(),
        )
        .await?;

        Ok(Self {
            memory_store,
            cozo_shuttle,
            qdrant_shuttle,
            tool_registry,
            session_manager,
            quota_manager,
            gemini_fallback,
        })
    }

    pub async fn start_http_server(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        use op_mcp::{HttpSseTransport, McpServer, McpServerConfig, Transport};

        let config = McpServerConfig {
            name: Some("cognitive-mcp".to_string()),
            compact_mode: false,
            ..Default::default()
        };

        let executor = Arc::new(RegistryExecutor::new(self.tool_registry.clone()));
        let mcp_server = Arc::new(McpServer::with_executor(config, executor));
        let transport = HttpSseTransport::new(addr.to_string());

        tracing::info!("Cognitive MCP Server listening on {}", addr);
        transport.serve(mcp_server).await?;
        Ok(())
    }

    pub async fn start_grpc_server(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let grpc_service = CognitiveGrpcService::new(
            self.memory_store.clone(),
            self.session_manager.clone(),
            self.quota_manager.clone(),
            self.gemini_fallback.clone(),
        );

        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("failed to build cognitive reflection service");

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
        health_reporter
            .set_serving::<CognitiveToolServiceServer<CognitiveGrpcService>>()
            .await;

        let socket_addr: std::net::SocketAddr = addr.parse()?;
        tracing::info!(addr = %socket_addr, "Cognitive gRPC Server listening");

        tonic::transport::Server::builder()
            .accept_http1(true)
            .add_service(tonic_web::enable(CognitiveToolServiceServer::with_interceptor(
                grpc_service,
                crate::interceptor::ghostbridge_interceptor,
            )))
            .add_service(tonic_web::enable(reflection))
            .add_service(tonic_web::enable(health_service))
            .serve(socket_addr)
            .await?;

        Ok(())
    }

    pub async fn start_dual(
        self,
        http_addr: &str,
        grpc_addr: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let grpc_addr = grpc_addr.to_string();
        let http_addr = http_addr.to_string();

        let grpc_memory = self.memory_store.clone();
        let grpc_session = self.session_manager.clone();
        let grpc_quota = self.quota_manager.clone();
        let grpc_gemini = self.gemini_fallback.clone();

        let grpc_handle = tokio::spawn(async move {
            let grpc_service = CognitiveGrpcService::new(
                grpc_memory, grpc_session, grpc_quota, grpc_gemini,
            );

            let reflection = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
                .build_v1()
                .expect("failed to build cognitive reflection service");

            let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
            health_reporter
                .set_serving::<CognitiveToolServiceServer<CognitiveGrpcService>>()
                .await;

            let socket_addr: std::net::SocketAddr = grpc_addr.parse().expect("invalid gRPC addr");
            tracing::info!(addr = %socket_addr, "Cognitive gRPC Server listening");

            tonic::transport::Server::builder()
                .accept_http1(true)
                .add_service(tonic_web::enable(CognitiveToolServiceServer::with_interceptor(
                    grpc_service,
                    crate::interceptor::ghostbridge_interceptor,
                )))
                .add_service(tonic_web::enable(reflection))
                .add_service(tonic_web::enable(health_service))
                .serve(socket_addr)
                .await
                .expect("gRPC server failed");
        });

        self.start_http_server(&http_addr).await?;
        grpc_handle.await?;
        Ok(())
    }

    pub fn memory_store(&self) -> Arc<CognitiveMemoryStore> {
        self.memory_store.clone()
    }

    pub fn cozo_shuttle(&self) -> Arc<CozoGraphShuttle> {
        self.cozo_shuttle.clone()
    }

    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Register on the session D-Bus and serve until the connection drops.
    /// Runs in the background — call this before start_http_server / start_dual.
    pub async fn start_dbus(&self) -> Result<zbus::Connection, Box<dyn std::error::Error>> {
        use crate::dbus_interface::CognitiveMcpInterface;

        let conn = zbus::Connection::system().await?;
        conn.request_name("org.opdbus.CognitiveMcp").await?;

        let iface = CognitiveMcpInterface::new(self.tool_registry.clone());
        conn.object_server()
            .at("/org/opdbus/v1/cognitive", iface)
            .await?;

        tracing::info!("Cognitive MCP D-Bus interface registered at /org/opdbus/v1/cognitive");
        Ok(conn)
    }

    pub fn qdrant_shuttle(&self) -> Option<Arc<QdrantSemanticShuttle>> {
        self.qdrant_shuttle.clone()
    }

    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    pub fn quota_manager(&self) -> Arc<QuotaManager> {
        self.quota_manager.clone()
    }
}
