//! Zeroclaw / Antigravity proxy handlers
//!
//! Proxies chat requests to the antigravity Python SDK bot running on
//! 127.0.0.1:8081, and serves the combined schema (OpenAPI + zeroclaw
//! projection) for schema-driven UI rendering.

use axum::{body::Body, extract::Extension, http::StatusCode, response::Response, Json};
use serde::Deserialize;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::{error, info};

use crate::state::AppState;

/// Antigravity bridge base URL - defaults to standard Antigravity IDE bridge port
/// Can be overridden with ANTIGRAVITY_BRIDGE_URL env var
fn antigravity_base() -> String {
    std::env::var("ANTIGRAVITY_BRIDGE_URL").unwrap_or_else(|_| "http://127.0.0.1:3333".to_string())
}

/// Get WireGuard identity for auth headers
fn wireguard_identity_headers() -> Vec<(&'static str, String)> {
    let mut headers = Vec::new();

    // X-WireGuard-Pubkey from environment (set by the host WireGuard
    // interface; the deprecated wg-xray container is no longer involved)
    if let Ok(pubkey) = std::env::var("WG_PUBKEY") {
        headers.push(("X-WireGuard-Pubkey", pubkey));
    }

    // X-Ghostbridge-Trace-ID for accountability loop
    if let Ok(trace_id) = std::env::var("GHOSTBRIDGE_TRACE_ID") {
        headers.push(("X-Ghostbridge-Trace-ID", trace_id));
    }

    // Host-scoped identity (was container-scoped for the deprecated wg-xray
    // container; xray + gRPC-bridge now run on the host)
    if let Ok(container_id) = std::env::var("CONTAINER_ID") {
        headers.push(("X-Container-ID", container_id));
    }

    headers
}

/// Schema-driven chat request — shape matches the antigravity bot's
/// `ChatRequest` so we can forward verbatim.
#[derive(Debug, Deserialize)]
pub struct ZeroclawChatRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Proxy chat request to the antigravity bot.
/// POST /api/zeroclaw/chat
pub async fn zeroclaw_chat_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Json(req): Json<ZeroclawChatRequest>,
) -> Response {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to build HTTP client: {}", e);
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build HTTP client",
            );
        }
    };

    let url = format!("{}/api/chat", antigravity_base());

    let body = json!({
        "message": req.message,
        "session_id": req.session_id,
        "provider": req.provider,
        "model": req.model,
    });

    info!(
        "Proxying zeroclaw chat to antigravity bot: {} chars",
        req.message.len()
    );

    let mut request_builder = client.post(&url).header("Content-Type", "application/json");

    // Add WireGuard identity headers for auth
    for (key, value) in wireguard_identity_headers() {
        request_builder = request_builder.header(key, value);
    }

    match request_builder
        .body(simd_json::to_string(&body).unwrap_or_default())
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body_bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    error!("Failed to read antigravity response body: {}", e);
                    return json_error_response(
                        StatusCode::BAD_GATEWAY,
                        "Failed to read antigravity response",
                    );
                }
            };

            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            Response::builder()
                .status(axum_status)
                .header("Content-Type", "application/json")
                .body(Body::from(body_bytes))
                .unwrap_or_else(|_| {
                    json_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to build response",
                    )
                })
        }
        Err(e) => {
            error!("Antigravity bot unreachable: {}", e);
            json_error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Antigravity bot unreachable: {}", e),
            )
        }
    }
}

/// Proxy streaming chat request to the antigravity bot SSE endpoint.
/// POST /api/zeroclaw/chat/stream
pub async fn zeroclaw_chat_stream_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Json(req): Json<ZeroclawChatRequest>,
) -> Response {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to build HTTP client: {}", e);
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build HTTP client",
            );
        }
    };

    let url = format!("{}/api/chat/stream", antigravity_base());

    let body = json!({
        "message": req.message,
        "session_id": req.session_id,
        "provider": req.provider,
        "model": req.model,
    });

    info!(
        "Proxying zeroclaw chat stream to antigravity bot: {} chars",
        req.message.len()
    );

    let mut request_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

    // Add WireGuard identity headers for auth
    for (key, value) in wireguard_identity_headers() {
        request_builder = request_builder.header(key, value);
    }

    match request_builder
        .body(simd_json::to_string(&body).unwrap_or_default())
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body_bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    error!("Failed to read antigravity stream body: {}", e);
                    return json_error_response(
                        StatusCode::BAD_GATEWAY,
                        "Failed to read antigravity stream",
                    );
                }
            };

            let axum_status =
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            Response::builder()
                .status(axum_status)
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .body(Body::from(body_bytes))
                .unwrap_or_else(|_| {
                    json_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to build stream response",
                    )
                })
        }
        Err(e) => {
            error!("Antigravity bot unreachable: {}", e);
            json_error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Antigravity bot unreachable: {}", e),
            )
        }
    }
}

