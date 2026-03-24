//! Compact Mode
//!
//! Provides 4 meta-tools for discovering and executing 148+ tools:
//! - list_tools: Browse available tools with filtering
//! - search_tools: Search tools by keyword
//! - get_tool_schema: Get input schema for a specific tool
//! - execute_tool: Execute any tool by name
//!
//! This mode saves ~95% of context tokens compared to exposing all tools.

use crate::{JsonRpcError, McpRequest, McpResponse, ToolExecutor};
use anyhow::Result;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Session context passed through from gateway
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    pub session_id: Option<String>,
    pub is_controller: bool,
    pub peer_pubkey: Option<String>,
}

/// Compact server wraps a tool executor and exposes 4 meta-tools
pub struct CompactServer {
    executor: Arc<dyn ToolExecutor>,
    server_name: String,
    session: RwLock<SessionContext>,
}

impl CompactServer {
    pub fn new(executor: Arc<dyn ToolExecutor>) -> Self {
        Self {
            executor,
            server_name: "op-mcp-compact".to_string(),
            session: RwLock::new(SessionContext::default()),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = name.into();
        self
    }

    /// Set session context (called by gateway after auth)
    pub async fn set_session(&self, ctx: SessionContext) {
        info!(
            session_id = ?ctx.session_id,
            is_controller = %ctx.is_controller,
            "Session context set"
        );
        *self.session.write().await = ctx;
    }

    /// Check if current session can execute controller-only tools
    pub async fn can_execute_controller_tools(&self) -> bool {
        self.session.read().await.is_controller
    }

    /// Handle MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        debug!(method = %request.method, "Handling compact MCP request");

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "initialized" => McpResponse::success(request.id, json!({})),
            "ping" => McpResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            "notifications/initialized" => McpResponse::success(request.id, json!({})),
            _ => McpResponse::error(request.id, JsonRpcError::method_not_found(&request.method)),
        }
    }

    async fn handle_initialize(&self, request: McpRequest) -> McpResponse {
        info!("Compact MCP initialized");

        McpResponse::success(
            request.id,
            json!({
                "protocolVersion": crate::PROTOCOL_VERSION,
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": self.server_name,
                    "version": crate::SERVER_VERSION
                },
                "instructions": "This server uses compact mode with 4 meta-tools. Use list_tools to discover available tools, get_tool_schema to get the input schema, then execute_tool to run any tool."
            }),
        )
    }

    async fn handle_tools_list(&self, request: McpRequest) -> McpResponse {
        McpResponse::success(
            request.id,
            json!({
                "tools": compact_tools_schema(),
                "_meta": { "compactMode": true }
            }),
        )
    }

    async fn handle_tools_call(&self, request: McpRequest) -> McpResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return McpResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing params"),
                )
            }
        };

        let tool_name = match params
            .as_object()
            .and_then(|o| o.get("name"))
            .and_then(|n| n.as_str())
        {
            Some(n) => n,
            None => {
                return McpResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing tool name"),
                )
            }
        };

        let arguments = params
            .as_object()
            .and_then(|o| o.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        // Route to meta-tool handlers
        match tool_name {
            "list_tools" => self.meta_list_tools(request.id, arguments).await,
            "search_tools" => self.meta_search_tools(request.id, arguments).await,
            "get_tool_schema" => self.meta_get_tool_schema(request.id, arguments).await,
            "execute_tool" => self.meta_execute_tool(request.id, arguments).await,
            _ => McpResponse::error(
                request.id,
                JsonRpcError::new(-32001, format!(
                    "Unknown meta-tool: {}. Use list_tools, search_tools, get_tool_schema, or execute_tool.",
                    tool_name
                )),
            ),
        }
    }

    async fn meta_list_tools(&self, id: Option<Value>, args: Value) -> McpResponse {
        let category = args
            .as_object()
            .and_then(|o| o.get("category"))
            .and_then(|c| c.as_str());
        let limit = args
            .as_object()
            .and_then(|o| o.get("limit"))
            .and_then(|l| l.as_u64())
            .unwrap_or(20) as usize;
        let offset = args
            .as_object()
            .and_then(|o| o.get("offset"))
            .and_then(|o| o.as_u64())
            .unwrap_or(0) as usize;

        match self.executor.list_tools().await {
            Ok(tools) => {
                let filtered: Vec<_> = tools
                    .into_iter()
                    .filter(|t| {
                        category
                            .map(|c| {
                                t.name.contains(c)
                                    || t.description.to_lowercase().contains(&c.to_lowercase())
                            })
                            .unwrap_or(true)
                    })
                    .skip(offset)
                    .take(limit)
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description
                        })
                    })
                    .collect();

                let total = filtered.len();

                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": simd_json::to_string_pretty(&json!({
                                "tools": filtered,
                                "count": total,
                                "offset": offset,
                                "limit": limit
                            })).unwrap()
                        }],
                        "isError": false
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "Failed to list tools");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                        "isError": true
                    }),
                )
            }
        }
    }

    async fn meta_search_tools(&self, id: Option<Value>, args: Value) -> McpResponse {
        let query = args
            .as_object()
            .and_then(|o| o.get("query"))
            .and_then(|q| q.as_str())
            .unwrap_or("");
        let limit = args
            .as_object()
            .and_then(|o| o.get("limit"))
            .and_then(|l| l.as_u64())
            .unwrap_or(10) as usize;

        match self.executor.search_tools(query, limit).await {
            Ok(tools) => {
                let results: Vec<_> = tools
                    .into_iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description
                        })
                    })
                    .collect();

                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": simd_json::to_string_pretty(&json!({
                                "query": query,
                                "results": results,
                                "count": results.len()
                            })).unwrap()
                        }],
                        "isError": false
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "Failed to search tools");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                        "isError": true
                    }),
                )
            }
        }
    }

    async fn meta_get_tool_schema(&self, id: Option<Value>, args: Value) -> McpResponse {
        let tool_name = args
            .as_object()
            .and_then(|o| o.get("tool_name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");

        if tool_name.is_empty() {
            return McpResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": "Error: tool_name is required" }],
                    "isError": true
                }),
            );
        }

        match self.executor.get_tool_schema(tool_name).await {
            Ok(Some(schema)) => McpResponse::success(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": simd_json::to_string_pretty(&json!({
                            "tool": tool_name,
                            "schema": schema
                        })).unwrap()
                    }],
                    "isError": false
                }),
            ),
            Ok(None) => McpResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("Tool not found: {}", tool_name) }],
                    "isError": true
                }),
            ),
            Err(e) => {
                error!(error = %e, "Failed to get tool schema");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                        "isError": true
                    }),
                )
            }
        }
    }

    async fn meta_execute_tool(&self, id: Option<Value>, args: Value) -> McpResponse {
        let tool_name = args
            .as_object()
            .and_then(|o| o.get("tool_name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let arguments = args
            .as_object()
            .and_then(|o| o.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        if tool_name.is_empty() {
            return McpResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": "Error: tool_name is required" }],
                    "isError": true
                }),
            );
        }

        info!(tool = %tool_name, "Executing tool via compact mode");

        match self.executor.execute_tool(tool_name, arguments).await {
            Ok(result) => {
                let text = simd_json::to_string_pretty(&result).unwrap_or_default();
                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": text
                        }],
                        "isError": false
                    }),
                )
            }
            Err(e) => {
                error!(tool = %tool_name, error = %e, "Tool execution failed");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Error executing {}: {}", tool_name, e)
                        }],
                        "isError": true
                    }),
                )
            }
        }
    }
}

