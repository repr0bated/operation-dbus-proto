use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

/// Voyage AI client for text embeddings
pub struct VoyageClient {
    client: Client,
    api_key: String,
    model: String,
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
    /// Create a new Voyage client
    pub fn new() -> Result<Self> {
        let api_key = env::var("VOYAGE_API_KEY").context("VOYAGE_API_KEY not found")?;
        // Use voyage-law-2 or voyage-4-large as specified
        let model = env::var("VOYAGE_MODEL").unwrap_or_else(|_| "voyage-law-2".to_string());

        Ok(Self {
            client: Client::new(),
            api_key,
            model,
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
            .post("https://api.voyageai.com/v1/embeddings")
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
