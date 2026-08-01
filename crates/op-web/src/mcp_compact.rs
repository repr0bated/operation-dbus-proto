//! MCP Compact Mode Implementation
//!
//! Provides 4 meta-tools instead of exposing all tools directly:
//! - list_tools: Browse available tools with pagination
//! - search_tools: Search tools by keyword
//! - get_tool_schema: Get input schema for a specific tool
//! - execute_tool: Execute any tool by name
//!
//! This allows LLMs to work with 750+ tools without exceeding context limits.

use axum::{
    extract::{Extension, Json},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::convert::Infallible;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use op_state_store::execution_job::{ExecutionJob, ExecutionResult, ExecutionStatus};
use uuid::Uuid;

use crate::AppState;

/// JSON-RPC request structure
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC response structure
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// Compact mode meta-tool definitions
fn get_compact_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "list_tools",
            "description": "List all available tools. Use pagination for large tool sets. Returns tool names and descriptions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Filter by category (e.g., 'networking', 'system', 'database')"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Pagination offset (default: 0)",
                        "default": 0
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum tools to return (default: 50, max: 100)",
                        "default": 50
                    }
                },
                "required": []
            }
        }),
        json!({
            "name": "search_tools",
            "description": "Search for tools by keyword in name or description. Returns matching tools.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (searches in tool name and description)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results (default: 20)",
                        "default": 20
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "get_tool_schema",
            "description": "Get the full input schema for a specific tool. Use this before calling execute_tool to understand required parameters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool to get schema for"
                    }
                },
                "required": ["tool_name"]
            }
        }),
        json!({
            "name": "execute_tool",
            "description": "Execute any tool by name with the provided arguments. First use get_tool_schema to understand the required input format.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool to execute"
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments to pass to the tool (must match tool's input schema)"
                    }
                },
                "required": ["tool_name"]
            }
        }),
    ]
}

/// SSE endpoint for compact MCP mode
/// Sends initial endpoint event then keeps connection alive
pub async fn mcp_compact_sse_handler(
    headers: HeaderMap,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("MCP Compact SSE client connected");

    // Get host from headers
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    // Determine scheme from forwarded headers
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");

    let post_url = format!("{}://{}/mcp/compact/message", scheme, host);
    info!("MCP Compact POST endpoint: {}", post_url);

    // Create initial endpoint event (required by MCP SSE transport spec)
    let endpoint_event = Event::default().event("endpoint").data(&post_url);

    // Stream the endpoint event
    let stream = stream::once(async move { Ok(endpoint_event) });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// POST endpoint for compact MCP JSON-RPC messages
/// Returns proper JSON-RPC responses, never HTML
pub async fn mcp_compact_message_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<JsonRpcRequest>,
) -> Response {
    debug!(
        "MCP Compact request: method={} id={}",
        request.method, request.id
    );

    let response = match request.method.as_str() {
        "initialize" => handle_initialize(&request),
        "initialized" => JsonRpcResponse::success(request.id.clone(), json!({})),
        "tools/list" => handle_tools_list(&request),
        "tools/call" => handle_tools_call(&state, &request).await,
        "ping" => JsonRpcResponse::success(request.id.clone(), json!({})),
        "notifications/initialized" => {
            // This is a notification, no response needed but we'll acknowledge
            JsonRpcResponse::success(request.id.clone(), json!({}))
        }
        _ => {
            warn!("Unknown MCP method: {}", request.method);
            JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            )
        }
    };

    // Always return JSON with correct content type
    let json_body = simd_json::to_string(&response).unwrap_or_else(|e| {
        error!("Failed to serialize response: {}", e);
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error"}}"#
            .to_string()
    });

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(json_body.into())
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response())
}

/// Handle initialize request
fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
    info!("MCP Compact initialize request");
    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "op-dbus-compact",
                "version": "1.0.0"
            },
            "instructions": "Compact MCP server with 4 meta-tools: list_tools (browse tools), search_tools (find tools), get_tool_schema (get tool details), execute_tool (run any tool)."
        }),
    )
}

/// Handle tools/list - returns only the 4 meta-tools
fn handle_tools_list(request: &JsonRpcRequest) -> JsonRpcResponse {
    info!("MCP Compact tools/list request");
    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "tools": get_compact_tools()
        }),
    )
}

/// Handle tools/call - execute meta-tools
async fn handle_tools_call(state: &Arc<AppState>, request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = &request.params;

    // MCP protocol: params = { "name": "tool_name", "arguments": { ... } }
    let tool_name = match params.get("name").and_then(|v| v.as_str()) {
        Some(name) => name,
        None => {
            error!("Missing tool name in params: {:?}", params);
            return JsonRpcResponse::error(
                request.id.clone(),
                -32602,
                "Missing required parameter: name".to_string(),
            );
        }
    };

    // Arguments can be missing (defaults to empty object)
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    info!(
        "MCP Compact tool call: {} with args: {}",
        tool_name, arguments
    );

    // Execute the meta-tool (no security needed for meta-tools themselves)
    let result = match tool_name {
        "list_tools" => execute_list_tools(&state.tool_registry, &arguments).await,
        "search_tools" => execute_search_tools(&state.tool_registry, &arguments).await,
        "get_tool_schema" => execute_get_tool_schema(&state.tool_registry, &arguments).await,
        "execute_tool" => execute_execute_tool(state, &arguments).await,
        _ => Err(format!("Unknown compact tool: {}. Available: list_tools, search_tools, get_tool_schema, execute_tool", tool_name)),
    };

    match result {
        Ok(content) => {
            let text =
                simd_json::to_string_pretty(&content).unwrap_or_else(|_| content.to_string());
            JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                }),
            )
        }
        Err(e) => {
            error!("Tool execution error: {}", e);
            JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: {}", e)
                    }],
                    "isError": true
                }),
            )
        }
    }
}

