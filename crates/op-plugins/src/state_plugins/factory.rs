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
//! It reads from the tched_router plugin's `model_routes` at
//! `/opdbus/v1/plugins/tched_router` and surfaces them as `byom_sources`.

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
    /// Credential metadata only. Secret values are supplied through the
    /// referenced environment variables and are never projected.
    pub api_keys: Value,
    pub computers: Value,
    pub sessions: Value,
    pub session_settings: Value,
    pub models: Value,
    pub providers: Value,
    pub tools: Value,
    pub config_schema: Value,
    pub ui_surfaces: Value,
    /// BYOM (Bring Your Own Model) sources discovered from external providers
    /// via D-Bus projection (e.g., tched_router's model_routes)
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

    /// Read tched_router model_routes from D-Bus projection cache
    pub fn read_zeroclaw_model_routes() -> Option<Value> {
        op_blob::catalog::read_plugin_schema_shm("tched_router")?;

        // Read router projection from D-Bus via /dev/shm projection cache
        // The actual projection is written by op-dbus at /dev/shm/plugin-{name}.json
        let proj_bytes = std::fs::read("/dev/shm/plugin-tched_router.json").ok()?;
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
                    "source": "tched_router",
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

    /// Discover BYOM sources from the tched_router D-Bus projection
    fn discover_byom_sources() -> Value {
        match projection::read_zeroclaw_model_routes() {
            Some(routes) => projection::routes_to_byom_sources(&routes),
            None => {
                json!({
                    "sources": [],
                    "discovery_status": "tched_router_projection_unavailable",
                    "discovery_path": "/opdbus/v1/plugins/tched_router",
                    "note": "BYOM sources will appear when tched_router is projected via D-Bus"
                })
            }
        }
    }

    pub(crate) fn current_state() -> FactoryState {
        let endpoint = Self::env_or("FACTORY_API_ENDPOINT", "https://api.factory.ai/api/v0");
        let auth_method = Self::env_or("FACTORY_AUTH_METHOD", "bearer");
        let openrouter_configured =
            std::env::var_os("FACTORY_OR_API").is_some_and(|value| !value.is_empty());

        FactoryState {
            status: "active".to_string(),
            endpoint,
            auth_method,
            api_keys: json!({
                "openrouter": {
                    "env_var": "FACTORY_OR_API",
                    "configured": openrouter_configured,
                    "mode": "byok",
                    "project_secret": false
                }
            }),
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
                    "factory", "openrouter", "google", "xai", "voyage", "bedrock-converse"
                ]
            }),
            models: json!({
                "categories": [
                    {"id": "factory", "name": "Factory"},
                    {"id": "openrouter", "name": "OpenRouter"},
                    {"id": "openrouter-free", "name": "OpenRouter Free"}
                ],
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
                    {"id": "almond-05-27", "family": "factory-internal"},

                    {"id": "openrouter/auto", "family": "openrouter", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "anthropic/claude-sonnet-4.6", "family": "anthropic", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "anthropic/claude-opus-4.6", "family": "anthropic", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "anthropic/claude-opus-4.7", "family": "anthropic", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "anthropic/claude-opus-4.8", "family": "anthropic", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "google/gemini-2.5-pro", "family": "google", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "google/gemini-2.5-flash", "family": "google", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "google/gemini-3.5-flash", "family": "google", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "deepseek/deepseek-v4-pro", "family": "deepseek", "category": "openrouter", "billing": "byok", "requires_api_key": true},
                    {"id": "deepseek/deepseek-v4-flash", "family": "deepseek", "category": "openrouter", "billing": "byok", "requires_api_key": true},

                    {"id": "openrouter/free", "family": "openrouter", "category": "openrouter-free"},
                    {"id": "tencent/hy3:free", "family": "tencent", "category": "openrouter-free"},
                    {"id": "poolside/laguna-xs-2.1:free", "family": "poolside", "category": "openrouter-free"},
                    {"id": "cohere/north-mini-code:free", "family": "cohere", "category": "openrouter-free"},
                    {"id": "nvidia/nemotron-3-ultra-550b-a55b:free", "family": "nvidia", "category": "openrouter-free"},
                    {"id": "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free", "family": "nvidia", "category": "openrouter-free"},
                    {"id": "poolside/laguna-m.1:free", "family": "poolside", "category": "openrouter-free"},
                    {"id": "google/gemma-4-26b-a4b-it:free", "family": "google", "category": "openrouter-free"},
                    {"id": "google/gemma-4-31b-it:free", "family": "google", "category": "openrouter-free"},
                    {"id": "nvidia/nemotron-3-super-120b-a12b:free", "family": "nvidia", "category": "openrouter-free"},
                    {"id": "nvidia/nemotron-3-nano-30b-a3b:free", "family": "nvidia", "category": "openrouter-free"},
                    {"id": "nvidia/nemotron-nano-12b-v2-vl:free", "family": "nvidia", "category": "openrouter-free"},
                    {"id": "nvidia/nemotron-nano-9b-v2:free", "family": "nvidia", "category": "openrouter-free"},
                    {"id": "openai/gpt-oss-20b:free", "family": "openai", "category": "openrouter-free"}
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
                    ,{"hint": "auto", "provider": "openrouter", "model": "openrouter/auto", "kind": "router", "available": openrouter_configured, "billing": "byok", "api_key_env": "FACTORY_OR_API", "status_reason": if openrouter_configured { "OpenRouter BYOK auto-router is configured" } else { "Set FACTORY_OR_API to enable OpenRouter BYOK models" }}
                    ,{"hint": "free", "provider": "openrouter", "model": "openrouter/free", "kind": "router", "available": openrouter_configured, "billing": "free", "api_key_env": "FACTORY_OR_API", "status_reason": if openrouter_configured { "OpenRouter selects an available free model" } else { "Set FACTORY_OR_API to use the OpenRouter free router" }}
                ]
            }),
            providers: json!([
                {
                    "id": "factory", "route": "factory", "kind": "orchestrator",
                    "aliases": ["default", "auto", "droid"],
                    "endpoint": "https://api.factory.ai/api/v0", "auth": "bearer"
                },
                {
                    "id": "openrouter", "route": "openrouter", "kind": "router",
                    "aliases": ["open-router"],
                    "endpoint": "https://openrouter.ai/api/v1", "auth": "bearer",
                    "credential_mode": "byok", "api_key_env": "FACTORY_OR_API",
                    "configured": openrouter_configured
                },
                {
                    "id": "tched_router", "route": "tched_router", "kind": "byom_router",
                    "aliases": ["byom", "local", "openrouter", "ollama"],
                    "endpoint": "/opdbus/v1/plugins/tched_router", "auth": "dbus_projection",
                    "description": "BYOM source - discovers models from tched_router D-Bus projection",
                    "discovery_plugin": "tched_router",
                    "discovery_path": "/opdbus/v1/plugins/tched_router"
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
        .category("service")
        .version("1.0.0")
        .category("llm")
        .description("Factory Droid agent platform — computers, sessions, models, autonomy controls, BYOM discovery");

    // Add BYOM dependency on zeroclaw for model discovery via D-Bus projection
    builder = builder.dependency("tched_router");

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

    schema.capabilities.insert(
        "factory.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "factory.read".to_string(),
            description: "Grants: list_computers, get_session, list_sessions, list_models."
                .to_string(),
        },
    );
    schema.capabilities.insert(
        "factory.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "factory.invoke".to_string(),
            description: "Grants: discover_byom, set_autonomy.".to_string(),
        },
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("factory", |_ctx| std::sync::Arc::new(FactoryPlugin::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::prelude::*;

    #[test]
    fn openrouter_byok_and_free_models_have_distinct_categories() {
        let state = FactoryPlugin::current_state();
        let catalog = state
            .models
            .get("catalog")
            .and_then(Value::as_array)
            .unwrap();

        let paid = catalog
            .iter()
            .filter(|model| model.get("category").and_then(Value::as_str) == Some("openrouter"));
        assert!(paid.count() > 0);
        assert!(catalog
            .iter()
            .filter(|model| { model.get("category").and_then(Value::as_str) == Some("openrouter") })
            .all(|model| {
                model.get("billing").and_then(Value::as_str) == Some("byok")
                    && model.get("requires_api_key").and_then(Value::as_bool) == Some(true)
            }));

        let free: Vec<_> = catalog
            .iter()
            .filter(|model| {
                model.get("category").and_then(Value::as_str) == Some("openrouter-free")
            })
            .collect();
        assert!(!free.is_empty());
        assert!(free.iter().all(|model| {
            let id = model.get("id").and_then(Value::as_str).unwrap_or_default();
            id == "openrouter/free" || id.ends_with(":free")
        }));
    }

    #[test]
    fn openrouter_provider_uses_non_secret_byok_reference() {
        let state = FactoryPlugin::current_state();
        let credential = state.api_keys.get("openrouter").unwrap();

        assert_eq!(
            credential.get("env_var").and_then(Value::as_str),
            Some("FACTORY_OR_API")
        );
        assert_eq!(credential.get("mode").and_then(Value::as_str), Some("byok"));
        assert_eq!(
            credential.get("project_secret").and_then(Value::as_bool),
            Some(false)
        );
    }
}
