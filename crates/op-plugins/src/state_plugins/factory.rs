//! Factory Droid agent platform plugin.
//!
//! Publishes the full Factory Public API surface (computers, sessions, models,
//! autonomy controls, token tracking) through PluginSchema so the UI can
//! render Factory configuration, session controls, and model selection from
//! the D-Bus projection.
//!
//! ## BYOM Integration
//!
//! The factory plugin discovers external model sources (BYOM) via D-Bus projection.
//! It reads from the zeroclaw plugin's `model_routes` projection at
//! `/opdbus/v1/plugins/zeroclaw` and surfaces them as `byom_sources`.

use super::plugin_scaffold_helpers::field_from_value;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryState {
    pub status: String,
    pub endpoint: String,
    pub auth_method: String,
    pub computers: Value,
    pub sessions: Value,
    pub session_settings: Value,
    pub models: Value,
    pub providers: Value,
    pub tools: Value,
    pub config_schema: Value,
    pub ui_surfaces: Value,
    /// BYOM (Bring Your Own Model) sources discovered from external providers
    /// via D-Bus projection (e.g., zeroclaw's model_routes)
    pub byom_sources: Value,
}

pub struct FactoryPlugin;

impl Default for FactoryPlugin {
    fn default() -> Self {
        Self
    }
}

/// D-Bus projection client for reading plugin states
mod projection {
    use simd_json::json;
    use simd_json::prelude::*;
    use simd_json::OwnedValue as Value;

    /// Read zeroclaw's model_routes from D-Bus projection cache
    pub fn read_zeroclaw_model_routes() -> Option<Value> {
        // The sealed blob IS the plugin: zeroclaw exists iff its blob is in
        // the SHM catalog.
        op_blob::catalog::read_plugin_schema_shm("zeroclaw")?;

        // Read zeroclaw projection from D-Bus via /dev/shm projection cache
        // The actual projection is written by op-dbus at /dev/shm/plugin-{name}.json
        let projection_path = "/dev/shm/plugin-zeroclaw.json";
        let proj_bytes = std::fs::read(projection_path).ok()?;
        let mut proj_bytes = proj_bytes;
        let zeroclaw_proj: Value = simd_json::to_owned_value(&mut proj_bytes).ok()?;

        // Extract model_routes
        zeroclaw_proj.get("model_routes").cloned()
    }

    /// Convert zeroclaw routes to BYOM model sources
    pub fn routes_to_byom_sources(routes: &Value) -> Value {
        use simd_json::prelude::*;

        let Some(routes_arr) = routes.as_array() else {
            return json!([]);
        };

        let sources: Vec<Value> = routes_arr
            .iter()
            .filter_map(|route| {
                let model = route.get("model")?.as_str()?;
                let provider = route.get("provider")?.as_str().unwrap_or("unknown");
                let upstream = route
                    .get("upstream_provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or(provider);
                let hint = route.get("hint").and_then(|v| v.as_str());
                let kind = route.get("kind").and_then(|v| v.as_str());
                let available = route
                    .get("available")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let transport = route.get("transport").and_then(|v| v.as_str());
                let status_reason = route.get("status_reason").and_then(|v| v.as_str());

                Some(json!({
                    "id": format!("{}/{}", upstream, model),
                    "model": model,
                    "provider": provider,
                    "upstream_provider": upstream,
                    "family": upstream,
                    "hint": hint,
                    "kind": kind,
                    "available": available,
                    "transport": transport,
                    "status_reason": status_reason,
                    "source": "zeroclaw",
                    "byom": true
                }))
            })
            .collect();

        json!(sources)
    }
}

impl FactoryPlugin {
    pub fn new() -> Self {
        Self
    }
    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    /// Discover BYOM sources from zeroclaw D-Bus projection
    fn discover_byom_sources() -> Value {
        // Try to read zeroclaw model routes via D-Bus projection
        match projection::read_zeroclaw_model_routes() {
            Some(routes) => projection::routes_to_byom_sources(&routes),
            None => {
                // Fallback: return empty array with discovery metadata
                json!({
                    "sources": [],
                    "discovery_status": "zeroclaw_projection_unavailable",
                    "discovery_path": "/opdbus/v1/plugins/zeroclaw",
                    "note": "BYOM sources will appear when zeroclaw plugin is projected via D-Bus"
                })
            }
        }
    }

