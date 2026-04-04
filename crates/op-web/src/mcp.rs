//! MCP Protocol Handler (Clean Implementation)
//!
//! Implements the Model Context Protocol (MCP) server endpoints.
//! - Standard Mode: Exposes all tools via `tools/list`
//! - Compact Mode: Exposes meta-tools via `mcp_compact` module
//! - SSE Support: For server-initiated events

use axum::{
    extract::{Extension, Json},
    response::sse::{Event, Sse},
    routing::{get, post},
    Router,
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{debug, error, info};

use crate::state::AppState;

/// SSE broadcaster for MCP responses
#[derive(Clone)]
pub struct SseBroadcaster {
    tx: broadcast::Sender<String>,
}

impl SseBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    pub fn broadcast(&self, data: &str) {
        let _ = self.tx.send(data.to_string());
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}

/// Global SSE broadcaster
lazy_static::lazy_static! {
    static ref GLOBAL_BROADCASTER: SseBroadcaster = SseBroadcaster::new();
}

#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Create MCP router
pub fn create_mcp_router() -> Router {
    Router::new()
        // Compact mode (Preferred)
        .route(
            "/compact",
            get(crate::mcp_compact::mcp_compact_sse_handler)
                .post(crate::mcp_compact::mcp_compact_message_handler),
        )
        .route(
            "/compact/message",
            post(crate::mcp_compact::mcp_compact_message_handler),
        )
        // Standard endpoints (all tools)
        .route("/", post(mcp_handler))
        .route("/sse", get(mcp_sse_handler))
        .route("/message", post(mcp_message_handler))
        // Configuration
        .route("/_config", get(config_handler))
}

/// Standard Handler (all tools)
async fn mcp_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<McpRequest>,
) -> Json<McpResponse> {
    let response = process_request(&state, request).await;
    Json(response)
}

/// JSON-RPC compatibility handler (1:1 mirror of MCP)
pub async fn jsonrpc_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<McpRequest>,
) -> Json<McpResponse> {
    let response = process_request(&state, request).await;
    Json(response)
}

/// SSE endpoint for MCP connections
async fn mcp_sse_handler(
    headers: axum::http::HeaderMap,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = GLOBAL_BROADCASTER.subscribe();

    info!("SSE client connected");

    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");

    let post_url = format!("{}://{}/mcp/message", scheme, host);

    // Initial endpoint event
    let endpoint_event = Event::default().event("endpoint").data(&post_url);

    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(data) => Some(Ok(Event::default().data(data))),
            Err(_) => None, // Skip lagged messages
        }
    });

    // Combine initial event with broadcast stream
    let combined_stream = stream::once(async move { Ok(endpoint_event) }).chain(stream);

    Sse::new(combined_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// Message handler for MCP requests (with SSE broadcasting)
async fn mcp_message_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<McpRequest>,
) -> Json<McpResponse> {
    info!("MCP message received (method: {})", request.method);

    let response = process_request(&state, request).await;

    // Broadcast to SSE clients
    if let Ok(json) = simd_json::to_string(&response) {
        GLOBAL_BROADCASTER.broadcast(&json);
    }

    Json(response)
}

/// Process a single MCP request
async fn process_request(state: &AppState, request: McpRequest) -> McpResponse {
    debug!("MCP request: {} (id: {:?})", request.method, request.id);

    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return McpResponse::error(request.id, -32600, "Invalid JSON-RPC version");
    }

    match request.method.as_str() {
        "initialize" => handle_initialize(request.id).await,
        "initialized" => handle_initialized(request.id).await,
        "tools/list" => handle_tools_list(state, request.id, request.params).await,
        "tools/call" => handle_tools_call(state, request.id, request.params).await,
        "resources/list" => handle_resources_list(request.id).await,
        "resources/read" => handle_resources_read(request.id, request.params).await,
        "prompts/list" => handle_prompts_list(request.id).await,
        "ping" => handle_ping(request.id).await,
        _ => McpResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

async fn handle_initialize(id: Option<Value>) -> McpResponse {
    McpResponse::success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": true
                },
                "resources": {
                    "subscribe": false,
                    "listChanged": false
                },
                "prompts": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "op-dbus-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
    )
}

async fn handle_initialized(id: Option<Value>) -> McpResponse {
    McpResponse::success(id, json!({}))
}

async fn handle_tools_list(
    state: &AppState,
    id: Option<Value>,
    _params: Option<Value>,
) -> McpResponse {
    let tools = state.tool_registry.list().await;

    let tool_list: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema.clone()
            })
        })
        .collect();

    McpResponse::success(
        id,
        json!({
            "tools": tool_list
        }),
    )
}

async fn handle_tools_call(
    state: &AppState,
    id: Option<Value>,
    params: Option<Value>,
) -> McpResponse {
    let params = match params {
        Some(p) => p,
        None => return McpResponse::error(id, -32602, "Missing params"),
    };

    let tool_name = match params.get("name").and_then(|n| n.as_str()) {
        Some(name) => name,
        None => return McpResponse::error(id, -32602, "Missing tool name"),
    };

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    info!("MCP tool call: {} with args: {:?}", tool_name, arguments);

    // Get and execute tool
    let tool = match state.tool_registry.get(tool_name).await {
        Some(t) => t,
        None => return McpResponse::error(id, -32602, format!("Tool not found: {}", tool_name)),
    };

    match tool.execute(arguments).await {
        Ok(result) => McpResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": simd_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
                }],
                "isError": false
            }),
        ),
        Err(e) => {
            error!("Tool execution failed: {}", e);
            McpResponse::success(
                id,
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

async fn handle_resources_list(id: Option<Value>) -> McpResponse {
    McpResponse::success(
        id,
        json!({
            "resources": []
        }),
    )
}

async fn handle_resources_read(id: Option<Value>, _params: Option<Value>) -> McpResponse {
    McpResponse::error(id, -32602, "Resource not found")
}

async fn handle_prompts_list(id: Option<Value>) -> McpResponse {
    McpResponse::success(id, json!({ "prompts": [] }))
}

async fn handle_ping(id: Option<Value>) -> McpResponse {
    McpResponse::success(id, json!({}))
}

/// GET /mcp/_config - Generate config for clients
pub async fn config_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "mcpServers": {
            "op-dbus": {
                "command": "op-mcp-server",
                "args": []
            }
        }
    }))
}
