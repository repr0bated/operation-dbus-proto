//! OpenAI-compatible HTTP server mode.
//!
//! POST /v1/chat/completions → Vertex AI via gRPC (StreamGenerateContent / GenerateContent)
//! GET  /v1/models            → list of Gemini models
//!
//! Activated by setting HTTP_SERVER_ADDR (e.g. "127.0.0.1:11435").
//! Set VERTEX_PROJECT=<gcp-project> to route to Vertex AI.

use crate::direct_llm::DirectLLM;
use crate::vertex_grpc::VertexGrpcClient;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{sse::Sse, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Token-bucket rate limiter — refills `capacity` tokens per minute.
struct TokenBucket {
    tokens: f64,
    capacity: f64,      // = rpm limit
    last_refill: Instant,
}

impl TokenBucket {
    fn new(rpm: u32) -> Self {
        let cap = rpm as f64;
        Self { tokens: cap, capacity: cap, last_refill: Instant::now() }
    }

    /// Try to consume one token. Returns how long to wait if empty.
    fn try_consume(&mut self) -> Result<(), std::time::Duration> {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.capacity / 60.0).min(self.capacity);
        self.last_refill = Instant::now();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            let wait_secs = (1.0 - self.tokens) * 60.0 / self.capacity;
            Err(std::time::Duration::from_secs_f64(wait_secs))
        }
    }
}

pub struct AppState {
    pub llm: Option<Arc<DirectLLM>>,
    pub vertex: Option<Arc<VertexGrpcClient>>,
    pub rate_limiter: Arc<Mutex<TokenBucket>>,
}

// ── OpenAI request/response types ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Serialize)]
struct Choice {
    index: u32,
    message: ChatMessage,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize)]
struct ModelObject {
    id: String,
    object: &'static str,
    created: i64,
    owned_by: &'static str,
}

#[derive(Serialize)]
struct ModelList {
    object: &'static str,
    data: Vec<ModelObject>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn list_models() -> Json<ModelList> {
    let models = [
        "gemini-3.5-flash",
        "gemini-2.5-pro",
        "gemini-2.5-flash",
        "gemini-2.5-flash-lite",
        "gemini-2.0-flash-001",
        "gemini-2.0-flash-lite",
    ];
    Json(ModelList {
        object: "list",
        data: models
            .iter()
            .map(|id| ModelObject {
                id: id.to_string(),
                object: "model",
                created: 1700000000,
                owned_by: "google",
            })
            .collect(),
    })
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    // Rate limit before touching Vertex AI.
    {
        let wait = state.rate_limiter.lock().await.try_consume().err();
        if let Some(delay) = wait {
            if delay.as_secs() > 5 {
                // Backlog too deep — reject rather than queue indefinitely.
                warn!(wait_ms = delay.as_millis(), "rate limit: rejecting request");
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({ "error": { "message": "rate limit exceeded", "type": "rate_limit_error" } })),
                ).into_response();
            }
            warn!(wait_ms = delay.as_millis(), "rate limit: throttling request");
            tokio::time::sleep(delay).await;
        }
    }

    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();
    let messages: Vec<serde_json::Value> = req
        .messages
        .iter()
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();

    // Vertex AI gRPC path.
    if let Some(ref vertex) = state.vertex {
        info!(model = %req.model, msgs = req.messages.len(), stream = req.stream, "chat request");
        if req.stream {
            match vertex
                .stream_generate(&req.model, &messages, req.max_tokens, id, created)
                .await
            {
                Ok(sse_stream) => {
                    return Sse::new(sse_stream)
                        .keep_alive(axum::response::sse::KeepAlive::default())
                        .into_response();
                }
                Err(e) => {
                    warn!("Vertex AI stream error: {}", e);
                    let body = serde_json::json!({ "error": { "message": e.to_string() } });
                    return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
                }
            }
        } else {
            match vertex
                .generate(&req.model, &messages, req.max_tokens)
                .await
            {
                Ok(text) => return ok_response(text, req.model, id, created),
                Err(e) => {
                    warn!("Vertex AI error: {}", e);
                    let body = serde_json::json!({ "error": { "message": e.to_string() } });
                    return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
                }
            }
        }
    }

    // CloudAI companion fallback.
    if let Some(ref llm) = state.llm {
        let mcp_req = simd_json::json!({
            "jsonrpc": "2.0",
            "id": "http-1",
            "method": "sampling/createMessage",
            "params": {
                "model": req.model,
                "messages": messages_to_simd(&messages),
            }
        });

        let llm_resp = llm.handle(&mcp_req).await;

        if let Some(err) = llm_resp.get("error") {
            let msg = err.get("message").and_then(|v| v.as_str()).unwrap_or("llm error");
            let code = err.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
            let body = serde_json::json!({ "error": { "message": msg, "code": code } });
            return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
        }

        let text = llm_resp["result"]["completion"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if req.stream {
            return single_chunk_sse(text, req.model, id, created);
        }
        return ok_response(text, req.model, id, created);
    }

    (StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": "no LLM backend configured" }))).into_response()
}

fn ok_response(text: String, model: String, id: String, created: i64) -> Response {
    let word_count = text.split_whitespace().count() as u32;
    let response = ChatCompletionResponse {
        id,
        object: "chat.completion",
        created,
        model,
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: serde_json::Value::String(text),
            },
            finish_reason: "stop",
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: word_count,
            total_tokens: word_count,
        },
    };
    Json(response).into_response()
}

