//! Zeroclaw route surface plugin.
//!
//! Publishes the Antigravity-facing model and CLI routing contract through
//! `PluginSchema` so the UI can render provider/model controls from D-Bus.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

use super::common::llm_projection::{
    ConfigSchema, LlmProjection, LlmTool, LlmTransport, ModelRoute, Provider, Router,
    SelectionInput, SelectionOutput, SelectorPolicy, StructuredOutput, UiSurface,
};
use super::plugin_schema_defs::{
    empty_args, mutation_method, plugin_schema_from_schemars, read_method, signal_decl,
};

/// The Zeroclaw plugin's typed state. This struct IS the schema contract: the
/// `PluginSchema` field set is generated entirely from
/// `schemars::schema_for!(ZeroclawState)` via `plugin_schema_from_schemars`
/// (spec §8). Adding a field here is the only way to change the schema.
///
/// `LlmProjection` is `#[serde(flatten)]`ed so the serialized shape stays flat
/// (providers/model_routes/router/tools/... at top level), matching the design
/// §2 projection tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ZeroclawState {
    pub status: String,
    pub selected_provider: String,
    pub selected_model: String,
    pub transport: LlmTransport,
    #[serde(flatten)]
    pub projection: LlmProjection,
    /// subid: sch.software.zeroclaw.selector-policy@v1
    pub selector_policy: SelectorPolicy,
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

    fn declared_route(
        hint: &str,
        provider: &str,
        upstream_provider: &str,
        transport: &str,
        model: &str,
        kind: &str,
        available: bool,
        status_reason: &str,
    ) -> ModelRoute {
        ModelRoute {
            hint: hint.to_string(),
            provider: provider.to_string(),
            upstream_provider: upstream_provider.to_string(),
            transport: transport.to_string(),
            model: model.to_string(),
            kind: kind.to_string(),
            status: "declared".to_string(),
            available,
            status_reason: status_reason.to_string(),
            source: None,
            api_key: None,
            ..ModelRoute::default()
        }
    }

    pub(crate) fn current_state() -> ZeroclawState {
        // Decoupled from factory: local routing defaults to the on-box gemma4
        // via ollama. gemma4 is also the universal router (see `router` below).
        let selected_provider = Self::env_or("LLM_PROVIDER", "ollama");
        let selected_model = Self::env_or("LLM_MODEL", "gemma4");
        let router_endpoint = Self::env_or("ZEROCLAW_ROUTER_ENDPOINT", "http://localhost:11434");
        let wg_xray_target = Self::env_or("ZEROCLAW_WG_XRAY_TARGET", "wg-xray");
        let grpc_target = Self::env_or("ZEROCLAW_GRPC_TARGET", "http://10.200.0.1:50051");

        let provider = |id: &str, route: &str, kind: &str, aliases: &[&str]| Provider {
            id: id.to_string(),
            route: route.to_string(),
            kind: kind.to_string(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            source: None,
        };

        let mut oscal_provider = provider(
            "oscal",
            "oscal",
            "policy",
            &["compliance", "subid", "nist"],
        );
        // OSCAL compliance routing: a policy-kind provider (assess, not chat).
        // `source` points at the canonical subid registry — the mapping
        // authority — instead of restating control mappings here.
        oscal_provider.source = Some("/org/opdbus/v1/plugins/oscal_subid_registry".to_string());

        let providers = vec![
            provider("factory", "factory", "orchestrator", &["default", "auto"]),
            provider("codex", "openai-codex", "provider", &["openai_codex", "codex"]),
            provider("ollama", "ollama", "local", &["gemma", "gemma4"]),
            provider("antigravity", "antigravity", "orchestrator", &["google.antigravity"]),
            provider("google", "gemini", "provider", &["gemini", "google-gemini"]),
            provider("opencode", "custom:opencode", "cli", &[]),
            provider("kilocode", "custom:kilocode", "cli", &["kilo-code", "kilo_code"]),
            provider("openrouter", "openrouter", "router", &[]),
            provider("anthropic", "anthropic", "provider", &[]),
            provider("openai", "openai", "provider", &[]),
            provider("gemini-cli", "gemini-cli", "cli", &[]),
            oscal_provider,
        ];

        // gemma4 is the universal router: it classifies EVERY request into a
        // route/tag (provider+model, hint, candidate OSCAL subids). It only
        // emits tags — the deterministic layer (routes below, oscal registry,
        // xray) enforces them. Not just compliance routing.
        let router = Router {
            provider: "ollama".to_string(),
            model: "gemma4".to_string(),
            endpoint: router_endpoint,
            scope: "all".to_string(),
            role: "context_aware_allocator".to_string(),
            emits: [
                "provider",
                "model",
                "hint",
                "candidate_subids",
                "confidence",
                "thinking_budget",
                "reasoning_effort",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        };

        let mut compliance_route = Self::declared_route(
            "compliance",
            "oscal",
            "oscal",
            "dbus",
            "NIST_SP_800-53_Rev5",
            "policy",
            false,
            "OSCAL policy provider is declared; unavailable until oscal_subid_registry is projected.",
        );
        compliance_route.source = Some("/org/opdbus/v1/plugins/oscal_subid_registry".to_string());

        let model_routes = vec![
            Self::declared_route(
                "code", "openrouter", "openrouter", "direct",
                "anthropic/claude-sonnet-4.6", "chat", false,
                "Declared route; backend availability must be projected before execution.",
            ),
            Self::declared_route(
                "fast", "gemini", "gemini", "direct", "gemini-2.5-flash", "chat", false,
                "Declared route; backend availability must be projected before execution.",
            ),
            Self::declared_route(
                "local", "ollama", "ollama", "loopback", "gemma4", "router", false,
                "gemma4 is the declared local classifier; unavailable until Ollama projects it.",
            ),
            // Factory auto-routing: selects best available provider based on query type
            Self::declared_route(
                "auto", "factory", "factory", "auto", "auto", "orchestrator", true,
                "Factory auto-router available - selects best provider based on query classification.",
            ),
            Self::declared_route(
                "code", "factory", "openrouter", "direct",
                "anthropic/claude-sonnet-4.6", "chat", false,
                "Factory code route -> openrouter/claude; requires backend projection.",
            ),
            Self::declared_route(
                "fast", "factory", "gemini", "direct", "gemini-2.5-flash", "chat", false,
                "Factory fast route -> gemini/flash; requires backend projection.",
            ),
            Self::declared_route(
                "local", "factory", "ollama", "loopback", "gemma4", "router", false,
                "Factory local route -> ollama/gemma4; requires backend projection.",
            ),
            Self::declared_route(
                "reasoning", "factory", "antigravity", "direct",
                "gemini-3-pro-preview", "chat", false,
                "Factory reasoning route; Gemma dynamically allocates thinking budget.",
            ),
            // Compliance baseline routed through the OSCAL policy provider; the
            // model id is a canonical OSCAL profile reference resolved via the
            // subid registry, not an LLM model.
            compliance_route,
        ];

        let tools = vec![
            LlmTool {
                name: "zeroclaw.chat".to_string(),
                description: "Send an Antigravity/Zeroclaw chat turn and return structured JSON when response_schema is present.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"},
                        "provider": {"type": "string"},
                        "model": {"type": "string"},
                        "response_schema": {"type": "object"}
                    },
                    "required": ["message"]
                }),
            },
            LlmTool {
                name: "zeroclaw.models.list".to_string(),
                description: "List cached or live models for a Zeroclaw provider.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {"provider": {"type": "string"}},
                    "required": []
                }),
            },
        ];

        let config_schema = ConfigSchema {
            source: "zeroclaw config schema".to_string(),
            schema_crate: "schemars".to_string(),
            native_type: "zeroclaw::config::schema::Config".to_string(),
            status: "available_via_cli_or_gateway".to_string(),
        };

        let ui_surfaces = vec![
            UiSurface {
                path: "/chat".to_string(),
                name: "Antigravity Chat".to_string(),
                schema: "zeroclaw".to_string(),
            },
            UiSurface {
                path: "/grpc".to_string(),
                name: "gRPC Diagnostics".to_string(),
                schema: "plugin-service".to_string(),
            },
            UiSurface {
                path: "/models".to_string(),
                name: "Routable Models".to_string(),
                schema: "zeroclaw.providers".to_string(),
            },
        ];

        let structured_output = StructuredOutput {
            sdk: "google.antigravity".to_string(),
            config_object: "LocalAgentConfig(response_schema=UiResponseSchema)".to_string(),
            extractor: "response.structured_output()".to_string(),
            response_schema: serde_json::json!({
                "action": "",
                "metadata": {},
                "confidence_score": 0.0
            }),
            pydantic_source: vec![
                "import pydantic".to_string(),
                "class UiResponseSchema(pydantic.BaseModel):".to_string(),
                "    action: str".to_string(),
                "    metadata: dict[str, str]".to_string(),
                "    confidence_score: float".to_string(),
            ],
            ui_renderer: "JsonRenderer".to_string(),
            required: true,
        };

        ZeroclawState {
            status: "declared".to_string(),
            selected_provider,
            selected_model,
            transport: LlmTransport {
                dbus_object: "/org/opdbus/v1/plugins/zeroclaw".to_string(),
                grpc_target,
                incus_container: wg_xray_target,
                browser_surface: "gRPC-Web through op-web".to_string(),
                rest_aliases: vec![
                    "/api/zeroclaw/chat".to_string(),
                    "/api/llm/chat".to_string(),
                ],
                // Canonical OSCAL/subid mapping authority — referenced, never copied.
                policy_source: "/org/opdbus/v1/plugins/oscal_subid_registry".to_string(),
            },
            projection: LlmProjection {
                providers,
                router,
                model_routes,
                tools,
                config_schema,
                ui_surfaces,
                structured_output,
            },
            selector_policy: SelectorPolicy::default(),
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
        Some(zeroclaw_plugin_schema())
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

/// The Zeroclaw `PluginSchema` — **defined in the plugin** (the plugin IS the
/// schema). The field set is generated entirely from
/// `schemars::schema_for!(ZeroclawState)` via the generic
/// `plugin_schema_from_schemars` converter (spec §8); only the declared method
/// surface (§3), signals, guarantees, and the authoritative `subids` map (§13)
/// are supplied here. `schema_from_state()` is NOT used for Zeroclaw.
pub(crate) fn zeroclaw_plugin_schema() -> PluginSchema {
    let selection_input_args = serde_json::to_value(schemars::schema_for!(SelectionInput))
        .unwrap_or_else(|_| serde_json::json!({"type": "object"}));
    let selection_output_ret = serde_json::to_value(schemars::schema_for!(SelectionOutput))
        .unwrap_or_else(|_| serde_json::json!({"type": "object"}));

    let mut select_model = mutation_method(
        "SelectModel",
        "mut.service.zeroclaw.select-model@v1",
        selection_input_args,
        Some("zeroclaw.select-model"),
        false,
    );
    select_model.returns = Some(selection_output_ret);

    let mut authorize_execution = mutation_method(
        "AuthorizeExecution",
        "mut.service.zeroclaw.authorize-execution@v1",
        serde_json::json!({
            "type": "object",
            "properties": {
                "provider": {"type": "string"},
                "model": {"type": "string"},
                "tool": {"type": "string"}
            },
            "required": ["provider", "model"]
        }),
        Some("zeroclaw.authorize-execution"),
        false,
    );
    authorize_execution.returns = Some(serde_json::json!({
        "type": "object",
        "properties": {"authorized": {"type": "boolean"}, "reason": {"type": "string"}},
        "required": ["authorized"]
    }));

    let methods = HashMap::from([
        (
            "GetState".to_string(),
            read_method(
                "GetState",
                "obs.service.zeroclaw.get-state@v1",
                empty_args(),
                None,
            ),
        ),
        (
            "ResolveRoute".to_string(),
            read_method(
                "ResolveRoute",
                "obs.service.zeroclaw.resolve-route@v1",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "hint": {"type": "string"},
                        "context_tokens": {"type": "integer"},
                        "task_class": {"type": "string"}
                    },
                    "required": ["hint"]
                }),
                None,
            ),
        ),
        (
            "GetProviderCatalog".to_string(),
            read_method(
                "GetProviderCatalog",
                "obs.service.zeroclaw.provider-catalog@v1",
                empty_args(),
                None,
            ),
        ),
        (
            "GetModelRoutes".to_string(),
            read_method(
                "GetModelRoutes",
                "obs.service.zeroclaw.model-routes@v1",
                empty_args(),
                None,
            ),
        ),
        (
            "GetTools".to_string(),
            read_method("GetTools", "obs.service.zeroclaw.tools@v1", empty_args(), None),
        ),
        ("SelectModel".to_string(), select_model),
        ("AuthorizeExecution".to_string(), authorize_execution),
        (
            "SetProvider".to_string(),
            mutation_method(
                "SetProvider",
                "mut.service.zeroclaw.set-provider@v1",
                serde_json::json!({
                    "type": "object",
                    "properties": {"provider_id": {"type": "string"}},
                    "required": ["provider_id"]
                }),
                Some("zeroclaw.set-provider"),
                false,
            ),
        ),
        (
            "SetModel".to_string(),
            mutation_method(
                "SetModel",
                "mut.service.zeroclaw.set-model@v1",
                serde_json::json!({
                    "type": "object",
                    "properties": {"model_id": {"type": "string"}},
                    "required": ["model_id"]
                }),
                Some("zeroclaw.set-model"),
                false,
            ),
        ),
    ]);

    let signals = vec![
        signal_decl(
            "ProviderChanged",
            "evt.service.zeroclaw.provider-changed@v1",
            Some(serde_json::json!({
                "type": "object",
                "properties": {"old": {"type": "string"}, "new": {"type": "string"}, "reason": {"type": "string"}, "actor_id": {"type": "string"}}
            })),
        ),
        signal_decl(
            "ModelChanged",
            "evt.service.zeroclaw.model-changed@v1",
            Some(serde_json::json!({
                "type": "object",
                "properties": {"old": {"type": "string"}, "new": {"type": "string"}, "reason": {"type": "string"}, "actor_id": {"type": "string"}}
            })),
        ),
        signal_decl(
            "RouteHealthChanged",
            "evt.service.zeroclaw.route-health-changed@v1",
            Some(serde_json::json!({
                "type": "object",
                "properties": {"route_hint": {"type": "string"}, "available": {"type": "boolean"}, "reason": {"type": "string"}}
            })),
        ),
        signal_decl(
            "ExecutionAuthorized",
            "evt.service.zeroclaw.execution-authorized@v1",
            Some(serde_json::json!({
                "type": "object",
                "properties": {"provider": {"type": "string"}, "model": {"type": "string"}, "tool": {"type": "string"}, "trace_id": {"type": "string"}}
            })),
        ),
        signal_decl(
            "ExecutionDenied",
            "evt.service.zeroclaw.execution-denied@v1",
            Some(serde_json::json!({
                "type": "object",
                "properties": {"provider": {"type": "string"}, "model": {"type": "string"}, "tool": {"type": "string"}, "reason": {"type": "string"}, "trace_id": {"type": "string"}}
            })),
        ),
    ];

    let guarantees = PluginCapabilities {
        supports_rollback: true,
        supports_checkpoints: true,
        supports_verification: true,
        atomic_operations: false,
    };

    // The `subids` map is the single subid authority (spec §13.1). Field subids
    // (incl. nested route/selector fields and the schema-typed structs) are
    // declared here; method/signal subids are backfilled from their decls by
    // `plugin_schema_from_schemars`.
    let subids: HashMap<String, String> = [
        // ── top-level ZeroclawState fields ──────────────────────────────────
        ("status", "obs.software.zeroclaw.status@v1"),
        ("selected_provider", "mut.service.zeroclaw.selected-provider@v1"),
        ("selected_model", "exp.service.zeroclaw.selected-model@v1"),
        ("transport", "prj.service.zeroclaw.transport@v1"),
        ("providers", "obs.software.zeroclaw.providers@v1"),
        ("router", "sch.software.zeroclaw.router@v1"),
        ("model_routes", "sch.software.zeroclaw.model-routes@v1"),
        ("tools", "sch.software.zeroclaw.tools@v1"),
        ("config_schema", "sch.software.zeroclaw.config-schema@v1"),
        ("ui_surfaces", "exp.software.zeroclaw.ui-surfaces@v1"),
        ("structured_output", "sch.software.zeroclaw.structured-output@v1"),
        ("selector_policy", "sch.software.zeroclaw.selector-policy@v1"),
        // ── ModelRoute selector fields (§13.2) ──────────────────────────────
        ("cost_profile", "obs.software.llm-model-route.cost-profile@v1"),
        ("effort_level", "obs.software.llm-model-route.effort-level@v1"),
        ("latency_class", "obs.software.llm-model-route.latency-class@v1"),
        ("privacy_tier", "obs.software.llm-model-route.privacy-tier@v1"),
        ("context_window", "obs.software.llm-model-route.context-window@v1"),
        ("health_score", "obs.software.llm-model-route.health-score@v1"),
        ("fallback_routes", "obs.software.llm-model-route.fallback-routes@v1"),
        ("tool_support", "obs.software.llm-model-route.tool-support@v1"),
        // ── SelectorPolicy fields (§13.2) ───────────────────────────────────
        ("effort_weight", "sch.software.selector-policy.effort-weight@v1"),
        ("cost_weight", "sch.software.selector-policy.cost-weight@v1"),
        ("latency_weight", "sch.software.selector-policy.latency-weight@v1"),
        ("health_weight", "sch.software.selector-policy.health-weight@v1"),
        ("default_effort", "sch.software.selector-policy.default-effort@v1"),
        ("default_privacy_tier", "sch.software.selector-policy.default-privacy@v1"),
        // ── schema-typed structs (§13.2) ────────────────────────────────────
        ("SelectionInput", "sch.software.llm-selection-input.schema@v1"),
        ("SelectionOutput", "sch.software.llm-selection-output.schema@v1"),
        ("SelectionEvent", "evt.service.zeroclaw.selection-event@v1"),
        ("ZeroclawError", "sch.software.zeroclaw-error.schema@v1"),
        // ── derived D-Bus property (§3) ─────────────────────────────────────
        ("SchemaJson", "prj.service.zeroclaw.schema-json@v1"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();

    plugin_schema_from_schemars::<ZeroclawState>(
        "zeroclaw",
        "llm",
        "1.0.0",
        "Zeroclaw schema/RPC-native model router for Antigravity UI, CLI providers, and structured JSON output",
        methods,
        signals,
        guarantees,
        subids,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_props_json(
        node: &serde_json::Value,
        defs: &serde_json::Value,
        out: &mut Vec<String>,
        depth: u8,
    ) {
        if depth > 12 {
            return;
        }
        if let Some(reference) = node.get("$ref").and_then(|r| r.as_str()) {
            if let Some(name) = reference.strip_prefix("#/definitions/") {
                if let Some(target) = defs.get(name) {
                    collect_props_json(target, defs, out, depth + 1);
                }
            }
            return;
        }
        if let Some(props) = node.get("properties").and_then(|p| p.as_object()) {
            for k in props.keys() {
                out.push(k.clone());
            }
        }
        if let Some(all_of) = node.get("allOf").and_then(|a| a.as_array()) {
            for s in all_of {
                collect_props_json(s, defs, out, depth + 1);
            }
        }
    }

    #[test]
    fn zeroclaw_schema_golden() {
        let schema = zeroclaw_plugin_schema();

        // Key route/selector fields must be present in the generated field set.
        for required in ["model_routes", "providers", "selector_policy", "router", "tools"] {
            assert!(
                schema.fields.contains_key(required),
                "schemars-generated PluginSchema is missing field `{required}`"
            );
        }

        // subids values are the single authority and MUST be unique (§13.1).
        let mut seen = std::collections::HashSet::new();
        for (name, subid) in &schema.subids {
            assert!(
                seen.insert(subid.clone()),
                "duplicate subid `{subid}` (key `{name}`)"
            );
        }

        // PluginSchema.fields MUST match the schemars property set exactly.
        let root = serde_json::to_value(schemars::schema_for!(ZeroclawState)).unwrap();
        let defs = root
            .get("definitions")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mut prop_names = Vec::new();
        collect_props_json(&root, &defs, &mut prop_names, 0);
        prop_names.sort();
        prop_names.dedup();
        let mut field_names: Vec<String> = schema.fields.keys().cloned().collect();
        field_names.sort();
        assert_eq!(
            field_names, prop_names,
            "PluginSchema.fields must equal the schemars::schema_for!(ZeroclawState) property set"
        );

        // SelectModel/SelectModel return shapes derive from SelectionInput/Output.
        let select = schema.methods.get("SelectModel").expect("SelectModel method");
        let args = serde_json::to_string(&select.args).unwrap();
        assert!(args.contains("task_class") && args.contains("requested_effort"));
        let returns = serde_json::to_string(select.returns.as_ref().unwrap()).unwrap();
        assert!(returns.contains("selected_provider") && returns.contains("confidence"));
    }
}
