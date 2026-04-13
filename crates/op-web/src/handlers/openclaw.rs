//! OpenClaw Gateway Handlers
//!
//! OpenClaw runs inside the loopback-only `services` container on 127.0.0.1:18789.
//! From the host, it is reached through the Unix socket /run/services0/gateway.sock
//! (bridged by socat inside the container). This module defaults to that socket path.
//!
//! Override with OPENCLAW_BASE_URL for development (e.g. "http://127.0.0.1:18789"
//! when op-web runs inside the same container).

use axum::{extract::Extension, response::Json};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::Request;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UnixStream;
use tracing::{debug, error};

use crate::state::AppState;
use op_identity::WireGuardIdentity;

const DEFAULT_OPENCLAW_SOCKET: &str = "/run/services0/gateway.sock";
const DEFAULT_OPENCLAW_MODEL: &str = "openclaw:main";

/// Returns the Unix socket path, or None if OPENCLAW_BASE_URL is set (TCP mode).
fn openclaw_socket_path() -> Option<String> {
    if std::env::var("OPENCLAW_BASE_URL").is_ok() {
        return None;
    }
    Some(
        std::env::var("OPENCLAW_SOCKET_PATH")
            .unwrap_or_else(|_| DEFAULT_OPENCLAW_SOCKET.to_string()),
    )
}

/// Returns the TCP base URL. Only used when OPENCLAW_BASE_URL is explicitly set.
fn openclaw_tcp_base_url() -> Option<String> {
    std::env::var("OPENCLAW_BASE_URL")
        .ok()
        .map(|u| u.trim_end_matches('/').to_string())
}

fn openclaw_default_model() -> String {
    std::env::var("OPENCLAW_DEFAULT_MODEL").unwrap_or_else(|_| DEFAULT_OPENCLAW_MODEL.to_string())
}

fn configured_openclaw_models() -> Value {
    let default_model = openclaw_default_model();
    json!({
        "models": [
            {
                "id": default_model,
                "name": default_model,
                "description": "Configured OpenClaw route key. OpenClaw selects the target agent and that agent's configured model stack.",
                "routing": "agent",
                "available": true
            }
        ],
        "source": "configured-default",
        "note": "OpenClaw's OpenAI-compatible endpoint routes requests by agent id (for example model=openclaw:<agentId> or x-openclaw-agent-id). /v1/models may not be implemented on the gateway."
    })
}

fn endpoint_label() -> String {
    if let Some(url) = openclaw_tcp_base_url() {
        url
    } else {
        format!(
            "unix:{}",
            openclaw_socket_path().unwrap_or_else(|| DEFAULT_OPENCLAW_SOCKET.to_string())
        )
    }
}

// ---------------------------------------------------------------------------
// Unix-socket HTTP helper
// ---------------------------------------------------------------------------

/// Send an HTTP request over a Unix socket and return (status_code, body_bytes).
async fn unix_request(
    socket_path: &str,
    method: &str,
    path: &str,
    headers: Vec<(&str, String)>,
    body: Option<Vec<u8>>,
    timeout: Duration,
) -> anyhow::Result<(u16, Vec<u8>)> {
    let stream = tokio::time::timeout(timeout, UnixStream::connect(socket_path)).await??;
    let io = TokioIo::new(stream);

    let (mut sender, conn) =
        hyper::client::conn::http1::handshake(io).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            debug!("Unix socket connection closed: {}", e);
        }
    });

    let body_bytes = body.unwrap_or_default();
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("Host", "localhost");

    for (k, v) in &headers {
        req = req.header(*k, v.as_str());
    }

    let req = req.body(Full::new(Bytes::from(body_bytes)))?;

    let resp = tokio::time::timeout(timeout, sender.send_request(req)).await??;
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await?.to_bytes().to_vec();
    Ok((status, body))
}

