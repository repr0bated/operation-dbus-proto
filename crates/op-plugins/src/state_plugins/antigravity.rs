//! Antigravity SDK provider plugin.
//!
//! Publishes the full Antigravity (Google Cloud / Vertex AI / Gemini) SDK
//! surface through PluginSchema so the UI can render provider configuration,
//! model selection, safety settings, generation controls, structured output
//! schemas, MCP tools, and usage tracking from the D-Bus projection.

use super::common::llm_projection::{
    ConfigSchema, LlmProjection, LlmTool, ModelRoute, Provider, Router, StructuredOutput, UiSurface,
};
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Vertex AI authentication configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.vertex-auth.schema@v1"))]
pub struct VertexAuth {
    /// Whether Vertex AI is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.vertex-auth.enabled@v1"))]
    pub enabled: bool,
    /// Whether to use the Vertex AI backend (true) or the Gemini API (false).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.vertex-auth.use-vertexai@v1"))]
    pub use_vertexai: String,
    /// Google Cloud project ID.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.vertex-auth.project@v1"))]
    pub project: String,
    /// Google Cloud location/region.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.vertex-auth.location@v1"))]
    pub location: String,
    /// Vertex AI API endpoint URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.vertex-auth.api-endpoint@v1"))]
    pub api_endpoint: String,
    /// Credentials type used for Vertex AI.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.vertex-auth.credentials-type@v1"))]
    pub credentials_type: String,
    /// Service account email for Vertex AI.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.vertex-auth.service-account@v1"))]
    pub service_account_email: String,
    /// Quota project for Vertex AI billing.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.vertex-auth.quota-project@v1"))]
    pub quota_project: String,
}

/// A single API key configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.api-key.schema@v1"))]
pub struct ApiKey {
    /// Whether the key is configured.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.api-key.configured@v1"))]
    pub configured: bool,
    /// Environment variable holding the key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.api-key.env-var@v1"))]
    pub env_var: String,
    /// Masked value displayed in the UI.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity.api-key.masked-value@v1"))]
    pub masked_value: String,
    /// Restriction class for the key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.api-key.restriction@v1"))]
    pub restriction: String,
}

/// Map of API keys by service name.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.api-keys.schema@v1"))]
pub struct ApiKeys {
    /// Generative Language API key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.api-keys.generative-language@v1"))]
    pub generative_language: ApiKey,
    /// Vertex AI API key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.api-keys.vertex-ai@v1"))]
    pub vertex_ai: ApiKey,
}

/// OAuth bridge status.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.oauth-bridge.schema@v1"))]
pub struct OauthBridge {
    /// Bridge URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.oauth-bridge.url@v1"))]
    pub url: String,
    /// Bridge status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.oauth-bridge.status@v1"))]
    pub status: String,
    /// Bridge version.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.oauth-bridge.version@v1"))]
    pub version: String,
}

/// Antigravity authentication state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.auth.schema@v1"))]
pub struct Auth {
    /// Authentication method.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.auth.method@v1"))]
    pub method: String,
    /// Identity provider.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.auth.provider@v1"))]
    pub provider: String,
    /// Token source (e.g. gcloud).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.antigravity.auth.token-source@v1"))]
    pub token_source: String,
    /// Path to the token file.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.auth.token-file@v1"))]
    pub token_file: String,
    /// Whether the current token is valid.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.auth.token-valid@v1"))]
    pub token_valid: bool,
    /// Token expiry timestamp, null when not known.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.auth.token-expiry@v1"))]
    pub token_expiry: Option<JsonValue>,
    /// OAuth scopes granted.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.auth.scopes@v1"))]
    pub scopes: Vec<String>,
    /// Vertex AI authentication details.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.auth.vertex-ai@v1"))]
    pub vertex_ai: VertexAuth,
    /// Configured API keys.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.auth.api-keys@v1"))]
    pub api_keys: ApiKeys,
    /// OAuth bridge endpoint.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.auth.oauth-bridge@v1"))]
    pub oauth_bridge: OauthBridge,
}

/// Google Cloud project metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.project.schema@v1"))]
pub struct Project {
    /// Google Cloud project ID.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.project.id@v1"))]
    pub project_id: String,
    /// Numeric project identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.project.number@v1"))]
    pub project_number: String,
    /// Display name of the project.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.project.name@v1"))]
    pub name: String,
    /// Default location/region.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.project.location@v1"))]
    pub location: String,
    /// Region (same as location for Antigravity).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.project.region@v1"))]
    pub region: String,
    /// Enabled Google Cloud APIs.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.project.api-enabled@v1"))]
    pub api_enabled: Vec<String>,
    /// Project labels.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.project.labels@v1"))]
    pub labels: HashMap<String, String>,
}

/// A single model entry in the Antigravity catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.model-entry.schema@v1"))]
pub struct ModelEntry {
    /// Model identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity.model-entry.id@v1"))]
    pub id: String,
    /// Human-readable model name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity.model-entry.name@v1"))]
    pub name: String,
    /// Model family (gemini, gemma, embedding).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.family@v1"))]
    pub family: String,
    /// Generation number.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.generation@v1"))]
    pub generation: Option<f64>,
    /// Capability tags.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.model-entry.capabilities@v1"))]
    pub capabilities: Vec<String>,
    /// Context window size in tokens.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.context-window@v1"))]
    pub context_window: Option<i64>,
    /// Maximum output tokens.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.max-output-tokens@v1"))]
    pub max_output_tokens: Option<i64>,
    /// Supported input modalities.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.model-entry.modalities@v1"))]
    pub supported_modalities: Vec<String>,
    /// Whether the model supports an extended thinking mode.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.thinking-mode@v1"))]
    pub thinking_mode: bool,
    /// Rate limit in requests per minute, null when unlimited.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.rate-limit-rpm@v1"))]
    pub rate_limit_rpm: Option<i64>,
    /// Embedding dimensions, for embedding models.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.dimensions@v1"))]
    pub dimensions: Option<i64>,
    /// Maximum input tokens, for embedding models.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-entry.max-input-tokens@v1"))]
    pub max_input_tokens: Option<i64>,
}

