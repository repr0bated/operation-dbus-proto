use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

/// Default public Voyage AI endpoint.
const VOYAGE_PUBLIC_URL: &str = "https://api.voyageai.com/v1/embeddings";
/// MongoDB-hosted Voyage AI endpoint (for keys prefixed with `al-`).
const VOYAGE_MONGODB_URL: &str = "https://ai.mongodb.com/v1/embeddings";

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
    /// Create a new Voyage client.
    ///
    /// Auto-detects the endpoint: MongoDB-hosted keys (`al-*`) route to
    /// `https://ai.mongodb.com/v1/embeddings`; all others use the public
    /// `https://api.voyageai.com/v1/embeddings` endpoint.
    pub fn new() -> Result<Self> {
        let api_key = env::var("VOYAGE_API_KEY").context("VOYAGE_API_KEY not found")?;
        let model = env::var("VOYAGE_MODEL").unwrap_or_else(|_| "voyage-4".to_string());
        let base_url = voyage_url_for_key(&api_key);

        Ok(Self {
            client: Client::new(),
            api_key,
            model,
            base_url,
        })
    }

    /// Embed text using Voyage API
    pub async fn embed(&self, text: &str, input_type: Option<&str>) -> Result<Vec<f32>> {
        let req = EmbeddingRequest {
            input: vec![text],
            model: &self.model,
            input_type,
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

/// Determine the correct Voyage endpoint for a given API key.
fn voyage_url_for_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.starts_with("al-") {
        VOYAGE_MONGODB_URL.to_string()
    } else {
        VOYAGE_PUBLIC_URL.to_string()
    }
}
