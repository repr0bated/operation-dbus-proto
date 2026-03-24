//! Cognitive MCP Server
//!
//! Main server implementation that provides cognitive memory and dynamic loading capabilities.

use crate::memory_store::CognitiveMemoryStore;
use crate::cognitive_tools::MemoryTool;
use op_mcp::{McpServer, McpServerConfig, Transport};
use op_mcp::tool_registry::ToolRegistry;
use std::sync::Arc;

pub struct CognitiveMcpServer {
    memory_store: Arc<CognitiveMemoryStore>,
    tool_registry: Arc<ToolRegistry>,
}

impl CognitiveMcpServer {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let memory_store = Arc::new(CognitiveMemoryStore::new(10000)); // Max 10k entries
        let tool_registry = Arc::new(ToolRegistry::new());

        // Register cognitive tools
        let memory_tool = MemoryTool::new(memory_store.clone());
        tool_registry.register(Arc::new(memory_tool)).await?;

        // TODO: Add more cognitive tools
        // - Dynamic content loader
        // - Context analyzer
        // - Learning tools
        // - Pattern recognition

        Ok(Self {
            memory_store,
            tool_registry,
        })
    }

    pub async fn start_http_server(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        use op_mcp::transport::HttpSseTransport;

        let config = McpServerConfig {
            name: Some("cognitive-mcp".to_string()),
            compact_mode: true,
            ..Default::default()
        };

        let mcp_server = McpServer::new(config).await?;
        let transport = HttpSseTransport::new(addr.to_string());

        println!("Starting Cognitive MCP Server on {}", addr);
        println!("Features:");
        println!("  - Cognitive Memory Store");
        println!("  - Dynamic Content Loading");
        println!("  - Context-Aware Tool Discovery");

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