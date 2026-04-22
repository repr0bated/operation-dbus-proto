//! NotebookLM MCP bridge for the cognitive server.
//!
//! Launches the npm-based NotebookLM MCP sidecar over stdio and re-exposes its
//! tools through the local Rust `ToolRegistry`.

use anyhow::Result;
use async_trait::async_trait;
use op_mcp::external_client::{ExternalMcpClient, ExternalMcpConfig, ExternalTool};
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_NOTEBOOKLM_COMMAND: &str = "npx";
const DEFAULT_NOTEBOOKLM_ARGS: &[&str] = &["-y", "notebooklm-mcp@latest"];
const DEFAULT_NOTEBOOKLM_SERVER_NAME: &str = "notebooklm";
const DEFAULT_NOTEBOOKLM_PROFILE: &str = "minimal";

#[derive(Debug, Clone)]
struct NotebookLmConfig {
    enabled: bool,
    command: String,
    args: Vec<String>,
    server_name: String,
    profile: String,
    disabled_tools: Option<String>,
}

impl NotebookLmConfig {
    fn from_env() -> Self {
        Self {
            enabled: env_flag("COGNITIVE_MCP_NOTEBOOKLM_ENABLED", true),
            command: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_COMMAND")
                .unwrap_or_else(|_| DEFAULT_NOTEBOOKLM_COMMAND.to_string()),
            args: env_list(
                "COGNITIVE_MCP_NOTEBOOKLM_ARGS",
                DEFAULT_NOTEBOOKLM_ARGS
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
            ),
            server_name: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_SERVER_NAME")
                .unwrap_or_else(|_| DEFAULT_NOTEBOOKLM_SERVER_NAME.to_string()),
            profile: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_PROFILE")
                .unwrap_or_else(|_| DEFAULT_NOTEBOOKLM_PROFILE.to_string()),
            disabled_tools: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_DISABLED_TOOLS").ok(),
        }
    }

    fn external_config(&self) -> ExternalMcpConfig {
        let mut env = HashMap::new();
        env.insert("NOTEBOOKLM_PROFILE".to_string(), self.profile.clone());
        if let Some(disabled_tools) = &self.disabled_tools {
            env.insert(
                "NOTEBOOKLM_DISABLED_TOOLS".to_string(),
                disabled_tools.clone(),
            );
        }

        ExternalMcpConfig {
            name: self.server_name.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            env,
            api_key: None,
            api_key_env: "API_KEY".to_string(),
            auth_method: op_mcp::external_client::AuthMethod::None,
            headers: HashMap::new(),
        }
    }

    fn published_tool_name(&self, upstream_name: &str) -> String {
        let raw_name = upstream_name
            .split_once(':')
            .map(|(_, name)| name)
            .unwrap_or(upstream_name);

        let mut normalized = String::with_capacity(raw_name.len());
        let mut last_was_underscore = false;

        for ch in raw_name.chars() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                normalized.push(ch);
                last_was_underscore = false;
            } else if !last_was_underscore {
                normalized.push('_');
                last_was_underscore = true;
            }
        }

        let normalized = normalized.trim_matches('_');
        if normalized.is_empty() {
            raw_name.to_string()
        } else {
            normalized.to_string()
        }
    }
}

pub async fn register_notebooklm_tools(registry: &ToolRegistry) -> Result<usize> {
    let config = NotebookLmConfig::from_env();
    if !config.enabled {
        tracing::info!("NotebookLM MCP bridge disabled");
        return Ok(0);
    }

    let mut client = ExternalMcpClient::new(config.external_config());
    if let Err(error) = client.start().await {
        tracing::warn!(
            error = %error,
            command = %config.command,
            args = ?config.args,
            "NotebookLM MCP sidecar failed to start; continuing without NotebookLM tools"
        );
        return Ok(0);
    }

    let upstream_tools = client.get_tools().await;
    if upstream_tools.is_empty() {
        tracing::warn!("NotebookLM MCP sidecar started but returned no tools");
        return Ok(0);
    }

    let shared_client = Arc::new(Mutex::new(client));
    let mut registered = 0usize;

    for tool in upstream_tools {
        let published_name = config.published_tool_name(&tool.name);
        let wrapper = NotebookLmTool::new(shared_client.clone(), tool, published_name);
        registry.register(Arc::new(wrapper) as BoxedTool).await?;
        registered += 1;
    }

    tracing::info!(registered, "Registered NotebookLM MCP tools");
    Ok(registered)
}

struct NotebookLmTool {
    client: Arc<Mutex<ExternalMcpClient>>,
    upstream_name: String,
    name: String,
    description: String,
    input_schema: Value,
}

impl NotebookLmTool {
    fn new(client: Arc<Mutex<ExternalMcpClient>>, tool: ExternalTool, name: String) -> Self {
        let upstream_name = tool
            .name
            .split_once(':')
            .map(|(_, raw_name)| raw_name.to_string())
            .unwrap_or_else(|| tool.name.clone());

        Self {
            client,
            upstream_name,
            name,
            description: format!("NotebookLM MCP: {}", tool.description),
            input_schema: tool.input_schema,
        }
    }
}

#[async_trait]
impl Tool for NotebookLmTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn namespace(&self) -> &str {
        "notebooklm"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "notebooklm".to_string(),
            "rag".to_string(),
            "research".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let mut client = self.client.lock().await;
        client.call_tool(&self.upstream_name, input).await
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_list(name: &str, default: Vec<String>) -> Vec<String> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .split_whitespace()
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::NotebookLmConfig;

    #[test]
    fn should_normalize_notebooklm_tool_names() {
        let config = NotebookLmConfig {
            enabled: true,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "notebooklm-mcp@latest".to_string()],
            server_name: "notebooklm".to_string(),
            profile: "minimal".to_string(),
            disabled_tools: None,
        };

        assert_eq!(
            config.published_tool_name("notebooklm:create-notebook"),
            "create_notebook"
        );
        assert_eq!(config.published_tool_name("ask question"), "ask_question");
    }
}
