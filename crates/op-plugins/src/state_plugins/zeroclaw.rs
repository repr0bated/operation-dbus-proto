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

use super::common::errors::ZeroclawError;
use super::common::llm_projection::{
    ConfigSchema, LlmProjection, LlmTool, LlmTransport, ModelRoute, Provider, Router,
    SelectionInput, SelectionOutput, SelectorPolicy, StructuredOutput, UiSurface,
};
use super::common::selector::select_model;
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

    #[allow(clippy::too_many_arguments)]
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

    /// The in-memory typed Zeroclaw state. This is the projection authority:
    /// the bridge projection hook and method dispatch read it directly here and
    /// never re-read `/dev/shm`.
    pub fn current_state() -> ZeroclawState {
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
///
/// Public so the single schema can be **included/embedded** by other crates
/// (e.g. `op-llm`) instead of duplicating the contract — the plugin remains the
/// sole source of truth, referenced in more than one place.
pub fn zeroclaw_plugin_schema() -> PluginSchema {
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

// ── Orchestration Layer (design §5, Layer 2) ────────────────────────────────
//
// The gRPC bridge auto-generates the D-Bus/gRPC *surface* from `PluginSchema`
// (method existence, arg-schema validation, `required_capability` gate) and
// validates each request before this layer runs. The bridge's generic
// dispatcher only records an accountability event and echoes args, so it
// supplies no domain decision — these handlers do, ensuring declared methods
// are never inert (mission hard rule).
//
// Every handler reads **only** the in-memory `ZeroclawState` passed in (never
// `/dev/shm`, never a network call), so execution decisions stay on the
// schema/state authority. Provider/model branching is data-driven from declared
// routes — there are no `match` arms on provider or model name.

/// A signal the bridge should emit after a successful dispatch, alongside the
/// method's JSON return value.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchSignal {
    pub name: String,
    pub payload: serde_json::Value,
}

/// Outcome of a plugin-owned method dispatch: the JSON return value plus any
/// declared signal the bridge should emit through its accountability pipeline.
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    pub result: serde_json::Value,
    pub signal: Option<DispatchSignal>,
}

impl DispatchOutcome {
    fn plain(result: serde_json::Value) -> Self {
        Self { result, signal: None }
    }
}

/// Plugin-owned entry point the bridge routes a validated D-Bus/gRPC method
/// call to. `method` is the verbatim declared method name; `json_args` is the
/// verbatim caller payload (already validated against the method's arg schema
/// at the bridge layer).
pub fn dispatch_zeroclaw_method(
    method: &str,
    json_args: &str,
    state: &ZeroclawState,
) -> Result<DispatchOutcome, ZeroclawError> {
    match method {
        "GetState" => Ok(DispatchOutcome::plain(to_json(state))),
        "GetModelRoutes" => Ok(DispatchOutcome::plain(to_json(&state.projection.model_routes))),
        "GetProviderCatalog" => {
            Ok(DispatchOutcome::plain(to_json(&state.projection.providers)))
        }
        "GetTools" => Ok(DispatchOutcome::plain(to_json(&state.projection.tools))),
        "ResolveRoute" => resolve_route(json_args, state).map(DispatchOutcome::plain),
        "SelectModel" => select_model_handler(json_args, state).map(DispatchOutcome::plain),
        "AuthorizeExecution" => authorize_execution_handler(json_args, state),
        "SetProvider" => set_provider_handler(json_args, state),
        "SetModel" => set_model_handler(json_args, state),
        other => Err(ZeroclawError::ExecutionDenied {
            reason: format!("undeclared method: {other}"),
        }),
    }
}

fn to_json<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

fn parse_args(method: &str, json_args: &str) -> Result<serde_json::Value, ZeroclawError> {
    if json_args.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(json_args).map_err(|e| ZeroclawError::ExecutionDenied {
        reason: format!("invalid json_args for {method}: {e}"),
    })
}

fn require_str(
    args: &serde_json::Value,
    field: &str,
    method: &str,
) -> Result<String, ZeroclawError> {
    args.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ZeroclawError::ExecutionDenied {
            reason: format!("{method}: missing required field '{field}'"),
        })
}

/// `ResolveRoute`: resolve a route hint against the declared `model_routes`.
/// An undeclared hint is rejected (never silently substituted); a declared but
/// unavailable route is reported with its `status_reason`.
fn resolve_route(
    json_args: &str,
    state: &ZeroclawState,
) -> Result<serde_json::Value, ZeroclawError> {
    let args = parse_args("ResolveRoute", json_args)?;
    let hint = require_str(&args, "hint", "ResolveRoute")?;

    let route = state
        .projection
        .model_routes
        .iter()
        .find(|r| r.hint == hint)
        .ok_or_else(|| ZeroclawError::RouteNotDeclared { hint: hint.clone() })?;

    if !route.available {
        return Err(ZeroclawError::RouteUnavailable {
            hint: hint.clone(),
            reason: route.status_reason.clone(),
        });
    }

    if let Some(limit) = args.get("context_tokens").and_then(|v| v.as_u64()) {
        if route.context_window > 0 && limit as u32 > route.context_window {
            return Err(ZeroclawError::ContextWindowExceeded {
                limit: route.context_window,
                requested: limit as u32,
            });
        }
    }

    Ok(to_json(route))
}

