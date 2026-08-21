//! NotebookLM MCP bridge for the cognitive server.
//!
//! Launches the npm-based NotebookLM MCP sidecar over stdio and re-exposes its
//! tools through the local Rust `ToolRegistry`.

use anyhow::Result;
use async_trait::async_trait;
use op_mcp::external_client::{ExternalMcpClient, ExternalMcpConfig, ExternalTool};
use op_mcp::tool_registry::{BoxedTool, Tool, ToolReadiness, ToolRegistry};
use simd_json::{json, OwnedValue as Value};
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
    tool_prefix: Option<String>,
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
            tool_prefix: std::env::var("COGNITIVE_MCP_NOTEBOOKLM_TOOL_PREFIX")
                .ok()
                .filter(|value| !value.trim().is_empty()),
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
        // Pass through the NOTEBOOKLM_COOKIE from the parent environment for authentication.
        if let Ok(cookie) = std::env::var("NOTEBOOKLM_COOKIE") {
            env.insert("NOTEBOOKLM_COOKIE".to_string(), cookie);
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
        let tool_name = if normalized.is_empty() {
            raw_name.to_string()
        } else {
            normalized.to_string()
        };

        if let Some(prefix) = &self.tool_prefix {
            let prefix = prefix.trim_matches('_');
            if prefix.is_empty() {
                tool_name
            } else {
                format!("{prefix}_{tool_name}")
            }
        } else {
            tool_name
        }
    }
}

