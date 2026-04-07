//! MCP Discovery - Well-Known Endpoint for Auto-Discovery
//!
//! Provides `/.well-known/mcp.json` for clients to auto-discover MCP servers
//! without manual configuration. This is the standard MCP discovery mechanism.

use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use simd_json::{json, OwnedValue as Value};
use tracing::info;

/// Well-known MCP discovery endpoint
/// Returns all available MCP servers for automatic client configuration
pub async fn mcp_discovery_handler(headers: HeaderMap) -> Response {
    info!("MCP discovery request from {:?}", headers.get("user-agent"));

    // Get the host from the request to build proper URLs
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("op-dbus.ghostbridge.tech:3001");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");

    let base_url = format!("{}://{}", scheme, host);

    let discovery = json!({
        "mcpServers": {
            "op-dbus-compact": {
                "url": format!("{}/mcp/compact", base_url),
                "transport": {
                    "type": "sse"
                },
                "name": "OP-DBUS Compact",
                "description": "Compact MCP - 4 meta-tools for LLM tool discovery (list_tools, search_tools, get_tool_schema, execute_tool). Exposes 148+ tools through minimal interface.",
                "features": ["streaming", "tool-discovery"],
                "recommended": true,
                "meta_tools": [
                    {"name": "list_tools", "description": "List available tools with pagination"},
                    {"name": "search_tools", "description": "Search tools by keyword"},
                    {"name": "get_tool_schema", "description": "Get full schema for a specific tool"},
                    {"name": "execute_tool", "description": "Execute any discovered tool"}
                ]
            },
            "op-dbus-agents": {
                "url": format!("{}/mcp/agents", base_url),
                "transport": {
                    "type": "sse"
                },
                "name": "OP-DBUS Agents",
                "description": "Critical Agents MCP - Live streaming agents for dynamic LLM use. Includes memory, cognitive, backend architect, backend security coder, rust pro, and frequently used agents.",
                "features": ["streaming", "agents", "memory", "cognitive"],
                "agents": {
                    "memory": ["memory_remember", "memory_recall", "memory_forget"],
                    "cognitive": ["context_manager_save", "context_manager_load", "sequential_thinking_think"],
                    "semantic_memory": ["mem0_add", "mem0_search", "mem0_get_all", "mem0_delete"],
                    "code": ["rust_pro_analyze", "rust_pro_refactor", "rust_pro_implement", "python_pro_analyze", "python_pro_refactor"],
                    "architecture": ["backend_architect_design", "backend_architect_review"],
                    "security": ["backend_security_coder_secure_endpoint", "backend_security_coder_audit_code", "backend_security_coder_fix_vulnerability", "backend_security_coder_analyze"],
                    "utility": ["debugger_analyze", "debugger_trace", "prompt_engineer_generate", "prompt_engineer_optimize", "deployment_deploy", "deployment_status"]
                }
            }
        },
        "_links": {
            "self": format!("{}/.well-known/mcp.json", base_url),
            "compact_sse": format!("{}/mcp/compact", base_url),
            "compact_message": format!("{}/mcp/compact/message", base_url),
            "agents_sse": format!("{}/mcp/agents", base_url),
            "agents_message": format!("{}/mcp/agents/message", base_url),
            "documentation": "https://spec.modelcontextprotocol.io/"
        },
        "protocol": {
            "version": "2024-11-05",
            "transports": ["sse", "http+sse"]
        },
        "server": {
            "name": "op-dbus",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Operation D-Bus - Linux System Management via MCP"
        }
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "public, max-age=300") // Cache for 5 minutes
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Json(discovery).into_response().into_body())
        .unwrap_or_else(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build response",
            )
                .into_response()
        })
}

/// Simplified discovery for embedding in other responses
pub fn get_mcp_servers_config(base_url: &str) -> Value {
    json!({
        "compact": {
            "url": format!("{}/mcp/compact", base_url),
            "transport": "sse",
            "description": "4 meta-tools for tool discovery"
        },
        "agents": {
            "url": format!("{}/mcp/agents", base_url),
            "transport": "sse",
            "description": "Streaming agents: memory, cognitive, security, rust_pro"
        }
    })
}
