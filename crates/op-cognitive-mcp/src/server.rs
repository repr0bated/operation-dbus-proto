//! Cognitive MCP Server — stdio transport + in-process registry for the bridge.
//!
//! Single persistent backend (CozoDB) hosts every relation:
//! memory namespaces/entries, users, sessions, compliance graph, subid registry, audit log.

use crate::code_tools::{register_code_tools, register_disabled_code_tools};
use crate::cognitive_tools::CognitiveToolRegistry;
use crate::context_awareness::{ContextAwarenessConfig, ContextAwarenessEngine};
use crate::cozo_shuttle::CozoGraphShuttle;
use crate::grpc_service::CognitiveGrpcService;
use crate::memory_store::CognitiveMemoryStore;
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
        CognitiveToolRegistry::register_all(
            &tool_registry,
            memory_store.clone(),
            qdrant_shuttle.clone(),
        )
        .await?;

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
                let reason = format!("Code RAG unavailable: {e}");
                let disabled = register_disabled_code_tools(&tool_registry, reason.clone()).await?;
                tracing::warn!(
                    error = %e,
                    disabled,
                    "RAG pipeline unavailable; disabled code-context tools remain discoverable"
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
            rag_pipeline,
            context_engine,
        })
    }

    /// Run the cognitive MCP server over stdio (stdin/stdout JSON-RPC).
    pub async fn start_stdio(self) -> Result<(), Box<dyn std::error::Error>> {
        use op_mcp::{McpServer, McpServerConfig, StdioTransport, Transport};

        let config = McpServerConfig {
            name: Some("cognitive-mcp".to_string()),
            // A cognitive runtime can register hundreds of agent operations.
            // Advertising every schema consumes the model's working context
            // before it can answer; compact mode retains deterministic tool
            // discovery through list/search/schema/execute meta-tools.
            compact_mode: compact_mode_from_env(),
            ..Default::default()
        };

        let executor = Arc::new(RegistryExecutor::new(self.tool_registry.clone()));
        let mcp_server = Arc::new(McpServer::with_executor(config, executor));

        tracing::info!("Cognitive MCP Server starting (stdio transport)");

        StdioTransport::new().serve(mcp_server).await?;
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

    /// Build the gRPC ingress surface for mounting on the bridge's `:50051` listener.
    pub fn cognitive_grpc_service(&self) -> CognitiveGrpcService {
        CognitiveGrpcService::new(
            self.memory_store.clone(),
            self.session_manager.clone(),
            self.quota_manager.clone(),
            self.tool_registry.clone(),
        )
    }
}

/// Cognitive MCP defaults to compact tool discovery. Set
/// `COGNITIVE_MCP_COMPACT_MODE=0` for clients which explicitly require the
/// complete raw `tools/list` response.
fn compact_mode_from_env() -> bool {
    let value = std::env::var("COGNITIVE_MCP_COMPACT_MODE").ok();
    compact_mode_from_value(value.as_deref())
}

fn compact_mode_from_value(value: Option<&str>) -> bool {
    value
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::compact_mode_from_value;

    #[test]
    fn compact_mode_parser_defaults_to_compact_and_allows_full_override() {
        assert!(compact_mode_from_value(None));
        assert!(compact_mode_from_value(Some("true")));
        assert!(!compact_mode_from_value(Some("0")));
        assert!(!compact_mode_from_value(Some("false")));
        assert!(!compact_mode_from_value(Some("off")));
    }
}
