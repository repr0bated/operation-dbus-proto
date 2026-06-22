//! Cognitive MCP Server — Dual Transport (HTTP/SSE + gRPC)
//!
//! Single persistent backend (CozoDB) hosts every relation:
//! memory namespaces/entries, users, sessions, compliance graph, subid registry, audit log.

use crate::code_tools::register_code_tools;
use crate::cognitive_tools::CognitiveToolRegistry;
use crate::context_awareness::{ContextAwarenessConfig, ContextAwarenessEngine};
use crate::cozo_shuttle::CozoGraphShuttle;
use crate::gemini_fallback::GeminiFallback;
use crate::grpc_service::CognitiveGrpcService;
use crate::memory_store::CognitiveMemoryStore;
use crate::proto::cognitive_tool_service_server::CognitiveToolServiceServer;
use crate::qdrant_shuttle::QdrantSemanticShuttle;
use crate::quota::QuotaManager;
use crate::rag_pipeline::{default_collection_from_env, RagPipeline};
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
    /// Code-RAG pipeline (Voyage + Qdrant). `None` when no Voyage key is found.
    rag_pipeline: Option<Arc<RagPipeline>>,
    /// Proactive coding-context awareness engine.
    context_engine: Arc<ContextAwarenessEngine>,
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

        // Code-RAG pipeline: optional, like qdrant_shuttle. Without a Voyage key
        // (env or ~/.ssh/mongo-voyage) the cognitive MCP still serves memory tools.
        let rag_pipeline = match RagPipeline::from_env() {
            Ok(p) => Some(Arc::new(p)),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "RAG pipeline unavailable; code-context tools will not be registered"
                );
                None
            }
        };

        // Context-awareness engine drives session signals and proactive pushes.
        let rag_collection = default_collection_from_env();
        let context_engine = Arc::new(ContextAwarenessEngine::new(
            ContextAwarenessConfig {
                rag_collection: rag_collection.clone(),
                ..Default::default()
            },
            memory_store.clone(),
            rag_pipeline.clone(),
        ));
        context_engine.clone().start_monitoring();

        if let Some(rag) = &rag_pipeline {
            let n = register_code_tools(
                &tool_registry,
                rag.clone(),
                context_engine.clone(),
                rag_collection,
            )
            .await?;
            tracing::info!(registered = n, "Registered code-context tools");
        }

        Ok(Self {
            memory_store,
            cozo_shuttle,
            qdrant_shuttle,
            tool_registry,
            session_manager,
            quota_manager,
            gemini_fallback,
            rag_pipeline,
            context_engine,
        })
    }

    /// Run the cognitive MCP server over stdio (stdin/stdout JSON-RPC).
    /// This is the preferred transport for local MCP clients — no network
    /// overhead, direct pipe communication.
    pub async fn start_stdio(self) -> Result<(), Box<dyn std::error::Error>> {
        use op_mcp::{McpServer, McpServerConfig, StdioTransport, Transport};

        let config = McpServerConfig {
            name: Some("cognitive-mcp".to_string()),
            compact_mode: false,
            ..Default::default()
        };

        let executor = Arc::new(RegistryExecutor::new(self.tool_registry.clone()));
        let mcp_server = Arc::new(McpServer::with_executor(config, executor));

        tracing::info!("Cognitive MCP Server starting (stdio transport)");

        StdioTransport::new().serve(mcp_server).await?;
        Ok(())
    }

    pub async fn start_http_server(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        use crate::context_server::build_context_router;
        use op_mcp::{HttpSseTransport, McpServer, McpServerConfig, Transport};

        let config = McpServerConfig {
            name: Some("cognitive-mcp".to_string()),
            compact_mode: false,
            ..Default::default()
        };

        let executor = Arc::new(RegistryExecutor::new(self.tool_registry.clone()));
        let mcp_server = Arc::new(McpServer::with_executor(config, executor));

        let mut transport = HttpSseTransport::new(addr.to_string());
        if std::env::var("COGNITIVE_MCP_CONTEXT_HTTP_DISABLED").as_deref() != Ok("1") {
            // Mount the context-awareness SSE endpoints on the same HTTP server
            // so they share auth, CORS, and the port with the MCP protocol routes.
            let context_router = build_context_router(
                self.context_engine.clone(),
                self.memory_store.clone(),
                self.session_manager.clone(),
            );
            transport = transport.with_extra_router(context_router);
        }

        tracing::info!(
            addr = %addr,
            "Cognitive MCP Server listening (MCP + context-awareness endpoints)"
        );
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

        let cors = tower_http::cors::CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
            .expose_headers([
                "grpc-status".parse().unwrap(),
                "grpc-message".parse().unwrap(),
                "grpc-status-details-bin".parse().unwrap(),
            ]);

        tonic::transport::Server::builder()
            .accept_http1(true)
            .layer(cors)
            .add_service(tonic_web::enable(
                CognitiveToolServiceServer::with_interceptor(
                    grpc_service,
                    crate::interceptor::ghostbridge_interceptor,
                ),
            ))
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
            let grpc_service =
                CognitiveGrpcService::new(grpc_memory, grpc_session, grpc_quota, grpc_gemini);

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

            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
                .expose_headers([
                    "grpc-status".parse().unwrap(),
                    "grpc-message".parse().unwrap(),
                    "grpc-status-details-bin".parse().unwrap(),
                ]);

            tonic::transport::Server::builder()
                .accept_http1(true)
                .layer(cors)
                .add_service(tonic_web::enable(
                    CognitiveToolServiceServer::with_interceptor(
                        grpc_service,
                        crate::interceptor::ghostbridge_interceptor,
                    ),
                ))
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

    pub fn qdrant_shuttle(&self) -> Option<Arc<QdrantSemanticShuttle>> {
        self.qdrant_shuttle.clone()
    }

    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    pub fn quota_manager(&self) -> Arc<QuotaManager> {
        self.quota_manager.clone()
    }

    pub fn rag_pipeline(&self) -> Option<Arc<RagPipeline>> {
        self.rag_pipeline.clone()
    }

    pub fn context_engine(&self) -> Arc<ContextAwarenessEngine> {
        self.context_engine.clone()
    }
}
