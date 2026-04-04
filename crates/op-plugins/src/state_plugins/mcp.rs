//! MCP state plugin - manages MCP server configurations and tool groups
//! Wires MCP configuration to the state store for auditing and rollback

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{ExecutionJob, ExecutionStatus, StateStore};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// MCP configuration schema - mirrors the state JSON structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    /// External MCP servers indexed by name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<HashMap<String, McpServerConfig>>,

    /// Tool groups configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_groups: Option<ToolGroupsConfig>,

    /// Compact mode settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_mode: Option<CompactModeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    /// Server command to execute
    pub command: String,

    /// Command arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Whether server is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Transport type (stdio, sse, http)
    #[serde(default = "default_stdio")]
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolGroupsConfig {
    /// Enabled group IDs
    pub enabled: Vec<String>,

    /// Maximum tools limit
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,

    /// Access zone (localhost, trusted_mesh, private_network, public)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_zone: Option<String>,

    /// Trusted network prefixes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted_networks: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactModeConfig {
    /// Whether compact mode is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Meta-tools to expose
    #[serde(default = "default_meta_tools")]
    pub meta_tools: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_stdio() -> String {
    "stdio".to_string()
}

fn default_max_tools() -> usize {
    40
}

fn default_meta_tools() -> Vec<String> {
    vec![
        "list_tools".to_string(),
        "search_tools".to_string(),
        "get_tool_schema".to_string(),
        "execute_tool".to_string(),
    ]
}

/// Tool definition - canonical schema for all tools
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_schema_version() -> String {
    "https://json-schema.org/draft/next/schema".to_string()
}

fn default_namespace() -> String {
    "system.v1".to_string()
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// MCP state plugin
pub struct McpStatePlugin {
    /// State store for execution tracking
    state_store: Arc<dyn StateStore>,
    /// Configuration file path
    config_path: String,
}

impl McpStatePlugin {
    pub fn new(state_store: Arc<dyn StateStore>, config_path: impl Into<String>) -> Self {
        Self {
            state_store,
            config_path: config_path.into(),
        }
    }

    /// Load current MCP configuration from file
    async fn load_config(&self) -> Result<McpConfig> {
        let content = tokio::fs::read_to_string(&self.config_path).await;

        match content {
            Ok(c) => {
                let mut c_mut = c;
                unsafe { simd_json::from_str(&mut c_mut) }.context("Failed to parse MCP config")
            }
            Err(_) => {
                // Return default config with requested agents auto-loaded
                let mut servers = HashMap::new();

                // simple, flat list of agents to auto-load
                let agents = vec![
                    "rust-pro",
                    "backend-architect",
                    "network-engineer",
                    "context-manager",
                    "memory",
                    "sequential-thinking",
                ];

                for agent in agents {
                    servers.insert(
                        agent.to_string(),
                        McpServerConfig {
                            command: "dbus-agent".to_string(),
                            args: Some(vec![agent.to_string()]),
                            env: None,
                            enabled: true,
                            transport: "stdio".to_string(),
                        },
                    );
                }

                Ok(McpConfig {
                    servers: Some(servers),
                    tool_groups: Some(ToolGroupsConfig {
                        enabled: vec!["default".to_string()],
                        max_tools: default_max_tools(),
                        access_zone: Some("local".to_string()),
                        trusted_networks: None,
                    }),
                    compact_mode: Some(CompactModeConfig {
                        enabled: true,
                        meta_tools: default_meta_tools(),
                    }),
                })
            }
        }
    }

    /// Save MCP configuration to file
    async fn save_config(&self, config: &McpConfig) -> Result<()> {
        let content = simd_json::to_string_pretty(config)?;
        tokio::fs::write(&self.config_path, content)
            .await
            .context("Failed to write MCP config file")
    }

    /// Apply server configuration changes
    async fn apply_server_config(&self, server_name: &str, config: &McpServerConfig) -> Result<()> {
        // Create execution job for state tracking
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            tool_name: format!("mcp:configure_server:{}", server_name),
            arguments: simd_json::serde::to_owned_value(config)?,
            status: ExecutionStatus::Running,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            result: None,
        };

        // Save job to state store
        self.state_store.save_job(&job).await?;

        // Load current config
        let mut current = self.load_config().await.unwrap_or_else(|_| McpConfig {
            servers: Some(HashMap::new()),
            tool_groups: None,
            compact_mode: None,
        });

        // Update server config
        let servers = current.servers.get_or_insert_with(HashMap::new);
        servers.insert(server_name.to_string(), config.clone());

        // Save updated config
        self.save_config(&current).await?;

        // Update job status
        let mut job = job;
        job.status = ExecutionStatus::Completed;
        job.updated_at = chrono::Utc::now();
        job.result = Some(op_state_store::ExecutionResult {
            success: true,
            output: Some(simd_json::serde::to_owned_value(
                "Server configured successfully",
            )?),
            error: None,
        });
        self.state_store.update_job(&job).await?;

        log::info!("Configured MCP server: {}", server_name);
        Ok(())
    }

    /// Apply tool groups configuration
    async fn apply_tool_groups_config(&self, config: &ToolGroupsConfig) -> Result<()> {
        // Create execution job
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            tool_name: "mcp:configure_tool_groups".to_string(),
            arguments: simd_json::serde::to_owned_value(config)?,
            status: ExecutionStatus::Running,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            result: None,
        };

        self.state_store.save_job(&job).await?;

        // Load current config
        let mut current = self.load_config().await.unwrap_or_else(|_| McpConfig {
            servers: None,
            tool_groups: Some(config.clone()),
            compact_mode: None,
        });

        // Update tool groups
        current.tool_groups = Some(config.clone());

        // Save updated config
        self.save_config(&current).await?;

        // Update job status
        let mut job = job;
        job.status = ExecutionStatus::Completed;
        job.updated_at = chrono::Utc::now();
        job.result = Some(op_state_store::ExecutionResult {
            success: true,
            output: Some(simd_json::serde::to_owned_value(
                "Tool groups configured successfully",
            )?),
            error: None,
        });
        self.state_store.update_job(&job).await?;

        log::info!("Configured tool groups: {:?}", config.enabled);
        Ok(())
    }
}

