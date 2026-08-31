//! Qdrant semantic search integration for gallery generation.
//!
//! Provides vectorized schema fragment search with domain-framing metadata.
//! Uses the same Qdrant instance as op-cognitive-mcp but a dedicated collection
//! (`gallery-gen-schemas`) with per-fragment domain tags.
//!
//! The indexer chunks each plugin schema into fragments (per-field, per-method)
//! and embeds them via the Voyage AI API (same as op-cognitive-mcp). Domain
//! tags are assigned based on plugin category and field/method characteristics.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_QDRANT_URL: &str = "http://127.0.0.1:6334";
const COLLECTION_NAME: &str = "gallery-gen-schemas";
const DEFAULT_VOYAGE_API_URL: &str = "https://api.voyageai.com/v1/embeddings";
const DEFAULT_VOYAGE_QUERY_MODEL: &str = "voyage-4";
const VECTOR_DIMENSION: u32 = 1024;

/// Domain tags for schema fragments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DomainTag {
    PrivacyOps,
    NetworkEngineering,
    Compliance,
    Accessibility,
    Operations,
    Development,
    Security,
    Identity,
}

impl DomainTag {
    /// Classify a plugin category into a domain tag.
    pub fn from_category(category: &str) -> Self {
        match category {
            "network" => DomainTag::NetworkEngineering,
            "security" => DomainTag::Security,
            "compliance" => DomainTag::Compliance,
            "observability" => DomainTag::Operations,
            "system" => DomainTag::Operations,
            "software" => DomainTag::Development,
            "identity" => DomainTag::Identity,
            "privacy" => DomainTag::PrivacyOps,
            _ => DomainTag::Development,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DomainTag::PrivacyOps => "privacy-ops",
            DomainTag::NetworkEngineering => "network-engineering",
            DomainTag::Compliance => "compliance",
            DomainTag::Accessibility => "accessibility",
            DomainTag::Operations => "operations",
            DomainTag::Development => "development",
            DomainTag::Security => "security",
            DomainTag::Identity => "identity",
        }
    }
}

/// A schema fragment for vectorization.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaFragment {
    pub plugin_id: String,
    pub fragment_type: String, // "field", "method", "summary"
    pub fragment_text: String,
    pub domain_tag: String,
}

/// Result from a semantic search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    pub plugin_id: String,
    pub fragment: String,
    pub domain_tag: String,
    pub score: f32,
}

/// Qdrant client for gallery-gen schema search.
pub struct GalleryQdrantClient {
    http_client: Client,
    qdrant_url: String,
    voyage_api_url: String,
    voyage_api_key: String,
    voyage_model: String,
}

impl GalleryQdrantClient {
    /// Create a new client from environment variables.
    pub fn from_env() -> Result<Self> {
        let qdrant_url = std::env::var("GALLERY_GEN_QDRANT_URL")
            .or_else(|_| std::env::var("COGNITIVE_MCP_QDRANT_URL"))
            .unwrap_or_else(|_| DEFAULT_QDRANT_URL.to_string());

        let voyage_api_key = std::env::var("COGNITIVE_MCP_VOYAGE_API_KEY")
            .or_else(|_| std::env::var("VOYAGE_API_KEY"))
            .or_else(|_| std::env::var("VOYAGE_API_KEY_RUST"))
            .or_else(|_| voyage_key_from_file())
            .context("missing Voyage API key for gallery-gen Qdrant integration")?;

        let voyage_api_url = std::env::var("COGNITIVE_MCP_VOYAGE_API_URL")
            .unwrap_or_else(|_| DEFAULT_VOYAGE_API_URL.to_string());

        let voyage_model = std::env::var("COGNITIVE_MCP_VOYAGE_QUERY_MODEL")
            .unwrap_or_else(|_| DEFAULT_VOYAGE_QUERY_MODEL.to_string());

        Ok(Self {
            http_client: Client::new(),
            qdrant_url,
            voyage_api_url,
            voyage_api_key,
            voyage_model,
        })
    }

