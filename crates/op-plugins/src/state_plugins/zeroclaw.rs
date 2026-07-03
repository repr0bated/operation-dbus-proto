//! Zeroclaw route surface plugin — GB.Zeroclaw.
//!
//! Publishes the Antigravity-facing model and CLI routing contract through
//! `PluginSchema` so the UI can render provider/model controls from D-Bus.

use super::common::errors::ZeroclawError;
use super::common::llm_projection::{
    ConfigSchema, LlmProjection, LlmTool, ModelRoute, Provider, Router, StructuredOutput, UiSurface,
};
use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;

// =============================================================================
// PLUGIN ENTRY: identity and typed schema seed
// =============================================================================

const PLUGIN_NAME: &str = "zeroclaw";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "llm";
const PLUGIN_DESCRIPTION: &str = "Zeroclaw schema/RPC-native model router for Antigravity UI, CLI providers, and structured JSON output";
const PLUGIN_DISPLAY_NAME: &str = "GB.Zeroclaw";

/// Transport layer metadata for the Zeroclaw plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-transport.schema@v1"))]
pub struct LlmTransport {
    /// D-Bus object path served by this plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw-transport.dbus-object@v1"))]
    pub dbus_object: String,
    /// gRPC upstream target.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw-transport.grpc-target@v1"))]
    pub grpc_target: String,
    /// Incus / WireGuard container target for xray routing. Kept as a
    /// published-schema field for backward compatibility, but zeroclaw's LLM
    /// transport now runs on the host (xray via the `gbr-xray` s6 service and
    /// the gRPC-bridge via `op-grpc-bridge-zeroclaw`); there is no per-service
    /// incus container. Defaults to the `"host"` sentinel.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw-transport.incus-container@v1"))]
    pub incus_container: String,
    /// Browser-facing surface description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw-transport.browser-surface@v1"))]
    pub browser_surface: String,
    /// REST aliases exposed by the bridge.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw-transport.rest-aliases@v1"))]
    pub rest_aliases: Vec<String>,
    /// Canonical OSCAL/subid mapping authority.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.zeroclaw-transport.policy-source@v1"))]
    pub policy_source: String,
}

/// Nested per-capability model assignments.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw.model-assignments.schema@v1"))]
pub struct ModelAssignments {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.ovs-routing@v1"))]
    pub ovs_routing: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.obfuscation@v1"))]
    pub obfuscation: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.vectorization@v1"))]
    pub vectorization: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.qdrant-retrieval@v1"))]
    pub qdrant_retrieval: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.cozo-retrieval@v1"))]
    pub cozo_retrieval: String,
}

/// Top-level Zeroclaw state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.zeroclaw.schema@v1"))]
pub struct ZeroclawState {
    /// Operational status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.zeroclaw.status@v1"))]
    pub status: String,
    /// Selected provider identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.selected-provider@v1"))]
    pub selected_provider: String,
    /// Selected model identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.selected-model@v1"))]
    pub selected_model: String,
    /// Per-capability model assignments.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments@v1"))]
    pub model_assignments: ModelAssignments,
    /// Transport layer metadata.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.transport@v1"))]
    pub transport: LlmTransport,
    /// Shared LLM projection fields (flattened to the top level).
    #[serde(flatten)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw.llm-projection@v1"))]
    pub projection: LlmProjection,
}

/// Empty input for read-only ZeroClaw methods.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EmptyZeroclawInput {}

/// Input for resolving a model route by hint or model identifier.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveRouteInput {
    /// Route hint or model identifier.
    pub hint: String,
}

/// Input for selecting a provider.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetProviderInput {
    /// Provider identifier declared in the provider catalog.
    pub provider_id: String,
}

/// Input for selecting a model.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetModelInput {
    /// Model identifier declared in the model route catalog.
    pub model_id: String,
}

