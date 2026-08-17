//! Generic, provider-agnostic embedding-model plugin.
//!
//! The single EMBEDDING surface — provider, model id, endpoint, and the shared
//! output dimensionality. The model is a FIELD, not the plugin's identity: swap
//! `voyage-4` → `voyage-4-lite` (or an Ollama/local embedder) by changing
//! `model_id`. This is the ONLY plugin that knows how to reach an embedding
//! provider; consumers (ctl_plane_chatbot, knowledge, memory, notebooklm)
//! delegate here and embed no model of their own. It is the embedding sibling of
//! [`super::large_language_model`].
//!
//! Embeddings are a different KIND from generation: the method surface is
//! `embed` (text → vector), and vectors in one shared space compare directly —
//! the voyage-4 trio writes one 1024-dim Qdrant collection with no re-index on
//! model switch.
//!
//! The plugin reports the *live* observed state. Providers differ: Ollama serves
//! embeddings and enumerates models (`GET {endpoint}/api/tags`), so the active
//! model can be resolved by lookup; Voyage is a cloud API (`POST /v1/embeddings`)
//! with no model-list endpoint, so it is reachable iff an API key is present and
//! the active model is config-driven. Either way, no active model is hardcoded.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::time::Duration;

/// Default public Voyage AI endpoint. The only copy of this constant — consumers
/// (e.g. `op-cognitive-mcp`) resolve their call-time endpoint through
/// [`voyage_embed_params`] rather than hardcoding it themselves.
const VOYAGE_PUBLIC_URL: &str = "https://api.voyageai.com/v1/embeddings";
/// MongoDB-hosted Voyage AI endpoint (for keys prefixed with `al-`). The only
/// copy of this constant.
const VOYAGE_MONGODB_URL: &str = "https://ai.mongodb.com/v1/embeddings";

/// Top-level state for the generic embedding surface.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.embedding-model.schema@v1"))]
#[schemars(extend("x-oscal-category" = "llm"))]
pub struct EmbeddingModelState {
    /// Embedding provider / runtime — the only provider-aware field
    /// ("voyage", "ollama", …).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.embedding-model.provider@v1"))]
    pub provider: String,
    /// Active embedding model identifier, e.g. "voyage-4", "voyage-4-lite".
    /// Swappable; resolved from config / provider, never hardcoded.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.embedding-model.model-id@v1"))]
    pub model_id: String,
    /// Provider endpoint URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.embedding-model.endpoint@v1"))]
    pub endpoint: String,
    /// Shared output dimensionality of the embedding space. The voyage-4 trio
    /// all emit 1024-dim vectors into one Qdrant collection; one dim per space.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.embedding-model.dimensions@v1"))]
    pub dimensions: u32,
    /// Reachability of the provider: "online" or "offline" (observed).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.embedding-model.status@v1"))]
    pub status: String,
    /// Models reported available by the provider (observed live where the
    /// provider supports enumeration; empty for cloud providers that don't).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.embedding-model.available-models@v1"))]
    pub available_models: Vec<String>,
    /// Content digest of the active model as reported by the provider (observed).
    /// Pins the exact weights the surface is bound to for audit / provenance;
    /// empty for providers that don't expose a digest.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.embedding-model.model-digest@v1"))]
    pub model_digest: String,
}

/// Interchangeable Voyage-4 embedding models — the canonical Voyage catalog.
///
/// `voyage-4-large`, `voyage-4` and `voyage-4-lite` share **one** embedding
/// space: vectors from any of the three are directly comparable, so all three
/// write to the *same* Qdrant collection at the *same* dimensionality
/// ([`VoyageModel::SHARED_EMBEDDING_DIMS`]). Switching models is a quality/cost
/// tradeoff only — it never requires re-vectorizing existing data.
///
/// `voyage-4-nano` is **deliberately excluded from this collection**, but for a
/// different reason than the domain models: per Voyage, "all 4 series models are
/// compatible with each other," so nano *is* in the same embedding space — it is
/// just dimensionally **capped at ≤512** (128/256/512). Our collection is pinned
/// at 1024, which nano cannot produce, so it can't join here. (It could join a
/// 256/512 collection.) Adding it to the 1024 trio would force everything down to
/// 512. Out for the 1024 setup — not out of the space.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VoyageModel {
    #[serde(rename = "voyage-4-large")]
    Voyage4Large,
    #[serde(rename = "voyage-4")]
    Voyage4,
    #[serde(rename = "voyage-4-lite")]
    Voyage4Lite,
}

