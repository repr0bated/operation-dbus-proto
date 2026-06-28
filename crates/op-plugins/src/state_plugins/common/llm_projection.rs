//! Shared, schemars-typed projection types for the Zeroclaw LLM surface.
//!
//! Every struct here derives `schemars::JsonSchema` — this is mandatory per the
//! `zeroclaw-absorbs-op-llm` spec (§1). The `PluginSchema` JSON, D-Bus method
//! shapes, gRPC proto fields, MCP tool parameter schemas, and UI renderers are
//! all derived from this `schemars` output. A type without `JsonSchema` is
//! invisible to the D-Bus/gRPC/MCP surface and cannot participate in routing.
//!
//! These are the *contract* types (spec §5, Layer 1). They carry no HTTP/runtime
//! behavior — only the declared schema shape. The grpc-bridge projection hook
//! (spec §12) reads these as the single source of truth when materializing the
//! per-route / per-provider D-Bus object tree; there is no second projection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Transport / addressing metadata for the Zeroclaw surface.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmTransport {
    pub dbus_object: String,
    pub grpc_target: String,
    pub incus_container: String,
    pub browser_surface: String,
    pub rest_aliases: Vec<String>,
    /// Canonical OSCAL/subid mapping authority — referenced, never copied.
    pub policy_source: String,
}

/// A declared provider in the Zeroclaw catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Provider {
    pub id: String,
    pub route: String,
    pub kind: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Optional pointer to a canonical mapping authority (e.g. the OSCAL subid
    /// registry for policy-kind providers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// The universal router/classifier declaration (gemma4 on-box).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Router {
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub scope: String,
    pub role: String,
    pub emits: Vec<String>,
}

/// A single declared model route. Extended with selector metadata (spec §2a)
/// consumed by `selector::select_model`. New selector fields are
/// `#[serde(default)]` so existing route declarations remain valid.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModelRoute {
    pub hint: String,
    pub provider: String,
    pub upstream_provider: String,
    pub transport: String,
    pub model: String,
    pub kind: String,
    pub status: String,
    pub available: bool,
    pub status_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Present (often `null`) on every route for backend key projection.
    pub api_key: Option<String>,

    // ── Selector metadata (spec §2a) ───────────────────────────────────────
    /// subid: obs.software.llm-model-route.cost-profile@v1
    #[serde(default)]
    pub cost_profile: String,
    /// subid: obs.software.llm-model-route.effort-level@v1
    #[serde(default)]
    pub effort_level: String,
    /// subid: obs.software.llm-model-route.latency-class@v1
    #[serde(default)]
    pub latency_class: String,
    /// subid: obs.software.llm-model-route.privacy-tier@v1
    #[serde(default)]
    pub privacy_tier: String,
    /// subid: obs.software.llm-model-route.context-window@v1
    #[serde(default)]
    pub context_window: u32,
    /// subid: obs.software.llm-model-route.health-score@v1
    #[serde(default)]
    pub health_score: f64,
    /// subid: obs.software.llm-model-route.fallback-routes@v1
    #[serde(default)]
    pub fallback_routes: Vec<String>,
    /// subid: obs.software.llm-model-route.tool-support@v1
    #[serde(default)]
    pub tool_support: Vec<String>,
}

/// A declared tool on the Zeroclaw surface. `parameters` is a free-form JSON
/// Schema object (kept as `serde_json::Value` because tool argument shapes are
/// dynamic and provider-defined).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Pointer to the native (schemars) config schema for Zeroclaw.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigSchema {
    pub source: String,
    pub schema_crate: String,
    pub native_type: String,
    pub status: String,
}

/// A UI surface entry rendered from a plugin schema.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UiSurface {
    pub path: String,
    pub name: String,
    pub schema: String,
}

/// Structured-output policy for the Antigravity/Zeroclaw chat surface.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StructuredOutput {
    pub sdk: String,
    pub config_object: String,
    pub extractor: String,
    pub response_schema: serde_json::Value,
    pub pydantic_source: Vec<String>,
    pub ui_renderer: String,
    pub required: bool,
}

/// The flattened LLM projection (spec design §2): provider catalog, route table,
/// router config, tools, structured-output policy, config schema, and UI
/// surfaces. Flattened into `ZeroclawState` so the serialized shape stays flat.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProjection {
    pub providers: Vec<Provider>,
    pub router: Router,
    pub model_routes: Vec<ModelRoute>,
    pub tools: Vec<LlmTool>,
    pub config_schema: ConfigSchema,
    pub ui_surfaces: Vec<UiSurface>,
    pub structured_output: StructuredOutput,
}

/// Weights and defaults that drive automatic model selection (spec §2c).
/// subid: sch.software.zeroclaw.selector-policy@v1
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelectorPolicy {
    /// subid: sch.software.selector-policy.effort-weight@v1
    pub effort_weight: f64,
    /// Coefficient applied to Zeroclaw's native cost field.
    /// subid: sch.software.selector-policy.cost-weight@v1
    pub cost_weight: f64,
    /// subid: sch.software.selector-policy.latency-weight@v1
    pub latency_weight: f64,
    /// subid: sch.software.selector-policy.health-weight@v1
    pub health_weight: f64,
    /// subid: sch.software.selector-policy.default-effort@v1
    pub default_effort: String,
    /// subid: sch.software.selector-policy.default-privacy@v1
    pub default_privacy_tier: String,
}

impl Default for SelectorPolicy {
    fn default() -> Self {
        Self {
            effort_weight: 1.0,
            cost_weight: 1.0,
            latency_weight: 0.5,
            health_weight: 0.5,
            default_effort: "medium".to_string(),
            default_privacy_tier: "internal".to_string(),
        }
    }
}

/// Input to the dynamic model selector (spec §2d).
/// subid: sch.software.llm-selection-input.schema@v1
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SelectionInput {
    pub task_class: String,
    pub requested_effort: String,
    /// Budget class or cost ceiling policy (unit-neutral; Zeroclaw-native).
    #[serde(default)]
    pub cost_policy: Option<String>,
    #[serde(default)]
    pub latency_target: Option<String>,
    pub context_tokens: u32,
    pub privacy_tier: String,
    #[serde(default)]
    pub tool_needs: Vec<String>,
    #[serde(default)]
    pub explicit_provider: Option<String>,
    #[serde(default)]
    pub explicit_model: Option<String>,
}

/// Result of a dynamic model selection (spec §2d).
/// subid: sch.software.llm-selection-output.schema@v1
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelectionOutput {
    pub selected_provider: String,
    pub selected_model: String,
    pub reason: String,
    /// Cost estimate in Zeroclaw's native units (unit-neutral string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<String>,
    pub effort_level: String,
    pub confidence: f64,
    pub fallback_routes: Vec<String>,
    pub event_metadata: SelectionEvent,
}

/// Audit/trace metadata emitted with every selection (spec §2d).
/// subid: evt.service.zeroclaw.selection-event@v1
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SelectionEvent {
    pub trace_id: String,
    pub timestamp: String,
    pub selector_version: String,
}
