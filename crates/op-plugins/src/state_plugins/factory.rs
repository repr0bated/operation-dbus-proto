//! Factory Droid agent platform plugin.
//!
//! Publishes the full Factory Public API surface (computers, sessions, models,
//! autonomy controls, token tracking) through PluginSchema so the UI can
//! render Factory configuration, session controls, and model selection from
//! the D-Bus projection.

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
}

pub struct FactoryPlugin;

impl Default for FactoryPlugin {
    fn default() -> Self { Self }
}

impl FactoryPlugin {
    pub fn new() -> Self { Self }
    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
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
            providers: json!([{
                "id": "factory", "route": "factory", "kind": "orchestrator",
                "aliases": ["default", "auto", "droid"],
                "endpoint": "https://api.factory.ai/api/v0", "auth": "bearer"
            }]),
            tools: json!([
                {"name": "factory.session.create", "description": "Create a Factory Droid coding session", "parameters": {"type": "object", "properties": {"computerId": {"type": "string"}, "sessionSettings": {"type": "object"}}, "required": ["computerId"]}},
                {"name": "factory.session.send", "description": "Send a message to a Factory session", "parameters": {"type": "object", "properties": {"sessionId": {"type": "string"}, "text": {"type": "string"}}, "required": ["sessionId", "text"]}},
                {"name": "factory.session.list", "description": "List active Factory sessions", "parameters": {"type": "object", "properties": {"computerId": {"type": "string"}}, "required": []}},
                {"name": "factory.computer.create", "description": "Create a Factory computer environment", "parameters": {"type": "object", "properties": {"name": {"type": "string"}, "provider": {"type": "string", "enum": ["byom", "e2b"]}}, "required": ["name"]}},
                {"name": "factory.models.list", "description": "List available models through Factory", "parameters": {"type": "object", "properties": {"family": {"type": "string"}}, "required": []}}
            ]),
            config_schema: json!({
                "source": "factory public api v0",
                "openapi_url": "https://api.factory.ai/api/v0/openapi.json",
                "api_version": "0.1.0"
            }),
            ui_surfaces: json!([
                {"path": "/factory", "name": "Factory Droid", "schema": "factory"},
                {"path": "/factory/sessions", "name": "Sessions", "schema": "factory.sessions"},
                {"path": "/factory/models", "name": "Models", "schema": "factory.models"}
            ]),
        }
    }
}

#[async_trait]
impl StatePlugin for FactoryPlugin {
    fn name(&self) -> &str { "factory" }
    fn version(&self) -> &str { "1.0.0" }
    fn schema(&self) -> Option<PluginSchema> { Some(super::plugin_schema_defs::factory_plugin_schema()) }
    async fn query_current_state(&self) -> Result<Value> { Ok(simd_json::serde::to_owned_value(Self::current_state())?) }
    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff { plugin: self.name().to_string(), actions: vec![], metadata: DiffMetadata { timestamp: chrono::Utc::now().timestamp(), current_hash: "schema-declared".to_string(), desired_hash: "schema-declared".to_string() } })
    }
    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult { success: true, changes_applied: vec![], errors: vec![], checkpoint: None })
    }
    async fn verify_state(&self, _desired: &Value) -> Result<bool> { Ok(true) }
    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint { id: uuid::Uuid::new_v4().to_string(), plugin: self.name().to_string(), timestamp: chrono::Utc::now().timestamp(), state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?, backend_checkpoint: None })
    }
    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> { Ok(()) }
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities { supports_rollback: false, supports_checkpoints: true, supports_verification: true, atomic_operations: false }
    }
}
