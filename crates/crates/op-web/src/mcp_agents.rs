//! MCP Agents Server - Cognitive Agent MCP with persisted selection state.
//!
//! Supports:
//! - Enabling/disabling which agents are exposed as MCP tools.
//! - Selecting which agents should be actively running (prewarmed).
//! - Lazy-starting non-prewarmed agents on first tool call.

use axum::{
    extract::{Extension, Json},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use op_agents::agents::base::{AgentTask, AgentTrait as Agent};
use op_agents::{builtin_agent_descriptors, create_agent, AgentDescriptor};

const DEFAULT_AGENT_CONFIG_PATH: &str = "/var/lib/op-dbus/cognitive-mcp-agents.json";

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentSelectionConfig {
    enabled_agents: Vec<String>,
    active_agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedAgentInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub active: bool,
    pub running: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CognitiveRuntimeSnapshot {
    pub total_agents: usize,
    pub enabled_agents: usize,
    pub active_agents: Vec<String>,
    pub running_agents: Vec<String>,
    pub tools_count: usize,
}

#[derive(Debug, Clone)]
struct AgentEntry {
    descriptor: AgentDescriptor,
    enabled: bool,
    active: bool,
}

pub struct CriticalAgentsState {
    entries: HashMap<String, AgentEntry>,
    running: HashMap<String, Arc<dyn Agent + Send + Sync>>,
}

impl CriticalAgentsState {
    pub fn new() -> Self {
        let descriptors = builtin_agent_descriptors();
        let mut entries = HashMap::new();

        for descriptor in descriptors {
            let key = normalize_agent_type(&descriptor.agent_type);
            entries.insert(
                key,
                AgentEntry {
                    descriptor,
                    enabled: false,
                    active: false,
                },
            );
        }

        let mut state = Self {
            entries,
            running: HashMap::new(),
        };

        let default_config = state.default_config();
        let config = load_agent_config().unwrap_or(default_config);
        let applied = state.apply_config(config);

        if let Err(err) = save_agent_config(&applied) {
            warn!(error = %err, "Failed to persist cognitive MCP agent config");
        }

        info!(
            total = state.entries.len(),
            enabled = state.entries.values().filter(|entry| entry.enabled).count(),
            active = state.entries.values().filter(|entry| entry.active).count(),
            running = state.running.len(),
            "Initialized Cognitive MCP agent state"
        );

        state
    }

    fn default_config(&self) -> AgentSelectionConfig {
        let mut enabled_agents: Vec<String> = self.entries.keys().cloned().collect();
        enabled_agents.sort();

        let preferred_active = [
            "memory",
            "context-manager",
            "sequential-thinking",
            "rust-pro",
        ];
        let active_agents: Vec<String> = preferred_active
            .iter()
            .map(|agent| normalize_agent_type(agent))
            .filter(|agent| self.entries.contains_key(agent))
            .collect();

        AgentSelectionConfig {
            enabled_agents,
            active_agents,
        }
    }

    fn apply_config(&mut self, config: AgentSelectionConfig) -> AgentSelectionConfig {
        let known_agents: HashSet<String> = self.entries.keys().cloned().collect();

        let enabled: HashSet<String> = config
            .enabled_agents
            .iter()
            .map(|agent| normalize_agent_type(agent))
            .filter(|agent| known_agents.contains(agent))
            .collect();

        let active: HashSet<String> = config
            .active_agents
            .iter()
            .map(|agent| normalize_agent_type(agent))
            .filter(|agent| enabled.contains(agent))
            .collect();

        for (agent_type, entry) in &mut self.entries {
            entry.enabled = enabled.contains(agent_type);
            entry.active = active.contains(agent_type);
        }

        self.sync_running_agents();
        self.current_config()
    }

    fn current_config(&self) -> AgentSelectionConfig {
        let mut enabled_agents: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.enabled)
            .map(|(agent_type, _)| agent_type.clone())
            .collect();
        enabled_agents.sort();

        let mut active_agents: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.active)
            .map(|(agent_type, _)| agent_type.clone())
            .collect();
        active_agents.sort();

        AgentSelectionConfig {
            enabled_agents,
            active_agents,
        }
    }

    fn sync_running_agents(&mut self) {
        let desired: HashSet<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.enabled && entry.active)
            .map(|(agent_type, _)| agent_type.clone())
            .collect();

        self.running
            .retain(|agent_type, _| desired.contains(agent_type));

        for agent_type in desired {
            self.ensure_runtime_agent(&agent_type);
        }
    }

    fn ensure_runtime_agent(&mut self, agent_type: &str) {
        if self.running.contains_key(agent_type) {
            return;
        }

        let agent_id = format!("{}-mcp", agent_type.replace('-', "_"));
        match create_agent(agent_type, agent_id) {
            Ok(agent) => {
                let agent: Arc<dyn Agent + Send + Sync> = Arc::from(agent);
                self.running.insert(agent_type.to_string(), agent);
                info!(agent = %agent_type, "Started cognitive MCP runtime agent");
            }
            Err(err) => {
                warn!(agent = %agent_type, error = %err, "Failed to start cognitive MCP runtime agent");
            }
        }
    }

    fn tool_name(agent_type: &str, operation: &str) -> String {
        format!("{}_{}", agent_type.replace('-', "_"), operation)
    }

    fn operation_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Optional path hint" },
                "args": { "type": "object", "description": "Operation arguments" }
            },
            "additionalProperties": true
        })
    }

    fn get_tools(&self) -> Vec<Value> {
        let mut tools = Vec::new();

        let mut entries: Vec<(&String, &AgentEntry)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.enabled)
            .collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));

        for (agent_type, entry) in entries {
            for operation in &entry.descriptor.operations {
                tools.push(json!({
                    "name": Self::tool_name(agent_type, operation),
                    "description": format!("{} ({})", entry.descriptor.description, operation),
                    "inputSchema": Self::operation_schema(),
                    "agent": agent_type,
                    "operation": operation
                }));
            }
        }

        tools
    }

    fn resolve_tool(&self, tool_name: &str) -> Option<(String, String)> {
        for (agent_type, entry) in &self.entries {
            if !entry.enabled {
                continue;
            }

            for operation in &entry.descriptor.operations {
                if Self::tool_name(agent_type, operation) == tool_name {
                    return Some((agent_type.clone(), operation.clone()));
                }
            }
        }

        None
    }

    async fn execute_tool(&mut self, tool_name: &str, arguments: &Value) -> Result<Value, String> {
        let (agent_type, operation) = self
            .resolve_tool(tool_name)
            .ok_or_else(|| format!("Unknown or disabled tool: {}", tool_name))?;

        self.ensure_runtime_agent(&agent_type);

        let agent = self
            .running
            .get(&agent_type)
            .cloned()
            .ok_or_else(|| format!("Agent {} is not running", agent_type))?;

        let task = AgentTask {
            task_type: agent_type.clone(),
            operation,
            path: arguments
                .get("path")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            args: Some(simd_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string())),
            config: arguments
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, Value>>()
                })
                .unwrap_or_default(),
        };

        let result = agent.execute(task).await.map_err(|err| err.to_string())?;
        let mut result_bytes = result.to_json().into_bytes();
        simd_json::to_owned_value(&mut result_bytes).map_err(|err| err.to_string())
    }

    fn list_agents(&self) -> Vec<ManagedAgentInfo> {
        let mut agents = Vec::new();

        let mut keys: Vec<String> = self.entries.keys().cloned().collect();
        keys.sort();

        for key in keys {
            if let Some(entry) = self.entries.get(&key) {
                agents.push(ManagedAgentInfo {
                    id: key.clone(),
                    name: entry.descriptor.name.clone(),
                    description: entry.descriptor.description.clone(),
                    enabled: entry.enabled,
                    active: entry.active,
                    running: self.running.contains_key(&key),
                    capabilities: entry.descriptor.operations.clone(),
                });
            }
        }

        agents
    }

    fn snapshot(&self) -> CognitiveRuntimeSnapshot {
        let active_agents = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.active)
            .map(|(agent_type, _)| agent_type.clone())
            .collect::<Vec<String>>();

        let mut running_agents = self.running.keys().cloned().collect::<Vec<String>>();
        running_agents.sort();

        let tools_count = self
            .entries
            .values()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.descriptor.operations.len())
            .sum();

        CognitiveRuntimeSnapshot {
            total_agents: self.entries.len(),
            enabled_agents: self.entries.values().filter(|entry| entry.enabled).count(),
            active_agents,
            running_agents,
            tools_count,
        }
    }
}

