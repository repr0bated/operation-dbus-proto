//! Qdrant vector search tools
//!
//! Provides two MCP tools:
//!   - `qdrant_search` — embed a query then search Qdrant
//!   - `qdrant_upsert` — store a pre-computed vector + payload (used by embedding_worker)
//!
//! Collection management (create-if-absent) happens at registration time and on
//! first use, using dimensions from the embedding provider.

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::{Context, Result};
use async_trait::async_trait;
use op_llm::provider::{EmbeddingIntent, EmbeddingProvider};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, SearchPointsBuilder,
    UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::{Payload, Qdrant};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Default collection for ctl-plane reasoning episodes (per spec: 1024 dims).
const DEFAULT_COLLECTION: &str = "ctl_plane_reasoning_episodes";

pub struct QdrantSearchTool {
    client: Arc<Qdrant>,
    embedder: Arc<dyn EmbeddingProvider>,
    default_collection: String,
}

impl QdrantSearchTool {
    pub fn new(
        client: Arc<Qdrant>,
        embedder: Arc<dyn EmbeddingProvider>,
        default_collection: String,
    ) -> Self {
        Self { client, embedder, default_collection }
    }
}

#[async_trait]
impl Tool for QdrantSearchTool {
    fn name(&self) -> &str { "qdrant_search" }
    fn description(&self) -> &str { "Search the vector database for semantically similar reasoning episodes or knowledge fragments" }
    fn category(&self) -> &str { "vector" }
    fn tags(&self) -> Vec<String> { vec!["qdrant".into(), "vector".into(), "search".into()] }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Natural language query to embed and search"},
                "collection": {"type": "string", "description": "Qdrant collection name (default: ctl_plane_reasoning_episodes)"},
                "limit": {"type": "integer", "description": "Maximum results to return (default: 10)"},
                "score_threshold": {"type": "number", "description": "Minimum cosine similarity score (0.0–1.0, optional)"},
                "filter": {"type": "object", "description": "Qdrant filter expression (optional)"}
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let query = input.get("query")
            .and_then(|v| v.as_str())
            .context("'query' field is required")?
            .to_string();

        let collection = input.get("collection")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_collection)
            .to_string();

        let limit = input.get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u64;

        let score_threshold = input.get("score_threshold")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);

        debug!("qdrant_search: collection={}, limit={}", collection, limit);

        // Embed the query
        let embedding = self.embedder
            .embed(query.clone(), EmbeddingIntent::Query)
            .await
            .context("Failed to embed query")?;

        ensure_collection_exists(&self.client, &collection, embedding.vector.len()).await?;

        // Build search request
        let mut search = SearchPointsBuilder::new(&collection, embedding.vector, limit)
            .with_payload(true);

        if let Some(threshold) = score_threshold {
            search = search.score_threshold(threshold);
        }

        let results = self.client
            .search_points(search)
            .await
            .context("Qdrant search failed")?;

        let hits: Vec<Value> = results.result.into_iter().map(|hit| {
            let id_str = hit.id
                .and_then(|id| id.point_id_options)
                .map(|opt| match opt {
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(n) => n.to_string(),
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(s) => s,
                })
                .unwrap_or_default();

            let mut payload_obj = simd_json::value::owned::Object::new();
            for (k, v) in hit.payload {
                payload_obj.insert(k, qdrant_value_to_json(v));
            }

            json!({
                "id": id_str,
                "score": hit.score,
                "payload": Value::from(payload_obj)
            })
        }).collect();

        info!("qdrant_search: {} results for query '{}'", hits.len(), &query[..query.len().min(50)]);

        Ok(json!({
            "success": true,
            "collection": collection,
            "query": query,
            "results": hits
        }))
    }
}

pub struct QdrantUpsertTool {
    client: Arc<Qdrant>,
    default_collection: String,
}

impl QdrantUpsertTool {
    pub fn new(client: Arc<Qdrant>, default_collection: String) -> Self {
        Self { client, default_collection }
    }
}

#[async_trait]
impl Tool for QdrantUpsertTool {
    fn name(&self) -> &str { "qdrant_upsert" }
    fn description(&self) -> &str { "Store a pre-computed embedding vector with payload into the vector database (used by the embedding worker)" }
    fn category(&self) -> &str { "vector" }
    fn tags(&self) -> Vec<String> { vec!["qdrant".into(), "vector".into(), "upsert".into()] }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "Point ID (UUID or integer string)"},
                "vector": {
                    "type": "array",
                    "items": {"type": "number"},
                    "description": "Pre-computed embedding vector"
                },
                "payload": {"type": "object", "description": "Filterable metadata (outcome_class, plugin_id, conversation_id, etc.)"},
                "collection": {"type": "string", "description": "Qdrant collection name"}
            },
            "required": ["id", "vector", "payload"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let id_str = input.get("id")
            .and_then(|v| v.as_str())
            .context("'id' field is required")?
            .to_string();

        let vector: Vec<f32> = input.get("vector")
            .and_then(|v| v.as_array())
            .context("'vector' field is required")?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        if vector.is_empty() {
            anyhow::bail!("'vector' must be a non-empty array of numbers");
        }

        let payload_val = input.get("payload")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let collection = input.get("collection")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_collection)
            .to_string();

        ensure_collection_exists(&self.client, &collection, vector.len()).await?;

        // Convert payload JSON → Qdrant Payload
        let qdrant_payload = json_to_qdrant_payload(&payload_val);

        // Parse point ID — integer if it parses, otherwise string (UUID)
        let point: PointStruct = if let Ok(n) = id_str.parse::<u64>() {
            PointStruct::new(n, vector, qdrant_payload)
        } else {
            PointStruct::new(id_str.clone(), vector, qdrant_payload)
        };

        self.client
            .upsert_points(UpsertPointsBuilder::new(&collection, vec![point]))
            .await
            .context("Qdrant upsert failed")?;

        info!("qdrant_upsert: stored point {} in {}", id_str, collection);

        Ok(json!({
            "success": true,
            "collection": collection,
            "id": id_str
        }))
    }
}