impl Default for VoyageModel {
    fn default() -> Self {
        // POC target; cheapest of the shared-space trio.
        VoyageModel::Voyage4Lite
    }
}

impl VoyageModel {
    /// The single shared embedding dimensionality for the interchangeable
    /// voyage-4 trio. One collection, no re-index across models.
    pub const SHARED_EMBEDDING_DIMS: u32 = 1024;

    /// The interchangeable models, all in the shared embedding space.
    pub const SHARED_SPACE: [VoyageModel; 3] = [
        VoyageModel::Voyage4Large,
        VoyageModel::Voyage4,
        VoyageModel::Voyage4Lite,
    ];

    /// Canonical Voyage API model id.
    pub fn model_id(&self) -> &'static str {
        match self {
            VoyageModel::Voyage4Large => "voyage-4-large",
            VoyageModel::Voyage4 => "voyage-4",
            VoyageModel::Voyage4Lite => "voyage-4-lite",
        }
    }

    /// Output dimensionality. Identical across the trio so their vectors fuse
    /// and compare in the same Qdrant collection.
    pub fn dimensions(&self) -> u32 {
        Self::SHARED_EMBEDDING_DIMS
    }
}

/// Call-time parameters for a Voyage embed request, resolved from the
/// `embedding_model` projection (or its env-var fallback) plus a freshly-read
/// API key. This is the shared config surface `op-cognitive-mcp` calls into —
/// it never executes HTTP itself; that stays in the consuming crate.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.embedding-model.embed-params.schema@v1"))]
pub struct VoyageEmbedParams {
    /// Resolved Voyage endpoint URL (public or MongoDB, based on al- key prefix).
    #[schemars(extend("x-oscal-subid" = "obs.software.embedding-model.embed-params.endpoint@v1"))]
    pub endpoint: String,
    /// Active model identifier (e.g. "voyage-4").
    #[schemars(extend("x-oscal-subid" = "obs.software.embedding-model.embed-params.model-id@v1"))]
    pub model_id: String,
    /// Voyage API key, sourced from env vars in priority order. Never persisted
    /// to the projection.
    #[schemars(extend("x-oscal-subid" = "obs.software.embedding-model.embed-params.api-key@v1"))]
    pub api_key: String,
}

/// Resolve the call-time Voyage embed parameters: read the `embedding_model`
/// projection if present (endpoint + model_id come from there), otherwise fall
/// back to `EmbeddingModelPlugin::config_state()`'s env-var reads. The API key
/// is always read fresh from env — it is never stored in the projection.
/// Returns `None` when no Voyage API key exists in the environment.
///
/// OSCAL subid: obs.service.embedding-model.embed-params.resolve@v1
pub fn voyage_embed_params() -> Option<VoyageEmbedParams> {
    let api_key = EmbeddingModelPlugin::voyage_api_key()?;

    let state = op_core::projection_shm::read_projection_bytes("embedding_model")
        .and_then(|bytes| serde_json::from_slice::<EmbeddingModelState>(&bytes).ok())
        .unwrap_or_else(EmbeddingModelPlugin::config_state);

    // model_id is deliberately unresolved ("no hardcoded model") in config_state
    // when nothing configures it; a Voyage call still needs a concrete model, so
    // fall back to the documented POC-target default of the shared-space trio.
    let model_id = if state.model_id.is_empty() {
        VoyageModel::default().model_id().to_string()
    } else {
        state.model_id
    };

    Some(VoyageEmbedParams {
        endpoint: state.endpoint,
        model_id,
        api_key,
    })
}