// Fake SSE (single chunk) for the CloudAI fallback path which isn't natively streaming.
fn single_chunk_sse(text: String, model: String, id: String, created: i64) -> Response {
    let chunk = serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{ "index": 0, "delta": { "role": "assistant", "content": text }, "finish_reason": "stop" }]
    });
    let body = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        serde_json::to_string(&chunk).unwrap_or_default()
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn messages_to_simd(messages: &[serde_json::Value]) -> OwnedValue {
    let mut arr = Vec::new();
    for m in messages {
        let role = m["role"].as_str().unwrap_or("user").to_string();
        let content_owned;
        let content = if let Some(s) = m["content"].as_str() {
            s
        } else {
            content_owned = m["content"].to_string();
            content_owned.trim_matches('"')
        };
        arr.push(simd_json::json!({
            "role": role,
            "content": content,
        }));
    }
    OwnedValue::Array(arr)
}

// ── Request logging middleware ─────────────────────────────────────────────────

async fn log_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let resp = next.run(req).await;
    let status = resp.status();
    if status.is_success() {
        info!(method = %method, path = %uri.path(), status = %status.as_u16(), "http");
    } else {
        warn!(method = %method, path = %uri.path(), status = %status.as_u16(), "http");
    }
    resp
}

// ── Server entry point ────────────────────────────────────────────────────────

pub async fn run(llm: Option<Arc<DirectLLM>>, addr: &str) -> anyhow::Result<()> {
    let vertex = if let Ok(project) = std::env::var("VERTEX_PROJECT").ok().filter(|v| !v.is_empty()).ok_or(()) {
        let region = std::env::var("VERTEX_REGION").unwrap_or_else(|_| "us-central1".to_string());
        info!(project = %project, region = %region, "Using Vertex AI gRPC backend");
        match VertexGrpcClient::new(project, region).await {
            Ok(c) => Some(c),
            Err(e) => {
                warn!("Vertex AI gRPC init failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    if vertex.is_none() && llm.is_some() {
        info!("Using CloudAI companion backend");
    } else if vertex.is_none() {
        warn!("No LLM backend configured");
    }

    let rpm: u32 = std::env::var("VERTEX_RATE_LIMIT_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    info!(rpm, "Vertex AI rate limit");

    let state = Arc::new(AppState {
        llm,
        vertex,
        rate_limiter: Arc::new(Mutex::new(TokenBucket::new(rpm))),
    });
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        .with_state(state)
        .layer(middleware::from_fn(log_request));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
