//! MCP Live Dispatcher - Real-time streaming and agent mode
//!
//! Provides live agent capabilities with streaming responses

use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::mcp::{McpRequest, McpResponse, McpError, PROTOCOL_VERSION, SERVER_VERSION};
use op_tools::ToolRegistry;

// ============================================================================
// LIVE AGENT
// ============================================================================

/// A live agent that can process requests with streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub enabled: bool,
}

impl LiveAgent {
    pub fn new(id: &str, name: &str, description: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            capabilities: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_capability(mut self, cap: &str) -> Self {
        self.capabilities.push(cap.to_string());
        self
    }
}

// ============================================================================
// COGNITIVE STREAM
// ============================================================================

/// A stream of cognitive responses (thinking, planning, executing)
pub struct CognitiveStream {
    receiver: mpsc::Receiver<CognitiveChunk>,
}

impl CognitiveStream {
    pub fn new(receiver: mpsc::Receiver<CognitiveChunk>) -> Self {
        Self { receiver }
    }

    pub async fn next(&mut self) -> Option<CognitiveChunk> {
        self.receiver.recv().await
    }
}

/// A chunk of cognitive output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveChunk {
    pub chunk_type: CognitiveChunkType,
    pub content: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveChunkType {
    Thinking,
    Planning,
    Executing,
    Result,
    Error,
}

// ============================================================================
// MCP LIVE DISPATCHER
// ============================================================================

/// MCP Live Dispatcher for streaming and agent operations
pub struct McpLiveDispatcher {
    tool_registry: Arc<ToolRegistry>,
    agents: Vec<LiveAgent>,
    server_name: String,
}

impl McpLiveDispatcher {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            agents: Vec::new(),
            server_name: "op-dbus-live".to_string(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = name.into();
        self
    }

    pub fn register_agent(&mut self, agent: LiveAgent) {
        self.agents.push(agent);
    }

    /// Handle an MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        tracing::debug!(method = %request.method, "Handling MCP Live request");

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id).await,
            "initialized" | "notifications/initialized" => McpResponse::success(request.id, json!({})),
            "ping" => McpResponse::success(request.id, json!({})),
            "agents/list" => self.handle_agents_list(request.id).await,
            "agents/start" => self.handle_agent_start(request.id, request.params).await,
            "tools/list" => self.handle_tools_list(request.id).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,
            _ => McpResponse::error(request.id, McpError::method_not_found(&request.method)),
        }
    }

    async fn handle_initialize(&self, id: Option<Value>) -> McpResponse {
        tracing::info!("MCP Live Dispatcher initialized");

        McpResponse::success(id, json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": { "listChanged": true },
                "agents": { "streaming": true }
            },
            "serverInfo": {
                "name": self.server_name,
                "version": SERVER_VERSION
            }
        }))
    }

    async fn handle_agents_list(&self, id: Option<Value>) -> McpResponse {
        let agents: Vec<_> = self.agents.iter()
            .filter(|a| a.enabled)
            .map(|a| json!({
                "id": a.id,
                "name": a.name,
                "description": a.description,
                "capabilities": a.capabilities
            }))
            .collect();

        McpResponse::success(id, json!({
            "agents": agents
        }))
    }

    async fn handle_agent_start(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let params = match params {
            Some(p) => p,
            None => return McpResponse::error(id, McpError::invalid_params("Missing params")),
        };

        let agent_id = match params.get("agent_id").and_then(|a| a.as_str()) {
            Some(a) => a,
            None => return McpResponse::error(id, McpError::invalid_params("Missing agent_id")),
        };

        // Find the agent
        if self.agents.iter().any(|a| a.id == agent_id && a.enabled) {
            tracing::info!(agent = %agent_id, "Starting agent");
            McpResponse::success(id, json!({
                "status": "started",
                "agent_id": agent_id
            }))
        } else {
            McpResponse::error(id, McpError::new(-32002, format!("Agent not found: {}", agent_id)))
        }
    }

    async fn handle_tools_list(&self, id: Option<Value>) -> McpResponse {
        let tools: Vec<_> = self.tool_registry.list().await
            .iter()
            .map(|t| json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema
            }))
            .collect();

        McpResponse::success(id, json!({
            "tools": tools
        }))
    }

    async fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let params = match params {
            Some(p) => p,
            None => return McpResponse::error(id, McpError::invalid_params("Missing params")),
        };

        let tool_name = match params.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => return McpResponse::error(id, McpError::invalid_params("Missing tool name")),
        };

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let tool = match self.tool_registry.get(tool_name).await {
            Some(t) => t,
            None => {
                return McpResponse::success(id, json!({
                    "content": [{ "type": "text", "text": format!("Tool not found: {}", tool_name) }],
                    "isError": true
                }));
            }
        };

        match tool.execute(arguments).await {
            Ok(output) => {
                McpResponse::success(id, json!({
                    "content": [{
                        "type": "text",
                        "text": simd_json::to_string_pretty(&output).unwrap_or_default()
                    }],
                    "isError": false
                }))
            }
            Err(e) => {
                McpResponse::error(id, McpError::internal(e.to_string()))
            }
        }
    }

    /// Get registered agents
    pub fn agents(&self) -> &[LiveAgent] {
        &self.agents
    }

    /// Create a cognitive stream for an agent
    pub fn create_cognitive_stream(&self) -> (mpsc::Sender<CognitiveChunk>, CognitiveStream) {
        let (tx, rx) = mpsc::channel(100);
        (tx, CognitiveStream::new(rx))
    }
}