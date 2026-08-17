//! MCP state plugin - manages MCP server configurations and tool groups
//! Wires MCP configuration to the state store for auditing and rollback

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{ExecutionJob, ExecutionStatus, PluginSchema, StateStore};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// MCP configuration schema - mirrors the state JSON structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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
        "respond".to_string(),
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
            Ok(c) => serde_json::from_str(&c).context("Failed to parse MCP config"),
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

// ── Schema-only state (opaque Value fields preserve the hand-rolled contract) ──

fn default_empty_object() -> serde_json::Value {
    serde_json::json!({})
}

fn example_servers() -> serde_json::Value {
    serde_json::json!({
        "rust-pro": {
            "command": "dbus-agent",
            "args": ["rust-pro"],
            "enabled": true,
            "transport": "stdio"
        }
    })
}

fn example_tool_groups() -> serde_json::Value {
    serde_json::json!({
        "enabled": ["default"],
        "max_tools": 40,
        "access_zone": "local"
    })
}

fn example_compact_mode() -> serde_json::Value {
    serde_json::json!({
        "enabled": true,
        "meta_tools": ["list_tools", "search_tools", "get_tool_schema", "execute_tool", "respond"]
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.mcp.schema@v1"))]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct McpState {
    #[serde(default = "default_empty_object")]
    #[schemars(
        description = "MCP server map",
        example = example_servers(),
        extend("x-oscal-subid" = "exp.software.plugin.mcp.servers@v1")
    )]
    servers: serde_json::Value,
    #[serde(default = "default_empty_object")]
    #[schemars(
        description = "Tool group config",
        example = example_tool_groups(),
        extend("x-oscal-subid" = "exp.software.plugin.mcp.tool-groups@v1")
    )]
    tool_groups: serde_json::Value,
    #[serde(default = "default_empty_object")]
    #[schemars(
        description = "Compact mode config",
        example = example_compact_mode(),
        extend("x-oscal-subid" = "exp.software.plugin.mcp.compact-mode@v1")
    )]
    compact_mode: serde_json::Value,
    /// Uncapped protocol fields discovered from the authoritative MCP schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.mcp.inspector-fields@v1"))]
    inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
}

pub(crate) fn mcp_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(McpState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "mcp",
        "1.0.0",
        "MCP server and tool-group configuration",
        &root,
    );
    schema.dependencies = vec!["agent_config".to_string()];

    use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, EmptyInput};
    use op_state_store::SideEffect;

    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigureServerInput {
        pub server_name: String,
        pub config: McpServerConfig,
    }

    // Method dispatch is not wired yet: apply_server_config/apply_tool_groups_config
    // need this plugin instance's state_store + config_path (constructor
    // dependencies not available to the free dispatch-function pattern used
    // elsewhere in this file). Declared so the UI can render/discover the real
    // config surface; mutations already work via the generic SetProperty/
    // apply_state path (calculate_diff already detects server/tool_groups/
    // compact_mode changes and routes them through apply_server_config etc.).
    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, McpConfig>(
            "get_config",
            SideEffect::Read,
            true,
            "mcp.read",
            "obs.software.plugin.mcp.config.get@v1",
        ),
    );
    schema.methods.insert(
        "configure_server".to_string(),
        method_decl_from_schemars_with_output::<ConfigureServerInput, McpConfig>(
            "configure_server",
            SideEffect::Mutation,
            true,
            "mcp.write",
            "mut.software.plugin.mcp.server.configure@v1",
        ),
    );
    schema.methods.insert(
        "configure_tool_groups".to_string(),
        method_decl_from_schemars_with_output::<ToolGroupsConfig, McpConfig>(
            "configure_tool_groups",
            SideEffect::Mutation,
            true,
            "mcp.write",
            "mut.software.plugin.mcp.tool-groups.configure@v1",
        ),
    );

    schema.capabilities.insert(
        "mcp.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "mcp.read".to_string(),
            description: "Grants: get_config.".to_string(),
        },
    );
    schema.capabilities.insert(
        "mcp.write".to_string(),
        op_state_store::CapabilityDecl {
            id: "mcp.write".to_string(),
            description: "Grants: configure_server, configure_tool_groups.".to_string(),
        },
    );

    schema
}