/// Output for the complete ZeroClaw state surface.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.state.result@v1"))]
pub struct GetStateOutput {
    /// Complete projected ZeroClaw state.
    pub state: ZeroclawState,
}

/// Output for the model route catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.model-routes.result@v1"))]
pub struct GetModelRoutesOutput {
    /// Declared model routes.
    pub model_routes: Vec<ModelRoute>,
}

/// Output for the provider catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.provider-catalog.result@v1"))]
pub struct GetProviderCatalogOutput {
    /// Declared providers.
    pub providers: Vec<Provider>,
}

/// Output for the declared tool catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.tools.result@v1"))]
pub struct GetToolsOutput {
    /// Declared LLM tools.
    pub tools: Vec<LlmTool>,
}

/// Output for the provider list accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.providers.result@v1"))]
pub struct ListProvidersOutput {
    pub providers: Vec<Provider>,
}

/// Output for the router accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.router.result@v1"))]
pub struct GetRouterOutput {
    pub router: Router,
}

/// Output for the config schema accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.config-schema.result@v1"))]
pub struct GetConfigSchemaOutput {
    pub config_schema: ConfigSchema,
}

/// Output for the UI surfaces accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.ui-surfaces.result@v1"))]
pub struct ListUiSurfacesOutput {
    pub ui_surfaces: Vec<UiSurface>,
}

/// Output for the structured output accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.structured-output.result@v1"))]
pub struct GetStructuredOutputOutput {
    pub structured_output: StructuredOutput,
}

/// Output for a resolved model route.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.route.result@v1"))]
pub struct ResolveRouteOutput {
    /// Resolved route.
    pub route: ModelRoute,
}

/// Output for provider selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.selected-provider.result@v1"))]
pub struct SetProviderOutput {
    /// Selected provider identifier.
    pub selected_provider: String,
}

/// Output for model selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.selected-model.result@v1"))]
pub struct SetModelOutput {
    /// Selected model identifier.
    pub selected_model: String,
}

/// Output for the model assignments accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.model-assignments.result@v1"))]
pub struct GetModelAssignmentsOutput {
    pub model_assignments: ModelAssignments,
}