/// Helper to build a JSON error response.
fn json_error_response(status: StatusCode, message: &str) -> Response {
    let body = simd_json::to_string(&json!({"error": message})).unwrap_or_default();
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .expect("empty body should always succeed")
        })
}

/// Read zeroclaw schema directly from shared memory (Absolute Base)
/// Per AGENTS.md: "The Absolute Base: PluginSchema is the single source of truth"
fn read_zeroclaw_from_shm() -> Option<Value> {
    use std::fs;

    const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

    let content = fs::read_to_string(SHM_SCHEMA_PATH).ok()?;
    let mut bytes = content.into_bytes();
    let schema: Value = simd_json::to_owned_value(&mut bytes).ok()?;

    // Extract zeroclaw from the schema examples (the canonical source)
    let zeroclaw_schema = schema.get("zeroclaw")?.as_array()?.first()?;
    let fields = zeroclaw_schema.get("fields")?.as_array()?;

    // Helper: find field example by name
    fn find_example(fields: &[Value], name: &str) -> Option<Value> {
        for f in fields {
            let field_name = f.get("name")?.as_str()?;
            if field_name == name {
                return f.get("example").cloned();
            }
        }
        None
    }

    Some(json!({
        "active": true,
        "status": find_example(fields, "status").and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| "declared".to_string()),
        "selected_provider": find_example(fields, "selected_provider").and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| "antigravity".to_string()),
        "selected_model": find_example(fields, "selected_model").and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| "gemini-2.5-flash".to_string()),
        "providers": find_example(fields, "providers")?,
        "model_routes": find_example(fields, "model_routes")?,
        "router": find_example(fields, "router")?,
        "tools": find_example(fields, "tools")?,
        "structured_output": find_example(fields, "structured_output")?,
        "transport": find_example(fields, "transport")?,
        "ui_surfaces": find_example(fields, "ui_surfaces")?,
    }))
}

/// Serve the combined schema for schema-driven UI rendering.
///
/// Merges the antigravity bot's OpenAPI schema with the zeroclaw plugin
/// projection (providers, model_routes, tools, structured_output) so the
/// frontend can render the entire chat interface from a single schema.
/// GET /api/zeroclaw/schema
pub async fn zeroclaw_schema_handler(Extension(state): Extension<Arc<AppState>>) -> Response {
    // Fetch antigravity OpenAPI schema
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to build HTTP client: {}", e);
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build HTTP client",
            );
        }
    };

    let openapi = match client
        .get(format!("{}/openapi.json", antigravity_base()))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse antigravity OpenAPI: {}", e);
                json!({})
            }
        },
        Err(e) => {
            error!("Failed to fetch antigravity OpenAPI: {}", e);
            json!({})
        }
    };

    // Fetch zeroclaw from shared memory (Absolute Base) first
    // Per AGENTS.md: "1:1 Direct Read (Zero-Copy)" from /dev/shm
    let zeroclaw = if let Some(shm_data) = read_zeroclaw_from_shm() {
        info!("Using zeroclaw from shared memory (Absolute Base)");
        shm_data
    } else {
        // Fallback to D-Bus projection
        match crate::projection_client::get_projection(&state.projection_cache, "zeroclaw").await {
            Some(v) => v,
            None => {
                error!("Zeroclaw not available from shared memory or D-Bus");
                return json_error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Zeroclaw projection not available",
                );
            }
        }
    };

    // Build a JSON Schema for the chat form from zeroclaw providers + model_routes
    let providers = zeroclaw
        .get("providers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let model_routes = zeroclaw
        .get("model_routes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let model = r.get("model").and_then(|v| v.as_str())?;
                    let hint = r.get("hint").and_then(|v| v.as_str());
                    let provider = r
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let available = r
                        .get("available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    // Build label: "provider/model (hint) [status]"
                    let mut label = format!("{}/{}", provider, model);
                    if let Some(h) = hint {
                        label.push_str(&format!(" ({})", h));
                    }
                    if !available {
                        label.push_str(" [declared]");
                    }

                    Some(json!({"const": model, "title": label}))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let chat_schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "title": "Zeroclaw Chat",
        "properties": {
            "provider": {
                "type": "string",
                "enum": providers,
                "description": "LLM provider route"
            },
            "model": {
                "type": "string",
                "oneOf": model_routes,
                "description": "Model to use for this turn"
            },
            "message": {
                "type": "string",
                "description": "Chat message",
                "minLength": 1
            }
        },
        "required": ["message"]
    });

    let schema = json!({
        "openapi": openapi,
        "zeroclaw": {
            "plugin_state": zeroclaw,
            "chat_form_schema": chat_schema,
            "structured_output": zeroclaw.get("structured_output").cloned().unwrap_or(json!({})),
            "tools": zeroclaw.get("tools").cloned().unwrap_or(json!([])),
        }
    });

    let body = simd_json::to_string(&schema).unwrap_or_default();
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| {
            json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build schema response",
            )
        })
}
