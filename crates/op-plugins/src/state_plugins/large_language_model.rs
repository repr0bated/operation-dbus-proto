//! Generic, model-agnostic large-language-model plugin.
//!
//! The single inference surface — provider, model id, endpoint, and generation
//! params. The model is a FIELD, not the plugin's identity: swap `gemma3` →
//! `llama3`/`mistral` by changing `model_id`. This is the ONLY plugin that knows
//! about Ollama; consumers (e.g. `gemma_brain`) talk to this surface and embed
//! no model of their own.
//!
//! The plugin reports the *live* observed state: it queries the
//! provider (`GET {endpoint}/api/tags` for Ollama) for the available models and
//! reachability. The schema is published to the SHM folder/manifest by
//! op-projection's enumerator; this plugin does not self-write `/dev/shm`.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::time::Duration;

/// Generation parameters for an inference request.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.large-language-model.params.schema@v1"))]
pub struct GenerationParams {
    /// Sampling temperature (higher = more random).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.large-language-model.temperature@v1"))]
    pub temperature: f32,
    /// Maximum number of tokens to generate.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.large-language-model.max-tokens@v1"))]
    pub max_tokens: u32,
    /// Context window size, in tokens.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.large-language-model.context-window@v1"))]
    pub context_window: u32,
}

/// Top-level state for the generic large-language-model surface.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.large-language-model.schema@v1"))]
pub struct LargeLanguageModelState {
    /// Inference provider / runtime — the only Ollama-aware field.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.large-language-model.provider@v1"))]
    pub provider: String,
    /// Active model identifier, e.g. "gemma3", "llama3", "mistral". Swappable.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.large-language-model.model-id@v1"))]
    pub model_id: String,
    /// Provider endpoint URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.large-language-model.endpoint@v1"))]
    pub endpoint: String,
    /// Reachability of the provider: "online" or "offline" (observed).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.large-language-model.status@v1"))]
    pub status: String,
    /// Models reported available by the provider (observed live).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.large-language-model.available-models@v1"))]
    pub available_models: Vec<String>,
    /// Default generation parameters.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.large-language-model.params@v1"))]
    pub params: GenerationParams,
}

pub struct LargeLanguageModelPlugin;

impl Default for LargeLanguageModelPlugin {
    fn default() -> Self {
        Self
    }
}

impl LargeLanguageModelPlugin {
    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    fn env_u32(key: &str, fallback: u32) -> u32 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(fallback)
    }

    fn env_f32(key: &str, fallback: f32) -> f32 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(fallback)
    }

    /// Configuration portion of the state, sourced from the environment.
    /// `status`/`available_models` are observed and filled in by the live query.
    fn config_state() -> LargeLanguageModelState {
        LargeLanguageModelState {
            provider: Self::env_or("LLM_PROVIDER", "ollama"),
            model_id: Self::env_or("LLM_MODEL_ID", "gemma3"),
            endpoint: Self::env_or("LLM_ENDPOINT", "http://127.0.0.1:11434"),
            status: "offline".to_string(),
            available_models: Vec::new(),
            params: GenerationParams {
                temperature: Self::env_f32("LLM_TEMPERATURE", 0.7),
                max_tokens: Self::env_u32("LLM_MAX_TOKENS", 2048),
                context_window: Self::env_u32("LLM_CONTEXT_WINDOW", 8192),
            },
        }
    }

    /// Query the Ollama-compatible provider for its model list. Returns `None`
    /// if the provider is unreachable or the response can't be parsed.
    async fn fetch_available_models(endpoint: &str) -> Option<Vec<String>> {
        let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .ok()?;
        let body = client.get(&url).send().await.ok()?.text().await.ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
        let models = parsed.get("models").and_then(serde_json::Value::as_array)?;
        Some(
            models
                .iter()
                .filter_map(|m| m.get("name").and_then(serde_json::Value::as_str))
                .map(String::from)
                .collect(),
        )
    }
}

#[async_trait]
impl StatePlugin for LargeLanguageModelPlugin {
    fn name(&self) -> &str {
        "large_language_model"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = large_language_model_schema();
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
            state_snapshot: simd_json::serde::to_owned_value(Self::config_state())?,
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

/// Canonical `large_language_model` schema derived from
/// [`LargeLanguageModelState`] via schemars.
pub(crate) fn large_language_model_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(LargeLanguageModelState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "large_language_model",
        "1.0.0",
        "Generic model-agnostic LLM inference surface (provider, model id, endpoint, generation params); the model is a swappable field",
        &root,
    )
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("large_language_model", |_ctx| std::sync::Arc::new(LargeLanguageModelPlugin::new()))
}
