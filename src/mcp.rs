//! MCP (Model Context Protocol) Compact Dispatcher
//!
//! Provides 4 meta-tools for discovering and executing all tools:
//! - list_tools: Browse available tools with filtering
//! - search_tools: Search tools by keyword
//! - get_tool_schema: Get input schema for a specific tool
//! - execute_tool: Execute any tool by name
//!
//! This saves ~95% of context tokens compared to exposing all tools directly.

use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::sync::Arc;

use op_tools::registry::ToolRegistry;
use crate::json_rpc::{JsonRpcRequest, JsonRpcError, error_codes};

/// MCP Protocol version
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server version
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// MCP REQUEST/RESPONSE
// ============================================================================

/// MCP Request (extends JSON-RPC)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    pub id: Option<Value>,
}

impl From<JsonRpcRequest> for McpRequest {
    fn from(req: JsonRpcRequest) -> Self {
        Self {
            jsonrpc: req.jsonrpc,
            method: req.method,
            params: if req.params.is_null() { None } else { Some(req.params) },
            id: Some(req.id),
        }
    }
}

/// MCP Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
    pub id: Option<Value>,
}

impl McpResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Option<Value>, error: McpError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

/// MCP Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(error_codes::METHOD_NOT_FOUND, format!("Method not found: {}", method))
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self::new(error_codes::INVALID_PARAMS, msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(error_codes::INTERNAL_ERROR, msg)
    }
}

impl From<JsonRpcError> for McpError {
    fn from(e: JsonRpcError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            data: e.data,
        }
    }
}

// ============================================================================
// TOOL DEFINITION (for listing)
// ============================================================================

/// Simplified tool definition for MCP responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

// ============================================================================
// COMPACT DISPATCHER
// ============================================================================

/// MCP Compact Dispatcher - exposes 4 meta-tools
pub struct McpCompactDispatcher {
    tool_registry: Arc<ToolRegistry>,
    server_name: String,
}

impl McpCompactDispatcher {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            server_name: "op-dbus-mcp".to_string(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = name.into();
        self
    }