    /// Ensure the gallery-gen-schemas collection exists.
    pub async fn ensure_collection(&self) -> Result<()> {
        let url = format!("{}/collections/{}", self.qdrant_url, COLLECTION_NAME);

        // Check if collection exists
        let resp = self.http_client.get(&url).send().await;
        if let Ok(r) = resp {
            if r.status().is_success() {
                return Ok(()); // already exists
            }
        }

        // Create collection
        let body = serde_json::json!({
            "vectors": {
                "size": VECTOR_DIMENSION,
                "distance": "Cosine"
            }
        });

        self.http_client
            .put(&url)
            .json(&body)
            .send()
            .await
            .context("failed to create gallery-gen-schemas collection")?
            .error_for_status()
            .context("Qdrant returned error creating collection")?;

        tracing::info!("Created Qdrant collection '{}'", COLLECTION_NAME);
        Ok(())
    }

    /// Refresh the collection from the live sealed catalog.
    ///
    /// Chunks each plugin schema into fragments, embeds them, and upserts
    /// to the gallery-gen-schemas collection with domain tags.
    pub async fn refresh_from_catalog(
        &self,
        schemas: &[crate::context::SchemaPayload],
    ) -> Result<usize> {
        self.ensure_collection().await?;

        let fragments = chunk_schemas(schemas);
        let mut points = Vec::with_capacity(fragments.len());

        for (idx, fragment) in fragments.iter().enumerate() {
            let vector = self.embed_document(&fragment.fragment_text).await?;

            let point = serde_json::json!({
                "id": idx,
                "vector": vector,
                "payload": {
                    "plugin_id": fragment.plugin_id,
                    "fragment_type": fragment.fragment_type,
                    "text": fragment.fragment_text,
                    "domain_tag": fragment.domain_tag,
                }
            });

            points.push(point);
        }

        // Batch upsert (Qdrant REST API)
        let url = format!("{}/collections/{}/points", self.qdrant_url, COLLECTION_NAME);
        let body = serde_json::json!({ "points": points });

        self.http_client
            .put(&url)
            .json(&body)
            .send()
            .await
            .context("failed to upsert points to gallery-gen-schemas")?
            .error_for_status()
            .context("Qdrant returned error on upsert")?;

        tracing::info!(
            "Refreshed gallery-gen-schemas: {} fragments from {} plugins",
            fragments.len(),
            schemas.len()
        );

        Ok(fragments.len())
    }

    /// Semantic search over the gallery-gen-schemas collection.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        domain_filter: Option<&str>,
    ) -> Result<Vec<SemanticSearchResult>> {
        let vector = self.embed_query(query).await?;

        let mut search_body = serde_json::json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true,
        });

        // Add domain filter if specified
        if let Some(domain) = domain_filter {
            search_body["filter"] = serde_json::json!({
                "must": [{
                    "key": "domain_tag",
                    "match": { "value": domain }
                }]
            });
        }

        let url = format!(
            "{}/collections/{}/points/search",
            self.qdrant_url, COLLECTION_NAME
        );

        let resp = self
            .http_client
            .post(&url)
            .json(&search_body)
            .send()
            .await
            .context("failed to search gallery-gen-schemas")?
            .error_for_status()
            .context("Qdrant search returned error")?;

        let resp_json: Value = resp.json().await.context("failed to parse search response")?;

        let results = resp_json
            .get("result")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|point| {
                        let payload = point.get("payload")?;
                        Some(SemanticSearchResult {
                            plugin_id: payload
                                .get("plugin_id")?
                                .as_str()?
                                .to_string(),
                            fragment: payload.get("text")?.as_str()?.to_string(),
                            domain_tag: payload
                                .get("domain_tag")?
                                .as_str()?
                                .to_string(),
                            score: point.get("score")?.as_f64()? as f32,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// Embed text as a query vector.
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text, "query").await
    }

    /// Embed text as a document vector.
    async fn embed_document(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text, "document").await
    }

    async fn embed(&self, input: &str, input_type: &str) -> Result<Vec<f32>> {
        let body = serde_json::json!({
            "input": input,
            "model": self.voyage_model,
            "input_type": input_type,
            "truncation": true,
            "output_dimension": VECTOR_DIMENSION,
            "output_dtype": "float",
        });

        let response = self
            .http_client
            .post(&self.voyage_api_url)
            .bearer_auth(&self.voyage_api_key)
            .json(&body)
            .send()
            .await
            .context("failed to call Voyage embeddings API")?
            .error_for_status()
            .context("Voyage API returned error")?;

        let resp_json: Value = response.json().await.context("failed to parse embedding response")?;

        resp_json
            .get("data")
            .and_then(|d| d.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("embedding"))
            .and_then(|e| e.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .context("failed to extract embedding vector from Voyage response")
    }
}