/// Ensure a collection exists with the correct dimensions.
/// No-op if it already exists.
pub async fn ensure_collection_exists(
    client: &Qdrant,
    collection: &str,
    dims: usize,
) -> Result<()> {
    match client.collection_exists(collection).await {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => {
            warn!("Could not check collection existence for '{}': {}", collection, e);
        }
    }

    info!("Creating Qdrant collection '{}' with {} dims (Cosine)", collection, dims);
    client
        .create_collection(
            CreateCollectionBuilder::new(collection)
                .vectors_config(VectorParamsBuilder::new(dims as u64, Distance::Cosine)),
        )
        .await
        .with_context(|| format!("Failed to create Qdrant collection '{}'", collection))?;

    Ok(())
}

/// Convert a simd_json Value payload object to Qdrant Payload.
fn json_to_qdrant_payload(val: &Value) -> Payload {
    let mut map = std::collections::HashMap::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj.iter() {
            if let Some(qv) = simd_value_to_qdrant(v) {
                map.insert(k.clone(), qv);
            }
        }
    }
    Payload::from(map)
}

fn simd_value_to_qdrant(v: &Value) -> Option<qdrant_client::qdrant::Value> {
    use qdrant_client::qdrant::value::Kind;
    use qdrant_client::qdrant::Value as QValue;
    use simd_json::prelude::TypedValue;

    let kind = if v.is_null() {
        Kind::NullValue(0)
    } else if let Some(b) = v.as_bool() {
        Kind::BoolValue(b)
    } else if let Some(n) = v.as_i64() {
        Kind::IntegerValue(n)
    } else if let Some(n) = v.as_u64() {
        Kind::IntegerValue(n as i64)
    } else if let Some(f) = v.as_f64() {
        Kind::DoubleValue(f)
    } else if let Some(s) = v.as_str() {
        Kind::StringValue(s.to_string())
    } else {
        return None;
    };
    Some(QValue { kind: Some(kind) })
}

fn qdrant_value_to_json(v: qdrant_client::qdrant::Value) -> Value {
    use qdrant_client::qdrant::value::Kind;
    match v.kind {
        Some(Kind::NullValue(_)) => Value::Static(simd_json::StaticNode::Null),
        Some(Kind::BoolValue(b)) => Value::Static(simd_json::StaticNode::Bool(b)),
        Some(Kind::IntegerValue(n)) => Value::Static(simd_json::StaticNode::I64(n)),
        Some(Kind::DoubleValue(f)) => Value::Static(simd_json::StaticNode::F64(f)),
        Some(Kind::StringValue(s)) => Value::String(s.into()),
        _ => Value::Static(simd_json::StaticNode::Null),
    }
}

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    // Qdrant Rust client uses gRPC (port 6334), not REST (6333)
    let qdrant_url = std::env::var("QDRANT_URL")
        .unwrap_or_else(|_| "http://10.149.181.190:6334".to_string());

    let client = Qdrant::from_url(&qdrant_url)
        .build()
        .with_context(|| format!("Failed to create Qdrant client for {}", qdrant_url))?;
    let client = Arc::new(client);

    // Build embedding provider from env — falls back gracefully if OpenClaw is unavailable
    let embedder: Arc<dyn EmbeddingProvider> = match op_llm::openclaw::OpenClawProvider::from_env() {
        Ok(provider) => {
            info!("Qdrant tools: using OpenClaw embedding provider");
            Arc::new(provider)
        }
        Err(e) => {
            warn!("Qdrant tools: OpenClaw unavailable ({}), search will fail without embedder", e);
            // Register tools anyway — they will error at call time if no embedder
            let collection = std::env::var("QDRANT_COLLECTION")
                .unwrap_or_else(|_| DEFAULT_COLLECTION.to_string());
            registry.register(Arc::new(QdrantUpsertTool::new(client.clone(), collection))).await?;
            return Ok(1);
        }
    };

    let collection = std::env::var("QDRANT_COLLECTION")
        .unwrap_or_else(|_| DEFAULT_COLLECTION.to_string());

    registry.register(Arc::new(QdrantSearchTool::new(
        client.clone(),
        embedder,
        collection.clone(),
    ))).await?;

    registry.register(Arc::new(QdrantUpsertTool::new(
        client,
        collection,
    ))).await?;

    Ok(2)
}