    /// Get a reference to the tool registry
    pub fn tool_registry_ref(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// Get the tool registry as Arc
    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Get the tool count
    pub async fn tool_count(&self) -> usize {
        self.tool_registry.len().await
    }

    /// Handle an MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        tracing::debug!(method = %request.method, "Handling MCP request");

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id).await,
            "initialized" | "notifications/initialized" => McpResponse::success(request.id, json!({})),
            "ping" => McpResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request.id).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,
            _ => McpResponse::error(request.id, McpError::method_not_found(&request.method)),
        }
    }

    async fn handle_initialize(&self, id: Option<Value>) -> McpResponse {
        tracing::info!("MCP Compact Dispatcher initialized");

        McpResponse::success(id, json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": { "listChanged": false }
            },
            "serverInfo": {
                "name": self.server_name,
                "version": SERVER_VERSION
            },
            "instructions": "This server uses compact mode with 4 meta-tools. Use list_tools to discover available tools, get_tool_schema to get the input schema, then execute_tool to run any tool."
        }))
    }

    async fn handle_tools_list(&self, id: Option<Value>) -> McpResponse {
        McpResponse::success(id, json!({
            "tools": compact_meta_tools_schema(),
            "_meta": { "compactMode": true }
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

        // Route to meta-tool handlers
        match tool_name {
            "list_tools" => self.meta_list_tools(id, arguments).await,
            "search_tools" => self.meta_search_tools(id, arguments).await,
            "get_tool_schema" => self.meta_get_tool_schema(id, arguments).await,
            "execute_tool" => self.meta_execute_tool(id, arguments).await,
            _ => McpResponse::error(
                id,
                McpError::new(-32001, format!(
                    "Unknown meta-tool: {}. Use list_tools, search_tools, get_tool_schema, or execute_tool.",
                    tool_name
                )),
            ),
        }
    }

    async fn meta_list_tools(&self, id: Option<Value>, args: Value) -> McpResponse {
        let category = args.get("category").and_then(|c| c.as_str());
        let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(20) as usize;
        let offset = args.get("offset").and_then(|o| o.as_u64()).unwrap_or(0) as usize;

        let all_tools = self.tool_registry.list().await;
        let filtered: Vec<_> = all_tools
            .iter()
            .filter(|t| {
                category.map(|c| {
                    t.name.contains(c) ||
                    t.description.to_lowercase().contains(&c.to_lowercase())
                }).unwrap_or(true)
            })
            .skip(offset)
            .take(limit)
            .map(|t| json!({
                "name": t.name,
                "description": t.description
            }))
            .collect();

        let total = filtered.len();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": simd_json::to_string_pretty(&json!({
                    "tools": filtered,
                    "count": total,
                    "offset": offset,
                    "limit": limit,
                    "total_available": all_tools.len()
                })).unwrap()
            }],
            "isError": false
        }))
    }

    async fn meta_search_tools(&self, id: Option<Value>, args: Value) -> McpResponse {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;
        let query_lower = query.to_lowercase();

        let all_tools = self.tool_registry.list().await;
        let results: Vec<_> = all_tools
            .iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&query_lower) ||
                t.description.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .map(|t| json!({
                "name": t.name,
                "description": t.description
            }))
            .collect();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": simd_json::to_string_pretty(&json!({
                    "query": query,
                    "results": results,
                    "count": results.len()
                })).unwrap()
            }],
            "isError": false
        }))
    }

    async fn meta_get_tool_schema(&self, id: Option<Value>, args: Value) -> McpResponse {
        let tool_name = args.get("tool_name").and_then(|n| n.as_str()).unwrap_or("");

        if tool_name.is_empty() {
            return McpResponse::success(id, json!({
                "content": [{ "type": "text", "text": "Error: tool_name is required" }],
                "isError": true
            }));
        }

        match self.tool_registry.get(tool_name).await {
            Some(tool) => {
                McpResponse::success(id, json!({
                    "content": [{
                        "type": "text",
                        "text": simd_json::to_string_pretty(&json!({
                            "tool": tool_name,
                            "description": tool.description(),
                            "inputSchema": tool.input_schema(),
                        })).unwrap()
                    }],
                    "isError": false
                }))
            }
            None => {
                McpResponse::success(id, json!({
                    "content": [{ "type": "text", "text": format!("Tool not found: {}", tool_name) }],
                    "isError": true
                }))
            }
        }
    }

    async fn meta_execute_tool(&self, id: Option<Value>, args: Value) -> McpResponse {
        let tool_name = args.get("tool_name").and_then(|n| n.as_str()).unwrap_or("");
        let arguments = args.get("arguments").cloned().unwrap_or(json!({}));

        if tool_name.is_empty() {
            return McpResponse::success(id, json!({
                "content": [{ "type": "text", "text": "Error: tool_name is required" }],
                "isError": true
            }));
        }

        tracing::info!(tool = %tool_name, "Executing tool via compact mode");

        let tool = match self.tool_registry.get(tool_name).await {
            Some(t) => t,
            None => {
                return McpResponse::success(id, json!({
                    "content": [{ "type": "text", "text": format!("Tool not found: {}", tool_name) }],
                    "isError": true
                }));
            }
        };

        // Note: tool.execute now takes 1 arg (input) and returns Result<Value>
        match tool.execute(arguments).await {
            Ok(output) => {
                let text = simd_json::to_string_pretty(&output).unwrap_or_default();
                McpResponse::success(id, json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }],
                    "isError": false
                }))
            }
            Err(e) => {
                tracing::error!(tool = %tool_name, error = %e, "Tool execution failed");
                McpResponse::success(id, json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error executing {}: {}", tool_name, e)
                    }],
                    "isError": true
                }))
            }
        }
    }
}

/// Get the 4 compact meta-tool schemas
pub fn compact_meta_tools_schema() -> Vec<Value> {
    vec![
        json!({
            "name": "list_tools",
            "description": "List available tools. Filter by category. Returns tool names and descriptions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Filter by category (e.g., 'ovs', 'dbus', 'file', 'service')"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_response_success() {
        let resp = McpResponse::success(Some(Value::Number(1.into())), json!({"ok": true}));
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_mcp_response_error() {
        let resp = McpResponse::error(Some(Value::Number(1.into())), McpError::new(-1, "test"));
        assert!(resp.error.is_some());
        assert!(resp.result.is_none());
    }

    #[test]
    fn test_compact_tools_schema() {
        let tools = compact_meta_tools_schema();
        assert_eq!(tools.len(), 4);
        
        let names: Vec<_> = tools.iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();
        assert!(names.contains(&"list_tools"));
        assert!(names.contains(&"search_tools"));
        assert!(names.contains(&"get_tool_schema"));
        assert!(names.contains(&"execute_tool"));
    }
}