pub struct EmbeddingModelPlugin;

impl Default for EmbeddingModelPlugin {
    fn default() -> Self {
        Self
    }
}

impl EmbeddingModelPlugin {
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

    /// Whether a Voyage API key is present in the environment — the cloud
    /// provider's notion of reachability (it has no list/health endpoint).
    fn voyage_key_present() -> bool {
        Self::voyage_api_key().is_some()
    }

    /// Read the Voyage API key from the environment, in priority order. The key
    /// is never stored in the projection (secrets do not live in shm) — it is
    /// always read fresh from env at call time.
    fn voyage_api_key() -> Option<String> {
        [
            "COGNITIVE_MCP_VOYAGE_API_KEY",
            "VOYAGE_API_KEY",
            "VOYAGE_API_KEY_RUST",
        ]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
    }

    /// Determine the correct Voyage endpoint for a given API key: MongoDB-hosted
    /// keys (`al-*`) route to the MongoDB endpoint, all others to the public one.
    /// The only copy of this detection logic.
    fn voyage_url_for_key(key: &str) -> String {
        if key.trim().starts_with("al-") {
            VOYAGE_MONGODB_URL.to_string()
        } else {
            VOYAGE_PUBLIC_URL.to_string()
        }
    }

    /// Configuration portion of the state, sourced from the environment.
    /// `status`/`available_models`/`model_digest` are observed and filled in by
    /// the live query. No active model is hardcoded: an explicit override wins,
    /// then the provider-specific env, else empty (resolved in `live_state`).
    fn config_state() -> EmbeddingModelState {
        let provider = Self::env_or("EMBEDDING_PROVIDER", "voyage");
        // Explicit override wins; otherwise for voyage, resolve public-vs-MongoDB
        // by the api key's `al-` prefix; otherwise the public URL is a harmless
        // default for non-voyage providers (they set their own endpoint).
        let endpoint = std::env::var("EMBEDDING_ENDPOINT").unwrap_or_else(|_| {
            if provider == "voyage" {
                Self::voyage_api_key()
                    .map(|key| Self::voyage_url_for_key(&key))
                    .unwrap_or_else(|| VOYAGE_PUBLIC_URL.to_string())
            } else {
                VOYAGE_PUBLIC_URL.to_string()
            }
        });
        EmbeddingModelState {
            provider,
            model_id: std::env::var("EMBEDDING_MODEL_ID")
                .or_else(|_| std::env::var("COGNITIVE_MCP_VOYAGE_MODEL"))
                .or_else(|_| std::env::var("VOYAGE_MODEL"))
                .unwrap_or_default(),
            endpoint,
            dimensions: Self::env_u32("EMBEDDING_DIMENSIONS", VoyageModel::SHARED_EMBEDDING_DIMS),
            status: "offline".to_string(),
            available_models: Vec::new(),
            model_digest: String::new(),
        }
    }