/// Input for setting an ovs routing model.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetOvsRoutingModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetOvsRoutingModelOutput {
    pub ovs_routing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetObfuscationModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetObfuscationModelOutput {
    pub obfuscation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetVectorizationModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetVectorizationModelOutput {
    pub vectorization: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetQdrantRetrievalModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetQdrantRetrievalModelOutput {
    pub qdrant_retrieval: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetCozoRetrievalModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetCozoRetrievalModelOutput {
    pub cozo_retrieval: String,
}

// =============================================================================
// PLUGIN BODY: D-Bus-backed behavior only
// =============================================================================

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

    pub fn current_state() -> ZeroclawState {
        let selected_provider = Self::env_or("LLM_PROVIDER", "ollama");
        let selected_model = Self::env_or("LLM_MODEL", "gemma4");
        let router_endpoint = Self::env_or("ZEROCLAW_ROUTER_ENDPOINT", "http://localhost:11434");
        let grpc_target = Self::env_or("ZEROCLAW_GRPC_TARGET", "http://10.200.0.2:50051");
        let grpc_target_for_provider = grpc_target.clone();

        ZeroclawState {
            status: "declared".to_string(),
            selected_provider,
            selected_model,
            model_assignments: ModelAssignments {
                ovs_routing: "gemma4".to_string(),
                obfuscation: "gemma4".to_string(),
                vectorization: "gemini-embedding-001".to_string(),
                qdrant_retrieval: "gemini-embedding-001".to_string(),
                cozo_retrieval: "gemma4".to_string(),
            },
            transport: LlmTransport {
                dbus_object: "/opdbus/v1/plugins/zeroclaw".to_string(),
                grpc_target: grpc_target_for_provider,
                incus_container: "host".to_string(),
                browser_surface: "gRPC-Web through op-web".to_string(),
                rest_aliases: vec![
                    "/api/zeroclaw/chat".to_string(),
                    "/api/llm/chat".to_string(),
                ],
                policy_source: "/opdbus/v1/plugins/oscal_subid_registry".to_string(),
            },
            projection: LlmProjection {
                providers: vec![
                    Provider {
                        id: "factory".to_string(),
                        route: "factory".to_string(),
                        kind: "orchestrator".to_string(),
                        aliases: vec!["default".to_string(), "auto".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "codex".to_string(),
                        route: "openai-codex".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec!["openai_codex".to_string(), "codex".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "ollama".to_string(),
                        route: "ollama".to_string(),
                        kind: "local".to_string(),
                        aliases: vec!["gemma".to_string(), "gemma4".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "antigravity".to_string(),
                        route: "antigravity".to_string(),
                        kind: "orchestrator".to_string(),
                        aliases: vec!["google.antigravity".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "google".to_string(),
                        route: "gemini".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec!["gemini".to_string(), "google-gemini".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "opencode".to_string(),
                        route: "custom:opencode".to_string(),
                        kind: "cli".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "kilocode".to_string(),
                        route: "custom:kilocode".to_string(),
                        kind: "cli".to_string(),
                        aliases: vec!["kilo-code".to_string(), "kilo_code".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "openrouter".to_string(),
                        route: "openrouter".to_string(),
                        kind: "router".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "anthropic".to_string(),
                        route: "anthropic".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "openai".to_string(),
                        route: "openai".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "gemini-cli".to_string(),
                        route: "gemini-cli".to_string(),
                        kind: "cli".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "oscal".to_string(),
                        route: "oscal".to_string(),
                        kind: "policy".to_string(),
                        aliases: vec![
                            "compliance".to_string(),
                            "subid".to_string(),
                            "nist".to_string(),
                        ],
                        source: "/opdbus/v1/plugins/oscal_subid_registry".to_string(),
                        ..Default::default()
                    },
                ],
                router: Router {
                    provider: "ollama".to_string(),
                    model: "gemma4".to_string(),
                    endpoint: router_endpoint,
                    scope: "all".to_string(),
                    role: "context_aware_allocator".to_string(),
                    emits: vec![
                        "provider".to_string(),
                        "model".to_string(),
                        "hint".to_string(),
                        "candidate_subids".to_string(),
                        "confidence".to_string(),
                        "thinking_budget".to_string(),
                        "reasoning_effort".to_string(),
                    ],
                    ..Default::default()
                },
                model_routes: vec![
                    ModelRoute {
                        hint: "code".to_string(),
                        provider: "openrouter".to_string(),
                        upstream_provider: "openrouter".to_string(),
                        transport: "direct".to_string(),
                        model: "anthropic/claude-sonnet-4.6".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Declared route; backend availability must be projected before execution.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "gemini".to_string(),
                        upstream_provider: "gemini".to_string(),
                        transport: "direct".to_string(),
                        model: "gemini-2.5-flash".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Declared route; backend availability must be projected before execution.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "local".to_string(),
                        provider: "ollama".to_string(),
                        upstream_provider: "ollama".to_string(),
                        transport: "loopback".to_string(),
                        model: "gemma4".to_string(),
                        kind: "router".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "gemma4 is the declared local classifier; unavailable until Ollama projects it.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "auto".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "factory".to_string(),
                        transport: "auto".to_string(),
                        model: "auto".to_string(),
                        kind: "orchestrator".to_string(),
                        status: "declared".to_string(),
                        available: true,
                        status_reason: "Factory auto-router available - selects best provider based on query classification.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "code".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "openrouter".to_string(),
                        transport: "direct".to_string(),
                        model: "anthropic/claude-sonnet-4.6".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Factory code route -> openrouter/claude; requires backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "gemini".to_string(),
                        transport: "direct".to_string(),
                        model: "gemini-2.5-flash".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Factory fast route -> gemini/flash; requires backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "local".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "ollama".to_string(),
                        transport: "loopback".to_string(),
                        model: "gemma4".to_string(),
                        kind: "router".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Factory local route -> ollama/gemma4; requires backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "reasoning".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "antigravity".to_string(),
                        transport: "direct".to_string(),
                        model: "gemini-3-pro-preview".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Factory reasoning route; Gemma dynamically allocates thinking budget.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "compliance".to_string(),
                        provider: "oscal".to_string(),
                        upstream_provider: "oscal".to_string(),
                        transport: "dbus".to_string(),
                        model: "NIST_SP_800-53_Rev5".to_string(),
                        kind: "policy".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "OSCAL policy provider is declared; unavailable until oscal_subid_registry is projected.".to_string(),
                        source: "/opdbus/v1/plugins/oscal_subid_registry".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                ],
                tools: vec![
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
                        ..Default::default()
                    },
                    LlmTool {
                        name: "zeroclaw.models.list".to_string(),
                        description: "List cached or live models for a Zeroclaw provider.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {"provider": {"type": "string"}},
                            "required": []
                        }),
                        ..Default::default()
                    },
                ],
                config_schema: ConfigSchema {
                    source: "zeroclaw config schema".to_string(),
                    schema_crate: "schemars".to_string(),
                    native_type: "zeroclaw::config::schema::Config".to_string(),
                    status: "available_via_cli_or_gateway".to_string(),
                    ..Default::default()
                },
                ui_surfaces: vec![
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
                ],
                structured_output: StructuredOutput {
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
                    ..Default::default()
                },
            },
        }
    }
}

#[async_trait]
impl StatePlugin for ZeroclawPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = zeroclaw_schema();
        super::common::llm_projection::rewrite_projection_subids_for_plugin(
            &mut schema,
            PLUGIN_NAME,
        );
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: PLUGIN_NAME.to_string(),
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
            plugin: PLUGIN_NAME.to_string(),
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

// =============================================================================
// PLUGIN EXIT: publish the single PluginSchema contract
// =============================================================================

/// Canonical `zeroclaw` schema derived from [`ZeroclawState`] via schemars.
pub(crate) fn zeroclaw_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(ZeroclawState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());

    if let Ok(state) = simd_json::serde::to_owned_value(ZeroclawPlugin::current_state()) {
        super::schemars_adapter::apply_state_defaults(&mut schema, &state);
        schema.example = Some(state);
    }

    schema.methods.insert(
        "GetState".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetStateOutput>(
            "GetState",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.state.read@v1",
            "obs.service.zeroclaw.state.get@v1",
        ),
    );
    schema.methods.insert(
        "GetModelAssignments".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetModelAssignmentsOutput>(
            "GetModelAssignments",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.model-assignments.read@v1",
            "obs.service.zeroclaw.model-assignments.get@v1",
        ),
    );
    schema.methods.insert(
        "GetModelRoutes".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetModelRoutesOutput>(
            "GetModelRoutes",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.routes.read@v1",
            "obs.service.zeroclaw.model-routes.list@v1",
        ),
    );
    schema.methods.insert(
        "ListProviders".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, ListProvidersOutput>(
            "ListProviders",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.providers.read@v1",
            "obs.service.zeroclaw.providers.list@v1",
        ),
    );
    schema.methods.insert(
        "GetProviderCatalog".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetProviderCatalogOutput>(
            "GetProviderCatalog",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.providers.read@v1",
            "obs.service.zeroclaw.provider-catalog.list@v1",
        ),
    );
    schema.methods.insert(
        "GetTools".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetToolsOutput>(
            "GetTools",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.tools.read@v1",
            "obs.service.zeroclaw.tools.list@v1",
        ),
    );
    schema.methods.insert(
        "GetRouter".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetRouterOutput>(
            "GetRouter",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.router.read@v1",
            "obs.service.zeroclaw.router.get@v1",
        ),
    );
    schema.methods.insert(
        "GetConfigSchema".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetConfigSchemaOutput>(
            "GetConfigSchema",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.config-schema.read@v1",
            "obs.service.zeroclaw.config-schema.get@v1",
        ),
    );
    schema.methods.insert(
        "ListUiSurfaces".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, ListUiSurfacesOutput>(
            "ListUiSurfaces",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.ui-surfaces.read@v1",
            "obs.service.zeroclaw.ui-surfaces.list@v1",
        ),
    );
    schema.methods.insert(
        "GetStructuredOutput".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetStructuredOutputOutput>(
            "GetStructuredOutput",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.structured-output.read@v1",
            "obs.service.zeroclaw.structured-output.get@v1",
        ),
    );
    schema.methods.insert(
        "ResolveRoute".to_string(),
        method_decl_from_schemars_with_output::<ResolveRouteInput, ResolveRouteOutput>(
            "ResolveRoute",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.route.resolve@v1",
            "obs.service.zeroclaw.route.resolve@v1",
        ),
    );
    schema.methods.insert(
        "SetProvider".to_string(),
        method_decl_from_schemars_with_output::<SetProviderInput, SetProviderOutput>(
            "SetProvider",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.provider.set@v1",
            "mut.service.zeroclaw.provider.set@v1",
        ),
    );
    schema.methods.insert(
        "SetModel".to_string(),
        method_decl_from_schemars_with_output::<SetModelInput, SetModelOutput>(
            "SetModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model.set@v1",
            "mut.service.zeroclaw.model.set@v1",
        ),
    );
    schema.methods.insert(
        "SetOvsRoutingModel".to_string(),
        method_decl_from_schemars_with_output::<SetOvsRoutingModelInput, SetOvsRoutingModelOutput>(
            "SetOvsRoutingModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.ovs-routing.set@v1",
            "mut.service.zeroclaw.model-assignments.ovs-routing.set@v1",
        ),
    );
    schema.methods.insert(
        "SetObfuscationModel".to_string(),
        method_decl_from_schemars_with_output::<SetObfuscationModelInput, SetObfuscationModelOutput>(
            "SetObfuscationModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.obfuscation.set@v1",
            "mut.service.zeroclaw.model-assignments.obfuscation.set@v1",
        ),
    );
    schema.methods.insert(
        "SetVectorizationModel".to_string(),
        method_decl_from_schemars_with_output::<SetVectorizationModelInput, SetVectorizationModelOutput>(
            "SetVectorizationModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.vectorization.set@v1",
            "mut.service.zeroclaw.model-assignments.vectorization.set@v1",
        ),
    );
    schema.methods.insert(
        "SetQdrantRetrievalModel".to_string(),
        method_decl_from_schemars_with_output::<SetQdrantRetrievalModelInput, SetQdrantRetrievalModelOutput>(
            "SetQdrantRetrievalModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.qdrant-retrieval.set@v1",
            "mut.service.zeroclaw.model-assignments.qdrant-retrieval.set@v1",
        ),
    );
    schema.methods.insert(
        "SetCozoRetrievalModel".to_string(),
        method_decl_from_schemars_with_output::<SetCozoRetrievalModelInput, SetCozoRetrievalModelOutput>(
            "SetCozoRetrievalModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.cozo-retrieval.set@v1",
            "mut.service.zeroclaw.model-assignments.cozo-retrieval.set@v1",
        ),
    );

    schema
}

/// Public accessor for crates that embed the Zeroclaw plugin contract.
pub fn zeroclaw_plugin_schema() -> PluginSchema {
    zeroclaw_schema()
}

/// A signal the bridge can emit after a successful plugin-owned dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchSignal {
    pub name: String,
    pub payload: JsonValue,
}

/// Outcome of a plugin-owned method dispatch.
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    pub result: JsonValue,
    pub signal: Option<DispatchSignal>,
}

impl DispatchOutcome {
    fn plain(result: JsonValue) -> Self {
        Self { result, signal: None }
    }
}

/// Plugin-owned method dispatch for the Zeroclaw D-Bus/gRPC surface.
pub fn dispatch_zeroclaw_method(
    method: &str,
    json_args: &str,
    state: &ZeroclawState,
) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    match method {
        "GetState" => Ok(DispatchOutcome::plain(serde_json::json!({ "state": to_json(state) }))),
        "GetModelRoutes" => Ok(DispatchOutcome::plain(serde_json::json!({ "model_routes": to_json(&state.projection.model_routes) }))),
        "GetProviderCatalog" => Ok(DispatchOutcome::plain(serde_json::json!({ "providers": to_json(&state.projection.providers) }))),
        "GetTools" => Ok(DispatchOutcome::plain(serde_json::json!({ "tools": to_json(&state.projection.tools) }))),
        "ListProviders" => Ok(DispatchOutcome::plain(serde_json::json!({ "providers": to_json(&state.projection.providers) }))),
        "GetModelAssignments" => Ok(DispatchOutcome::plain(serde_json::json!({ "model_assignments": to_json(&state.model_assignments) }))),
        "GetRouter" => Ok(DispatchOutcome::plain(serde_json::json!({ "router": to_json(&state.projection.router) }))),
        "GetConfigSchema" => Ok(DispatchOutcome::plain(serde_json::json!({ "config_schema": to_json(&state.projection.config_schema) }))),
        "ListUiSurfaces" => Ok(DispatchOutcome::plain(serde_json::json!({ "ui_surfaces": to_json(&state.projection.ui_surfaces) }))),
        "GetStructuredOutput" => Ok(DispatchOutcome::plain(serde_json::json!({ "structured_output": to_json(&state.projection.structured_output) }))),
        "ResolveRoute" => resolve_route(json_args, state).map(DispatchOutcome::plain),
        "SetProvider" => set_provider_handler(json_args, state),
        "SetModel" => set_model_handler(json_args, state),
        "SetOvsRoutingModel" => set_role_model_handler(json_args, state, "ovs_routing"),
        "SetObfuscationModel" => set_role_model_handler(json_args, state, "obfuscation"),
        "SetVectorizationModel" => set_role_model_handler(json_args, state, "vectorization"),
        "SetQdrantRetrievalModel" => set_role_model_handler(json_args, state, "qdrant_retrieval"),
        "SetCozoRetrievalModel" => set_role_model_handler(json_args, state, "cozo_retrieval"),
        other => Err(ZeroclawError::ExecutionDenied {
            reason: format!("undeclared method: {other}"),
        }),
    }
}

fn to_json<T: Serialize>(value: &T) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

fn parse_args(method: &str, json_args: &str) -> std::result::Result<JsonValue, ZeroclawError> {
    if json_args.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(json_args).map_err(|error| ZeroclawError::ExecutionDenied {
        reason: format!("{method} arguments are not valid JSON: {error}"),
    })
}

fn require_str(args: &JsonValue, field: &str, method: &str) -> std::result::Result<String, ZeroclawError> {
    args.get(field).and_then(JsonValue::as_str).map(str::to_string).ok_or_else(|| ZeroclawError::ExecutionDenied {
        reason: format!("{method} requires string field '{field}'"),
    })
}

fn resolve_route(json_args: &str, state: &ZeroclawState) -> std::result::Result<JsonValue, ZeroclawError> {
    let args = parse_args("ResolveRoute", json_args)?;
    let hint = require_str(&args, "hint", "ResolveRoute")?;
    state.projection.model_routes.iter()
        .find(|route| route.hint == hint || route.model == hint)
        .map(|route| serde_json::json!({ "route": to_json(route) }))
        .ok_or(ZeroclawError::RouteNotDeclared { hint })
}

fn set_provider_handler(json_args: &str, state: &ZeroclawState) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetProvider", json_args)?;
    let provider_id = require_str(&args, "provider_id", "SetProvider")?;
    if !state.projection.providers.iter().any(|p| p.id == provider_id) {
        return Err(ZeroclawError::ProviderNotDeclared { provider: provider_id });
    }
    let old = state.selected_provider.clone();
    Ok(DispatchOutcome {
        result: serde_json::json!({ "selected_provider": provider_id }),
        signal: Some(DispatchSignal {
            name: "ProviderChanged".to_string(),
            payload: serde_json::json!({ "old": old, "new": provider_id, "reason": "explicit set" }),
        }),
    })
}

fn set_model_handler(json_args: &str, state: &ZeroclawState) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetModel", json_args)?;
    let model_id = require_str(&args, "model_id", "SetModel")?;
    if !state.projection.model_routes.iter().any(|r| r.model == model_id) {
        return Err(ZeroclawError::ModelNotDeclared { model: model_id });
    }
    let old = state.selected_model.clone();
    Ok(DispatchOutcome {
        result: serde_json::json!({ "selected_model": model_id }),
        signal: Some(DispatchSignal {
            name: "ModelChanged".to_string(),
            payload: serde_json::json!({ "old": old, "new": model_id, "reason": "explicit set" }),
        }),
    })
}

