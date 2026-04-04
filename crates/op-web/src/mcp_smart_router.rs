//! Smart MCP Router - Single entry point that detects client capabilities

use axum::{
    extract::{State, Request},
    response::{Response, IntoResponse},
    http::{HeaderMap, StatusCode},
    Router, routing::{get, post},
};
use std::sync::Arc;

/// Detect client capabilities from headers
#[derive(Debug, Clone)]
enum ClientCapability {
    SSE,           // Server-Sent Events
    Streaming,     // HTTP streaming
    Compact,       // Compact mode (limited tools)
    Agents,        // Critical agents direct access
    JsonRpc,       // Standard JSON-RPC
}

fn detect_client_capability(headers: &HeaderMap) -> ClientCapability {
    // Check Accept header
    if let Some(accept) = headers.get("accept") {
        if let Ok(accept_str) = accept.to_str() {
            if accept_str.contains("text/event-stream") {
                return ClientCapability::SSE;
            }
            if accept_str.contains("application/x-ndjson") {
                return ClientCapability::Streaming;
            }
        }
    }
    
    // Check User-Agent for known clients
    if let Some(ua) = headers.get("user-agent") {
        if let Ok(ua_str) = ua.to_str() {
            if ua_str.contains("Claude") || ua_str.contains("cursor") {
                return ClientCapability::Compact;
            }
            if ua_str.contains("critical-agent") {
                return ClientCapability::Agents;
            }
        }
    }
    
    // Check custom header
    if let Some(mode) = headers.get("x-mcp-mode") {
        if let Ok(mode_str) = mode.to_str() {
            return match mode_str {
                "sse" => ClientCapability::SSE,
                "streaming" => ClientCapability::Streaming,
                "compact" => ClientCapability::Compact,
                "agents" => ClientCapability::Agents,
                _ => ClientCapability::JsonRpc,
            };
        }
    }
    
    ClientCapability::JsonRpc
}

/// Smart router handler - GET for SSE, POST for messages
pub async fn smart_mcp_handler(
    headers: HeaderMap,
    request: Request,
) -> Response {
    let capability = detect_client_capability(&headers);
    
    match capability {
        ClientCapability::SSE => {
            // Route to SSE handler
            crate::mcp::mcp_sse_handler(State(crate::mcp::get_app_state()), headers).await
        }
        ClientCapability::Streaming => {
            // Route to streaming handler (same as SSE for now)
            crate::mcp::mcp_sse_handler(State(crate::mcp::get_app_state()), headers).await
        }
        ClientCapability::Compact => {
            // Route to compact mode
            if request.method() == axum::http::Method::GET {
                crate::mcp_compact::mcp_compact_sse_handler(headers).await
            } else {
                crate::mcp_compact::mcp_compact_message_handler(
                    axum::extract::Json(serde_json::Value::Null)
                ).await.into_response()
            }
        }
        ClientCapability::Agents => {
            // Route to critical agents
            if request.method() == axum::http::Method::GET {
                crate::mcp_agents::mcp_agents_sse_handler_stateless(headers).await
            } else {
                crate::mcp_agents::mcp_agents_message_handler_stateless(
                    axum::extract::Json(serde_json::Value::Null)
                ).await.into_response()
            }
        }
        ClientCapability::JsonRpc => {
            // Route to standard JSON-RPC
            crate::mcp::jsonrpc_handler(
                State(crate::mcp::get_app_state()),
                axum::extract::Json(serde_json::Value::Null)
            ).await.into_response()
        }
    }
}

/// Create smart router - single entry point
pub fn create_smart_mcp_router() -> Router {
    Router::new()
        .route("/", get(smart_mcp_handler).post(smart_mcp_handler))
        .route("/message", post(smart_mcp_handler))
}
