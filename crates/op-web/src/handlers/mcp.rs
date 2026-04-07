//! MCP Server Management Handlers

use axum::{
    extract::{Extension, Path},
    response::Json,
};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

use crate::mcp_agents;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub server_type: String, // "compact" or "cognitive"
    pub status: String,      // "running", "stopped", "error"
    pub url: String,
    pub tools_count: usize,
    pub agents: Option<Vec<String>>, // Only for cognitive server
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub active: bool,
    pub running: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetAgentsRequest {
    #[serde(default)]
    pub agent_ids: Vec<String>,
    #[serde(default)]
    pub enabled_agent_ids: Option<Vec<String>>,
    #[serde(default)]
    pub active_agent_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub key_pattern: Option<String>,
    pub memory_type: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<usize>,
}

/// GET /api/mcp/servers - List all MCP servers
pub async fn list_servers_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<Vec<McpServer>> {
    let cognitive = mcp_agents::cognitive_runtime_snapshot().await;
    let servers = vec![
        McpServer {
            id: "compact".to_string(),
            name: "Compact MCP Server".to_string(),
            server_type: "compact".to_string(),
            status: "running".to_string(),
            url: "http://localhost:3001".to_string(),
            tools_count: 15,
            agents: None,
        },
        McpServer {
            id: "cognitive".to_string(),
            name: "Cognitive MCP Server".to_string(),
            server_type: "cognitive".to_string(),
            status: "running".to_string(),
            url: "http://localhost:3002".to_string(),
            tools_count: cognitive.tools_count,
            agents: Some(cognitive.active_agents),
        },
    ];

    Json(servers)
}

/// GET /api/mcp/servers/:id - Get server details
pub async fn get_server_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Path(server_id): Path<String>,
) -> Json<Value> {
    let cognitive = mcp_agents::cognitive_runtime_snapshot().await;

    if server_id == "cognitive" {
        return Json(json!({
            "id": "cognitive",
            "name": "Cognitive MCP Server",
            "status": "running",
            "tools_count": cognitive.tools_count,
            "total_agents": cognitive.total_agents,
            "enabled_agents": cognitive.enabled_agents,
            "active_agents": cognitive.active_agents,
            "running_agents": cognitive.running_agents
        }));
    }

    Json(json!({
        "id": server_id,
        "name": format!("{} MCP Server", server_id),
        "status": "running",
        "tools_count": 15
    }))
}

/// GET /api/mcp/cognitive/agents - List available agents for cognitive server
pub async fn list_agents_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Vec<Agent>> {
    let agents = mcp_agents::list_cognitive_agents()
        .await
        .into_iter()
        .map(|agent| Agent {
            id: agent.id,
            name: agent.name,
            description: agent.description,
            enabled: agent.enabled,
            active: agent.active,
            running: agent.running,
            capabilities: agent.capabilities,
        })
        .collect();

    Json(agents)
}

/// POST /api/mcp/cognitive/agents - Set enabled agents for cognitive server
pub async fn set_agents_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Json(request): Json<SetAgentsRequest>,
) -> Json<Value> {
    let enabled = if let Some(explicit) = request.enabled_agent_ids {
        Some(explicit)
    } else if !request.agent_ids.is_empty() || request.active_agent_ids.is_none() {
        Some(request.agent_ids)
    } else {
        None
    };

    match mcp_agents::set_cognitive_agents(enabled, request.active_agent_ids).await {
        Ok(snapshot) => Json(json!({
            "success": true,
            "enabled_agents": snapshot.enabled_agents,
            "active_agents": snapshot.active_agents,
            "running_agents": snapshot.running_agents,
            "tools_count": snapshot.tools_count,
            "message": "Cognitive MCP agent configuration updated"
        })),
        Err(err) => Json(json!({
            "success": false,
            "error": err
        })),
    }
}

/// GET /api/mcp/cognitive/memory - Query cognitive memory
pub async fn query_memory_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Json(query): Json<MemoryQuery>,
) -> Json<Value> {
    // TODO: Query actual memory store
    Json(json!({
        "entries": [
            {
                "id": "mem_123",
                "key": "user_preference",
                "value": {"theme": "dark", "language": "en"},
                "memory_type": "persistent",
                "tags": ["user", "settings"],
                "created_at": "2026-03-03T08:00:00Z",
                "access_count": 42
            },
            {
                "id": "mem_456",
                "key": "session_context",
                "value": {"last_topic": "MCP servers", "mood": "helpful"},
                "memory_type": "ephemeral",
                "tags": ["session", "context"],
                "created_at": "2026-03-03T08:30:00Z",
                "access_count": 5
            }
        ],
        "total": 2,
        "query": query
    }))
}

/// DELETE /api/mcp/cognitive/memory/:key - Delete memory entry
pub async fn delete_memory_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Path(key): Path<String>,
) -> Json<Value> {
    // TODO: Delete from actual memory store
    Json(json!({
        "success": true,
        "deleted_key": key,
        "message": "Memory entry deleted"
    }))
}

/// GET /api/mcp/cognitive/memory/stats - Get memory statistics
pub async fn memory_stats_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    // TODO: Get actual stats
    Json(json!({
        "total_entries": 127,
        "ephemeral": 45,
        "persistent": 72,
        "shared": 10,
        "total_memory_bytes": 1024000,
        "max_entries": 10000,
        "oldest_entry": "2026-02-01T00:00:00Z",
        "most_accessed_key": "user_preference"
    }))
}