/// Get the 4 compact meta-tool schemas
pub fn compact_tools_schema() -> Vec<Value> {
    vec![
        json!({
            "name": "list_tools",
            "description": "List available tools. Filter by category. Returns tool names and descriptions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Filter by category (e.g., 'ovs', 'dbus', 'file', 'agent')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum tools to return",
                        "default": 20
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Offset for pagination",
                        "default": 0
                    }
                }
            }
        }),
        json!({
            "name": "search_tools",
            "description": "Search tools by keyword in name or description.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results",
                        "default": 10
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "get_tool_schema",
            "description": "Get the input schema for a specific tool. Call this before execute_tool to know the required arguments.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool"
                    }
                },
                "required": ["tool_name"]
            }
        }),
        json!({
            "name": "execute_tool",
            "description": "Execute any tool by name with arguments. First use get_tool_schema to see required arguments.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool to execute"
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments to pass to the tool"
                    }
                },
                "required": ["tool_name"]
            }
        }),
    ]
}

/// Run compact server in stdio mode
pub async fn run_compact_stdio_server() -> Result<()> {
    use crate::transport::{StdioTransport, Transport};
    use crate::DefaultToolExecutor;

    // Initialize logging to stderr
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Create tool registry and executor
    let registry = Arc::new(op_tools::ToolRegistry::new());
    op_tools::register_builtin_tools(&registry).await?;

    let executor: Arc<dyn ToolExecutor> = Arc::new(DefaultToolExecutor::new(registry));
    let server = Arc::new(CompactServer::new(executor));

    info!("Starting compact MCP server (stdio)");

    StdioTransport::new().serve(server).await
}

// Implement McpHandler for CompactServer
#[async_trait::async_trait]
impl crate::transport::McpHandler for CompactServer {
    async fn handle_request(&self, request: McpRequest) -> McpResponse {
        self.handle_request(request).await
    }
}
