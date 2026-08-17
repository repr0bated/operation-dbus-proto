use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Agent configuration plugin state schema.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.agent-config.schema@v1"))]
#[schemars(extend("x-oscal-category" = "agents"))]
pub struct AgentConfigState {
    /// List of agent configurations.
    #[serde(default)]
    #[schemars(
        description = "List of agent configurations",
        extend("x-oscal-subid" = "exp.software.plugin.agent-config.agents.render@v1")
    )]
    pub agents: Vec<AgentConfig>,
}

/// Individual agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AgentConfig {
    /// Agent name.
    #[schemars(
        description = "Agent name",
        extend("x-oscal-subid" = "exp.software.agent-config.agent.name.render@v1")
    )]
    pub name: String,
    /// Whether the agent is enabled.
    #[schemars(
        description = "Whether the agent is enabled",
        extend("x-oscal-subid" = "exp.software.agent-config.agent.enabled.render@v1")
    )]
    pub enabled: bool,
    /// Default model override.
    #[schemars(
        description = "Default model override",
        extend("x-oscal-subid" = "exp.software.agent-config.agent.model.render@v1")
    )]
    pub model: Option<String>,
    /// Enabled tool names.
    #[serde(default)]
    #[schemars(
        description = "Enabled tool names",
        extend("x-oscal-subid" = "exp.software.agent-config.agent.tools.render@v1")
    )]
    pub tools: Vec<String>,
}

pub struct AgentConfigPlugin;

impl Default for AgentConfigPlugin {
    fn default() -> Self {
        Self
    }
}

impl AgentConfigPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for AgentConfigPlugin {
    fn name(&self) -> &str {
        "agent_config"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(agent_config_schema())
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

/// Canonical `agent_config` schema, derived from the structs via schemars.
pub fn agent_config_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(AgentConfigState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "agent_config",
        "1.0.0",
        "Agent configuration and tool assignments",
        &root,
    );

    /// Output for GetConfig method
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetConfigOutput {
        pub config: serde_json::Value,
    }

    /// Output for ListTools method
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListToolsOutput {
        pub tools: Vec<String>,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<(), GetConfigOutput>(
            "get_config",
            SideEffect::Read,
            true,
            "agent.read",
            "obs.service.agent.config.get@v1",
        ),
    );
    schema.methods.insert(
        "update_config".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "update_config",
            SideEffect::Mutation,
            false,
            "agent.invoke",
            "mut.service.agent.config.update@v1",
        ),
    );
    schema.methods.insert(
        "list_tools".to_string(),
        method_decl_from_schemars_with_output::<(), ListToolsOutput>(
            "list_tools",
            SideEffect::Read,
            true,
            "agent.read",
            "obs.service.agent.tool.list@v1",
        ),
    );
    schema.methods.insert(
        "register_tool".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "register_tool",
            SideEffect::Mutation,
            false,
            "agent.invoke",
            "mut.service.agent.tool.register@v1",
        ),
    );
    schema.methods.insert(
        "reset_config".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "reset_config",
            SideEffect::Mutation,
            true,
            "agent.invoke",
            "mut.service.agent.config.reset@v1",
        ),
    );

    schema.capabilities.insert(
        "agent.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "agent.read".to_string(),
            description: "Grants: get_config, list_tools.".to_string(),
        },
    );
    schema.capabilities.insert(
        "agent.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "agent.invoke".to_string(),
            description: "Grants: update_config, register_tool, reset_config.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;

    #[test]
    fn schema_is_schemars_seeded_and_typed() {
        let schema = agent_config_schema();
        assert_eq!(schema.name, "agent_config");
        assert_eq!(schema.version, "1.0.0");
        assert_eq!(
            schema.description,
            "Agent configuration and tool assignments"
        );
        assert!(schema.fields.contains_key("agents"));
        assert!(schema.methods.contains_key("get_config"));
        assert!(schema.methods.contains_key("update_config"));
        assert!(schema.methods.contains_key("list_tools"));
        assert!(schema.methods.contains_key("register_tool"));
        assert!(schema.methods.contains_key("reset_config"));
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(AgentConfigState)).unwrap();
        let mut stack = vec![&raw];
        while let Some(node) = stack.pop() {
            if let Some(subid) = node.get("x-oscal-subid").and_then(|v| v.as_str()) {
                validate_subid(subid).expect("invalid subid");
            }
            if let Some(props) = node.get("properties").and_then(|v| v.as_object()) {
                for (_, v) in props {
                    stack.push(v);
                }
            }
            if let Some(defs) = node
                .get("$defs")
                .or_else(|| node.get("definitions"))
                .and_then(|v| v.as_object())
            {
                for (_, v) in defs {
                    stack.push(v);
                }
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("agent_config", |_ctx| std::sync::Arc::new(AgentConfigPlugin::new()))
}
