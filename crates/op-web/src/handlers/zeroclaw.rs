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
use tracing::{error, info, warn};

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

/// Read the zeroclaw `PluginSchema` directly from the sealed blob catalog
/// (Absolute Base). Per AGENTS.md the sealed blob IS the plugin; the monolithic
/// `/dev/shm/live-schema.json` is gone. The live provider/model catalog lives
/// in the D-Bus projection (see `zeroclaw_schema_handler`), not in the blob —
/// this returns the schema contract only.
fn read_zeroclaw_schema_shm() -> Option<Value> {
    let schema = op_blob::catalog::read_plugin_schema_shm("zeroclaw")?;
    // `PluginSchema` serializes to { name, version, fields, methods, … }.
    let mut v: Value = simd_json::to_owned_value(&mut serde_json::to_vec(&schema).ok()?).ok()?;
    // Stamp the catalog_hash so consumers can verify lineage.
    if let Ok(bytes) = std::fs::read("/dev/shm/opdbus/plugin-blobs/.manifest.json") {
        if let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(hash) = manifest.get("catalog_hash").and_then(|h| h.as_str()) {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("catalog_hash".to_string(), simd_json::json!(hash));
                }
            }
        }
    }
    Some(v)
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

    // Prefer the D-Bus projection (live provider/model catalog), since the
    // sealed blob only carries the PluginSchema contract, not the live
    // `projection.providers` / `projection.model_routes`. Fall back to the SHM
    // PluginSchema so the schema surface still renders when D-Bus is down.
    let (zeroclaw, zeroclaw_schema) = match crate::projection_client::get_projection(
        &state.projection_cache,
        "zeroclaw",
    )
    .await
    {
        Some(v) => {
            info!("Using zeroclaw from D-Bus projection (live provider/model catalog)");
            (v, read_zeroclaw_schema_shm())
        }
        None => match read_zeroclaw_schema_shm() {
            Some(schema) => {
                warn!("Zeroclaw D-Bus projection unavailable; using SHM PluginSchema only");
                (schema.clone(), Some(schema))
            }
            None => {
                error!("Zeroclaw not available from D-Bus or SHM");
                return json_error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Zeroclaw projection not available",
                );
            }
        },
    };

    // Providers + model_routes live under `projection` in the live state.
    let projection = zeroclaw.get("projection").and_then(|v| v.as_object());

    // Build a JSON Schema for the chat form from zeroclaw providers + model_routes
    let providers = projection
        .and_then(|p| p.get("providers"))
        .and_then(|v| v.as_array())
        .or_else(|| zeroclaw.get("providers").and_then(|v| v.as_array()))
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let model_routes = projection
        .and_then(|p| p.get("model_routes"))
        .and_then(|v| v.as_array())
        .or_else(|| zeroclaw.get("model_routes").and_then(|v| v.as_array()))
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
            "schema": zeroclaw_schema,
            "chat_form_schema": chat_schema,
            "structured_output": projection
                .and_then(|p| p.get("structured_output"))
                .cloned()
                .or_else(|| zeroclaw.get("structured_output").cloned())
                .unwrap_or(json!({})),
            "tools": projection
                .and_then(|p| p.get("tools"))
                .cloned()
                .or_else(|| zeroclaw.get("tools").cloned())
                .unwrap_or(json!([])),
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