/// Antigravity model catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.model-catalog.schema@v1"))]
pub struct ModelCatalog {
    /// Available models.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.model-catalog.entries@v1"))]
    pub catalog: Vec<ModelEntry>,
    /// Default model for general chat.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.model-catalog.default-model@v1"))]
    pub default_model: String,
    /// Default model for fast/low-latency chat.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.model-catalog.default-fast-model@v1"))]
    pub default_fast_model: String,
}

/// Thinking mode configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.thinking-config.schema@v1"))]
pub struct ThinkingConfig {
    /// Whether to include thoughts in the output.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.thinking-config.include-thoughts@v1"))]
    pub include_thoughts: bool,
    /// Thinking budget in tokens.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.thinking-config.budget@v1"))]
    pub thinking_budget: i64,
}

/// Default generation settings for a specific use case.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generation-defaults.schema@v1"))]
pub struct GenerationDefaultsEntry {
    /// Sampling temperature.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.temperature@v1"))]
    pub temperature: f64,
    /// Nucleus sampling threshold.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.top-p@v1"))]
    pub top_p: f64,
    /// Top-k sampling threshold.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.top-k@v1"))]
    pub top_k: i64,
    /// Maximum output tokens.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.max-output-tokens@v1"))]
    pub max_output_tokens: i64,
    /// Optional thinking override.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generation-defaults.thinking-config@v1"))]
    pub thinking_config: Option<ThinkingConfig>,
}

/// Default generation settings by use case.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generation-defaults-map.schema@v1"))]
pub struct GenerationDefaults {
    /// General chat defaults.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.chat@v1"))]
    pub chat: GenerationDefaultsEntry,
    /// Code generation defaults.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.code@v1"))]
    pub code: GenerationDefaultsEntry,
    /// Reasoning defaults.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.reasoning@v1"))]
    pub reasoning: GenerationDefaultsEntry,
    /// Creative generation defaults.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.creative@v1"))]
    pub creative: GenerationDefaultsEntry,
    /// Precise generation defaults.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-defaults.precise@v1"))]
    pub precise: GenerationDefaultsEntry,
}

/// Generation configuration controls.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generation-config.schema@v1"))]
pub struct GenerationConfig {
    /// Sampling temperature.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.temperature@v1"))]
    pub temperature: f64,
    /// Nucleus sampling threshold.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.top-p@v1"))]
    pub top_p: f64,
    /// Top-k sampling threshold.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.top-k@v1"))]
    pub top_k: i64,
    /// Maximum output tokens.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.max-output-tokens@v1"))]
    pub max_output_tokens: i64,
    /// Number of candidate responses to generate.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.generation-config.candidate-count@v1"))]
    pub candidate_count: i64,
    /// Stop sequences.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.stop-sequences@v1"))]
    pub stop_sequences: Vec<String>,
    /// Response modalities.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.generation-config.response-modalities@v1"))]
    pub response_modalities: Vec<String>,
    /// Presence penalty.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.presence-penalty@v1"))]
    pub presence_penalty: f64,
    /// Frequency penalty.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.frequency-penalty@v1"))]
    pub frequency_penalty: f64,
    /// Deterministic seed, null for random.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generation-config.seed@v1"))]
    pub seed: Option<JsonValue>,
    /// Thinking mode configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generation-config.thinking-config@v1"))]
    pub thinking_config: ThinkingConfig,
    /// Per-use-case defaults.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generation-config.defaults@v1"))]
    pub defaults: GenerationDefaults,
}

/// A single safety filter threshold.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.safety-setting.schema@v1"))]
pub struct SafetySetting {
    /// Harm category.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.safety-setting.category@v1"))]
    pub category: String,
    /// Default blocking threshold.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.safety-setting.threshold@v1"))]
    pub threshold: String,
    /// Maximum blocking threshold.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.safety-setting.max-threshold@v1"))]
    pub max_threshold: String,
    /// Harm detection method.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.safety-setting.method@v1"))]
    pub method: String,
}

/// Token usage counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.token-tracking.schema@v1"))]
pub struct TokenTracking {
    /// Total input tokens consumed.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.token-tracking.total-input@v1"))]
    pub total_input_tokens: i64,
    /// Total output tokens generated.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.token-tracking.total-output@v1"))]
    pub total_output_tokens: i64,
    /// Total cached tokens used.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.token-tracking.total-cached@v1"))]
    pub total_cached_tokens: i64,
    /// Total thinking tokens consumed.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.token-tracking.total-thinking@v1"))]
    pub total_thinking_tokens: i64,
    /// Input tokens in the current session.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.token-tracking.session-input@v1"))]
    pub session_input_tokens: i64,
    /// Output tokens in the current session.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.token-tracking.session-output@v1"))]
    pub session_output_tokens: i64,
}

/// A single rate/quota limit bucket.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.quota-limit.schema@v1"))]
pub struct QuotaLimit {
    /// Limit value, null when unlimited.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.quota-limit.limit@v1"))]
    pub limit: Option<i64>,
    /// Tokens/requests already consumed.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.quota-limit.used@v1"))]
    pub used: i64,
    /// Remaining tokens/requests, null when unlimited.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.quota-limit.remaining@v1"))]
    pub remaining: Option<i64>,
}

/// Per-model quota limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.model-quota.schema@v1"))]
pub struct ModelQuota {
    /// Requests per minute limit.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-quota.requests-per-minute@v1"))]
    pub requests_per_minute: QuotaLimit,
    /// Tokens per minute limit.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-quota.tokens-per-minute@v1"))]
    pub tokens_per_minute: QuotaLimit,
    /// Requests per day limit.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.model-quota.requests-per-day@v1"))]
    pub requests_per_day: QuotaLimit,
}