impl Default for CriticalAgentsState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AgentsMcpState {
    pub agents: RwLock<CriticalAgentsState>,
}

impl AgentsMcpState {
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(CriticalAgentsState::new()),
        }
    }
}

impl Default for AgentsMcpState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_router() -> axum::Router {
    let state = GLOBAL_AGENTS_STATE.clone();
    axum::Router::new()
        .route("/mcp/agents", get(mcp_agents_sse_handler))
        .route("/mcp/agents/message", post(mcp_agents_message_handler))
        .layer(Extension(state))
}

// Global state for stateless handlers (lazy initialization)
lazy_static::lazy_static! {
    static ref GLOBAL_AGENTS_STATE: Arc<AgentsMcpState> = Arc::new(AgentsMcpState::new());
}

pub async fn list_cognitive_agents() -> Vec<ManagedAgentInfo> {
    let state = GLOBAL_AGENTS_STATE.clone();
    let agents = state.agents.read().await;
    agents.list_agents()
}

pub async fn set_cognitive_agents(
    enabled_agents: Option<Vec<String>>,
    active_agents: Option<Vec<String>>,
) -> Result<CognitiveRuntimeSnapshot, String> {
    let state = GLOBAL_AGENTS_STATE.clone();
    let mut agents = state.agents.write().await;

    let mut next = agents.current_config();
    if let Some(enabled) = enabled_agents {
        next.enabled_agents = enabled;
    }
    if let Some(active) = active_agents {
        next.active_agents = active;
    }

    let applied = agents.apply_config(next);
    save_agent_config(&applied)?;

    Ok(agents.snapshot())
}