/// Send an HTTP request using TCP (reqwest). Fallback for OPENCLAW_BASE_URL mode.
async fn tcp_request(
    base_url: &str,
    method: &str,
    path: &str,
    headers: Vec<(&str, String)>,
    body: Option<Vec<u8>>,
    timeout: Duration,
) -> anyhow::Result<(u16, Vec<u8>)> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()?;

    let url = format!("{}{}", base_url, path);
    let mut req = match method {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        _ => client.get(&url),
    };

    for (k, v) in &headers {
        req = req.header(*k, v.as_str());
    }

    if let Some(b) = body {
        req = req.body(b);
    }

    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let body = resp.bytes().await?.to_vec();
    Ok((status, body))
}

/// Unified request: prefers Unix socket, falls back to TCP if OPENCLAW_BASE_URL is set.
async fn openclaw_request(
    method: &str,
    path: &str,
    headers: Vec<(&str, String)>,
    body: Option<Vec<u8>>,
    timeout: Duration,
) -> anyhow::Result<(u16, Vec<u8>)> {
    if let Some(socket) = openclaw_socket_path() {
        unix_request(&socket, method, path, headers, body, timeout).await
    } else if let Some(base_url) = openclaw_tcp_base_url() {
        tcp_request(&base_url, method, path, headers, body, timeout).await
    } else {
        unix_request(DEFAULT_OPENCLAW_SOCKET, method, path, headers, body, timeout).await
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct OpenClawStatusResponse {
    pub available: bool,
    pub endpoint: String,
    pub model: String,
    pub transport: String,
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct OpenClawConfigResponse {
    pub endpoint: String,
    pub model: String,
    pub token_configured: bool,
    pub transport: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/openclaw/status - Check OpenClaw gateway health
pub async fn openclaw_status_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<OpenClawStatusResponse> {
    let token = std::env::var("OPENCLAW_TOKEN").unwrap_or_default();
    let default_model = openclaw_default_model();
    let authenticated = !token.is_empty();
    let endpoint = endpoint_label();
    let transport = if openclaw_socket_path().is_some() {
        "unix-socket"
    } else {
        "tcp"
    };

    let mut response = OpenClawStatusResponse {
        available: false,
        endpoint,
        model: default_model,
        transport: transport.to_string(),
        authenticated,
        error: None,
    };

    let headers = vec![("Authorization", format!("Bearer {}", token))];

    match openclaw_request("GET", "/v1/chat/completions", headers, None, Duration::from_secs(5)).await
    {
        Ok((status, _body)) => {
            if status < 400 || status == 400 || status == 405 {
                debug!("OpenClaw gateway is available via {}", transport);
                response.available = true;
            } else if status == 401 || status == 403 {
                response.available = true;
                response.authenticated = false;
            } else {
                response.error = Some(format!("HTTP {}", status));
                debug!("OpenClaw returned status {}", status);
            }
        }
        Err(e) => {
            error!("Failed to reach OpenClaw: {}", e);
            response.error = Some(e.to_string());
        }
    }

    Json(response)
}

/// GET /api/openclaw/config - Get OpenClaw configuration
pub async fn openclaw_config_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<OpenClawConfigResponse> {
    let token = std::env::var("OPENCLAW_TOKEN").unwrap_or_default();

    Json(OpenClawConfigResponse {
        endpoint: endpoint_label(),
        model: openclaw_default_model(),
        token_configured: !token.is_empty(),
        transport: if openclaw_socket_path().is_some() {
            "unix-socket".to_string()
        } else {
            "tcp".to_string()
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct OpenClawChatRequest {
    pub message: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
}

/// POST /api/openclaw/chat - Direct chat via OpenClaw (bypasses op-llm)
pub async fn openclaw_chat_handler(
    Extension(_state): Extension<Arc<AppState>>,
    axum::extract::Host(host): axum::extract::Host,
    headers: axum::http::HeaderMap,
    Json(request): Json<OpenClawChatRequest>,
) -> Json<Value> {
    // WireGuard authentication for dashboard.3tched.com requests
    if host.contains("dashboard.3tched.com") {
        let client_ip = headers
            .get("x-real-ip")
            .or_else(|| headers.get("x-forwarded-for"))
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown");

        if let Ok(false) = is_wireguard_authenticated(client_ip).await {
            return Json(json!({
                "success": false,
                "error": "WireGuard authentication required for dashboard.3tched.com"
            }));
        }
    }

    let token = match std::env::var("OPENCLAW_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            return Json(json!({
                "success": false,
                "error": "OPENCLAW_TOKEN not configured"
            }));
        }
    };

    let model = request.model.unwrap_or_else(openclaw_default_model);

    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": request.message
            }
        ],
        "max_tokens": request.max_tokens.unwrap_or(2048),
        "temperature": request.temperature.unwrap_or(0.7)
    });

    let body = simd_json::to_string(&payload).unwrap_or_default().into_bytes();
    let req_headers = vec![
        ("Authorization", format!("Bearer {}", token)),
        ("Content-Type", "application/json".to_string()),
    ];

    match openclaw_request("POST", "/v1/chat/completions", req_headers, Some(body), Duration::from_secs(60)).await {
        Ok((status, body)) => {
            if status < 400 {
                match simd_json::from_slice::<Value>(&mut body.clone()) {
                    Ok(data) => {
                        let message = data
                            .get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|choice| choice.get("message"))
                            .and_then(|msg| msg.get("content"))
                            .and_then(|c| c.as_str())
                            .unwrap_or("No response");

                        Json(json!({
                            "success": true,
                            "message": message,
                            "model": model,
                            "provider": "openclaw",
                            "raw_response": data
                        }))
                    }
                    Err(e) => Json(json!({
                        "success": false,
                        "error": format!("Failed to parse response: {}", e)
                    })),
                }
            } else {
                let error_text = String::from_utf8_lossy(&body);
                Json(json!({
                    "success": false,
                    "error": format!("OpenClaw API error {}: {}", status, error_text)
                }))
            }
        }
        Err(e) => Json(json!({
            "success": false,
            "error": format!("Request failed: {}", e)
        })),
    }
}