/// `SelectModel`: run the pure selector, then stamp the non-deterministic audit
/// fields (`trace_id`, `timestamp`) the selector intentionally leaves blank.
fn select_model_handler(
    json_args: &str,
    state: &ZeroclawState,
) -> Result<serde_json::Value, ZeroclawError> {
    let input: SelectionInput =
        serde_json::from_str(json_args).map_err(|e| ZeroclawError::ExecutionDenied {
            reason: format!("invalid SelectionInput: {e}"),
        })?;

    let mut output: SelectionOutput = select_model(&input, state)?;
    output.event_metadata.trace_id = uuid::Uuid::new_v4().to_string();
    output.event_metadata.timestamp = chrono::Utc::now().to_rfc3339();
    Ok(to_json(&output))
}

/// `AuthorizeExecution`: deterministic gate that a (provider, model, tool?)
/// triple is declared and reachable. Emits `ExecutionAuthorized` on success or
/// `ExecutionDenied` on a declared-but-unavailable route.
fn authorize_execution_handler(
    json_args: &str,
    state: &ZeroclawState,
) -> Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("AuthorizeExecution", json_args)?;
    let provider = require_str(&args, "provider", "AuthorizeExecution")?;
    let model = require_str(&args, "model", "AuthorizeExecution")?;
    let tool = args.get("tool").and_then(|v| v.as_str()).map(|s| s.to_string());
    let trace_id = uuid::Uuid::new_v4().to_string();

    if !state.projection.providers.iter().any(|p| p.id == provider) {
        return Err(ZeroclawError::ProviderNotDeclared { provider });
    }

    let route = state
        .projection
        .model_routes
        .iter()
        .find(|r| r.model == model && r.provider == provider);
    let route = match route {
        Some(r) => r,
        None => return Err(ZeroclawError::ModelNotDeclared { model }),
    };

    if let Some(t) = &tool {
        if !state.projection.tools.iter().any(|d| &d.name == t) {
            return Err(ZeroclawError::ToolNotDeclared { tool: t.clone() });
        }
    }

    if !route.available {
        let reason = route.status_reason.clone();
        return Ok(DispatchOutcome {
            result: serde_json::json!({"authorized": false, "reason": reason}),
            signal: Some(DispatchSignal {
                name: "ExecutionDenied".to_string(),
                payload: serde_json::json!({
                    "provider": provider, "model": model, "tool": tool,
                    "reason": route.status_reason, "trace_id": trace_id,
                }),
            }),
        });
    }

    Ok(DispatchOutcome {
        result: serde_json::json!({"authorized": true, "reason": "declared route available"}),
        signal: Some(DispatchSignal {
            name: "ExecutionAuthorized".to_string(),
            payload: serde_json::json!({
                "provider": provider, "model": model, "tool": tool, "trace_id": trace_id,
            }),
        }),
    })
}

/// `SetProvider`: validate the target provider is declared, then surface the
/// effective change for the bridge to persist through the mutation pipeline and
/// emit `ProviderChanged`. (Validation here; durable write is the bridge's
/// `MutationEngine` path so a single accountability event is recorded.)
fn set_provider_handler(
    json_args: &str,
    state: &ZeroclawState,
) -> Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetProvider", json_args)?;
    let provider_id = require_str(&args, "provider_id", "SetProvider")?;

    if !state.projection.providers.iter().any(|p| p.id == provider_id) {
        return Err(ZeroclawError::ProviderNotDeclared {
            provider: provider_id,
        });
    }

    let old = state.selected_provider.clone();
    Ok(DispatchOutcome {
        result: serde_json::json!({"selected_provider": provider_id}),
        signal: Some(DispatchSignal {
            name: "ProviderChanged".to_string(),
            payload: serde_json::json!({"old": old, "new": provider_id, "reason": "explicit set"}),
        }),
    })
}

