//! Zeroclaw route surface plugin.
//!
//! Publishes the Antigravity-facing model and CLI routing contract through
//! `PluginSchema` so the UI can render provider/model controls from D-Bus.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use super::plugin_schema_defs::schema_from_state;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroclawState {
    pub status: String,
    pub selected_provider: String,
    pub selected_model: String,
    pub transport: Value,
    pub providers: Value,
    pub router: Value,
    pub model_routes: Value,
    pub tools: Value,
    pub config_schema: Value,
    pub ui_surfaces: Value,
    pub structured_output: Value,
}

pub struct ZeroclawPlugin;

impl Default for ZeroclawPlugin {
    fn default() -> Self {
        Self
    }
}

impl ZeroclawPlugin {
    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    pub(crate) fn current_state() -> ZeroclawState {
        // Decoupled from factory: local routing defaults to the on-box gemma4
        // via ollama. gemma4 is also the universal router (see `router` below).
        let selected_provider = Self::env_or("LLM_PROVIDER", "ollama");
        let selected_model = Self::env_or("LLM_MODEL", "gemma4");
        let router_endpoint = Self::env_or("ZEROCLAW_ROUTER_ENDPOINT", "http://localhost:11434");
        let wg_xray_target = Self::env_or("ZEROCLAW_WG_XRAY_TARGET", "wg-xray");
        let grpc_target = Self::env_or("ZEROCLAW_GRPC_TARGET", "http://10.200.0.1:50051");

        ZeroclawState {
            status: "declared".to_string(),
            selected_provider,
            selected_model,
            transport: json!({
                "dbus_object": "/opdbus/v1/plugins/zeroclaw",
                "grpc_target": grpc_target,
                "incus_container": wg_xray_target,
                "browser_surface": "gRPC-Web through op-web",
                "rest_aliases": ["/api/zeroclaw/chat", "/api/llm/chat"],
                // Canonical OSCAL/subid mapping authority — referenced, never copied.
                "policy_source": "/opdbus/v1/plugins/oscal_subid_registry",
            }),
            providers: json!([
                {"id": "factory", "route": "factory", "kind": "orchestrator", "aliases": ["default", "auto"]},
                {"id": "codex", "route": "openai-codex", "kind": "provider", "aliases": ["openai_codex", "codex"]},
                {"id": "ollama", "route": "ollama", "kind": "local", "aliases": ["gemma", "gemma4"]},
                {"id": "antigravity", "route": "antigravity", "kind": "orchestrator", "aliases": ["google.antigravity"]},
                {"id": "google", "route": "gemini", "kind": "provider", "aliases": ["gemini", "google-gemini"]},
                {"id": "opencode", "route": "custom:opencode", "kind": "cli", "aliases": []},
                {"id": "kilocode", "route": "custom:kilocode", "kind": "cli", "aliases": ["kilo-code", "kilo_code"]},
                {"id": "openrouter", "route": "openrouter", "kind": "router", "aliases": []},
                {"id": "anthropic", "route": "anthropic", "kind": "provider", "aliases": []},
                {"id": "openai", "route": "openai", "kind": "provider", "aliases": []},
                {"id": "gemini-cli", "route": "gemini-cli", "kind": "cli", "aliases": []},
                // OSCAL compliance routing: a policy-kind provider (assess, not chat).
                // `source` points at the canonical subid registry — the mapping
                // authority — instead of restating control mappings here.
                {"id": "oscal", "route": "oscal", "kind": "policy", "aliases": ["compliance", "subid", "nist"], "source": "/opdbus/v1/plugins/oscal_subid_registry"}
            ]),
            // gemma4 is the universal router: it classifies EVERY request into a
            // route/tag (provider+model, hint, candidate OSCAL subids). It only
            // emits tags — the deterministic layer (routes below, oscal registry,
            // xray) enforces them. Not just compliance routing.
            router: json!({
                "provider": "ollama",
                "model": "gemma4",
                "endpoint": router_endpoint,
                "scope": "all",
                "role": "context_aware_allocator",
                "emits": ["provider", "model", "hint", "candidate_subids", "confidence", "thinking_budget", "reasoning_effort"],
            }),
            model_routes: json!([
                {
                    "hint": "code",
                    "provider": "openrouter",
                    "upstream_provider": "openrouter",
                    "transport": "direct",
                    "model": "anthropic/claude-sonnet-4.6",
                    "kind": "chat",
                    "status": "declared",
                    "available": false,
                    "status_reason": "Declared route; backend availability must be projected before execution.",
                    "api_key": null
                },
                {
                    "hint": "fast",
                    "provider": "gemini",
                    "upstream_provider": "gemini",
                    "transport": "direct",
                    "model": "gemini-2.5-flash",
                    "kind": "chat",
                    "status": "declared",
                    "available": false,
                    "status_reason": "Declared route; backend availability must be projected before execution.",
                    "api_key": null
                },
                {
                    "hint": "local",
                    "provider": "ollama",
                    "upstream_provider": "ollama",
                    "transport": "loopback",
                    "model": "gemma4",
                    "kind": "router",
                    "status": "declared",
                    "available": false,
                    "status_reason": "gemma4 is the declared local classifier; unavailable until Ollama projects it.",
                    "api_key": null
                },
                // Factory auto-routing: selects best available provider based on query type
                {
                    "hint": "auto",
                    "provider": "factory",
                    "upstream_provider": "factory",
                    "transport": "auto",
                    "model": "auto",
                    "kind": "orchestrator",
                    "status": "declared",
                    "available": true,
                    "status_reason": "Factory auto-router available - selects best provider based on query classification.",
                    "api_key": null
                },
                {
                    "hint": "code",
                    "provider": "factory",
                    "upstream_provider": "openrouter",
                    "transport": "direct",
                    "model": "anthropic/claude-sonnet-4.6",
                    "kind": "chat",
                    "status": "declared",
                    "available": false,
                    "status_reason": "Factory code route -> openrouter/claude; requires backend projection.",
                    "api_key": null
                },
                {
                    "hint": "fast",
                    "provider": "factory",
                    "upstream_provider": "gemini",
                    "transport": "direct",
                    "model": "gemini-2.5-flash",
                    "kind": "chat",
                    "status": "declared",
                    "available": false,
                    "status_reason": "Factory fast route -> gemini/flash; requires backend projection.",
                    "api_key": null
                },
                {
                    "hint": "local",
                    "provider": "factory",
                    "upstream_provider": "ollama",
                    "transport": "loopback",
                    "model": "gemma4",
                    "kind": "router",
                    "status": "declared",
                    "available": false,
                    "status_reason": "Factory local route -> ollama/gemma4; requires backend projection.",
                    "api_key": null
                },
                {
                    "hint": "reasoning",
                    "provider": "factory",
                    "upstream_provider": "antigravity",
                    "transport": "direct",
                    "model": "gemini-3-pro-preview",
                    "kind": "chat",
                    "status": "declared",
                    "available": false,
                    "status_reason": "Factory reasoning route; Gemma dynamically allocates thinking budget.",
                    "api_key": null
                },
                // Compliance baseline routed through the OSCAL policy provider; the
                // model id is a canonical OSCAL profile reference resolved via the
                // subid registry, not an LLM model.
                {
                    "hint": "compliance",
                    "provider": "oscal",
                    "upstream_provider": "oscal",
                    "transport": "dbus",
                    "model": "NIST_SP_800-53_Rev5",
                    "kind": "policy",
                    "status": "declared",
                    "available": false,
                    "status_reason": "OSCAL policy provider is declared; unavailable until oscal_subid_registry is projected.",
                    "source": "/opdbus/v1/plugins/oscal_subid_registry",
                    "api_key": null
                }
            ]),
            tools: json!([
                {
                    "name": "zeroclaw.chat",
                    "description": "Send an Antigravity/Zeroclaw chat turn and return structured JSON when response_schema is present.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "message": {"type": "string"},
                            "provider": {"type": "string"},
                            "model": {"type": "string"},
                            "response_schema": {"type": "object"}
                        },
                        "required": ["message"]
                    }
                },
                {
                    "name": "zeroclaw.models.list",
                    "description": "List cached or live models for a Zeroclaw provider.",
                    "parameters": {
                        "type": "object",
                        "properties": {"provider": {"type": "string"}},
                        "required": []
                    }
                }
            ]),
            config_schema: json!({
                "source": "zeroclaw config schema",
                "schema_crate": "schemars",
                "native_type": "zeroclaw::config::schema::Config",
                "status": "available_via_cli_or_gateway"
            }),
            ui_surfaces: json!([
                {"path": "/chat", "name": "Antigravity Chat", "schema": "zeroclaw"},
                {"path": "/grpc", "name": "gRPC Diagnostics", "schema": "plugin-service"},
                {"path": "/models", "name": "Routable Models", "schema": "zeroclaw.providers"}
            ]),
            structured_output: json!({
                "sdk": "google.antigravity",
                "config_object": "LocalAgentConfig(response_schema=UiResponseSchema)",
                "extractor": "response.structured_output()",
                "response_schema": {
                    "action": "",
                    "metadata": {},
                    "confidence_score": 0.0
                },
                "pydantic_source": [
                    "import pydantic",
                    "class UiResponseSchema(pydantic.BaseModel):",
                    "    action: str",
                    "    metadata: dict[str, str]",
                    "    confidence_score: float"
                ],
                "ui_renderer": "JsonRenderer",
                "required": true
            }),
        }
    }
}

#[async_trait]
impl StatePlugin for ZeroclawPlugin {
    fn name(&self) -> &str {
        "zeroclaw"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(zeroclaw_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(Self::current_state())?)
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

pub(crate) fn zeroclaw_schema() -> PluginSchema {
    // The plugin IS the schema: derive directly from ZeroclawPlugin's live
    // state. Nothing is indexed here — adding a field to ZeroclawState is the
    // only way to change this schema.
    let state = simd_json::serde::to_owned_value(super::zeroclaw::ZeroclawPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "zeroclaw",
        "llm",
        "1.0.0",
        "Zeroclaw schema/RPC-native model router for Antigravity UI, CLI providers, and structured JSON output",
        &state,
    )
}
