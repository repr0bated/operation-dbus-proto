use anyhow::{Context, Result};
use op_plugins::state_plugins::embedding_model::{voyage_embed_params, VoyageModel};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Voyage AI client for text embeddings
pub struct VoyageClient {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    input: Vec<&'a str>,
    model: &'a str,
    input_type: Option<&'a str>,
    /// Pinned to the shared-space dimension so callers always land in the same
    /// Qdrant collection regardless of which model in the trio is active —
    /// without this, Voyage returns each model's native (differing) dimension.
    truncation: bool,
    output_dimension: u32,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

impl VoyageClient {
    /// Construct directly from resolved call-time parameters (endpoint,
    /// model, key) — the primary path, used when the `embedding_model`
    /// plugin's config is available.
    pub fn from_params(endpoint: String, api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url: endpoint,
        }
    }

    /// Create a new Voyage client by resolving config through the
    /// `embedding_model` plugin (projection, falling back to its own env-var
    /// reads — see [`op_plugins::state_plugins::embedding_model::voyage_embed_params`]).
    pub fn new() -> Result<Self> {
        let params = voyage_embed_params().context(
            "Voyage API key not found: set COGNITIVE_MCP_VOYAGE_API_KEY, \
             VOYAGE_API_KEY, or VOYAGE_API_KEY_RUST",
        )?;
        Ok(Self::from_params(params.endpoint, params.api_key, params.model_id))
    }

    /// Embed text using Voyage API
    pub async fn embed(&self, text: &str, input_type: Option<&str>) -> Result<Vec<f32>> {
        let req = EmbeddingRequest {
            input: vec![text],
            model: &self.model,
            input_type,
            truncation: true,
            output_dimension: VoyageModel::SHARED_EMBEDDING_DIMS,
        };

        let resp = self
            .client
            .post(&self.base_url)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .context("Failed to send Voyage API request")?
            .error_for_status()
            .context("Voyage API returned error status")?
            .json::<EmbeddingResponse>()
            .await
            .context("Failed to parse Voyage API response")?;

        resp.data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .context("Voyage API returned no embeddings")
    }
}