/// Cost and billing tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.cost-tracking.schema@v1"))]
pub struct CostTracking {
    /// Billing currency.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.cost-tracking.currency@v1"))]
    pub currency: String,
    /// Pricing model (e.g. pay_as_you_go).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.cost-tracking.pricing-model@v1"))]
    pub pricing_model: String,
    /// Total cost accrued.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.cost-tracking.total@v1"))]
    pub total_cost: f64,
    /// Cost in the current session.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.cost-tracking.session@v1"))]
    pub session_cost: f64,
    /// Billing account identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.cost-tracking.billing-account@v1"))]
    pub billing_account: String,
}

/// Monitoring endpoints.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.monitoring.schema@v1"))]
pub struct Monitoring {
    /// Metrics endpoint URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.monitoring.metrics-endpoint@v1"))]
    pub metrics_endpoint: String,
    /// Whether Cloud Logging is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.monitoring.logging-enabled@v1"))]
    pub logging_enabled: bool,
    /// Whether Cloud Trace is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.monitoring.trace-enabled@v1"))]
    pub trace_enabled: bool,
}

/// Usage and quota state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.usage.schema@v1"))]
pub struct Usage {
    /// Token counters.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.usage.token-tracking@v1"))]
    pub token_tracking: TokenTracking,
    /// Per-model quotas.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.usage.quotas@v1"))]
    pub quotas: HashMap<String, ModelQuota>,
    /// Cost tracking.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.usage.cost-tracking@v1"))]
    pub cost_tracking: CostTracking,
    /// Monitoring configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.usage.monitoring@v1"))]
    pub monitoring: Monitoring,
}

/// Vertex AI endpoint templates.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.vertex-endpoints.schema@v1"))]
pub struct VertexEndpoints {
    /// Base URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.vertex-endpoints.base-url@v1"))]
    pub base_url: String,
    /// Stream generate content URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.vertex-endpoints.stream-generate-content@v1"))]
    pub stream_generate_content: String,
    /// Predict URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.vertex-endpoints.predict@v1"))]
    pub predict: String,
    /// Count tokens URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.vertex-endpoints.count-tokens@v1"))]
    pub count_tokens: String,
    /// Batch prediction URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.vertex-endpoints.batch-predict@v1"))]
    pub batch_predict: String,
}

/// Generative Language API endpoint templates.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generative-language-endpoints.schema@v1"))]
pub struct GenerativeLanguageEndpoints {
    /// Base URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generative-language-endpoints.base-url@v1"))]
    pub base_url: String,
    /// Generate content URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generative-language-endpoints.generate-content@v1"))]
    pub generate_content: String,
    /// Stream generate content URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generative-language-endpoints.stream-generate-content@v1"))]
    pub stream_generate_content: String,
    /// Count tokens URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generative-language-endpoints.count-tokens@v1"))]
    pub count_tokens: String,
    /// Models list URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.generative-language-endpoints.models-list@v1"))]
    pub models_list: String,
}

/// MCP compact endpoint reference.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.mcp-compact-endpoint.schema@v1"))]
pub struct McpCompactEndpoint {
    /// Endpoint URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.mcp-compact-endpoint.url@v1"))]
    pub url: String,
    /// HTTP method.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.mcp-compact-endpoint.method@v1"))]
    pub method: String,
    /// Content-Type header.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.mcp-compact-endpoint.content-type@v1"))]
    pub content_type: String,
    /// Human-readable description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity.mcp-compact-endpoint.description@v1"))]
    pub description: String,
    /// Endpoint status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.mcp-compact-endpoint.status@v1"))]
    pub status: String,
}

/// OAuth bridge endpoint details.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.oauth-bridge-endpoint.schema@v1"))]
pub struct OauthBridgeEndpoint {
    /// Bridge URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.oauth-bridge-endpoint.url@v1"))]
    pub url: String,
    /// Health check URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.service.antigravity.oauth-bridge-endpoint.health-check@v1"))]
    pub health_check: String,
    /// Token refresh URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.oauth-bridge-endpoint.token-refresh@v1"))]
    pub token_refresh: String,
    /// Endpoint status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.oauth-bridge-endpoint.status@v1"))]
    pub status: String,
}

/// Gemma API endpoint templates.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.gemma-endpoints.schema@v1"))]
pub struct GemmaEndpoints {
    /// Base URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.gemma-endpoints.base-url@v1"))]
    pub base_url: String,
    /// Generate URL template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.gemma-endpoints.generate@v1"))]
    pub generate: String,
}

/// Service endpoint collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity.endpoints.schema@v1"))]
pub struct Endpoints {
    /// Vertex AI endpoints.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.endpoints.vertex-ai@v1"))]
    pub vertex_ai: VertexEndpoints,
    /// Generative Language API endpoints.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.endpoints.generative-language@v1"))]
    pub generative_language_api: GenerativeLanguageEndpoints,
    /// MCP compact endpoint.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.endpoints.mcp-compact@v1"))]
    pub mcp_compact: McpCompactEndpoint,
    /// OAuth bridge endpoint.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.endpoints.oauth-bridge@v1"))]
    pub oauth_bridge: OauthBridgeEndpoint,
    /// Gemma API endpoints.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.endpoints.gemma@v1"))]
    pub gemma_api: GemmaEndpoints,
}

/// Top-level Antigravity state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.antigravity.schema@v1"))]
pub struct AntigravityState {
    /// Operational status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity.status@v1"))]
    pub status: String,
    /// Authentication state.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.auth@v1"))]
    pub auth: Auth,
    /// Google Cloud project metadata.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.project@v1"))]
    pub project: Project,
    /// Model catalog.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.antigravity.models@v1"))]
    pub models: ModelCatalog,
    /// Generation configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.antigravity.generation-config@v1"))]
    pub generation_config: GenerationConfig,
    /// Safety filter settings.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.safety-settings@v1"))]
    pub safety_settings: Vec<SafetySetting>,
    /// Usage and quota tracking.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.service.antigravity.usage@v1"))]
    pub usage: Usage,
    /// Service endpoint collection.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.antigravity.endpoints@v1"))]
    pub endpoints: Endpoints,
    /// Shared LLM projection fields (flattened to the top level).
    #[serde(flatten)]
    #[schemars(extend("x-oscal-subid" = "sch.software.antigravity.llm-projection@v1"))]
    pub projection: LlmProjection,
}

