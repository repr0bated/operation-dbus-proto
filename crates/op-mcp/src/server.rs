//! Unified MCP Server
//!
//! Core server implementation that handles all MCP protocol logic.
//! Transport-agnostic - works with stdio, HTTP, WebSocket, gRPC, etc.

use crate::protocol::{JsonRpcError, McpRequest, McpResponse};
use crate::resources::ResourceRegistry;
use crate::{PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Server name override
    pub name: Option<String>,
    /// Enable compact mode (4 meta-tools instead of all tools)
    pub compact_mode: bool,
    /// Tool categories to expose (None = all)
    pub allowed_categories: Option<Vec<String>>,
    /// Tool name patterns to block
    pub blocked_patterns: Vec<String>,
    /// Maximum tools to return in list
    pub max_tools: usize,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: None,
            compact_mode: false,
            allowed_categories: None,
            blocked_patterns: vec![
                "shell_execute".into(),
                "write_file".into(),
                "systemd_start".into(),
                "systemd_stop".into(),
                "systemd_restart".into(),
                "systemd_enable".into(),
                "systemd_disable".into(),
            ],
            max_tools: 500,
        }
    }
}

/// Tool information for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
}

/// Tool executor trait - implement this to provide tools
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// List available tools
    async fn list_tools(&self) -> Result<Vec<ToolInfo>>;

    /// Execute a tool by name
    async fn execute_tool(&self, name: &str, arguments: Value) -> Result<Value>;

    /// Get schema for a specific tool
    async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>>;

    /// Search tools by query
    async fn search_tools(&self, query: &str, limit: usize) -> Result<Vec<ToolInfo>>;
}

/// Default tool executor using op_tools registry
pub struct DefaultToolExecutor {
    registry: Arc<op_tools::ToolRegistry>,
}

impl DefaultToolExecutor {
    pub fn new(registry: Arc<op_tools::ToolRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for DefaultToolExecutor {
    async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.list().await;
        Ok(tools
            .into_iter()
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }

    async fn execute_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        if let Some(tool) = self.registry.get(name).await {
            tool.execute(arguments).await
        } else {
            Err(anyhow::anyhow!("Tool not found: {}", name))
        }
    }

    async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>> {
        if let Some(def) = self.registry.get_definition(name).await {
            Ok(Some(def.input_schema))
        } else {
            Ok(None)
        }
    }

    async fn search_tools(&self, query: &str, limit: usize) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.list().await;
        let query_lower = query.to_lowercase();
        Ok(tools
            .into_iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&query_lower)
                    || t.description.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }
}

/// Unified MCP Server
pub struct McpServer {
    config: McpServerConfig,
    tool_executor: Arc<dyn ToolExecutor>,
    resources: ResourceRegistry,
    /// Client info from last initialize
    client_info: RwLock<Option<ClientInfo>>,
    /// Session data
    sessions: RwLock<HashMap<String, SessionData>>,
}

#[derive(Debug, Clone)]
struct ClientInfo {
    name: String,
    version: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct SessionData {
    initialized: bool,
    compact_mode: bool,
}

impl McpServer {
    /// Create server with default tool executor
    pub async fn new(config: McpServerConfig) -> Result<Arc<Self>> {
        let registry = Arc::new(op_tools::ToolRegistry::new());
        op_tools::register_builtin_tools(&registry).await?;

        let tool_executor = Arc::new(DefaultToolExecutor::new(registry));
        Ok(Arc::new(Self::with_executor(config, tool_executor)))
    }

    /// Create server with custom tool executor
    pub fn with_executor(config: McpServerConfig, tool_executor: Arc<dyn ToolExecutor>) -> Self {
        Self {
            config,
            tool_executor,
            resources: ResourceRegistry::new(),
            client_info: RwLock::new(None),
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Handle an MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        debug!(method = %request.method, "Handling MCP request");

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "initialized" => self.handle_initialized(request).await,
            "ping" => McpResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request).await,
            "resources/read" => self.handle_resources_read(request).await,
            // Compact mode meta-tools
            "list_tools" | "search_tools" | "get_tool_schema" | "execute_tool" => {
                self.handle_compact_tool(request).await
            }
            _ => McpResponse::error(request.id, JsonRpcError::method_not_found(&request.method)),
        }
    }