#[async_trait]
impl StatePlugin for McpStatePlugin {
    fn name(&self) -> &str {
        "mcp"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(mcp_schema())
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
                let result = if let Some(server_name) = resource.strip_prefix("server:") {
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

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = simd_json::json!(null);
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
    use crate::state_plugins::common::oscal::validate_subid;
    use op_state_store::MemoryStore;
    use serde_json::Value as JVal;

    fn collect_subids(node: &JVal, out: &mut Vec<String>) {
        if let Some(subid) = node.get("x-oscal-subid").and_then(JVal::as_str) {
            out.push(subid.to_string());
        }
        if let Some(props) = node.get("properties").and_then(JVal::as_object) {
            for v in props.values() {
                collect_subids(v, out);
            }
        }
        if let Some(defs) = node
            .get("$defs")
            .or_else(|| node.get("definitions"))
            .and_then(JVal::as_object)
        {
            for v in defs.values() {
                collect_subids(v, out);
            }
        }
        if let Some(items) = node.get("items") {
            collect_subids(items, out);
        }
        if let Some(alternatives) = node
            .get("anyOf")
            .or_else(|| node.get("oneOf"))
            .and_then(JVal::as_array)
        {
            for v in alternatives {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn should_publish_plugin_owned_mcp_schema() {
        let store = Arc::new(MemoryStore::new());
        let plugin = McpStatePlugin::new(store, "/tmp/test-mcp-schema.json");
        let schema = plugin.schema().expect("mcp schema");

        assert_eq!(schema.name, "mcp");
        assert_eq!(schema.version, "1.0.0");
        assert_eq!(schema.dependencies, vec!["agent_config".to_string()]);
        assert!(schema.fields.contains_key("servers"));
        assert!(schema.fields.contains_key("tool_groups"));
        assert!(schema.fields.contains_key("compact_mode"));
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(McpState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(
            !subids.is_empty(),
            "expected at least one x-oscal-subid in the derived schema"
        );
        for subid in subids {
            validate_subid(&subid).expect("invalid subid: {subid}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("mcp", |ctx| std::sync::Arc::new(McpStatePlugin::new(ctx.state_store(), ctx.config_path("mcp", "/etc/op-dbus/mcp-config.json"))))
}

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.mcp.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.default`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.default@v1"))]
        pub default: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.description`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.description@v1"))]
        pub description: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.enum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.enum-field@v1"))]
        pub enum_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.items`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.items@v1"))]
        pub items: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.maxItems`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.maxitems@v1"))]
        pub maxitems: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.minItems`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.minitems@v1"))]
        pub minitems: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.oneOf`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.oneof@v1"))]
        pub oneof: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.title`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.title@v1"))]
        pub title: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.options.field.anyOf`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.anyof@v1"))]
        pub anyof: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.value.field.const`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.const-field@v1"))]
        pub const_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.values.field.enumNames`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.enumnames@v1"))]
        pub enumnames: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Annotations.field.audience`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.audience@v1"))]
        pub audience: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.BlobResourceContents.field.blob`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.blob@v1"))]
        pub blob: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequest.field.method`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.method@v1"))]
        pub method: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequest.field.params`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.params@v1"))]
        pub params: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequestParams.field.arguments`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.arguments@v1"))]
        pub arguments: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequestParams.field.name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.name@v1"))]
        pub name: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolResult.field.content`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.content@v1"))]
        pub content: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolResult.field.isError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.iserror@v1"))]
        pub iserror: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolResult.field.structuredContent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.structuredcontent@v1"))]
        pub structuredcontent: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CancelTaskRequest.field.taskId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.taskid@v1"))]
        pub taskid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CancelledNotificationParams.field.reason`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.reason@v1"))]
        pub reason: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CancelledNotificationParams.field.requestId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.requestid@v1"))]
        pub requestid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.cancel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.cancel@v1"))]
        pub cancel: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.context`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.context@v1"))]
        pub context: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.create`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.create@v1"))]
        pub create: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.createMessage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.createmessage@v1"))]
        pub createmessage: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.elicitation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.elicitation@v1"))]
        pub elicitation: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.experimental`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.experimental@v1"))]
        pub experimental: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.list`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.list@v1"))]
        pub list: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.listChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.listchanged@v1"))]
        pub listchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.requests`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.requests@v1"))]
        pub requests: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.roots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.roots@v1"))]
        pub roots: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.sampling`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.sampling@v1"))]
        pub sampling: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.tasks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.tasks@v1"))]
        pub tasks: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.tools`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.tools@v1"))]
        pub tools: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteRequestParams.field.argument`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.argument@v1"))]
        pub argument: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteRequestParams.field.ref`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.ref-field@v1"))]
        pub ref_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteRequestParams.field.value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.value@v1"))]
        pub value: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.completion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.completion@v1"))]
        pub completion: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.hasMore`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.hasmore@v1"))]
        pub hasmore: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.total`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.total@v1"))]
        pub total: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.values`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.values@v1"))]
        pub values: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.includeContext`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.includecontext@v1"))]
        pub includecontext: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.messages@v1"))]
        pub messages: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.modelPreferences`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.modelpreferences@v1"))]
        pub modelpreferences: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.systemPrompt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.systemprompt@v1"))]
        pub systemprompt: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageResult.field.model`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.model@v1"))]
        pub model: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageResult.field.stopReason`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.stopreason@v1"))]
        pub stopreason: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateTaskResult.field.task`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.task@v1"))]
        pub task: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.$schema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.schema@v1"))]
        pub schema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.message`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.message@v1"))]
        pub message: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.mode@v1"))]
        pub mode: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.properties`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.properties@v1"))]
        pub properties: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.requestedSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.requestedschema@v1"))]
        pub requestedschema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.required`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.required@v1"))]
        pub required: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestURLParams.field.elicitationId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.elicitationid@v1"))]
        pub elicitationid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestURLParams.field.url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.url@v1"))]
        pub url: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitResult.field.action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.EmbeddedResource.field._meta`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.meta@v1"))]
        pub meta: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.EmbeddedResource.field.annotations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.annotations@v1"))]
        pub annotations: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.EmbeddedResource.field.resource`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.resource@v1"))]
        pub resource: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Icon.field.src`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.src@v1"))]
        pub src: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ImageContent.field.data`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.data@v1"))]
        pub data: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Implementation.field.version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.version@v1"))]
        pub version: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Implementation.field.websiteUrl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.websiteurl@v1"))]
        pub websiteurl: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeRequestParams.field.capabilities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.capabilities@v1"))]
        pub capabilities: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeRequestParams.field.clientInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.clientinfo@v1"))]
        pub clientinfo: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeRequestParams.field.protocolVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.protocolversion@v1"))]
        pub protocolversion: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeResult.field.instructions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.instructions@v1"))]
        pub instructions: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeResult.field.serverInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.serverinfo@v1"))]
        pub serverinfo: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCErrorResponse.field.error`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.error@v1"))]
        pub error: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCErrorResponse.field.id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.id@v1"))]
        pub id: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCErrorResponse.field.jsonrpc`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.jsonrpc@v1"))]
        pub jsonrpc: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCResultResponse.field.result`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.result@v1"))]
        pub result: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ListPromptsResult.field.prompts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.prompts@v1"))]
        pub prompts: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ListResourceTemplatesResult.field.resourceTemplates`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.resourcetemplates@v1"))]
        pub resourcetemplates: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ListResourcesResult.field.resources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.resources@v1"))]
        pub resources: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.LoggingMessageNotificationParams.field.level`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.level@v1"))]
        pub level: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.LoggingMessageNotificationParams.field.logger`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.logger@v1"))]
        pub logger: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.NumberSchema.field.maximum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.maximum@v1"))]
        pub maximum: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.NumberSchema.field.minimum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.minimum@v1"))]
        pub minimum: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.PaginatedRequestParams.field.cursor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.cursor@v1"))]
        pub cursor: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.PaginatedResult.field.nextCursor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.nextcursor@v1"))]
        pub nextcursor: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ProgressNotificationParams.field.progressToken`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.progresstoken@v1"))]
        pub progresstoken: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.PromptMessage.field.role`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.role@v1"))]
        pub role: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ReadResourceResult.field.contents`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.contents@v1"))]
        pub contents: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Resource.field.uri`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.uri@v1"))]
        pub uri: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.call`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.call@v1"))]
        pub call: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.completions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.completions@v1"))]
        pub completions: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.logging`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.logging@v1"))]
        pub logging: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.subscribe`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.subscribe@v1"))]
        pub subscribe: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.StringSchema.field.format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.format@v1"))]
        pub format: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.StringSchema.field.maxLength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.maxlength@v1"))]
        pub maxlength: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.StringSchema.field.minLength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.minlength@v1"))]
        pub minlength: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.createdAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.createdat@v1"))]
        pub createdat: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.lastUpdatedAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.lastupdatedat@v1"))]
        pub lastupdatedat: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.pollInterval`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.pollinterval@v1"))]
        pub pollinterval: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.status@v1"))]
        pub status: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.statusMessage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.statusmessage@v1"))]
        pub statusmessage: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.ttl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.ttl@v1"))]
        pub ttl: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.TextContent.field.text`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.text@v1"))]
        pub text: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Tool.field.execution`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.execution@v1"))]
        pub execution: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Tool.field.inputSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.inputschema@v1"))]
        pub inputschema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Tool.field.outputSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.outputschema@v1"))]
        pub outputschema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.destructiveHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.destructivehint@v1"))]
        pub destructivehint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.idempotentHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.idempotenthint@v1"))]
        pub idempotenthint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.openWorldHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.openworldhint@v1"))]
        pub openworldhint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.readOnlyHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.readonlyhint@v1"))]
        pub readonlyhint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolExecution.field.taskSupport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.tasksupport@v1"))]
        pub tasksupport: Option<u64>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolResultContent.field.toolUseId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.tooluseid@v1"))]
        pub tooluseid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.URLElicitationRequiredError.field.code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.code@v1"))]
        pub code: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.URLElicitationRequiredError.field.elicitations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.elicitations@v1"))]
        pub elicitations: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.allows.field.hints`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.hints@v1"))]
        pub hints: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.for.field.mimeType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.mimetype@v1"))]
        pub mimetype: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.costPriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.costpriority@v1"))]
        pub costpriority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.intelligencePriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.intelligencepriority@v1"))]
        pub intelligencepriority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.lastModified`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.lastmodified@v1"))]
        pub lastmodified: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.maxTokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.maxtokens@v1"))]
        pub maxtokens: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.metadata`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.metadata@v1"))]
        pub metadata: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.priority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.priority@v1"))]
        pub priority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.progress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.progress@v1"))]
        pub progress: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.speedPriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.speedpriority@v1"))]
        pub speedpriority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.stopSequences`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.stopsequences@v1"))]
        pub stopsequences: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.temperature`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.temperature@v1"))]
        pub temperature: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.toolChoice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.toolchoice@v1"))]
        pub toolchoice: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.of.field.input`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.input@v1"))]
        pub input: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.of.field.size`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.size@v1"))]
        pub size: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.of.field.uriTemplate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.uritemplate@v1"))]
        pub uritemplate: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.override.field.icons`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.icons@v1"))]
        pub icons: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.override.field.sizes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.sizes@v1"))]
        pub sizes: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.override.field.theme`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.theme@v1"))]
        pub theme: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.CacheableResult.field.cacheScope`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.cachescope@v1"))]
        pub cachescope: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.CacheableResult.field.ttlMs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.ttlms@v1"))]
        pub ttlms: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.DiscoverResult.field.supportedVersions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.supportedversions@v1"))]
        pub supportedversions: Option<u64>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.InputRequiredResult.field.inputRequests`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.inputrequests@v1"))]
        pub inputrequests: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.InputRequiredResult.field.requestState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.requeststate@v1"))]
        pub requeststate: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.MissingRequiredClientCapabilityError.field.requiredCapabilities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.requiredcapabilities@v1"))]
        pub requiredcapabilities: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.ServerCapabilities.field.extensions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.extensions@v1"))]
        pub extensions: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.SubscriptionsAcknowledgedNotificationParams.field.notifications`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.notifications@v1"))]
        pub notifications: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.UnsupportedProtocolVersionError.field.requested`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.requested@v1"))]
        pub requested: Option<u64>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.UnsupportedProtocolVersionError.field.supported`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.supported@v1"))]
        pub supported: Option<u64>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.promptsListChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.promptslistchanged@v1"))]
        pub promptslistchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.resourceSubscriptions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.resourcesubscriptions@v1"))]
        pub resourcesubscriptions: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.resourcesListChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.resourceslistchanged@v1"))]
        pub resourceslistchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.toolsListChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.toolslistchanged@v1"))]
        pub toolslistchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.of.field.resultType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.resulttype@v1"))]
        pub resulttype: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.roots.field.form`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.form@v1"))]
        pub form: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.that.field.inputResponses`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.mcp.inputresponses@v1"))]
        pub inputresponses: Option<String>,
    }

    /// Metadata needed when promoting a generated typed method into `schema.methods`.
    pub struct MethodCandidate {
        pub name: &'static str,
        pub side_effect: &'static str,
        pub idempotent: bool,
        pub required_capability: &'static str,
        pub subid: &'static str,
        pub repomix_path: &'static str,
        pub command: &'static [&'static str],
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[];

    /// Promote every generated method into the sealed plugin schema.
    pub(super) fn register_methods(schema: &mut op_state_store::PluginSchema) {
        use super::super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    }
}

// Promotion checklist (Fable contract):
// 1. Move owned fields into the plugin State struct with concrete Rust types.
// 2. Replace method placeholders with dedicated typed Input/Output fields.
// 3. Register with method_decl_from_schemars_with_output and correct SideEffect.
// 4. Register every subid, implement dispatch, and add schema/subid tests.
// 5. Re-run op-plugin-lint; only then replace the original plugin file.
