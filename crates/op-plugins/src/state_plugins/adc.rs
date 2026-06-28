use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// ADC plugin state schema.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.adc.schema@v1"))]
pub struct AdcState {
    /// Whether ADC is configured.
    #[schemars(
        description = "Whether ADC is configured",
        extend("x-oscal-subid" = "exp.software.plugin.adc.configured.render@v1")
    )]
    pub configured: bool,
}

pub struct AdcPlugin;

impl Default for AdcPlugin {
    fn default() -> Self {
        Self
    }
}

impl AdcPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for AdcPlugin {
    fn name(&self) -> &str {
        "adc"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(adc_schema())
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
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

/// Canonical `adc` schema, derived from the structs via schemars.
pub fn adc_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(AdcState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "adc",
        "1.0.0",
        "Application default credentials state",
        &root,
    )
}

/// Frozen golden reference: the original hand-rolled schema, kept test-only so
/// `derived_schema_matches_hand_rolled` can prove the derived schema still
/// matches the contract this plugin shipped with.
#[cfg(test)]
pub(crate) fn adc_schema_golden() -> PluginSchema {
    PluginSchema::builder("adc")
        .version("1.0.0")
        .description("Application default credentials state")
        .subid("__schema__", "sch.software.plugin.adc.schema@v1")
        .field(
            "configured",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether ADC is configured".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .subid("configured", "exp.software.plugin.adc.configured.render@v1")
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = adc_schema_golden();
        let derived = adc_schema();
        let diffs = super::super::schemars_adapter::schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(AdcState)).unwrap();
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
    crate::default_registry::PluginReg::new("adc", |_ctx| std::sync::Arc::new(AdcPlugin::new()))
}