    async fn handle_initialize(&self, request: McpRequest) -> McpResponse {
        // Extract client info
        let client_name = request
            .params
            .as_ref()
            .and_then(|p| p.as_object())
            .and_then(|obj| obj.get("clientInfo"))
            .and_then(|ci| ci.as_object())
            .and_then(|ci_obj| ci_obj.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");

        let client_version = request
            .params
            .as_ref()
            .and_then(|p| p.as_object())
            .and_then(|obj| obj.get("clientInfo"))
            .and_then(|ci| ci.as_object())
            .and_then(|ci_obj| ci_obj.get("version"))
            .and_then(|v| v.as_str());

        // Store client info
        *self.client_info.write().await = Some(ClientInfo {
            name: client_name.to_string(),
            version: client_version.map(String::from),
        });

        // Auto-detect compact mode for known clients
        let use_compact = self.config.compact_mode || Self::should_use_compact_mode(client_name);

        info!(
            client = %client_name,
            version = %client_version.unwrap_or("?"),
            compact = %use_compact,
            "Client connected"
        );

        let server_name = self.config.name.as_deref().unwrap_or(SERVER_NAME);

        McpResponse::success(
            request.id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false }
                },
                "serverInfo": {
                    "name": server_name,
                    "version": SERVER_VERSION
                },
                "_meta": {
                    "compactMode": use_compact
                }
            }),
        )
    }

    async fn handle_initialized(&self, request: McpRequest) -> McpResponse {
        McpResponse::success(request.id, json!({}))
    }

    async fn handle_tools_list(&self, request: McpRequest) -> McpResponse {
        // Check if compact mode
        let client_info = self.client_info.read().await;
        let use_compact = self.config.compact_mode
            || client_info
                .as_ref()
                .map(|c| Self::should_use_compact_mode(&c.name))
                .unwrap_or(false);

        if use_compact {
            return self.get_compact_tools_response(request.id).await;
        }

        // Full mode - return all tools
        match self.tool_executor.list_tools().await {
            Ok(tools) => {
                let filtered: Vec<_> = tools
                    .into_iter()
                    .filter(|t| !self.is_tool_blocked(&t.name))
                    .take(self.config.max_tools)
                    .collect();

                McpResponse::success(
                    request.id,
                    json!({
                        "tools": filtered
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "Failed to list tools");
                McpResponse::error(request.id, JsonRpcError::internal_error(e.to_string()))
            }
        }
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
            .and_then(|obj| obj.get("name"))
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

        // Check if blocked
        if self.is_tool_blocked(tool_name) {
            warn!(tool = %tool_name, "Blocked tool execution attempt");
            return McpResponse::error(
                request.id,
                JsonRpcError::new(-32001, format!("Tool '{}' is not available", tool_name)),
            );
        }

        let mut arguments = params
            .as_object()
            .and_then(|obj| obj.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        // Inject code context for smart suggestions (if op-tools has code_search)
        #[cfg(feature = "code_search")]
        {
            let current_file = arguments
                .get("path")
                .and_then(|p| p.as_str())
                .or(arguments.get("file").and_then(|f| f.as_str()));

            if let Ok(code_context) =
                op_tools::code_search::inject_code_context(tool_name, &arguments, current_file)
                    .await
            {
                if !code_context.is_empty() {
                    arguments["_code_context"] = code_context.to_json();
                    debug!(tool = %tool_name, "Injected code context");
                }
            }
        }

        match self.tool_executor.execute_tool(tool_name, arguments).await {
            Ok(result) => McpResponse::success(
                request.id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": simd_json::to_string_pretty(&result).unwrap_or_default()
                    }],
                    "isError": false
                }),
            ),
            Err(e) => McpResponse::success(
                request.id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: {}", e)
                    }],
                    "isError": true
                }),
            ),
        }
    }

    async fn handle_resources_list(&self, request: McpRequest) -> McpResponse {
        let resources: Vec<_> = self
            .resources
            .list_resources()
            .iter()
            .map(|r| {
                json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type
                })
            })
            .collect();

        McpResponse::success(request.id, json!({ "resources": resources }))
    }

    async fn handle_resources_read(&self, request: McpRequest) -> McpResponse {
        let uri = request
            .params
            .as_ref()
            .and_then(|p| p.as_object())
            .and_then(|obj| obj.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");

        if uri.is_empty() {
            return McpResponse::error(request.id, JsonRpcError::invalid_params("Missing uri"));
        }

        match self.resources.read_resource(uri).await {
            Some(content) => McpResponse::success(
                request.id,
                json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "text/plain",
                        "text": content
                    }]
                }),
            ),
            None => McpResponse::error(
                request.id,
                JsonRpcError::new(-32002, format!("Resource not found: {}", uri)),
            ),
        }
    }

    /// Handle compact mode meta-tools
    async fn handle_compact_tool(&self, request: McpRequest) -> McpResponse {
        let params = request.params.as_ref().cloned().unwrap_or(json!({}));

        match request.method.as_str() {
            "list_tools" => {
                let category = params
                    .as_object()
                    .and_then(|obj| obj.get("category"))
                    .and_then(|c| c.as_str());
                let limit = params
                    .as_object()
                    .and_then(|obj| obj.get("limit"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(20) as usize;

                match self.tool_executor.list_tools().await {
                    Ok(tools) => {
                        let filtered: Vec<_> = tools
                            .into_iter()
                            .filter(|t| !self.is_tool_blocked(&t.name))
                            .filter(|t| {
                                category
                                    .map(|c| {
                                        t.name.contains(c)
                                            || t.description
                                                .to_lowercase()
                                                .contains(&c.to_lowercase())
                                    })
                                    .unwrap_or(true)
                            })
                            .take(limit)
                            .map(|t| {
                                json!({
                                    "name": t.name,
                                    "description": t.description
                                })
                            })
                            .collect();

                        McpResponse::success(
                            request.id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": simd_json::to_string_pretty(&filtered).unwrap()
                                }],
                                "isError": false
                            }),
                        )
                    }
                    Err(e) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                            "isError": true
                        }),
                    ),
                }
            }
            "search_tools" => {
                let query = params
                    .as_object()
                    .and_then(|obj| obj.get("query"))
                    .and_then(|q| q.as_str())
                    .unwrap_or("");
                let limit = params
                    .as_object()
                    .and_then(|obj| obj.get("limit"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(10) as usize;

                match self.tool_executor.search_tools(query, limit).await {
                    Ok(tools) => {
                        let results: Vec<_> = tools
                            .into_iter()
                            .filter(|t| !self.is_tool_blocked(&t.name))
                            .map(|t| {
                                json!({
                                    "name": t.name,
                                    "description": t.description
                                })
                            })
                            .collect();

                        McpResponse::success(
                            request.id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": simd_json::to_string_pretty(&results).unwrap()
                                }],
                                "isError": false
                            }),
                        )
                    }
                    Err(e) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                            "isError": true
                        }),
                    ),
                }
            }
            "get_tool_schema" => {
                let tool_name = params
                    .as_object()
                    .and_then(|obj| obj.get("tool_name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");

                match self.tool_executor.get_tool_schema(tool_name).await {
                    Ok(Some(schema)) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{
                                "type": "text",
                                "text": simd_json::to_string_pretty(&schema).unwrap()
                            }],
                            "isError": false
                        }),
                    ),
                    Ok(None) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Tool not found: {}", tool_name) }],
                            "isError": true
                        }),
                    ),
                    Err(e) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                            "isError": true
                        }),
                    ),
                }
            }
            "execute_tool" => {
                let tool_name = params
                    .as_object()
                    .and_then(|obj| obj.get("tool_name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let arguments = params
                    .as_object()
                    .and_then(|obj| obj.get("arguments"))
                    .cloned()
                    .unwrap_or(json!({}));

                // Delegate to tools/call logic
                let call_request = McpRequest {
                    jsonrpc: "2.0".into(),
                    id: request.id.clone(),
                    method: "tools/call".into(),
                    params: Some(json!({
                        "name": tool_name,
                        "arguments": arguments
                    })),
                };
                self.handle_tools_call(call_request).await
            }
            _ => McpResponse::error(request.id, JsonRpcError::method_not_found(&request.method)),
        }
    }

    /// Get compact mode tools response
    async fn get_compact_tools_response(&self, id: Option<Value>) -> McpResponse {
        let compact_tools = vec![
            json!({
                "name": "list_tools",
                "description": "List available tools. Filter by 'category'. Returns names and descriptions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "category": { "type": "string", "description": "Filter by category" },
                        "limit": { "type": "integer", "description": "Max tools (default: 20)" }
                    }
                }
            }),
            json!({
                "name": "search_tools",
                "description": "Search tools by keyword.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results (default: 10)" }
                    },
                    "required": ["query"]
                }
            }),
            json!({
                "name": "get_tool_schema",
                "description": "Get input schema for a tool. Call before execute_tool.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string", "description": "Tool name" }
                    },
                    "required": ["tool_name"]
                }
            }),
            json!({
                "name": "execute_tool",
                "description": "Execute a tool by name.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string", "description": "Tool name" },
                        "arguments": { "type": "object", "description": "Tool arguments" }
                    },
                    "required": ["tool_name"]
                }
            }),
        ];

        McpResponse::success(
            id,
            json!({
                "tools": compact_tools,
                "_meta": { "compactMode": true }
            }),
        )
    }

    /// Check if a tool should be blocked
    fn is_tool_blocked(&self, name: &str) -> bool {
        self.config
            .blocked_patterns
            .iter()
            .any(|p| name.contains(p))
    }

    /// Check if client should use compact mode
    fn should_use_compact_mode(client_name: &str) -> bool {
        let name_lower = client_name.to_lowercase();
        name_lower.contains("gemini")
            || name_lower.contains("claude")
            || name_lower.contains("cursor")
    }

    /// Get tool executor reference
    pub fn tool_executor(&self) -> &Arc<dyn ToolExecutor> {
        &self.tool_executor
    }
}
