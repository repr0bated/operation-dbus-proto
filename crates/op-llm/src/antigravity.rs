//! Antigravity Provider - Uses OAuth token from headless Antigravity service
//!
//! ## Authentication Flow
//!
//! 1. Antigravity IDE runs headless with virtual Wayland display
//! 2. User logs in once via VNC
//! 3. OAuth token is extracted and saved
//! 4. This provider uses that token for Gemini API calls
//!
//! ## Features
//!
//! - Uses enterprise Code Assist subscription (no API charges)
//! - Auto-refreshes expired tokens
//! - Falls back to API key if OAuth token not available
//!
//! ## Configuration
//!
//! ```bash
//! # OAuth token (from Antigravity headless service)
//! export GOOGLE_AUTH_TOKEN_FILE=~/.config/antigravity/token.json
//!
//! # Or fallback to API key
//! export GEMINI_API_KEY=xxx
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};
use uuid::Uuid;

use crate::headless_oauth::HeadlessOAuthProvider;
use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
    ToolCallInfo, ToolChoice, ToolDefinition,
};

/// Gemini API base URL
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Available models through Antigravity/Gemini
const ANTIGRAVITY_MODELS: &[(&str, &str, &str)] = &[
    (
        "gemini-2.0-flash",
        "Gemini 2.0 Flash",
        "Fast, efficient, and cost-effective",
    ),
    (
        "gemini-1.5-pro",
        "Gemini 1.5 Pro",
        "High quality, long context window",
    ),
    ("gemini-1.5-flash", "Gemini 1.5 Flash", "Legacy flash model"),
];

/// Authentication method
#[derive(Debug, Clone)]
enum AuthMethod {
    /// OAuth token from headless Antigravity service
    OAuth(Arc<HeadlessOAuthProvider>),
    /// Direct API key
    ApiKey(String),
    /// Local Antigravity Bridge (OpenAI-compatible)
    Bridge(String),
}

/// Antigravity Provider
///
/// Uses OAuth token captured from Antigravity headless service,
/// or falls back to API key.
pub struct AntigravityProvider {
    client: Client,
    auth: AuthMethod,
    default_model: String,
}

impl AntigravityProvider {
    /// Create from environment
    ///
    /// Tries in order:
    /// 1. OAuth token from `GOOGLE_AUTH_TOKEN_FILE`
    /// 2. API key from `GEMINI_API_KEY`
    pub fn from_env() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("Failed to create HTTP client")?;

        // 1. Try Bridge URL
        if let Ok(bridge_url) = std::env::var("ANTIGRAVITY_BRIDGE_URL") {
            info!("✅ Antigravity: Using Bridge at {}", bridge_url);
            let default_model =
                std::env::var("LLM_MODEL").unwrap_or_else(|_| "ide-model".to_string());

            return Ok(Self {
                client,
                auth: AuthMethod::Bridge(bridge_url),
                default_model,
            });
        }

        // 2. Try OAuth first
        let oauth_provider = HeadlessOAuthProvider::from_env().ok();

