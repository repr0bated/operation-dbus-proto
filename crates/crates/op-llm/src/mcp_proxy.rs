//! MCP Proxy LLM provider – delegates to op-mcp-proxy in DIRECT_MODE.

use anyhow::{Context, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, warn};

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, ToolChoice,
};

pub struct McpProxyProvider {
    bin: String,
    env_extra: Vec<(String, String)>,
}

impl McpProxyProvider {
    /// Build from environment.  Requires OP_MCP_PROXY_BIN (or falls back to "op-mcp-proxy").
    pub fn from_env() -> Result<Self> {
        let bin = std::env::var("OP_MCP_PROXY_BIN").unwrap_or_else(|_| "op-mcp-proxy".to_string());

        // Verify binary exists
        if !bin.starts_with('/') {
            // relative name – trust PATH
        } else if !std::path::Path::new(&bin).exists() {
            anyhow::bail!("op-mcp-proxy binary not found at {}", bin);
        }

        let mut env_extra = vec![("DIRECT_MODE".to_string(), "1".to_string())];

        // Forward relevant MCP_PROXY_* env vars
        for (k, v) in std::env::vars() {
            if k.starts_with("MCP_PROXY_") || k.starts_with("OP_MCP_PROXY_") {
                env_extra.push((k, v));
            }
        }

        Ok(Self { bin, env_extra })
    }

    /// Spawn op-mcp-proxy (via select3 wrapper), send one JSON-RPC request, return the response.
    async fn call(&self, request: simd_json::OwnedValue) -> Result<simd_json::OwnedValue> {
        let mut cmd = tokio::process::Command::new(&self.bin);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(self.env_extra.clone()); // Apply collected environment variables

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn {}", self.bin))?;

        let stdin = child.stdin.as_mut().context("no stdin")?;
        let line = simd_json::to_string(&request)?;
        stdin.write_all(line.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        drop(child.stdin.take());

        let stdout = child.stdout.take().context("no stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        let _ = child.wait().await;

        if response_line.trim().is_empty() {
            // Read stderr for diagnostics
            if let Some(mut stderr) = child.stderr.take() {
                let mut err = String::new();
                tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut err)
                    .await
                    .ok();
                if !err.trim().is_empty() {
                    debug!("op-mcp-proxy stderr: {}", err.trim());
                }
            }
            anyhow::bail!("op-mcp-proxy returned empty response");
        }

        let mut bytes = response_line.into_bytes();
        let json: simd_json::OwnedValue =
            simd_json::from_slice(&mut bytes).context("failed to parse op-mcp-proxy response")?;

        if let Some(err) = json.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("op-mcp-proxy error: {}", msg);
        }

        Ok(json)
    }
}

#[async_trait]
impl LlmProvider for McpProxyProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::McpProxy
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "auto".to_string(),
                name: "Auto (Gemini 3 routing)".to_string(),
                description: Some("Automatic model selection via op-mcp-proxy".to_string()),
                parameters: None,
                available: true,
                tags: vec!["auto".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "gemini-2.5-flash".to_string(),
                name: "Gemini 2.5 Flash".to_string(),
                description: Some("Fast model via Code Assist".to_string()),
                parameters: None,
                available: true,
                tags: vec!["fast".to_string()],
                downloads: None,
                updated_at: None,
            },
        ])
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        let q = query.to_lowercase();
        Ok(models
            .into_iter()
            .filter(|m| m.id.to_lowercase().contains(&q) || m.name.to_lowercase().contains(&q))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        Ok(self
            .list_models()
            .await?
            .into_iter()
            .find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, _model_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let prompt = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let req = simd_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "generate",
            "params": {
                "prompt": prompt,
                "model": model
            }
        });

        let resp = self.call(req).await?;
        let result = resp.get("result").context("missing result in response")?;
        let text = result
            .get("completion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let used_model = result
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(model)
            .to_string();

        Ok(ChatResponse {
            message: ChatMessage::assistant(text),
            model: used_model,
            provider: "mcp-proxy".to_string(),
            finish_reason: Some("stop".to_string()),
            usage: None,
            tool_calls: None,
        })
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        // For tool-calling requests, flatten to a simple prompt since
        // op-mcp-proxy only supports generateContent.
        self.chat(model, request.messages).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        tx.send(Ok(response.message.content)).await.ok();
        Ok(rx)
    }
}