pub struct AntigravityPlugin;

impl Default for AntigravityPlugin {
    fn default() -> Self {
        Self
    }
}

impl AntigravityPlugin {
    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    pub(crate) fn current_state() -> AntigravityState {
        let project_id = Self::env_or("GOOGLE_CLOUD_PROJECT", "secure-bonbon-337711");
        let location = Self::env_or("GOOGLE_CLOUD_LOCATION", "us-central1");
        let use_vertexai = Self::env_or("GOOGLE_GENAI_USE_VERTEXAI", "true");

        AntigravityState {
            status: "active".to_string(),
            auth: Auth {
                method: "oauth".to_string(),
                provider: "google".to_string(),
                token_source: "gcloud".to_string(),
                token_file: "~/.config/gcloud/application_default_credentials.json".to_string(),
                token_valid: true,
                token_expiry: Some(JsonValue::Null),
                scopes: vec![
                    "https://www.googleapis.com/auth/cloud-platform".to_string(),
                    "https://www.googleapis.com/auth/generative-language".to_string(),
                ],
                vertex_ai: VertexAuth {
                    enabled: true,
                    use_vertexai: use_vertexai.clone(),
                    project: project_id.clone(),
                    location: location.clone(),
                    api_endpoint: format!("https://{}-aiplatform.googleapis.com", location),
                    credentials_type: "service_account".to_string(),
                    service_account_email: format!(
                        "antigravity-sdk@{}.iam.gserviceaccount.com",
                        project_id.clone()
                    ),
                    quota_project: project_id.clone(),
                },
                api_keys: ApiKeys {
                    generative_language: ApiKey {
                        configured: true,
                        env_var: "GOOGLE_API_KEY".to_string(),
                        masked_value: "********".to_string(),
                        restriction: "allowed_referers".to_string(),
                    },
                    vertex_ai: ApiKey {
                        configured: true,
                        env_var: "GOOGLE_VERTEX_AI_API_KEY".to_string(),
                        masked_value: "********".to_string(),
                        restriction: "private".to_string(),
                    },
                },
                oauth_bridge: OauthBridge {
                    url: "http://127.0.0.1:3333".to_string(),
                    status: "available".to_string(),
                    version: "0.1.0".to_string(),
                },
            },
            project: Project {
                project_id: project_id.clone(),
                project_number: "266510671468".to_string(),
                name: "secure-bonbon-337711".to_string(),
                location: location.clone(),
                region: location.clone(),
                api_enabled: vec![
                    "aiplatform.googleapis.com".to_string(),
                    "generativelanguage.googleapis.com".to_string(),
                    "compute.googleapis.com".to_string(),
                    "cloudresourcemanager.googleapis.com".to_string(),
                    "iam.googleapis.com".to_string(),
                    "monitoring.googleapis.com".to_string(),
                    "logging.googleapis.com".to_string(),
                ],
                labels: {
                    let mut m = HashMap::new();
                    m.insert("environment".to_string(), "production".to_string());
                    m.insert("managed_by".to_string(), "antigravity-sdk".to_string());
                    m.insert("framework".to_string(), "operation-dbus-proto".to_string());
                    m
                },
            },
            projection: LlmProjection {
                providers: vec![
                    Provider {
                        id: "antigravity".to_string(),
                        route: "antigravity".to_string(),
                        kind: "orchestrator".to_string(),
                        aliases: vec![
                            "google.antigravity".to_string(),
                            "antigravity-sdk".to_string(),
                            "agi".to_string(),
                        ],
                        endpoint: format!("https://{}-aiplatform.googleapis.com/v1", location),
                        auth: "oauth".to_string(),
                        sdk: "google.antigravity".to_string(),
                        oscal_source: "/opdbus/v1/plugins/oscal_subid_registry".to_string(),
                        ..Default::default()
                    },
                    Provider {
                        id: "google".to_string(),
                        route: "gemini".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec![
                            "gemini".to_string(),
                            "google-gemini".to_string(),
                            "vertex".to_string(),
                        ],
                        endpoint: format!("https://{}-aiplatform.googleapis.com/v1", location),
                        auth: "oauth".to_string(),
                        sdk: "google-cloud-aiplatform".to_string(),
                        ..Default::default()
                    },
                    Provider {
                        id: "gemma-virtual".to_string(),
                        route: "antigravity:gemma".to_string(),
                        kind: "virtual".to_string(),
                        aliases: vec!["gemma4".to_string(), "gemma-online".to_string()],
                        endpoint: "https://gemma.googleapis.com/v1".to_string(),
                        auth: "api_key".to_string(),
                        sdk: "google.gemma".to_string(),
                        description: "Gemma models accessible through Antigravity virtual provider".to_string(),
                        ..Default::default()
                    },
                    Provider {
                        id: "factory".to_string(),
                        route: "factory".to_string(),
                        kind: "orchestrator".to_string(),
                        aliases: vec![
                            "default".to_string(),
                            "auto".to_string(),
                            "droid".to_string(),
                        ],
                        endpoint: "https://api.factory.ai/api/v0".to_string(),
                        auth: "bearer".to_string(),
                        ..Default::default()
                    },
                    Provider {
                        id: "ollama".to_string(),
                        route: "ollama".to_string(),
                        kind: "local".to_string(),
                        aliases: vec!["gemma".to_string(), "gemma4".to_string()],
                        endpoint: "http://localhost:11434".to_string(),
                        auth: "none".to_string(),
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
                model_routes: vec![
                    ModelRoute {
                        hint: "best".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-3-pro-preview".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 3 Pro - primary Antigravity model".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "code".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-3-pro-preview".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 3 Pro for code generation tasks".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-3.5-flash".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 3.5 Flash for low-latency inference".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "reasoning".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-3-pro-preview".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 3 Pro with thinking mode for complex reasoning".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "vision".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-2.5-pro-vision".to_string(),
                        kind: "multimodal".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 2.5 Pro Vision for image/video understanding".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "embedding".to_string(),
                        provider: "antigravity".to_string(),
                        model: "text-embedding-005".to_string(),
                        kind: "embedding".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Vertex AI text embeddings for RAG and semantic search".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "code".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-2.5-pro".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 2.5 Pro - stable code generation".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-2.5-flash".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 2.5 Flash for quick tasks".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "audio".to_string(),
                        provider: "antigravity".to_string(),
                        model: "gemini-2.5-flash-audio".to_string(),
                        kind: "multimodal".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Gemini 2.5 Flash with native audio input".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "code".to_string(),
                        provider: "google".to_string(),
                        model: "gemini-2.5-pro".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Direct Vertex AI access to Gemini 2.5 Pro".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "google".to_string(),
                        model: "gemini-2.5-flash".to_string(),
                        kind: "chat".to_string(),
                        available: true,
                        status: "active".to_string(),
                        status_reason: "Direct Vertex AI access to Gemini 2.5 Flash".to_string(),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "compliance".to_string(),
                        provider: "oscal".to_string(),
                        model: "NIST_SP_800-53_Rev5".to_string(),
                        kind: "policy".to_string(),
                        available: false,
                        status: "declared".to_string(),
                        status_reason: "OSCAL policy provider; requires oscal_subid_registry projection".to_string(),
                        source: "/opdbus/v1/plugins/oscal_subid_registry".to_string(),
                        ..Default::default()
                    },
                ],
                router: Router {
                    provider: "ollama".to_string(),
                    model: "gemma4".to_string(),
                    endpoint: "http://localhost:11434".to_string(),
                    scope: "all".to_string(),
                    role: "classifier".to_string(),
                    emits: vec![
                        "provider".to_string(),
                        "model".to_string(),
                        "hint".to_string(),
                        "candidate_subids".to_string(),
                        "confidence".to_string(),
                    ],
                    oscal_source: "/opdbus/v1/plugins/oscal_subid_registry".to_string(),
                    classification_rules: serde_json::json!({
                        "code_task": {"hint": "code", "provider": "antigravity", "model": "gemini-3-pro-preview"},
                        "fast_task": {"hint": "fast", "provider": "antigravity", "model": "gemini-3.5-flash"},
                        "reasoning_task": {"hint": "reasoning", "provider": "antigravity", "model": "gemini-3-pro-preview"},
                        "vision_task": {"hint": "vision", "provider": "antigravity", "model": "gemini-2.5-pro-vision"},
                        "compliance_task": {"hint": "compliance", "provider": "oscal", "model": "NIST_SP_800-53_Rev5"}
                    }),
                },
                tools: vec![
                    LlmTool {
                        name: "antigravity.chat".to_string(),
                        description: "Send a chat turn to Antigravity/Gemini and return the response with optional structured output.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "message": {"type": "string", "description": "User message content"},
                                "model": {"type": "string", "enum": ["gemini-3-pro-preview", "gemini-3.5-flash", "gemini-2.5-pro", "gemini-2.5-flash"], "default": "gemini-3-pro-preview"},
                                "temperature": {"type": "number", "minimum": 0.0, "maximum": 2.0},
                                "response_schema": {"type": "object", "description": "Pydantic BaseModel to coerce structured output"},
                                "thinking_config": {"type": "object", "properties": {"include_thoughts": {"type": "boolean"}, "thinking_budget": {"type": "integer"}}}
                            },
                            "required": ["message"]
                        }),
                        ..Default::default()
                    },
                    LlmTool {
                        name: "antigravity.models.list".to_string(),
                        description: "List available Gemini models with capabilities and status.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "family": {"type": "string", "enum": ["gemini", "gemma", "embedding"]},
                                "capabilities": {"type": "array", "items": {"type": "string"}}
                            },
                            "required": []
                        }),
                        ..Default::default()
                    },
                    LlmTool {
                        name: "antigravity.auth.status".to_string(),
                        description: "Check OAuth token validity, Vertex AI config, and service account status.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "required": []
                        }),
                        ..Default::default()
                    },
                    LlmTool {
                        name: "antigravity.usage.report".to_string(),
                        description: "Return current token usage and quota statistics.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "project_id": {"type": "string"},
                                "model": {"type": "string"}
                            },
                            "required": []
                        }),
                        ..Default::default()
                    },
                    LlmTool {
                        name: "antigravity.safety.configure".to_string(),
                        description: "Update Gemini safety filter thresholds per category.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "harassment": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                                "hate_speech": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                                "sexually_explicit": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                                "dangerous": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]},
                                "civic_integrity": {"type": "string", "enum": ["BLOCK_NONE", "BLOCK_ONLY_HIGH", "BLOCK_MEDIUM_AND_ABOVE", "BLOCK_LOW_AND_ABOVE", "HARM_BLOCK_THRESHOLD_UNSPECIFIED"]}
                            },
                            "required": []
                        }),
                        ..Default::default()
                    },
                ],
                config_schema: ConfigSchema {
                    source: "antigravity sdk v1".to_string(),
                    sdk_package: "google-genai".to_string(),
                    openapi_url: format!("https://{}-aiplatform.googleapis.com/v1/openapi.json", location),
                    api_version: "v1".to_string(),
                    schema_crate: "op_plugins::state_plugins::antigravity".to_string(),
                    native_type: "AntigravityState".to_string(),
                    env_vars: serde_json::json!({
                        "GOOGLE_CLOUD_PROJECT": {"description": "Google Cloud project ID", "default": "secure-bonbon-337711"},
                        "GOOGLE_CLOUD_LOCATION": {"description": "Google Cloud region/location", "default": "us-central1"},
                        "GOOGLE_GENAI_USE_VERTEXAI": {"description": "Use Vertex AI backend (true) or Gemini API (false)", "default": "true"},
                        "GOOGLE_API_KEY": {"description": "Gemini API key for generative language API", "required": false},
                        "GOOGLE_VERTEX_AI_API_KEY": {"description": "Vertex AI API key", "required": false}
                    }),
                    ..Default::default()
                },
                ui_surfaces: vec![
                    UiSurface {
                        path: "/antigravity".to_string(),
                        name: "Antigravity SDK".to_string(),
                        schema: "antigravity".to_string(),
                    },
                    UiSurface {
                        path: "/antigravity/models".to_string(),
                        name: "Gemini Models".to_string(),
                        schema: "antigravity.models".to_string(),
                    },
                    UiSurface {
                        path: "/antigravity/safety".to_string(),
                        name: "Safety Settings".to_string(),
                        schema: "antigravity.safety".to_string(),
                    },
                    UiSurface {
                        path: "/antigravity/usage".to_string(),
                        name: "Usage & Quotas".to_string(),
                        schema: "antigravity.usage".to_string(),
                    },
                    UiSurface {
                        path: "/antigravity/auth".to_string(),
                        name: "Authentication".to_string(),
                        schema: "antigravity.auth".to_string(),
                    },
                    UiSurface {
                        path: "/antigravity/tools".to_string(),
                        name: "MCP Tools".to_string(),
                        schema: "antigravity.tools".to_string(),
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
                    response_mime_type: "application/json".to_string(),
                    pydantic_source: vec![
                        "import pydantic".to_string(),
                        "class UiResponseSchema(pydantic.BaseModel):".to_string(),
                        "    action: str".to_string(),
                        "    metadata: dict[str, str]".to_string(),
                        "    confidence_score: float".to_string(),
                    ],
                    ui_renderer: "JsonRenderer".to_string(),
                    schema_validation: "strict".to_string(),
                    refusals_as_none: true,
                    required: true,
                },
            },
            models: ModelCatalog {
                catalog: vec![
                    ModelEntry {
                        id: "gemini-3-pro-preview".to_string(),
                        name: "Gemini 3 Pro Preview".to_string(),
                        family: "gemini".to_string(),
                        generation: Some(3.0),
                        capabilities: vec![
                            "chat".to_string(),
                            "code".to_string(),
                            "reasoning".to_string(),
                            "vision".to_string(),
                            "tools".to_string(),
                            "structured_output".to_string(),
                            "function_calling".to_string(),
                        ],
                        context_window: Some(2097152),
                        max_output_tokens: Some(65536),
                        supported_modalities: vec![
                            "text".to_string(),
                            "image".to_string(),
                            "video".to_string(),
                            "audio".to_string(),
                        ],
                        thinking_mode: true,
                        rate_limit_rpm: Some(360),
                        ..Default::default()
                    },
                    ModelEntry {
                        id: "gemini-3.5-flash".to_string(),
                        name: "Gemini 3.5 Flash".to_string(),
                        family: "gemini".to_string(),
                        generation: Some(3.5),
                        capabilities: vec![
                            "chat".to_string(),
                            "fast".to_string(),
                            "reasoning".to_string(),
                            "vision".to_string(),
                            "tools".to_string(),
                            "structured_output".to_string(),
                        ],
                        context_window: Some(1048576),
                        max_output_tokens: Some(32768),
                        supported_modalities: vec![
                            "text".to_string(),
                            "image".to_string(),
                            "video".to_string(),
                        ],
                        thinking_mode: true,
                        rate_limit_rpm: Some(2000),
                        ..Default::default()
                    },
                    ModelEntry {
                        id: "gemini-2.5-pro".to_string(),
                        name: "Gemini 2.5 Pro".to_string(),
                        family: "gemini".to_string(),
                        generation: Some(2.5),
                        capabilities: vec![
                            "chat".to_string(),
                            "code".to_string(),
                            "reasoning".to_string(),
                            "vision".to_string(),
                            "tools".to_string(),
                            "structured_output".to_string(),
                            "function_calling".to_string(),
                        ],
                        context_window: Some(2097152),
                        max_output_tokens: Some(65536),
                        supported_modalities: vec![
                            "text".to_string(),
                            "image".to_string(),
                            "video".to_string(),
                            "audio".to_string(),
                        ],
                        thinking_mode: true,
                        rate_limit_rpm: Some(360),
                        ..Default::default()
                    },
                    ModelEntry {
                        id: "gemini-2.5-flash".to_string(),
                        name: "Gemini 2.5 Flash".to_string(),
                        family: "gemini".to_string(),
                        generation: Some(2.5),
                        capabilities: vec![
                            "chat".to_string(),
                            "fast".to_string(),
                            "code".to_string(),
                            "tools".to_string(),
                            "structured_output".to_string(),
                        ],
                        context_window: Some(1048576),
                        max_output_tokens: Some(32768),
                        supported_modalities: vec![
                            "text".to_string(),
                            "image".to_string(),
                        ],
                        thinking_mode: false,
                        rate_limit_rpm: Some(2000),
                        ..Default::default()
                    },
                    ModelEntry {
                        id: "gemini-2.5-pro-vision".to_string(),
                        name: "Gemini 2.5 Pro Vision".to_string(),
                        family: "gemini".to_string(),
                        generation: Some(2.5),
                        capabilities: vec![
                            "vision".to_string(),
                            "image_understanding".to_string(),
                            "video_understanding".to_string(),
                            "multimodal".to_string(),
                        ],
                        context_window: Some(1048576),
                        max_output_tokens: Some(32768),
                        supported_modalities: vec![
                            "text".to_string(),
                            "image".to_string(),
                            "video".to_string(),
                        ],
                        thinking_mode: false,
                        rate_limit_rpm: Some(360),
                        ..Default::default()
                    },
                    ModelEntry {
                        id: "gemini-2.5-flash-audio".to_string(),
                        name: "Gemini 2.5 Flash Audio".to_string(),
                        family: "gemini".to_string(),
                        generation: Some(2.5),
                        capabilities: vec![
                            "audio".to_string(),
                            "transcription".to_string(),
                            "multimodal".to_string(),
                        ],
                        context_window: Some(1048576),
                        max_output_tokens: Some(32768),
                        supported_modalities: vec![
                            "text".to_string(),
                            "audio".to_string(),
                        ],
                        thinking_mode: false,
                        rate_limit_rpm: Some(1000),
                        ..Default::default()
                    },
                    ModelEntry {
                        id: "text-embedding-005".to_string(),
                        name: "Text Embedding 005".to_string(),
                        family: "embedding".to_string(),
                        generation: Some(5.0),
                        capabilities: vec![
                            "embedding".to_string(),
                            "semantic_search".to_string(),
                            "rag".to_string(),
                        ],
                        context_window: None,
                        max_output_tokens: None,
                        supported_modalities: vec!["text".to_string()],
                        thinking_mode: false,
                        rate_limit_rpm: Some(1500),
                        dimensions: Some(768),
                        max_input_tokens: Some(2048),
                    },
                    ModelEntry {
                        id: "gemma4".to_string(),
                        name: "Gemma 4".to_string(),
                        family: "gemma".to_string(),
                        generation: None,
                        capabilities: vec![
                            "chat".to_string(),
                            "code".to_string(),
                            "router".to_string(),
                            "classifier".to_string(),
                        ],
                        context_window: Some(32768),
                        max_output_tokens: Some(8192),
                        supported_modalities: vec!["text".to_string()],
                        thinking_mode: false,
                        rate_limit_rpm: None,
                        ..Default::default()
                    },
                ],
                default_model: "gemini-3-pro-preview".to_string(),
                default_fast_model: "gemini-3.5-flash".to_string(),
            },
            generation_config: GenerationConfig {
                temperature: 0.7,
                top_p: 0.95,
                top_k: 40,
                max_output_tokens: 65536,
                candidate_count: 1,
                stop_sequences: vec![],
                response_modalities: vec!["TEXT".to_string()],
                presence_penalty: 0.0,
                frequency_penalty: 0.0,
                seed: Some(JsonValue::Null),
                thinking_config: ThinkingConfig {
                    include_thoughts: false,
                    thinking_budget: 4096,
                },
                defaults: GenerationDefaults {
                    chat: GenerationDefaultsEntry {
                        temperature: 0.7,
                        top_p: 0.95,
                        top_k: 40,
                        max_output_tokens: 65536,
                        thinking_config: None,
                    },
                    code: GenerationDefaultsEntry {
                        temperature: 0.1,
                        top_p: 0.95,
                        top_k: 40,
                        max_output_tokens: 65536,
                        thinking_config: None,
                    },
                    reasoning: GenerationDefaultsEntry {
                        temperature: 0.7,
                        top_p: 0.95,
                        top_k: 40,
                        max_output_tokens: 65536,
                        thinking_config: Some(ThinkingConfig {
                            include_thoughts: true,
                            thinking_budget: 16384,
                        }),
                    },
                    creative: GenerationDefaultsEntry {
                        temperature: 1.0,
                        top_p: 0.99,
                        top_k: 60,
                        max_output_tokens: 32768,
                        thinking_config: None,
                    },
                    precise: GenerationDefaultsEntry {
                        temperature: 0.0,
                        top_p: 1.0,
                        top_k: 1,
                        max_output_tokens: 65536,
                        thinking_config: None,
                    },
                },
            },
            safety_settings: vec![
                SafetySetting {
                    category: "HARM_CATEGORY_HARASSMENT".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                    max_threshold: "BLOCK_ONLY_HIGH".to_string(),
                    method: "probability".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                    max_threshold: "BLOCK_ONLY_HIGH".to_string(),
                    method: "probability".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                    max_threshold: "BLOCK_ONLY_HIGH".to_string(),
                    method: "probability".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                    max_threshold: "BLOCK_ONLY_HIGH".to_string(),
                    method: "probability".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_CIVIC_INTEGRITY".to_string(),
                    threshold: "BLOCK_NONE".to_string(),
                    max_threshold: "BLOCK_NONE".to_string(),
                    method: "probability".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_UNSPECIFIED".to_string(),
                    threshold: "BLOCK_NONE".to_string(),
                    max_threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                    method: "unspecified".to_string(),
                },
            ],
            usage: Usage {
                token_tracking: TokenTracking {
                    total_input_tokens: 0,
                    total_output_tokens: 0,
                    total_cached_tokens: 0,
                    total_thinking_tokens: 0,
                    session_input_tokens: 0,
                    session_output_tokens: 0,
                },
                quotas: {
                    let mut q = HashMap::new();
                    q.insert(
                        "gemini-3-pro-preview".to_string(),
                        ModelQuota {
                            requests_per_minute: QuotaLimit {
                                limit: Some(360),
                                used: 0,
                                remaining: Some(360),
                            },
                            tokens_per_minute: QuotaLimit {
                                limit: Some(4000000),
                                used: 0,
                                remaining: Some(4000000),
                            },
                            requests_per_day: QuotaLimit {
                                limit: Some(1500),
                                used: 0,
                                remaining: Some(1500),
                            },
                        },
                    );
                    q.insert(
                        "gemini-3.5-flash".to_string(),
                        ModelQuota {
                            requests_per_minute: QuotaLimit {
                                limit: Some(2000),
                                used: 0,
                                remaining: Some(2000),
                            },
                            tokens_per_minute: QuotaLimit {
                                limit: Some(4000000),
                                used: 0,
                                remaining: Some(4000000),
                            },
                            requests_per_day: QuotaLimit {
                                limit: None,
                                used: 0,
                                remaining: None,
                            },
                        },
                    );
                    q.insert(
                        "gemini-2.5-pro".to_string(),
                        ModelQuota {
                            requests_per_minute: QuotaLimit {
                                limit: Some(360),
                                used: 0,
                                remaining: Some(360),
                            },
                            tokens_per_minute: QuotaLimit {
                                limit: Some(4000000),
                                used: 0,
                                remaining: Some(4000000),
                            },
                            requests_per_day: QuotaLimit {
                                limit: Some(1500),
                                used: 0,
                                remaining: Some(1500),
                            },
                        },
                    );
                    q.insert(
                        "gemini-2.5-flash".to_string(),
                        ModelQuota {
                            requests_per_minute: QuotaLimit {
                                limit: Some(2000),
                                used: 0,
                                remaining: Some(2000),
                            },
                            tokens_per_minute: QuotaLimit {
                                limit: Some(4000000),
                                used: 0,
                                remaining: Some(4000000),
                            },
                            requests_per_day: QuotaLimit {
                                limit: None,
                                used: 0,
                                remaining: None,
                            },
                        },
                    );
                    q
                },
                cost_tracking: CostTracking {
                    currency: "USD".to_string(),
                    pricing_model: "pay_as_you_go".to_string(),
                    total_cost: 0.0,
                    session_cost: 0.0,
                    billing_account: format!("billingAccounts/{}", project_id.clone()),
                },
                monitoring: Monitoring {
                    metrics_endpoint: format!(
                        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/models:monitor",
                        location, project_id.clone(), location
                    ),
                    logging_enabled: true,
                    trace_enabled: true,
                },
            },
            endpoints: Endpoints {
                vertex_ai: VertexEndpoints {
                    base_url: format!("https://{}-aiplatform.googleapis.com/v1", location),
                    stream_generate_content: format!(
                        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent",
                        location, project_id.clone(), location, "{}"
                    ),
                    predict: format!(
                        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:predict",
                        location, project_id.clone(), location, "{}"
                    ),
                    count_tokens: format!(
                        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:countTokens",
                        location, project_id.clone(), location, "{}"
                    ),
                    batch_predict: format!(
                        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/batchPredictionJobs",
                        location, project_id.clone(), location
                    ),
                },
                generative_language_api: GenerativeLanguageEndpoints {
                    base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
                    generate_content: "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent".to_string(),
                    stream_generate_content: "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent".to_string(),
                    count_tokens: "https://generativelanguage.googleapis.com/v1beta/models/{}:countTokens".to_string(),
                    models_list: "https://generativelanguage.googleapis.com/v1beta/models".to_string(),
                },
                mcp_compact: McpCompactEndpoint {
                    url: "http://127.0.0.1:8080/mcp/compact".to_string(),
                    method: "POST".to_string(),
                    content_type: "application/json".to_string(),
                    description: "MCP compact endpoint - all registered tools in a single call".to_string(),
                    status: "available".to_string(),
                },
                oauth_bridge: OauthBridgeEndpoint {
                    url: "http://127.0.0.1:3333".to_string(),
                    health_check: "http://127.0.0.1:3333/health".to_string(),
                    token_refresh: "http://127.0.0.1:3333/refresh".to_string(),
                    status: "available".to_string(),
                },
                gemma_api: GemmaEndpoints {
                    base_url: "https://gemma.googleapis.com/v1".to_string(),
                    generate: "https://gemma.googleapis.com/v1/models/{}:generate".to_string(),
                },
            },
        }
    }
}