/// Chunk plugin schemas into fragments for vectorization.
fn chunk_schemas(schemas: &[crate::context::SchemaPayload]) -> Vec<SchemaFragment> {
    let mut fragments = Vec::new();

    for schema in schemas {
        let domain_tag = DomainTag::from_category(
            schema.category.as_deref().unwrap_or("uncategorized"),
        )
        .as_str()
        .to_string();

        // Summary fragment
        let summary = format!(
            "Plugin '{}' ({}): {}. Fields: {}. Methods: {}.",
            schema.name,
            schema.category.as_deref().unwrap_or("uncategorized"),
            schema.description.as_deref().unwrap_or("no description"),
            schema.fields.len(),
            schema.methods.len(),
        );
        fragments.push(SchemaFragment {
            plugin_id: schema.name.clone(),
            fragment_type: "summary".to_string(),
            fragment_text: summary,
            domain_tag: domain_tag.clone(),
        });

        // Field fragments
        for (field_name, field) in &schema.fields {
            let text = format!(
                "Plugin '{}' field '{}': type={:?}, required={}, read_only={}, description={}",
                schema.name,
                field_name,
                field.field_type,
                field.required,
                field.read_only,
                field.description.as_deref().unwrap_or(""),
            );
            fragments.push(SchemaFragment {
                plugin_id: schema.name.clone(),
                fragment_type: "field".to_string(),
                fragment_text: text,
                domain_tag: domain_tag.clone(),
            });
        }

        // Method fragments
        for (method_name, method) in &schema.methods {
            let text = format!(
                "Plugin '{}' method '{}': side_effect={:?}, idempotent={}, capability={}, description={}",
                schema.name,
                method_name,
                method.side_effect,
                method.idempotent,
                method.required_capability.as_deref().unwrap_or("none"),
                method.description.as_deref().unwrap_or(""),
            );
            fragments.push(SchemaFragment {
                plugin_id: schema.name.clone(),
                fragment_type: "method".to_string(),
                fragment_text: text,
                domain_tag: domain_tag.clone(),
            });
        }
    }

    fragments
}

/// Read Voyage API key from file (same locations as op-cognitive-mcp).
fn voyage_key_from_file() -> Result<String, std::env::VarError> {
    let path = std::env::var("COGNITIVE_MCP_VOYAGE_KEY_FILE")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| std::path::Path::new(&home).join(".ssh/mongo-voyage"))
        })
        .ok_or(std::env::VarError::NotPresent)?;

    let contents =
        std::fs::read_to_string(&path).map_err(|_| std::env::VarError::NotPresent)?;

    contents
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("mdb_sa_id_")
                && !line.starts_with("mdb_sa_sk_")
                && (line.starts_with("al-") || line.starts_with("pa-"))
        })
        .map(|s| s.to_string())
        .ok_or(std::env::VarError::NotPresent)
}
