use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Agent configuration plugin state schema.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.agent-config.schema@v1"))]
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
    super::schemars_adapter::plugin_schema_from_json(
        "agent_config",
        "1.0.0",
        "Agent configuration and tool assignments",
        &root,
    )
}

/// Frozen golden reference: the original hand-rolled schema, kept test-only so
/// `derived_schema_matches_hand_rolled` can prove the derived schema still
/// matches the contract this plugin shipped with.
#[cfg(test)]
pub(crate) fn agent_config_schema_golden() -> PluginSchema {
    let mut agent_fields = HashMap::new();
    agent_fields.insert(
        "name".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Agent name".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    agent_fields.insert(
        "enabled".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: true,
            description: "Whether the agent is enabled".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    agent_fields.insert(
        "model".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Default model override".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    agent_fields.insert(
        "tools".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Enabled tool names".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    PluginSchema::builder("agent_config")
        .version("1.0.0")
        .description("Agent configuration and tool assignments")
        .subid("__schema__", "sch.software.plugin.agent-config.schema@v1")
        .field(
            "agents",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(agent_fields))),
                required: false,
                description: "List of agent configurations".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .subid(
            "agents",
            "exp.software.plugin.agent-config.agents.render@v1",
        )
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = agent_config_schema_golden();
        let derived = agent_config_schema();
        let diffs = super::super::schemars_adapter::schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
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
