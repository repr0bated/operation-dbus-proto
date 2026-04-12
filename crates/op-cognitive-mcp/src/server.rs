//! Cognitive MCP Server
//!
//! Main server implementation that provides cognitive memory and dynamic loading capabilities.

use crate::cognitive_tools::CognitiveToolRegistry;
use crate::graph_store::KnowledgeGraphStore;
use op_mcp::tool_registry::ToolRegistry;
use std::sync::Arc;

pub struct CognitiveMcpServer {
    graph_store: Arc<KnowledgeGraphStore>,
    tool_registry: Arc<ToolRegistry>,
}

impl CognitiveMcpServer {
    pub async fn new(graph_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let graph_store = Arc::new(KnowledgeGraphStore::new_on_disk(graph_path)?);
        let tool_registry = Arc::new(ToolRegistry::new());

        CognitiveToolRegistry::register_all(&tool_registry, graph_store.clone()).await?;

        Ok(Self {
            graph_store,
            tool_registry,
        })
    }

    pub async fn start_http_server(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        use op_mcp::{HttpSseTransport, McpServer, McpServerConfig, Transport};

        let config = McpServerConfig {
            name: Some("cognitive-mcp".to_string()),
            compact_mode: true,
            ..Default::default()
        };

        let mcp_server = McpServer::new(config).await?;
        let transport = HttpSseTransport::new(addr.to_string());

        tracing::info!("Cognitive MCP Server listening on {}", addr);
        transport.serve(mcp_server).await?;
        Ok(())
    }

    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    pub fn graph_store(&self) -> Arc<KnowledgeGraphStore> {
        self.graph_store.clone()
    }
}