/// `SetModel`: validate the target model is declared on some route, then surface
/// the effective change (persistence + `ModelChanged` via the bridge).
fn set_model_handler(
    json_args: &str,
    state: &ZeroclawState,
) -> Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetModel", json_args)?;
    let model_id = require_str(&args, "model_id", "SetModel")?;

    if !state.projection.model_routes.iter().any(|r| r.model == model_id) {
        return Err(ZeroclawError::ModelNotDeclared { model: model_id });
    }

    let old = state.selected_model.clone();
    Ok(DispatchOutcome {
        result: serde_json::json!({"selected_model": model_id}),
        signal: Some(DispatchSignal {
            name: "ModelChanged".to_string(),
            payload: serde_json::json!({"old": old, "new": model_id, "reason": "explicit set"}),
        }),
    })
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

    fn state() -> ZeroclawState {
        ZeroclawPlugin::current_state()
    }

    fn first_route(state: &ZeroclawState) -> ModelRoute {
        state
            .projection
            .model_routes
            .iter()
            .find(|r| r.available)
            .cloned()
            .expect("at least one available declared route")
    }

    #[test]
    fn dispatch_unknown_method_is_denied() {
        let err = dispatch_zeroclaw_method("Frobnicate", "{}", &state()).unwrap_err();
        assert!(matches!(err, ZeroclawError::ExecutionDenied { .. }));
    }

    #[test]
    fn dispatch_get_methods_return_projection_slices() {
        let st = state();
        let routes = dispatch_zeroclaw_method("GetModelRoutes", "", &st).unwrap();
        assert!(routes.result.is_array());
        let providers = dispatch_zeroclaw_method("GetProviderCatalog", "", &st).unwrap();
        assert_eq!(
            providers.result.as_array().unwrap().len(),
            st.projection.providers.len()
        );
    }

    #[test]
    fn resolve_route_rejects_undeclared_hint() {
        let err = dispatch_zeroclaw_method(
            "ResolveRoute",
            r#"{"hint":"does-not-exist"}"#,
            &state(),
        )
        .unwrap_err();
        assert!(matches!(err, ZeroclawError::RouteNotDeclared { .. }));
    }

    #[test]
    fn resolve_route_returns_declared_route() {
        let st = state();
        let route = first_route(&st);
        let args = format!(r#"{{"hint":"{}"}}"#, route.hint);
        let out = dispatch_zeroclaw_method("ResolveRoute", &args, &st).unwrap();
        assert_eq!(out.result.get("hint").unwrap().as_str().unwrap(), route.hint);
    }

    #[test]
    fn select_model_stamps_audit_fields() {
        let st = state();
        let input = serde_json::json!({
            "task_class": "chat",
            "requested_effort": "medium",
            "context_tokens": 1000,
            "privacy_tier": "public"
        })
        .to_string();
        let out = dispatch_zeroclaw_method("SelectModel", &input, &st).unwrap();
        let meta = out.result.get("event_metadata").unwrap();
        assert!(!meta.get("trace_id").unwrap().as_str().unwrap().is_empty());
        assert!(!meta.get("timestamp").unwrap().as_str().unwrap().is_empty());
        assert!(out.result.get("selected_provider").is_some());
    }

    #[test]
    fn authorize_execution_rejects_undeclared_provider() {
        let err = dispatch_zeroclaw_method(
            "AuthorizeExecution",
            r#"{"provider":"nope","model":"x"}"#,
            &state(),
        )
        .unwrap_err();
        assert!(matches!(err, ZeroclawError::ProviderNotDeclared { .. }));
    }

    #[test]
    fn authorize_execution_emits_signal_on_declared_route() {
        let st = state();
        let route = first_route(&st);
        let args = format!(
            r#"{{"provider":"{}","model":"{}"}}"#,
            route.provider, route.model
        );
        let out = dispatch_zeroclaw_method("AuthorizeExecution", &args, &st).unwrap();
        assert_eq!(out.result.get("authorized").unwrap().as_bool().unwrap(), true);
        let sig = out.signal.expect("authorize emits a signal");
        assert_eq!(sig.name, "ExecutionAuthorized");
    }

    #[test]
    fn set_provider_rejects_undeclared_and_accepts_declared() {
        let st = state();
        let err = dispatch_zeroclaw_method(
            "SetProvider",
            r#"{"provider_id":"ghost"}"#,
            &st,
        )
        .unwrap_err();
        assert!(matches!(err, ZeroclawError::ProviderNotDeclared { .. }));

        let declared = &st.projection.providers[0].id;
        let args = format!(r#"{{"provider_id":"{declared}"}}"#);
        let out = dispatch_zeroclaw_method("SetProvider", &args, &st).unwrap();
        assert_eq!(
            out.result.get("selected_provider").unwrap().as_str().unwrap(),
            declared
        );
        assert_eq!(out.signal.unwrap().name, "ProviderChanged");
    }

    #[test]
    fn set_model_rejects_undeclared_model() {
        let err = dispatch_zeroclaw_method(
            "SetModel",
            r#"{"model_id":"imaginary-model"}"#,
            &state(),
        )
        .unwrap_err();
        assert!(matches!(err, ZeroclawError::ModelNotDeclared { .. }));
    }
}
