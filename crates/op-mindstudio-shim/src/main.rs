//! Loopback OpenAI-compatible shim for the MindStudio Model Router.
//!
//! MindStudio's only external surface is `POST /developer/v2/apps/run`; this
//! shim exposes `POST /v1/chat/completions` (plus `GET /v1/models`) on
//! loopback so OpenAI-compatible clients (zeroclaw custom provider) can use
//! the MindStudio-hosted routing/UI-generation agent as an ordinary model.

use std::sync::Arc;

use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};

const RUN_URL: &str = "https://api.mindstudio.ai/developer/v2/apps/run";
const MODEL_ID: &str = "mindstudio/routing-uigen";

struct Shim {
    http: reqwest::Client,
    api_key: String,
    app_id: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    #[serde(default)]
    content: Value,
}

/// Message content may be a plain string or OpenAI content-part arrays.
fn content_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Flatten the chat transcript into a single launch-variable prompt; the
/// MindStudio workflow is a stateless pipe with no thread reuse.
fn flatten_prompt(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for m in messages {
        let text = content_text(&m.content);
        if text.is_empty() {
            continue;
        }
        match m.role.as_str() {
            "system" => {
                out.push_str("Instructions:\n");
                out.push_str(&text);
                out.push_str("\n\n");
            }
            "assistant" => {
                out.push_str("Previous assistant reply:\n");
                out.push_str(&text);
                out.push_str("\n\n");
            }
            _ => {
                out.push_str(&text);
                out.push_str("\n\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// Strip a leading `<think>...</think>` block (reasoning-variant leakage).
fn strip_think(result: &str) -> &str {
    let trimmed = result.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<think>") {
        if let Some(end) = rest.find("</think>") {
            return rest[end + "</think>".len()..].trim_start();
        }
    }
    result
}

async fn chat_completions(State(shim): State<Arc<Shim>>, Json(req): Json<ChatRequest>) -> Response {
    let prompt = flatten_prompt(&req.messages);
    if prompt.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "no message content");
    }

    let body = json!({
        "appId": shim.app_id,
        "variables": { "prompt": prompt },
    });
    let resp = match shim
        .http
        .post(RUN_URL)
        .bearer_auth(&shim.api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("mindstudio request failed: {e}"),
            )
        }
    };
    let status = resp.status();
    let payload: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("mindstudio returned non-JSON ({status}): {e}"),
            )
        }
    };
    let Some(result) = payload.get("result") else {
        tracing::warn!(%status, %payload, "mindstudio run without result");
        return error_response(
            StatusCode::BAD_GATEWAY,
            &format!("mindstudio run returned no result (status {status})"),
        );
    };
    // JSON-render apps return a structured `{root,elements,state}` object,
    // while older MindStudio workflows return a text string. Preserve the
    // OpenAI-compatible string content envelope for both forms.
    let text = match result {
        Value::String(text) => strip_think(text).to_string(),
        structured => match serde_json::to_string(structured) {
            Ok(text) => text,
            Err(e) => {
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    &format!("mindstudio result was not serializable: {e}"),
                )
            }
        },
    };

    Json(json!({
        "id": payload
            .get("threadId")
            .and_then(Value::as_str)
            .unwrap_or("mindstudio-run"),
        "object": "chat.completion",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "model": MODEL_ID,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": text },
            "finish_reason": "stop",
        }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
    }))
    .into_response()
}

async fn models() -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [{ "id": MODEL_ID, "object": "model", "owned_by": "mindstudio" }],
    }))
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "message": message, "type": "shim_error" } })),
    )
        .into_response()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let api_key = std::env::var("MIND_STUDIO_API_KEY")
        .map_err(|_| anyhow::anyhow!("MIND_STUDIO_API_KEY is not set"))?;
    let app_id = std::env::var("MINDSTUDIO_APP_ID")
        .unwrap_or_else(|_| "83dc1fff-d16e-4a7f-80b9-fd68f6117346".to_string());
    let listen =
        std::env::var("MINDSTUDIO_SHIM_LISTEN").unwrap_or_else(|_| "127.0.0.1:8093".to_string());

    let shim = Arc::new(Shim {
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()?,
        api_key,
        app_id,
    });

    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(models))
        .with_state(shim);

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    tracing::info!(%listen, "op-mindstudio-shim listening");
    axum::serve(listener, app).await?;
    Ok(())
}