#[async_trait]
impl StatePlugin for McpStatePlugin {
    fn name(&self) -> &str {
        "mcp"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let config = self.load_config().await.unwrap_or_else(|_| McpConfig {
            servers: None,
            tool_groups: None,
            compact_mode: None,
        });

        Ok(simd_json::serde::to_owned_value(config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: McpConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: McpConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        // Check server changes
        if let Some(desired_servers) = &desired_config.servers {
            for (server_name, desired_server) in desired_servers {
                let current_server = current_config
                    .servers
                    .as_ref()
                    .and_then(|s| s.get(server_name));

                if current_server != Some(desired_server) {
                    actions.push(StateAction::Modify {
                        resource: format!("server:{}", server_name),
                        changes: simd_json::serde::to_owned_value(desired_server)?,
                    });
                }
            }
        }

        // Check tool groups changes
        if current_config.tool_groups != desired_config.tool_groups {
            if let Some(ref desired_groups) = desired_config.tool_groups {
                actions.push(StateAction::Modify {
                    resource: "tool_groups".to_string(),
                    changes: simd_json::serde::to_owned_value(desired_groups)?,
                });
            }
        }

        // Check compact mode changes
        if current_config.compact_mode != desired_config.compact_mode {
            if let Some(ref desired_compact) = desired_config.compact_mode {
                actions.push(StateAction::Modify {
                    resource: "compact_mode".to_string(),
                    changes: simd_json::serde::to_owned_value(desired_compact)?,
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let result = if resource.starts_with("server:") {
                    let server_name = resource.strip_prefix("server:").unwrap();
                    let server_config: McpServerConfig =
                        simd_json::serde::from_owned_value(changes.clone())?;
                    self.apply_server_config(server_name, &server_config).await
                } else if resource == "tool_groups" {
                    let groups_config: ToolGroupsConfig =
                        simd_json::serde::from_owned_value(changes.clone())?;
                    self.apply_tool_groups_config(&groups_config).await
                } else if resource == "compact_mode" {
                    // Compact mode changes don't require action - just config update
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Unknown resource: {}", resource))
                };

                match result {
                    Ok(_) => {
                        changes_applied.push(format!("Applied MCP config for: {}", resource));
                    }
                    Err(e) => {
                        errors.push(format!("Failed to apply config for {}: {}", resource, e));
                    }
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let current_config: McpConfig = simd_json::serde::from_owned_value(current)?;
        let desired_config: McpConfig = simd_json::serde::from_owned_value(desired.clone())?;

        Ok(current_config == desired_config)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("mcp-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_config: McpConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        self.save_config(&old_config).await?;
        log::info!("Rolled back MCP config to checkpoint: {}", checkpoint.id);
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // File writes are not atomic
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::SqliteStore;

    #[tokio::test]
    async fn test_mcp_plugin_state_tracking() {
        // Create in-memory state store
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let config_path = format!("/tmp/test-mcp-config-{}.json", Uuid::new_v4());
        let plugin = McpStatePlugin::new(store.clone(), &config_path);

        // Ensure file doesn't exist
        let _ = tokio::fs::remove_file(&config_path).await;

        // Create a test config
        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: "test-command".to_string(),
                args: Some(vec!["arg1".to_string()]),
                env: None,
                enabled: true,
                transport: "stdio".to_string(),
            },
        );

        let config = McpConfig {
            servers: Some(servers),
            tool_groups: None,
            compact_mode: None,
        };

        // Apply config (this should create execution jobs in state store)
        let desired = simd_json::serde::to_owned_value(&config).unwrap();
        let current = plugin.query_current_state().await.unwrap();
        let diff = plugin.calculate_diff(&current, &desired).await.unwrap();
        let result = plugin.apply_state(&diff).await.unwrap();

        assert!(result.success);
        assert!(!result.changes_applied.is_empty());

        // Clean up
        let _ = tokio::fs::remove_file(&config_path).await;
    }
}