pub async fn cognitive_runtime_snapshot() -> CognitiveRuntimeSnapshot {
    let state = GLOBAL_AGENTS_STATE.clone();
    let agents = state.agents.read().await;
    agents.snapshot()
}

/// Stateless SSE handler that uses global state
/// Used when nesting under the main MCP router without its own state
pub async fn mcp_agents_sse_handler_stateless(
    headers: HeaderMap,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    mcp_agents_sse_handler(headers).await
}

/// Stateless message handler that uses global state
/// Used when nesting under the main MCP router
pub async fn mcp_agents_message_handler_stateless(Json(request): Json<JsonRpcRequest>) -> Response {
    let state = GLOBAL_AGENTS_STATE.clone();
    mcp_agents_message_handler(Extension(state), Json(request)).await
}

pub async fn mcp_agents_sse_handler(
    headers: HeaderMap,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("MCP Agents SSE client connected");

    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");

    let post_url = format!("{}://{}/mcp/agents/message", scheme, host);
    info!("MCP Agents POST endpoint: {}", post_url);

    let endpoint_event = Event::default().event("endpoint").data(&post_url);

    let stream = stream::once(async move { Ok(endpoint_event) });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

pub async fn mcp_agents_message_handler(
    Extension(state): Extension<Arc<AgentsMcpState>>,
    Json(request): Json<JsonRpcRequest>,
) -> Response {
    debug!(
        "MCP Agents request: method={} id={}",
        request.method, request.id
    );

    if request.jsonrpc != "2.0" {
        let response = JsonRpcResponse::error(
            request.id,
            -32600,
            "Invalid JSON-RPC version (expected 2.0)".to_string(),
        );
        let json_body = simd_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error"}}"#
                .to_string()
        });
        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(json_body.into())
            .unwrap_or_else(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
            });
    }

    let response = match request.method.as_str() {
        "initialize" => handle_initialize(&request),
        "initialized" => JsonRpcResponse::success(request.id.clone(), json!({})),
        "tools/list" => handle_tools_list(&state, &request).await,
        "tools/call" => handle_tools_call(&state, &request).await,
        "ping" => JsonRpcResponse::success(request.id.clone(), json!({})),
        "notifications/initialized" => JsonRpcResponse::success(request.id.clone(), json!({})),
        _ => {
            warn!("Unknown MCP method: {}", request.method);
            JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            )
        }
    };

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

fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
    info!("MCP Agents initialize request");
    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": true
                }
            },
            "serverInfo": {
                "name": "op-dbus-agents",
                "version": "1.1.0"
            },
            "instructions": "Cognitive agents MCP: tool exposure is configurable via enabled agents, with separate active agent prewarming."
        }),
    )
}

async fn handle_tools_list(
    state: &Arc<AgentsMcpState>,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
    info!("MCP Agents tools/list request");
    let agents = state.agents.read().await;
    let tools = agents.get_tools();

    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "tools": tools
        }),
    )
}

async fn handle_tools_call(
    state: &Arc<AgentsMcpState>,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
    let params = &request.params;

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

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    info!(
        "MCP Agents tool call: {} with args: {}",
        tool_name, arguments
    );

    let mut agents = state.agents.write().await;

    match agents.execute_tool(tool_name, &arguments).await {
        Ok(result) => {
            let text =
                simd_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result));
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
        Err(err) => {
            error!("Agent execution error: {}", err);
            JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: {}", err)
                    }],
                    "isError": true
                }),
            )
        }
    }
}

fn normalize_agent_type(raw: &str) -> String {
    raw.trim().replace('_', "-").to_ascii_lowercase()
}

fn agent_config_path() -> PathBuf {
    std::env::var("OP_COGNITIVE_MCP_AGENT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_AGENT_CONFIG_PATH))
}

fn load_agent_config() -> Option<AgentSelectionConfig> {
    let path = agent_config_path();
    let data = std::fs::read(&path).ok()?;
    let mut bytes = data;

    match simd_json::from_slice::<AgentSelectionConfig>(&mut bytes) {
        Ok(config) => Some(config),
        Err(err) => {
            warn!(path = %path.display(), error = %err, "Failed to parse cognitive MCP config; using defaults");
            None
        }
    }
}

fn save_agent_config(config: &AgentSelectionConfig) -> Result<(), String> {
    let path = agent_config_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create config dir {}: {}", parent.display(), err))?;
    }

    let body = simd_json::to_string_pretty(config)
        .map_err(|err| format!("Failed to serialize cognitive MCP config: {}", err))?;

    std::fs::write(&path, body).map_err(|err| {
        format!(
            "Failed to write cognitive MCP config {}: {}",
            path.display(),
            err
        )
    })
}