#[async_trait]
impl StatePlugin for AntigravityPlugin {
    fn name(&self) -> &str {
        "antigravity"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = antigravity_schema();
        super::common::llm_projection::rewrite_projection_subids_for_plugin(
            &mut schema,
            "antigravity",
        );
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
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

/// Canonical `antigravity` schema derived from [`AntigravityState`] via schemars.
pub(crate) fn antigravity_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(AntigravityState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "antigravity",
        "1.0.0",
        "Google Antigravity SDK provider — Vertex AI Gemini models, OAuth auth, structured output, OSCAL compliance routing",
        &root,
    )
}

/// Frozen golden reference: the original schema inferred from the default state
/// and then reconciled to the schemars-derived contract, kept **test-only** so
/// `derived_schema_matches_hand_rolled` proves the derived schema still matches
/// the contract this plugin shipped with.
#[cfg(test)]
pub(crate) fn antigravity_schema_golden() -> PluginSchema {
    use super::common::llm_projection::schema_helpers::golden_from_state_and_schema;

    let state = simd_json::serde::to_owned_value(AntigravityState::default())
        .unwrap_or_else(|_| simd_json::json!({}));
    let schema_json = serde_json::to_value(schemars::schema_for!(AntigravityState))
        .expect("schemars schema serializes to JSON");
    golden_from_state_and_schema(
        &state,
        &schema_json,
        "antigravity",
        "llm",
        "1.0.0",
        "Google Antigravity SDK provider — Vertex AI Gemini models, OAuth auth, structured output, OSCAL compliance routing",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use crate::state_plugins::schemars_adapter::schema_diffs;
    use serde_json::Value as JVal;

    fn collect_subids(node: &JVal, out: &mut Vec<String>) {
        if let Some(subid) = node.get("x-oscal-subid").and_then(JVal::as_str) {
            out.push(subid.to_string());
        }
        if let Some(props) = node.get("properties").and_then(JVal::as_object) {
            for v in props.values() {
                collect_subids(v, out);
            }
        }
        if let Some(defs) = node
            .get("$defs")
            .or_else(|| node.get("definitions"))
            .and_then(JVal::as_object)
        {
            for v in defs.values() {
                collect_subids(v, out);
            }
        }
        if let Some(items) = node.get("items") {
            collect_subids(items, out);
        }
        if let Some(alternatives) = node
            .get("anyOf")
            .or_else(|| node.get("oneOf"))
            .and_then(JVal::as_array)
        {
            for v in alternatives {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = antigravity_schema_golden();
        let derived = antigravity_schema();
        let diffs = schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(AntigravityState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(
            !subids.is_empty(),
            "expected at least one x-oscal-subid in the derived schema"
        );
        for subid in subids {
            validate_subid(&subid).expect("invalid subid: {subid}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("antigravity", |_ctx| std::sync::Arc::new(AntigravityPlugin::new()))
}