    pub(crate) fn current_state() -> FactoryState {
        let endpoint = Self::env_or("FACTORY_API_ENDPOINT", "https://api.factory.ai/api/v0");
        let auth_method = Self::env_or("FACTORY_AUTH_METHOD", "bearer");

        FactoryState {
            status: "active".to_string(),
            endpoint,
            auth_method,
            computers: json!({
                "endpoint": "/api/v0/computers",
                "provider_types": ["byom", "e2b"],
                "statuses": ["provisioning", "active", "error"],
                "source_types": [
                    {"kind": "scratch", "description": "Empty environment"},
                    {"kind": "template", "description": "Preconfigured cloud template"},
                    {"kind": "live-computer", "description": "Clone existing computer"}
                ],
                "operations": ["create", "list", "get", "update", "delete", "refresh", "restart", "installDeps", "metrics"]
            }),
            sessions: json!({
                "endpoint": "/api/v0/sessions",
                "statuses": ["idle", "pending", "running"],
                "user_message_sources": [
                    "web", "desktop", "cli_tui", "cli_exec", "cli_acp",
                    "slack", "linear", "sessions_api", "api", "automation",
                    "readiness-remediation", "readiness-evaluation",
                    "wiki-generation", "wiki-ci-setup"
                ],
                "operations": ["create", "list", "get", "update", "delete", "interrupt"],
                "message_roles": ["user", "assistant", "tool", "system"],
                "content_block_types": ["text", "image", "thinking", "redacted_thinking", "tool_use", "tool_result", "document"],
                "visibility_modes": ["both", "llm_only", "user_only"]
            }),
            session_settings: json!({
                "interaction_modes": ["auto", "spec", "agi", "mission"],
                "autonomy_levels": ["off", "low", "medium", "high"],
                "reasoning_efforts": ["none", "dynamic", "off", "minimal", "low", "medium", "high", "xhigh", "max"],
                "api_provider_locks": [
                    "bedrock", "anthropic", "vertex_anthropic", "bedrock_anthropic",
                    "bedrock_converse", "openai", "azure_openai", "google", "xai",
                    "fireworks", "baseten", "snowflake"
                ],
                "provider_locks": [
                    "anthropic", "openai", "generic-chat-completion-api",
                    "factory", "google", "xai", "voyage", "bedrock-converse"
                ]
            }),
            models: json!({
                "catalog": [
                    {"id": "claude-sonnet-4-6", "family": "anthropic"},
                    {"id": "claude-opus-4-6", "family": "anthropic"},
                    {"id": "claude-opus-4-7", "family": "anthropic"},
                    {"id": "claude-opus-4-8", "family": "anthropic"},
                    {"id": "gpt-5.5", "family": "openai"},
                    {"id": "gpt-5.5-pro", "family": "openai"},
                    {"id": "gpt-5.5-fast", "family": "openai"},
                    {"id": "gpt-5.4", "family": "openai"},
                    {"id": "gpt-5.4-fast", "family": "openai"},
                    {"id": "gpt-5.3-codex", "family": "openai"},
                    {"id": "gemini-2.5-pro", "family": "google"},
                    {"id": "gemini-2.5-flash", "family": "google"},
                    {"id": "gemini-3-pro-preview", "family": "google"},
                    {"id": "gemini-3.5-flash", "family": "google"},
                    {"id": "deepseek-v4-pro", "family": "deepseek"},
                    {"id": "glm-4.7", "family": "zhipu"},
                    {"id": "glm-5", "family": "zhipu"},
                    {"id": "kimi-k2.5", "family": "moonshot"},
                    {"id": "minimax-m2.7", "family": "minimax"},
                    {"id": "aspen-05-15", "family": "factory-internal"},
                    {"id": "almond-05-27", "family": "factory-internal"}
                ],
                "model_routes": [
                    {"hint": "best", "provider": "factory", "model": "auto", "kind": "orchestrator", "available": true, "status_reason": "Factory auto-selects best model"},
                    {"hint": "code", "provider": "factory", "model": "claude-sonnet-4-6", "kind": "chat", "available": true, "status_reason": "Claude Sonnet 4.6 via Factory"},
                    {"hint": "code", "provider": "factory", "model": "claude-opus-4-6", "kind": "chat", "available": true, "status_reason": "Claude Opus 4.6 via Factory"},
                    {"hint": "code", "provider": "factory", "model": "gpt-5.5", "kind": "chat", "available": true, "status_reason": "GPT 5.5 via Factory"},
                    {"hint": "code", "provider": "factory", "model": "gpt-5.5-pro", "kind": "chat", "available": true, "status_reason": "GPT 5.5 Pro via Factory"},
                    {"hint": "code", "provider": "factory", "model": "gemini-2.5-pro", "kind": "chat", "available": true, "status_reason": "Gemini 2.5 Pro via Factory"},
                    {"hint": "fast", "provider": "factory", "model": "gemini-2.5-flash", "kind": "chat", "available": true, "status_reason": "Gemini 2.5 Flash via Factory"},
                    {"hint": "fast", "provider": "factory", "model": "gpt-5.5-fast", "kind": "chat", "available": true, "status_reason": "GPT 5.5 Fast via Factory"},
                    {"hint": "code", "provider": "factory", "model": "deepseek-v4-pro", "kind": "chat", "available": true, "status_reason": "DeepSeek V4 Pro via Factory"}
                ]
            }),
            providers: json!([
                {
                    "id": "factory", "route": "factory", "kind": "orchestrator",
                    "aliases": ["default", "auto", "droid"],
                    "endpoint": "https://api.factory.ai/api/v0", "auth": "bearer"
                },
                {
                    "id": "zeroclaw", "route": "zeroclaw", "kind": "byom_router",
                    "aliases": ["byom", "local", "openrouter", "ollama"],
                    "endpoint": "/opdbus/v1/plugins/zeroclaw", "auth": "dbus_projection",
                    "description": "BYOM source - discovers models from zeroclaw D-Bus projection",
                    "discovery_plugin": "zeroclaw",
                    "discovery_path": "/opdbus/v1/plugins/zeroclaw"
                }
            ]),
            tools: json!([
                {"name": "factory.session.create", "description": "Create a Factory Droid coding session", "parameters": {"type": "object", "properties": {"computerId": {"type": "string"}, "sessionSettings": {"type": "object"}}, "required": ["computerId"]}},
                {"name": "factory.session.send", "description": "Send a message to a Factory session", "parameters": {"type": "object", "properties": {"sessionId": {"type": "string"}, "text": {"type": "string"}}, "required": ["sessionId", "text"]}},
                {"name": "factory.session.list", "description": "List active Factory sessions", "parameters": {"type": "object", "properties": {"computerId": {"type": "string"}}, "required": []}},
                {"name": "factory.computer.create", "description": "Create a Factory computer environment", "parameters": {"type": "object", "properties": {"name": {"type": "string"}, "provider": {"type": "string", "enum": ["byom", "e2b"]}}, "required": ["name"]}},
                {"name": "factory.models.list", "description": "List available models through Factory", "parameters": {"type": "object", "properties": {"family": {"type": "string"}}, "required": []}},
                {"name": "factory.byom.list", "description": "List BYOM (Bring Your Own Model) sources discovered from zeroclaw", "parameters": {"type": "object", "properties": {"provider": {"type": "string"}, "available_only": {"type": "boolean"}}, "required": []}}
            ]),
            config_schema: json!({
                "source": "factory public api v0",
                "openapi_url": "https://api.factory.ai/api/v0/openapi.json",
                "api_version": "0.1.0"
            }),
            ui_surfaces: json!([
                {"path": "/factory", "name": "Factory Droid", "schema": "factory"},
                {"path": "/factory/sessions", "name": "Sessions", "schema": "factory.sessions"},
                {"path": "/factory/models", "name": "Models", "schema": "factory.models"},
                {"path": "/factory/byom", "name": "BYOM Sources", "schema": "factory.byom_sources"}
            ]),
            byom_sources: Self::discover_byom_sources(),
        }
    }
}

#[async_trait]
impl StatePlugin for FactoryPlugin {
    fn name(&self) -> &str {
        "factory"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(factory_schema())
    }
    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-declared".to_string(),
                desired_hash: "schema-declared".to_string(),
            },
        })
    }
    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }
    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }
    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?,
            backend_checkpoint: None,
        })
    }
    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