    /// Query an Ollama-compatible provider (`/api/tags`) for its embedding models
    /// and their content digests. Returns `None` if unreachable or unparseable.
    async fn fetch_ollama_models(endpoint: &str) -> Option<Vec<(String, String)>> {
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
                .filter_map(|m| {
                    let name = m.get("name").and_then(serde_json::Value::as_str)?;
                    let digest = m
                        .get("digest")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    Some((name.to_string(), digest.to_string()))
                })
                .collect(),
        )
    }

    /// Compose the live observed state. Config comes from the environment; the
    /// active model / reachability is a lookup to the provider — never a
    /// hardcoded active model. Ollama enumerates and resolves a model + digest;
    /// Voyage (and other cloud providers) are reachable iff an API key is present
    /// and keep the config-driven model with no enumerable catalog.
    async fn live_state() -> EmbeddingModelState {
        let mut state = Self::config_state();
        match state.provider.as_str() {
            "ollama" => {
                if let Some(models) = Self::fetch_ollama_models(&state.endpoint).await {
                    state.status = "online".to_string();
                    if state.model_id.is_empty() {
                        state.model_id = models
                            .first()
                            .map(|(name, _)| name.clone())
                            .unwrap_or_default();
                    }
                    if let Some((_, digest)) =
                        models.iter().find(|(name, _)| name == &state.model_id)
                    {
                        state.model_digest = digest.clone();
                    }
                    state.available_models = models.into_iter().map(|(name, _)| name).collect();
                }
            }
            // Voyage and other cloud providers: no list endpoint. Reachable iff
            // an API key is present; the model is config-driven and the catalog
            // is not enumerable, so available_models stays observed-empty.
            _ => {
                if Self::voyage_key_present() {
                    state.status = "online".to_string();
                    // Known Voyage catalog (the cloud API has no list endpoint):
                    // the shared-space trio. This is the enumerable catalog, not
                    // an active-model default.
                    state.available_models = VoyageModel::SHARED_SPACE
                        .iter()
                        .map(|m| m.model_id().to_string())
                        .collect();
                }
            }
        }
        state
    }
}

#[async_trait]
impl StatePlugin for EmbeddingModelPlugin {
    fn name(&self) -> &str {
        "embedding_model"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = embedding_model_schema();
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
            state_snapshot: simd_json::serde::to_owned_value(Self::live_state().await)?,
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

/// Canonical `embedding_model` schema derived from [`EmbeddingModelState`] via
/// schemars, plus the callable method surface.
pub(crate) fn embedding_model_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(EmbeddingModelState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "embedding_model",
        "1.0.0",
        "Generic provider-agnostic embedding surface (provider, model id, endpoint, shared dimensions); the model is a swappable field. The one embedding-kind sibling of large_language_model",
        &root,
    );

    // Typed method inputs / outputs.
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EmbedInput {
        /// Texts to embed. Documents and queries share one space; set
        /// `input_type` to "document" or "query" for the retrieval prompt.
        pub texts: Vec<String>,
        #[serde(default)]
        pub input_type: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EmbedOutput {
        pub vectors: Vec<Vec<f32>>,
        pub dimensions: u32,
        pub model: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListModelsOutput {
        pub models: Vec<String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetConfigOutput {
        pub config: serde_json::Value,
    }

    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "embed".to_string(),
        method_decl_from_schemars_with_output::<EmbedInput, EmbedOutput>(
            "embed",
            SideEffect::Mutation,
            true,
            "embedding.invoke",
            "mut.service.embedding.embed@v1",
        ),
    );
    schema.methods.insert(
        "list_models".to_string(),
        method_decl_from_schemars_with_output::<(), ListModelsOutput>(
            "list_models",
            SideEffect::Read,
            true,
            "embedding.read",
            "obs.service.embedding.model.list@v1",
        ),
    );
    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<(), GetConfigOutput>(
            "get_config",
            SideEffect::Read,
            true,
            "embedding.read",
            "obs.service.embedding.config.get@v1",
        ),
    );
    schema.methods.insert(
        "set_model".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "set_model",
            SideEffect::Mutation,
            false,
            "embedding.invoke",
            "mut.service.embedding.model.set@v1",
        ),
    );
    schema.methods.insert(
        "update_config".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "update_config",
            SideEffect::Mutation,
            false,
            "embedding.invoke",
            "mut.service.embedding.config.update@v1",
        ),
    );

    schema.capabilities.insert(
        "embedding.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "embedding.invoke".to_string(),
            description: "Grants: embed, set_model, update_config.".to_string(),
        },
    );
    schema.capabilities.insert(
        "embedding.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "embedding.read".to_string(),
            description: "Grants: list_models, get_config.".to_string(),
        },
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("embedding_model", |_ctx| std::sync::Arc::new(EmbeddingModelPlugin::new()))
}
