use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// Endpoint declaration state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.endpoint.schema@v1"))]
pub struct EndpointState {
    /// Declared endpoints.
    #[serde(default)]
    #[schemars(
        description = "Declared endpoints",
        example = &["192.168.1.100:8080"],
        extend("x-oscal-subid" = "exp.service.endpoint.endpoints.render@v1")
    )]
    pub endpoints: Vec<String>,
}

pub struct EndpointPlugin;

impl Default for EndpointPlugin {
    fn default() -> Self {
        Self
    }
}

impl EndpointPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for EndpointPlugin {
    fn name(&self) -> &str {
        "endpoint"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(endpoint_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(EndpointState {
            endpoints: vec![],
        })?)
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

/// Derived `endpoint` schema from the `EndpointState` struct.
pub fn endpoint_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(EndpointState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "endpoint",
        "1.0.0",
        "Endpoint configuration",
        &root,
    )
}

/// Frozen golden reference: the original hand-rolled schema, kept **test-only**
/// so `derived_schema_matches_hand_rolled` can prove the derived schema still
/// matches the contract this plugin shipped with. Production uses
/// [`endpoint_schema`].
#[cfg(test)]
pub(crate) fn endpoint_schema_golden() -> PluginSchema {
    PluginSchema::builder("endpoint")
        .version("1.0.0")
        .description("Endpoint configuration")
        .subid("__schema__", "sch.software.plugin.endpoint.schema@v1")
        .field(
            "endpoints",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Declared endpoints".to_string(),
                default: Some(json!([])),
                example: Some(json!(["192.168.1.100:8080"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .subid("endpoints", "exp.service.endpoint.endpoints.render@v1")
        .example(json!({
            "endpoints": ["192.168.1.100:8080"]
        }))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schemars-derived schema must match the hand-rolled golden reference.
    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = endpoint_schema_golden();
        let derived = endpoint_schema();
        let diffs = crate::state_plugins::schemars_adapter::schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    /// Every `x-oscal-subid` annotation in the derived schema must be a valid
    /// OSCAL subid according to the canonical taxonomy.
    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(EndpointState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            crate::state_plugins::common::oscal::validate_subid(&subid)
                .unwrap_or_else(|e| panic!("invalid subid {subid}: {e}"));
        }
    }

    fn collect_subids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let serde_json::Value::Object(map) = value {
            if let Some(subid) = map.get("x-oscal-subid").and_then(|v| v.as_str()) {
                out.push(subid.to_string());
            }
            for v in map.values() {
                collect_subids(v, out);
            }
        } else if let serde_json::Value::Array(arr) = value {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }
}
