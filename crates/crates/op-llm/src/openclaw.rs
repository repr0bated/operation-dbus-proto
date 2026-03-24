//! OpenClaw LLM Provider
//!
//! Connects to the OpenClaw agent platform via its OpenAI-compatible
//! `/v1/chat/completions` endpoint with bearer token auth.
//!
//! ## Configuration
//!
//! ```bash
//! OPENCLAW_TOKEN=your-token
//! OPENCLAW_BASE_URL=http://127.0.0.1:18789  # default
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
    ToolCallInfo,
};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:18789";
const DEFAULT_MODEL: &str = "openclaw-default";

pub struct OpenClawProvider {
    client: Client,
    token: String,
    base_url: String,
}

impl OpenClawProvider {
    pub fn new(token: String, base_url: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            token,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        }
    }

    pub fn from_env() -> Result<Self> {
        let token = std::env::var("OPENCLAW_TOKEN").context("OPENCLAW_TOKEN must be set")?;
        let base_url = std::env::var("OPENCLAW_BASE_URL").ok();
        Ok(Self::new(token, base_url))
    }
}

#[async_trait]
impl LlmProvider for OpenClawProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Custom("openclaw".to_string())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![ModelInfo {
            id: DEFAULT_MODEL.to_string(),
            name: "OpenClaw Default".to_string(),
            description: Some("OpenClaw agent platform default model".to_string()),
            parameters: None,
            available: true,
            tags: vec!["openclaw".to_string()],
            downloads: None,
            updated_at: None,
        }])
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| m.name.to_lowercase().contains(&query.to_lowercase()))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(self.get_model(model_id).await?.is_some())
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        warn!("Using chat() without tools - consider using chat_with_request()");
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let model = if model.is_empty() {
            DEFAULT_MODEL
        } else {
            model
        };
        let url = format!("{}/v1/chat/completions", self.base_url);

        // Convert messages to OpenAI format
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = json!({
                    "role": m.role,
                    "content": m.content
                });

                if let Some(ref id) = m.tool_call_id {
                    msg["tool_call_id"] = json!(id);
                }

                if let Some(ref calls) = m.tool_calls {
                    msg["tool_calls"] = json!(calls.iter().map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": simd_json::to_string(&tc.arguments).unwrap_or_default()
                            }
                        })
                    }).collect::<Vec<_>>());
                }

                msg
            })
            .collect();

        let tools: Vec<Value> = request.tools.iter().map(|t| t.to_openai_format()).collect();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = request.tool_choice.to_api_format();
            info!(
                "OpenClaw request with {} tools, tool_choice={:?}",
                tools.len(),
                request.tool_choice
            );
        }

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        debug!(
            "OpenClaw request: {}",
            simd_json::to_string_pretty(&body).unwrap_or_default()
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to OpenClaw")?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!(
            "OpenClaw response ({}): {}",
            status,
            &response_text[..response_text.len().min(500)]
        );

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "OpenClaw API error ({}): {}",
                status,
                response_text
            ));
        }

        // Parse OpenAI-compatible response
        let mut response_text_mut = response_text;
        let response_json: Value =
            unsafe { simd_json::from_str(&mut response_text_mut) }.map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse OpenClaw response: {}. Body: {}",
                    e,
                    response_text_mut
                )
            })?;

        let choice = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow::anyhow!("No choices returned from OpenClaw"))?;

        let message = choice
            .get("message")
            .ok_or_else(|| anyhow::anyhow!("No message in OpenClaw response"))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let role = message
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("assistant")
            .to_string();

        // Parse tool_calls
        let tool_calls: Option<Vec<ToolCallInfo>> = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|call| {
                        let id = call.get("id")?.as_str()?.to_string();
                        let function = call.get("function")?;
                        let name = function.get("name")?.as_str()?.to_string();
                        let args_str = function.get("arguments")?.as_str()?;
                        let mut args_mut = args_str.to_string();
                        let arguments: Value =
                            unsafe { simd_json::from_str(&mut args_mut) }.ok()?;

                        Some(ToolCallInfo {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect()
            });

        if let Some(ref calls) = tool_calls {
            info!("OpenClaw: parsed {} tool calls", calls.len());
        }

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());

        let usage = response_json.get("usage").map(|u| TokenUsage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role,
                content,
                tool_calls: tool_calls.clone(),
                tool_call_id: None,
            },
            model: model.to_string(),
            provider: "openclaw".to_string(),
            finish_reason,
            usage,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}
