//! OpenClaw Gateway Handlers
//!
//! Provides direct access to the OpenClaw container (10.149.181.114:18789)
//! for health checks, configuration, and proxied requests.

use axum::{extract::Extension, response::Json};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error};

use crate::state::AppState;

#[derive(Serialize)]
pub struct OpenClawStatusResponse {
    pub available: bool,
    pub endpoint: String,
    pub model: String,
    pub container_ip: String,
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct OpenClawConfigResponse {
    pub endpoint: String,
    pub model: String,
    pub token_configured: bool,
    pub container_ip: String,
    pub container_port: u16,
}

/// GET /api/openclaw/status - Check OpenClaw gateway health
pub async fn openclaw_status_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<OpenClawStatusResponse> {
    let token = std::env::var("OPENCLAW_TOKEN").unwrap_or_default();
    let base_url = std::env::var("OPENCLAW_BASE_URL")
        .unwrap_or_else(|_| "http://10.149.181.114:18789".to_string());

    let authenticated = !token.is_empty();
    let container_ip = "10.149.181.114".to_string();

    // Try to ping OpenClaw
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let ping_url = format!("{}/v1/models", base_url);
    let mut response = OpenClawStatusResponse {
        available: false,
        endpoint: base_url.clone(),
        model: "google-gemini-cli/gemini-2.5-flash".to_string(),
        container_ip,
        authenticated,
        error: None,
    };

    match client
        .get(&ping_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                debug!("OpenClaw gateway is available at {}", base_url);
                response.available = true;
            } else {
                let status = resp.status();
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
    let endpoint = std::env::var("OPENCLAW_BASE_URL")
        .unwrap_or_else(|_| "http://10.149.181.114:18789".to_string());

    Json(OpenClawConfigResponse {
        endpoint: endpoint.clone(),
        model: "google-gemini-cli/gemini-2.5-flash".to_string(),
        token_configured: !token.is_empty(),
        container_ip: "10.149.181.114".to_string(),
        container_port: 18789,
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
    Json(request): Json<OpenClawChatRequest>,
) -> Json<Value> {
    let token = match std::env::var("OPENCLAW_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            return Json(json!({
                "success": false,
                "error": "OPENCLAW_TOKEN not configured"
            }));
        }
    };

    let base_url = std::env::var("OPENCLAW_BASE_URL")
        .unwrap_or_else(|_| "http://10.149.181.114:18789".to_string());

    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();

    let chat_url = format!("{}/v1/chat/completions", base_url);

    let model = request
        .model
        .unwrap_or_else(|| "google-gemini-cli/gemini-2.5-flash".to_string());

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

    match client
        .post(&chat_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<Value>().await {
                    Ok(data) => {
                        // Extract message from OpenAI format response
                        let message = data
                            .get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.get(0))
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
                let status = resp.status();
                let error_text = resp.text().await.unwrap_or_default();
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

/// GET /api/openclaw/models - List models available through OpenClaw
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

    let base_url = std::env::var("OPENCLAW_BASE_URL")
        .unwrap_or_else(|_| "http://10.149.181.114:18789".to_string());

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let models_url = format!("{}/v1/models", base_url);

    match client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<Value>().await {
                    Ok(data) => Json(data),
                    Err(e) => Json(json!({
                        "models": [],
                        "error": format!("Failed to parse models: {}", e)
                    })),
                }
            } else {
                Json(json!({
                    "models": [],
                    "error": format!("HTTP {}", resp.status())
                }))
            }
        }
        Err(e) => Json(json!({
            "models": [],
            "error": e.to_string()
        })),
    }
}