fn set_role_model_handler(
    json_args: &str,
    state: &ZeroclawState,
    role: &str,
) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetRoleModel", json_args)?;
    let model_id = require_str(&args, "model_id", "SetRoleModel")?;
    let result = match role {
        "ovs_routing" => serde_json::json!({ "ovs_routing": model_id }),
        "obfuscation" => serde_json::json!({ "obfuscation": model_id }),
        "vectorization" => serde_json::json!({ "vectorization": model_id }),
        "qdrant_retrieval" => serde_json::json!({ "qdrant_retrieval": model_id }),
        "cozo_retrieval" => serde_json::json!({ "cozo_retrieval": model_id }),
        _ => return Err(ZeroclawError::ExecutionDenied { reason: format!("unknown model role: {role}") }),
    };
    let signal = serde_json::json!({
        "role": role,
        "model_id": model_id,
        "selected_provider": state.selected_provider,
    });
    Ok(DispatchOutcome {
        result,
        signal: Some(DispatchSignal {
            name: "ModelRoleChanged".to_string(),
            payload: signal,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use serde_json::Value as JVal;

    fn collect_subids(node: &JVal, out: &mut Vec<String>) {
        if let Some(subid) = node.get("x-oscal-subid").and_then(JVal::as_str) {
            out.push(subid.to_string());
        }
        if let Some(props) = node.get("properties").and_then(JVal::as_object) {
            for v in props.values() { collect_subids(v, out); }
        }
        if let Some(defs) = node.get("$defs").or_else(|| node.get("definitions")).and_then(JVal::as_object) {
            for v in defs.values() { collect_subids(v, out); }
        }
        if let Some(items) = node.get("items") { collect_subids(items, out); }
        if let Some(alts) = node.get("anyOf").or_else(|| node.get("oneOf")).and_then(JVal::as_array) {
            for v in alts { collect_subids(v, out); }
        }
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(ZeroclawState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(!subids.is_empty(), "expected at least one x-oscal-subid");
        for subid in subids {
            validate_subid(&subid).expect("invalid subid: {subid}");
        }
    }

    #[test]
    fn public_schema_accessor_returns_zeroclaw_schema() {
        let schema = zeroclaw_plugin_schema();
        assert_eq!(schema.name, PLUGIN_NAME);
        assert_eq!(schema.version, PLUGIN_VERSION);
        assert_eq!(schema.display_name, Some(PLUGIN_DISPLAY_NAME.to_string()));
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(ZeroclawPlugin::new()))
}