pub(crate) fn factory_schema() -> PluginSchema {
    use simd_json::prelude::*;
    let state = simd_json::serde::to_owned_value(super::factory::FactoryPlugin::current_state())
        .unwrap_or_else(|_| json!({}));

    let mut builder = PluginSchema::builder("factory")
        .version("1.0.0")
        .category("llm")
        .description("Factory Droid agent platform — computers, sessions, models, autonomy controls, BYOM discovery");

    // Add BYOM dependency on zeroclaw for model discovery via D-Bus projection
    builder = builder.dependency("zeroclaw");

    // Add fields from live state
    if let Some(obj) = state.as_object() {
        for (key, value) in obj.iter() {
            builder = builder.field(&key.to_string(), field_from_value(value));
        }
    }

    let mut schema = builder.example(state.clone()).build();

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListComputersOutput {
        pub computers: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetSessionOutput {
        pub session: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListSessionsOutput {
        pub sessions: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListModelsOutput {
        pub models: Vec<serde_json::Value>,
    }

    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "list_computers".to_string(),
        method_decl_from_schemars_with_output::<(), ListComputersOutput>(
            "list_computers",
            SideEffect::Read,
            true,
            "factory.read",
            "obs.service.factory.computer.list@v1",
        ),
    );
    schema.methods.insert(
        "get_session".to_string(),
        method_decl_from_schemars_with_output::<(), GetSessionOutput>(
            "get_session",
            SideEffect::Read,
            true,
            "factory.read",
            "obs.service.factory.session.get@v1",
        ),
    );
    schema.methods.insert(
        "list_sessions".to_string(),
        method_decl_from_schemars_with_output::<(), ListSessionsOutput>(
            "list_sessions",
            SideEffect::Read,
            true,
            "factory.read",
            "obs.service.factory.session.list@v1",
        ),
    );
    schema.methods.insert(
        "list_models".to_string(),
        method_decl_from_schemars_with_output::<(), ListModelsOutput>(
            "list_models",
            SideEffect::Read,
            true,
            "factory.read",
            "obs.service.factory.model.list@v1",
        ),
    );
    schema.methods.insert(
        "discover_byom".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "discover_byom",
            SideEffect::Mutation,
            false,
            "factory.invoke",
            "mut.service.factory.byom.discover@v1",
        ),
    );
    schema.methods.insert(
        "set_autonomy".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "set_autonomy",
            SideEffect::Mutation,
            false,
            "factory.invoke",
            "mut.service.factory.autonomy.set@v1",
        ),
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("factory", |_ctx| std::sync::Arc::new(FactoryPlugin::new()))
}
