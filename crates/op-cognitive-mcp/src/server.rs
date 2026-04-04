//! Cognitive MCP Server
//!
//! Main server implementation that provides cognitive memory and dynamic loading capabilities.

use crate::cognitive_tools::CognitiveToolRegistry;
use crate::memory_store::CognitiveMemoryStore;
use op_mcp::tool_registry::ToolRegistry;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

pub struct CognitiveMcpServer {
    memory_store: Arc<CognitiveMemoryStore>,
    tool_registry: Arc<ToolRegistry>,
}

impl CognitiveMcpServer {
    pub async fn new(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite://{}?mode=rwc", db_path))
            .await?;
        let memory_store = Arc::new(CognitiveMemoryStore::new(pool).await?);
        let tool_registry = Arc::new(ToolRegistry::new());

        CognitiveToolRegistry::register_all(&tool_registry, memory_store.clone()).await?;

        Ok(Self {
            memory_store,
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

    pub fn memory_store(&self) -> Arc<CognitiveMemoryStore> {
        self.memory_store.clone()
    }

    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }
}