        let auth = if let Some(ref oauth) = oauth_provider {
            if oauth.is_authenticated() {
                info!(
                    "✅ Antigravity: Using OAuth token from {}",
                    oauth.token_file().display()
                );
                AuthMethod::OAuth(Arc::new(oauth_provider.unwrap()))
            } else {
                // Try API key
                if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
                    info!("✅ Antigravity: Using API key (OAuth token not valid)");
                    AuthMethod::ApiKey(api_key)
                } else {
                    anyhow::bail!(
                        "No valid authentication found.\n\n\
                        Options:\n\
                        1. Connect to Antigravity Bridge: export ANTIGRAVITY_BRIDGE_URL=http://127.0.0.1:7788\n\
                        2. Start Antigravity headless and login\n\
                        3. Set GEMINI_API_KEY"
                    );
                }
            }
        } else if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
            info!("✅ Antigravity: Using API key");
            AuthMethod::ApiKey(api_key)
        } else {
            anyhow::bail!(
                "No authentication configured.\n\n\
                Set ANTIGRAVITY_BRIDGE_URL, GEMINI_API_KEY or configure OAuth via Antigravity headless service."
            );
        };

        let default_model =
            std::env::var("LLM_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());

        Ok(Self {
            client,
            auth,
            default_model,
        })
    }

    /// Create with API key directly
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: AuthMethod::ApiKey(api_key.into()),
            default_model: "gemini-2.0-flash".to_string(),
        }
    }

    /// Create with OAuth provider
    pub fn with_oauth(oauth: HeadlessOAuthProvider) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: AuthMethod::OAuth(Arc::new(oauth)),
            default_model: "gemini-2.0-flash".to_string(),
        }
    }

    /// Build authenticated request
    async fn build_request(&self, url: &str) -> Result<reqwest::RequestBuilder> {
        match &self.auth {
            AuthMethod::OAuth(oauth) => {
                let token = oauth.get_token().await?;
                Ok(self
                    .client
                    .post(url)
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Content-Type", "application/json"))
            }
            AuthMethod::ApiKey(key) => {
                // API key goes in URL for Gemini
                let url_with_key = if url.contains('?') {
                    format!("{}&key={}", url, key)
                } else {
                    format!("{}?key={}", url, key)
                };
                Ok(self
                    .client
                    .post(&url_with_key)
                    .header("Content-Type", "application/json"))
            }
            AuthMethod::Bridge(_) => Ok(self
                .client
                .post(url)
                .header("Content-Type", "application/json")),
        }
    }

    /// Convert messages to Gemini format
    fn convert_messages(&self, messages: &[ChatMessage]) -> (Vec<Value>, Option<Value>) {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                system_instruction = Some(json!({
                    "parts": [{"text": msg.content}]
                }));
                continue;
            }

            let role = match msg.role.as_str() {
                "assistant" => "model",
                _ => "user",
            };

            contents.push(json!({
                "role": role,
                "parts": [{"text": msg.content}]
            }));
        }

        (contents, system_instruction)
    }

    /// Convert messages to OpenAI (Bridge) format
    fn convert_messages_openai(&self, messages: &[ChatMessage]) -> Vec<Value> {
        messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect()
    }

    /// Convert tools to Gemini format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> Value {
        let function_declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema
                })
            })
            .collect();

        json!([{
            "functionDeclarations": function_declarations
        }])
    }

    /// Convert tool choice to Gemini format
    fn convert_tool_choice(&self, choice: &ToolChoice) -> Option<Value> {
        match choice {
            ToolChoice::Auto => Some(json!({"mode": "AUTO"})),
            ToolChoice::Required => Some(json!({"mode": "ANY"})),
            ToolChoice::None => Some(json!({"mode": "NONE"})),
            ToolChoice::Tool(name) => Some(json!({
                "mode": "ANY",
                "allowedFunctionNames": [name]
            })),
        }
    }
}

