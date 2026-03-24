//! Web Server Implementation

use axum::{Router, Extension};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use op_tools::ToolRegistry;
use crate::mcp::McpCompactDispatcher;
use crate::error::Result;

#[cfg(feature = "dev-antigravity")]
use crate::antigravity::InstanceRegistry;

use super::routes::{create_router, AppState};

/// Web server configuration
#[derive(Debug, Clone)]
pub struct WebServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_cors: bool,
    pub enable_websocket: bool,
}

impl Default for WebServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: true,
            enable_websocket: true,
        }
    }
}

use crate::chatbot::Chatbot;

/// Web server for MCP HTTP/WebSocket interface
pub struct WebServer {
    config: WebServerConfig,
    tool_registry: Arc<ToolRegistry>,
    chatbot: Option<Arc<Chatbot>>,
    #[cfg(feature = "dev-antigravity")]
    instance_registry: Arc<InstanceRegistry>,
}

impl WebServer {
    pub fn new(config: WebServerConfig, tool_registry: Arc<ToolRegistry>, chatbot: Option<Arc<Chatbot>>) -> Self {
        Self {
            config,
            tool_registry,
            chatbot,
            #[cfg(feature = "dev-antigravity")]
            instance_registry: Arc::new(InstanceRegistry::new()),
        }
    }

    /// Start the web server
    pub async fn start(self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| crate::error::OpDbusError::ConfigError(format!("Invalid address: {}", e)))?;

        let dispatcher = Arc::new(McpCompactDispatcher::new(Arc::clone(&self.tool_registry)));
        
        let state = AppState {
            dispatcher,
            chatbot: self.chatbot.clone(),
            #[cfg(feature = "dev-antigravity")]
            instance_registry: Arc::clone(&self.instance_registry),
        };
        
        let mut router = create_router(state);

        if self.config.enable_cors {
            router = router.layer(tower_http::cors::CorsLayer::permissive());
        }

        tracing::info!("Web server starting on {}", addr);

        let listener = TcpListener::bind(addr).await
            .map_err(|e| crate::error::OpDbusError::IoError(e))?;

        axum::serve(listener, router).await
            .map_err(|e| crate::error::OpDbusError::Internal(e.to_string()))?;

        Ok(())
    }
}

