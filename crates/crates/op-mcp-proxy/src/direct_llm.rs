//! Direct-mode handler for LLM MCP methods.
//! Prefers VSCode/Cloud Code OAuth cache, with Gemini CLI OAuth fallback.
//! Includes background auto-refresh so tokens never expire mid-session.

use crate::cloudaicompanion::{self, CloudAICompanion};
use crate::gcloud_auth::GCloudAuth;
use chrono::{DateTime, Utc};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

struct CachedToken {
    token: String,
    expiry: DateTime<Utc>,
}

pub struct DirectLLM {
    companion: CloudAICompanion,
    cached_token: Mutex<Option<CachedToken>>,
    gcloud_auth: GCloudAuth,
}

impl DirectLLM {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            companion: CloudAICompanion::new(),
            cached_token: Mutex::new(None),
            gcloud_auth: GCloudAuth::new(),
        })
    }

    /// Start background auto-refresh task. Call once after wrapping in Arc.
    pub fn start_auto_refresh(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            // Initial delay — let the first real request prime the token.
            sleep(Duration::from_secs(60)).await;
            loop {
                let should_refresh = {
                    let guard = this.cached_token.lock().await;
                    match guard.as_ref() {
                        Some(ct) => ct.expiry < Utc::now() + chrono::Duration::minutes(10),
                        None => true,
                    }
                };

                if should_refresh {
                    debug!("Auto-refresh: token expiring soon or missing, refreshing");
                    match this.fetch_fresh_token().await {
                        Ok((token, expiry)) => {
                            info!(
                                "Auto-refresh: token refreshed, valid until {}",
                                expiry.format("%H:%M:%S UTC")
                            );
                            *this.cached_token.lock().await = Some(CachedToken { token, expiry });
                        }
                        Err(e) => {
                            warn!("Auto-refresh: token refresh failed: {}", e);
                        }
                    }
                }

                // Check every 5 minutes.
                sleep(Duration::from_secs(300)).await;
            }
        });
    }

    /// Fetch a fresh token from the best available source.
    async fn fetch_fresh_token(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
        // 1. Try extension token refresh directly (has cloud-ide scope).
        match self.gcloud_auth.refresh_extension_token().await {
            Ok((token, expiry)) => return Ok((token, expiry)),
            Err(e) => {
                debug!("Extension token refresh failed: {}", e);
            }
        }

        // 2. Full auth chain (extension cache → gcloud CLI → etc).
        if env_flag("MCP_PROXY_PREFER_VSCODE_AUTH", true) {
            match self.gcloud_auth.get_token().await {
                Ok(pair) => return Ok(pair),
                Err(e) => {
                    warn!(
                        "Preferred VSCode/Cloud Code auth path failed, trying Gemini OAuth fallback: {}",
                        e
                    );
                }
            }
        }

        // 3. Gemini CLI OAuth.
        if !env_flag("MCP_PROXY_DISABLE_GEMINI_OAUTH", false)
            && !env_flag("OP_MCP_PROXY_DISABLE_GEMINI_OAUTH", false)
        {
            match cloudaicompanion::read_gemini_cli_token() {
                Ok((token, expiry_ms)) => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    if expiry_ms > now_ms + 300_000 {
                        let expiry = DateTime::from_timestamp_millis(expiry_ms)
                            .unwrap_or_else(|| Utc::now() + chrono::Duration::minutes(55));
                        return Ok((token, expiry));
                    }
                    info!("Gemini CLI token expired or expiring soon, refreshing...");
                }
                Err(e) => {
                    debug!("Cannot read gemini CLI token: {}", e);
                }
            }

            match cloudaicompanion::refresh_gemini_cli_token().await {
                Ok(token) => {
                    return Ok((token, Utc::now() + chrono::Duration::minutes(55)));
                }
                Err(e) => {
                    warn!("Gemini CLI token refresh failed: {}", e);
                }
            }
        }

        // 4. Final fallback to full auth chain.
        self.gcloud_auth.get_token().await
    }

    /// Get a valid token, using cache when possible.
    async fn get_token(&self) -> anyhow::Result<String> {
        // Check cached token first.
        {
            let guard = self.cached_token.lock().await;
            if let Some(ref ct) = *guard {
                if ct.expiry > Utc::now() + chrono::Duration::minutes(2) {
                    return Ok(ct.token.clone());
                }
                debug!("Cached token expiring in < 2 min, fetching fresh token");
            }
        }

        let (token, expiry) = self.fetch_fresh_token().await?;
        *self.cached_token.lock().await = Some(CachedToken {
            token: token.clone(),
            expiry,
        });
        Ok(token)
    }

    /// Handle any MCP LLM-style request and return a JSON-RPC result.
    pub async fn handle(&self, req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or_else(Value::null);
        let params = req.get("params").cloned().unwrap_or_else(Value::null);
        let prompt = match Self::extract_prompt(&params) {
            Ok(p) => p,
            Err(e) => return error(&id, -32700, e.to_string()),
        };
        let model = params
            .get("model")
            .and_then(|m| m.as_str())
            .filter(|m| !m.trim().is_empty());

        let token = match self.get_token().await {
            Ok(t) => t,
            Err(e) => return error(&id, -32603, format!("token: {e}")),
        };

        let max_attempts = std::env::var("MCP_PROXY_GENERATE_MAX_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v >= 1)
            .unwrap_or(3);

        let mut last_error: Option<String> = None;
        for attempt in 1..=max_attempts {
            match self.companion.generate(&prompt, &token, model).await {
                Ok(text) => {
                    return simd_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "completion": text,
                            "model": model.unwrap_or("gemini-2.5-flash"),
                            "stopReason": "stop"
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let retryable = is_retryable_generate_error(&msg);
                    last_error = Some(msg.clone());

                    if retryable && attempt < max_attempts {
                        let backoff_ms = 500u64.saturating_mul(attempt as u64);
                        warn!(
                            "Code Assist transient failure (attempt {}/{}): {}; retrying in {}ms",
                            attempt, max_attempts, msg, backoff_ms
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }

                    return error(&id, -32603, format!("generate: {msg}"));
                }
            }
        }

        error(
            &id,
            -32603,
            format!(
                "generate: {}",
                last_error.unwrap_or_else(|| "unknown generate error".to_string())
            ),
        )
    }

    fn extract_prompt(params: &Value) -> anyhow::Result<String> {
        if let Some(msg_array) = params.get("messages").and_then(|v| v.as_array()) {
            return Ok(msg_array
                .iter()
                .filter_map(|m| {
                    let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                    let content = m.get("content")?;
                    let txt = content
                        .get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| content.as_str())?;
                    Some(format!("{role}: {txt}"))
                })
                .collect::<Vec<_>>()
                .join("\n"));
        }

        if let Some(txt) = params.get("prompt").and_then(|v| v.as_str()) {
            return Ok(txt.to_string());
        }

        if let Some(txt) = params
            .get("ref")
            .and_then(|r| r.get("text"))
            .and_then(|v| v.as_str())
        {
            return Ok(txt.to_string());
        }

        anyhow::bail!("no prompt found")
    }
}

fn is_retryable_generate_error(msg: &str) -> bool {
    msg.contains("429 Too Many Requests")
        || msg.contains("RESOURCE_EXHAUSTED")
        || msg.contains("RATE_LIMIT_EXCEEDED")
        || msg.contains("MODEL_CAPACITY_EXHAUSTED")
        || msg.contains("empty response text from code-assist")
        || msg.contains("finish_reason=MALFORMED_FUNCTION_CALL")
        || msg.contains("UNEXPECTED_TOOL_CALL")
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn error(id: &Value, code: i32, msg: impl Into<String>) -> Value {
    simd_json::json!({
        "jsonrpc": "2.0",
        "id": id.clone(),
        "error": {
            "code": code,
            "message": msg.into()
        }
    })
}
