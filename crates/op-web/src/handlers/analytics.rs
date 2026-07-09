//! Analytics & Accountability Handlers
//!
//! Semantic search over Qdrant accountability traces via QdrantSemanticShuttle.

use axum::{
    extract::{Extension, Query},
    response::Json,
};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use tracing::{info, warn};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SemanticSearchParams {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

#[derive(Debug, Serialize)]
pub struct SemanticSearchResult {
    trace_id: String,
    timestamp: String,
    schema_name: String,
    content: String,
    relevance_score: f32,
    payload: Value,
}

#[derive(Debug, Serialize)]
pub struct SemanticSearchResponse {
    results: Vec<SemanticSearchResult>,
    query: String,
    total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn payload_str(
    payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>,
    key: &str,
) -> String {
    payload
        .get(key)
        .and_then(|v| v.kind.as_ref())
        .and_then(|k| match k {
            qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn payload_to_simd(
    payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>,
) -> Value {
    let Ok(json) = serde_json::to_value(payload) else {
        return Value::Object(Default::default());
    };
    let Ok(bytes) = serde_json::to_vec(&json) else {
        return Value::Object(Default::default());
    };
    let mut bytes = bytes;
    simd_json::to_owned_value(&mut bytes).unwrap_or_else(|_| Value::Object(Default::default()))
}

/// GET /api/analytics/semantic-search
///
/// Semantic search over accountability traces for the active Ghostbridge session.
/// Embeds the active schema via Voyage, queries Qdrant `ctl_plane_reasoning_episodes`
/// filtered by the sled trace_id.
pub async fn semantic_search_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Query(params): Query<SemanticSearchParams>,
) -> Json<SemanticSearchResponse> {
    info!(query = %params.query, limit = params.limit, "Semantic search request");

    let shuttle = match op_cognitive_mcp::QdrantSemanticShuttle::new().await {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "Qdrant Semantic Shuttle unavailable");
            return Json(SemanticSearchResponse {
                results: vec![],
                query: params.query,
                total: 0,
                error: Some(format!(
                    "Qdrant Semantic Shuttle unavailable: {e}. Check Voyage and Qdrant settings."
                )),
            });
        }
    };

    let trace = match shuttle.current_trace_context() {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "No active identity sled for semantic search");
            return Json(SemanticSearchResponse {
                results: vec![],
                query: params.query,
                total: 0,
                error: Some(format!("No active Ghostbridge session: {e}")),
            });
        }
    };

    let limit = params.limit as u64;
    match shuttle.search_semantic_trace(limit).await {
        Ok(points) => {
            let results: Vec<SemanticSearchResult> = points
                .into_iter()
                .map(|pt| SemanticSearchResult {
                    trace_id: trace.trace_id.clone(),
                    timestamp: payload_str(&pt.payload, "timestamp"),
                    schema_name: payload_str(&pt.payload, "schema_name"),
                    content: {
                        let c = payload_str(&pt.payload, "content");
                        if c.is_empty() {
                            payload_str(&pt.payload, "text")
                        } else {
                            c
                        }
                    },
                    relevance_score: pt.score,
                    payload: payload_to_simd(&pt.payload),
                })
                .collect();
            let total = results.len();
            Json(SemanticSearchResponse {
                results,
                query: params.query,
                total,
                error: None,
            })
        }
        Err(e) => {
            warn!(error = %e, "Semantic search query failed");
            Json(SemanticSearchResponse {
                results: vec![],
                query: params.query,
                total: 0,
                error: Some(e.to_string()),
            })
        }
    }
}