#[async_trait]
impl LlmProvider for AntigravityProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Antigravity
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        if let AuthMethod::Bridge(_) = self.auth {
            return Ok(vec![ModelInfo {
                id: "ide-model".to_string(),
                name: "IDE Model".to_string(),
                description: Some("Model provided by Antigravity IDE code assist".to_string()),
                parameters: None,
                available: true,
                tags: vec!["ide".to_string()],
                downloads: None,
                updated_at: None,
            }]);
        }

        Ok(ANTIGRAVITY_MODELS
            .iter()
            .map(|(id, name, desc)| ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                description: Some(desc.to_string()),
                parameters: None,
                available: true,
                tags: vec!["gemini".to_string()],
                downloads: None,
                updated_at: None,
            })
            .collect())
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let query_lower = query.to_lowercase();
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&query_lower)
                    || m.name.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        if let AuthMethod::Bridge(_) = self.auth {
            return Ok(true); // Bridge handles model selection
        }
        Ok(ANTIGRAVITY_MODELS.iter().any(|(id, _, _)| *id == model_id))
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let actual_model = if model.is_empty() || model == "auto" {
            &self.default_model
        } else {
            model
        };

        if let AuthMethod::Bridge(bridge_url) = &self.auth {
            // OpenAI-compatible Bridge
            let url = format!("{}/v1/chat/completions", bridge_url);
            debug!("Antigravity Bridge request to: {}", url);

            let body = json!({
                "model": actual_model,
                "messages": self.convert_messages_openai(&request.messages),
                "temperature": request.temperature.unwrap_or(0.7),
            });

            let http_request = self.build_request(&url).await?;
            let response = http_request
                .json(&body)
                .send()
                .await
                .context("Failed to send bridge request")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("Bridge error: {}", response.status()));
            }

            let result: Value = response
                .json()
                .await
                .context("Failed to parse bridge response")?;

            let content = result
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();

            let finish_reason = result
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
                .and_then(|c| c.get("finish_reason"))
                .and_then(|s| s.as_str())
                .map(String::from);

            return Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    tool_calls: None, // Bridge might not support tool calls yet
                    tool_call_id: None,
                },
                model: actual_model.to_string(),
                provider: "antigravity-bridge".to_string(),
                finish_reason,
                usage: None,
                tool_calls: None,
            });
        }

        let url = format!(
            "{}/models/{}:generateContent",
            GEMINI_API_BASE, actual_model
        );

        let (contents, system_instruction) = self.convert_messages(&request.messages);

        let mut body = json!({
            "contents": contents,
        });

        if let Some(sys) = system_instruction {
            body["systemInstruction"] = sys;
        }

        // Add tools if present
        if !request.tools.is_empty() {
            body["tools"] = self.convert_tools(&request.tools);

            if let Some(tool_config) = self.convert_tool_choice(&request.tool_choice) {
                body["toolConfig"] = json!({"functionCallingConfig": tool_config});
            }
        }

        // Generation config
        let mut gen_config = json!({});
        if let Some(temp) = request.temperature {
            gen_config["temperature"] = json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            gen_config["maxOutputTokens"] = json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            gen_config["topP"] = json!(top_p);
        }
        if gen_config != json!({}) {
            body["generationConfig"] = gen_config;
        }

        debug!("Antigravity request to: {}", url);

        let http_request = self.build_request(&url).await?;
        let response = http_request
            .json(&body)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.as_u16() == 401 {
                return Err(anyhow::anyhow!(
                    "Authentication failed (401).\n\n\
                    Token may have expired. Try:\n\
                    1. Reconnect to Antigravity VNC and re-login\n\
                    2. Run: ./scripts/antigravity-extract-token.sh\n\
                    3. Restart op-web: sudo systemctl restart op-web"
                ));
            }

            return Err(anyhow::anyhow!("API error {}: {}", status, body));
        }

        let result: Value = response.json().await.context("Failed to parse response")?;

        // Parse response
        let candidates = result
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| anyhow::anyhow!("No candidates in response"))?;

        let first_candidate = candidates
            .first()
            .ok_or_else(|| anyhow::anyhow!("Empty candidates"))?;

        // Extract text and tool calls
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        if let Some(parts) = first_candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
        {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                }
                if let Some(fc) = part.get("functionCall") {
                    let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or_default();
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    tool_calls.push(ToolCallInfo {
                        id: format!("call_{}", Uuid::new_v4()),
                        name: name.to_string(),
                        arguments: args,
                    });
                }
            }
        }

        let usage = result.get("usageMetadata").map(|u| TokenUsage {
            prompt_tokens: u
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".to_string(),
                content: text_parts.join(""),
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls.clone())
                },
                tool_call_id: None,
            },
            model: actual_model.to_string(),
            provider: "antigravity".to_string(),
            finish_reason: first_candidate
                .get("finishReason")
                .and_then(|f| f.as_str())
                .map(String::from),
            usage,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        // Fall back to non-streaming for now
        let response = self.chat(model, messages).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}