/// GET /api/openclaw/models - List configured OpenClaw route keys
pub async fn openclaw_models_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<Value> {
    let token = match std::env::var("OPENCLAW_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            return Json(json!({
                "models": [],
                "error": "OPENCLAW_TOKEN not configured"
            }));
        }
    };

    let headers = vec![("Authorization", format!("Bearer {}", token))];

    match openclaw_request("GET", "/v1/models", headers, None, Duration::from_secs(10)).await {
        Ok((status, mut body)) => {
            if status < 400 {
                match simd_json::from_slice::<Value>(&mut body) {
                    Ok(data) => Json(data),
                    Err(e) => {
                        debug!(
                            "OpenClaw /v1/models returned non-JSON, using configured default: {}",
                            e
                        );
                        Json(configured_openclaw_models())
                    }
                }
            } else {
                debug!(
                    "OpenClaw /v1/models returned {}, using configured default",
                    status
                );
                Json(configured_openclaw_models())
            }
        }
        Err(e) => {
            debug!(
                "OpenClaw /v1/models request failed, using configured default: {}",
                e
            );
            Json(configured_openclaw_models())
        }
    }
}

/// Check if the client IP is authenticated via WireGuard
async fn is_wireguard_authenticated(client_ip: &str) -> anyhow::Result<bool> {
    let wg_identity = WireGuardIdentity::new();
    let peers = wg_identity.get_connected_peers()?;

    for peer in peers {
        if peer.allowed_ips.iter().any(|allowed_ip| {
            allowed_ip.contains(client_ip)
                || client_ip == allowed_ip.split('/').next().unwrap_or("")
        }) {
            return Ok(true);
        }
    }

    Ok(false)
}
