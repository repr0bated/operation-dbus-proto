//! Agent API Handlers

use axum::{
    extract::{Extension, Path},
    response::Json,
};
use serde::Deserialize;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::info;

use crate::state::AppState;

/// GET /api/agents - List running agent instances
pub async fn list_agents_handler(Extension(state): Extension<Arc<AppState>>) -> Json<Value> {
    let registry = state.agent_registry.read().await;
    let agents = registry.list_instances().await;
    Json(json!({ "agents": agents }))
}

/// GET /api/agents/types - List available agent types
pub async fn list_agent_types_handler() -> Json<Value> {
    let types = op_agents::list_agent_types();
    Json(json!({ "types": types }))
}

/// GET /api/agents/:id - Get agent status
pub async fn get_agent_handler(
    Extension(state): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let registry = state.agent_registry.read().await;
    match registry.get_instance_status(&id).await {
        Ok(status) => Json(json!({ "agent": status })),
        Err(_) => Json(json!({ "error": "Agent not found" })),
    }
}

#[derive(Debug, Deserialize)]
pub struct SpawnAgentRequest {
    #[serde(rename = "type")]
    pub agent_type: String,
    #[serde(default)]
    pub config: Option<Value>,
}

/// POST /api/agents - Spawn a new agent
pub async fn spawn_agent_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<SpawnAgentRequest>,
) -> Json<Value> {
    info!("Spawning agent of type: {}", request.agent_type);

    let registry = state.agent_registry.write().await;
    match registry
        .spawn_agent(&request.agent_type, request.config)
        .await
    {
        Ok(id) => Json(json!({ "agent_id": id, "success": true })),
        Err(e) => Json(json!({ "error": e.to_string(), "success": false })),
    }
}

/// DELETE /api/agents/:id - Kill an agent
pub async fn kill_agent_handler(
    Extension(state): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    info!("Killing agent: {}", id);

    let registry = state.agent_registry.write().await;
    match registry.kill_agent(&id).await {
        Ok(_) => Json(json!({ "killed": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
