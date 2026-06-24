use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessDeclState {
    #[serde(default)]
    pub sessions: Vec<SessionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionConfig {
    pub id: String,
    pub user: String,
}

/// Schema-only state for the `sess_decl` plugin. The runtime state is fully
/// typed (`SessDeclState`), but the published schema preserves the original
/// opaque `sessions` field to avoid changing the downstream contract.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.sessdecl.schema@v1"))]
pub struct SessDeclSchemaState {
    #[schemars(
        description = "Session declarations",
        extend("default" = serde_json::json!([]), "x-oscal-subid" = "obs.software.plugin.sessdecl.sessions@v1")
    )]
    pub sessions: JsonValue,
}

pub struct SessDeclPlugin;

impl Default for SessDeclPlugin {
    fn default() -> Self {
        Self
    }
}

impl SessDeclPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for SessDeclPlugin {
    fn name(&self) -> &str {
        "sess_decl"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(sess_decl_schema())
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

pub(crate) fn sess_decl_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(SessDeclSchemaState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "sess_decl",
        "1.0.0",
        "Session declaration state",
        &root,
    )
}

/// Frozen golden reference for the `sess_decl` schema.
#[cfg(test)]
pub(crate) fn sess_decl_schema_golden() -> PluginSchema {
    PluginSchema::builder("sess_decl")
        .version("1.0.0")
        .description("Session declaration state")
        .dependency("users")
        .subid("__schema__", "sch.software.plugin.sessdecl.schema@v1")
        .field(
            "sessions",
            FieldSchema {
                field_type: FieldType::Any,
                required: true,
                description: "Session declarations".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .subid("sessions", "obs.software.plugin.sessdecl.sessions@v1")
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let diffs = crate::state_plugins::schemars_adapter::schema_diffs(
            &sess_decl_schema_golden(),
            &sess_decl_schema(),
        );
        assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(SessDeclSchemaState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        for subid in subids {
            assert!(
                crate::state_plugins::common::oscal::validate_subid(&subid).is_ok(),
                "invalid subid: {subid}"
            );
        }
    }

    fn collect_subids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(subid) = obj.get("x-oscal-subid").and_then(|v| v.as_str()) {
                out.push(subid.to_string());
            }
            for v in obj.values() {
                collect_subids(v, out);
            }
        }
        if let Some(arr) = value.as_array() {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("sess_decl", |_ctx| std::sync::Arc::new(SessDeclPlugin::new()))
}