pub async fn register_notebooklm_tools(registry: &ToolRegistry) -> Result<usize> {
    let config = NotebookLmConfig::from_env();
    if !config.enabled {
        let disabled = register_unavailable_notebooklm_tools(
            registry,
            &config,
            "NotebookLM MCP bridge is disabled by configuration",
        )
        .await?;
        tracing::info!(disabled, "NotebookLM MCP bridge disabled");
        return Ok(disabled);
    }

    let mut client = ExternalMcpClient::new(config.external_config());
    if let Err(error) = client.start().await {
        let disabled = register_unavailable_notebooklm_tools(
            registry,
            &config,
            format!("NotebookLM MCP sidecar failed to start: {error}"),
        )
        .await?;
        tracing::warn!(
            error = %error,
            command = %config.command,
            args = ?config.args,
            disabled,
            "NotebookLM MCP sidecar failed to start; expected tools are marked disabled"
        );
        return Ok(disabled);
    }

    let upstream_tools = client.get_tools().await;
    if upstream_tools.is_empty() {
        let disabled = register_unavailable_notebooklm_tools(
            registry,
            &config,
            "NotebookLM MCP sidecar started but returned no tools",
        )
        .await?;
        tracing::warn!(
            disabled,
            "NotebookLM MCP sidecar started but returned no tools"
        );
        return Ok(disabled);
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

/// Preserve the adapter contract when the external NotebookLM sidecar is unavailable.
///
/// The catalog exposes these entries as disabled rather than silently omitting
/// them. A later live tool with the same name replaces its disabled descriptor.
async fn register_unavailable_notebooklm_tools(
    registry: &ToolRegistry,
    config: &NotebookLmConfig,
    reason: impl Into<String>,
) -> Result<usize> {
    let profile = config
        .profile
        .parse()
        .unwrap_or(crate::tool_profiles::ToolProfile::Minimal);
    let reason = reason.into();
    let mut registered = 0;

    for upstream_name in crate::tool_profiles::tools_for_profile(profile) {
        let name = config.published_tool_name(upstream_name);
        if registry.get(&name).await.is_some() {
            continue;
        }

        registry
            .register(Arc::new(UnavailableNotebookLmTool {
                name,
                upstream_name: upstream_name.to_string(),
                reason: reason.clone(),
            }) as BoxedTool)
            .await?;
        registered += 1;
    }

    Ok(registered)
}

struct UnavailableNotebookLmTool {
    name: String,
    upstream_name: String,
    reason: String,
}

#[async_trait]
impl Tool for UnavailableNotebookLmTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "NotebookLM MCP tool unavailable in this runtime"
    }

    fn input_schema(&self) -> Value {
        // Never invent an upstream schema while its sidecar is unavailable.
        json!({"type": "object", "additionalProperties": true})
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn namespace(&self) -> &str {
        "notebooklm"
    }

    fn tags(&self) -> Vec<String> {
        vec!["notebooklm".to_string(), "unavailable".to_string()]
    }

    fn readiness(&self) -> ToolReadiness {
        ToolReadiness::Disabled {
            reason: format!("{} ({})", self.reason, self.upstream_name),
        }
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        Err(anyhow::anyhow!(
            "NotebookLM tool '{}' is unavailable: {}",
            self.name,
            self.reason
        ))
    }
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
        // Robustness: retry with exponential backoff + session rotation
        // per Operation_Dbus_Robustness_Recommendations.md
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 100;

        let mut last_error = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_DELAY_MS * (1 << (attempt - 1));
                tracing::warn!(
                    tool = %self.name,
                    attempt,
                    delay_ms = delay,
                    "Retrying NotebookLM tool call after backoff"
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            let result = {
                let mut client = self.client.lock().await;
                client.call_tool(&self.upstream_name, input.clone()).await
            };

            match result {
                Ok(value) => return Ok(value),
                Err(e) => {
                    tracing::warn!(
                        tool = %self.name,
                        attempt,
                        error = %e,
                        "NotebookLM tool call failed"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "NotebookLM tool '{}' failed after {} retries",
                self.name,
                MAX_RETRIES
            )
        }))
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
    use super::*;

    #[test]
    fn should_normalize_notebooklm_tool_names() {
        let config = NotebookLmConfig {
            enabled: true,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "notebooklm-mcp@latest".to_string()],
            server_name: "notebooklm".to_string(),
            profile: "minimal".to_string(),
            tool_prefix: None,
            disabled_tools: None,
        };

        assert_eq!(
            config.published_tool_name("notebooklm:create-notebook"),
            "create_notebook"
        );
        assert_eq!(config.published_tool_name("ask question"), "ask_question");
    }

    #[test]
    fn should_prefix_zero_claw_notebooklm_tool_names() {
        let config = NotebookLmConfig {
            enabled: true,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "notebooklm-mcp@latest".to_string()],
            server_name: "tched_router-model-transcripts".to_string(),
            profile: "model-transcripts".to_string(),
            tool_prefix: Some("tched_router_model_transcript".to_string()),
            disabled_tools: None,
        };

        assert_eq!(
            config.published_tool_name("notebooklm:ask-question"),
            "tched_router_model_transcript_ask_question"
        );
    }

    #[tokio::test]
    async fn unavailable_sidecar_registers_explicitly_disabled_tools() {
        let registry = ToolRegistry::new();
        let config = NotebookLmConfig {
            enabled: true,
            command: "missing-notebooklm".to_string(),
            args: Vec::new(),
            server_name: "notebooklm".to_string(),
            profile: "minimal".to_string(),
            tool_prefix: None,
            disabled_tools: None,
        };

        let registered =
            register_unavailable_notebooklm_tools(&registry, &config, "the sidecar is unavailable")
                .await
                .expect("register unavailable tools");

        assert_eq!(registered, 5);
        let catalog = registry.catalog(0, usize::MAX, Some("cognitive")).await;
        assert_eq!(catalog.len(), 5);
        assert!(catalog.iter().all(|entry| matches!(
            &entry.readiness,
            ToolReadiness::Disabled { reason } if reason.contains("the sidecar is unavailable")
        )));

        let error = registry
            .execute("list_notebooks", json!({}))
            .await
            .expect_err("unavailable tools must not execute");
        assert!(error.to_string().contains("is disabled"));
    }
}