/// Execute list_tools meta-tool
async fn execute_list_tools(
    registry: &Arc<op_tools::ToolRegistry>,
    args: &Value,
) -> Result<Value, String> {
    let category = args.get("category").and_then(|v| v.as_str());
    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .min(100) as usize;

    let all_tools = registry.list().await;

    // Filter by category if specified
    let filtered: Vec<_> = if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        all_tools
            .iter()
            .filter(|t| t.category.to_lowercase().contains(&cat_lower))
            .collect()
    } else {
        all_tools.iter().collect()
    };

    let total = filtered.len();
    let tools: Vec<Value> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "category": t.category
            })
        })
        .collect();

    let returned = tools.len();
    Ok(json!({
        "total": total,
        "offset": offset,
        "limit": limit,
        "returned": returned,
        "tools": tools,
        "has_more": offset + returned < total
    }))
}

/// Execute search_tools meta-tool
async fn execute_search_tools(
    registry: &Arc<op_tools::ToolRegistry>,
    args: &Value,
) -> Result<Value, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: query")?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let query_lower = query.to_lowercase();
    let all_tools = registry.list().await;

    let matches: Vec<Value> = all_tools
        .iter()
        .filter(|t| {
            t.name.to_lowercase().contains(&query_lower)
                || t.description.to_lowercase().contains(&query_lower)
        })
        .take(limit)
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "category": t.category
            })
        })
        .collect();

    Ok(json!({
        "query": query,
        "count": matches.len(),
        "tools": matches
    }))
}

/// Execute get_tool_schema meta-tool
async fn execute_get_tool_schema(
    registry: &Arc<op_tools::ToolRegistry>,
    args: &Value,
) -> Result<Value, String> {
    let tool_name = args
        .get("tool_name")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: tool_name")?;

    let all_tools = registry.list().await;
    let tool = all_tools
        .iter()
        .find(|t| t.name == tool_name)
        .ok_or_else(|| format!("Tool not found: {}", tool_name))?;

    Ok(json!({
        "name": tool.name,
        "description": tool.description,
        "category": tool.category,
        "inputSchema": tool.input_schema
    }))
}

/// Execute execute_tool meta-tool - runs any underlying tool
async fn execute_execute_tool(state: &Arc<AppState>, args: &Value) -> Result<Value, String> {
    let registry = &state.tool_registry;
    let tool_name = args
        .get("tool_name")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: tool_name")?;
    let arguments = args.get("arguments").cloned().unwrap_or(json!({}));

    info!(
        "Executing underlying tool: {} with args: {}",
        tool_name, arguments
    );

    // Create ExecutionJob for tracking
    let job_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let mut job = ExecutionJob {
        id: job_id,
        tool_name: tool_name.to_string(),
        arguments: arguments.clone(),
        status: ExecutionStatus::Running,
        created_at: now,
        updated_at: now,
        result: None,
    };

    // Save initial state to state store (audit log)
    if let Err(e) = state.state_store.save_job(&job).await {
        error!("Failed to save execution job start to state store: {}", e);
        // Continue execution even if logging fails?
        // For high security, we might want to fail, but for now we log and proceed.
    }

    // Find and execute the tool
    let tool_result = match registry.get(tool_name).await {
        Some(tool) => tool.execute(arguments).await,
        None => Err(anyhow::anyhow!(
            "Tool not found: {}. Use list_tools or search_tools to find available tools.",
            tool_name
        )),
    };

    // Update Job with result
    job.updated_at = chrono::Utc::now();

    match tool_result {
        Ok(res) => {
            job.status = ExecutionStatus::Completed;
            job.result = Some(ExecutionResult {
                success: true,
                output: Some(res.clone()),
                error: None,
            });
            if let Err(e) = state.state_store.update_job(&job).await {
                error!("Failed to update execution job success: {}", e);
            }

            Ok(json!({
                "tool": tool_name,
                "success": true,
                "result": res
            }))
        }
        Err(e) => {
            job.status = ExecutionStatus::Failed;
            job.result = Some(ExecutionResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
            });
            if let Err(log_err) = state.state_store.update_job(&job).await {
                error!("Failed to update execution job failure: {}", log_err);
            }

            error!("Tool {} execution failed: {}", tool_name, e);
            Ok(json!({
                "tool": tool_name,
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}
