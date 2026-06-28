use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Keypair declaration state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.keypair.schema@v1"))]
pub struct KeypairState {
    /// Managed keypairs.
    #[serde(default)]
    #[schemars(
        description = "Managed keypairs",
        extend("x-oscal-subid" = "exp.service.keypair.keypairs.render@v1")
    )]
    pub keypairs: Vec<Keypair>,
}

/// A single SSH keypair.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Keypair {
    /// Keypair name.
    #[schemars(
        description = "Keypair name",
        example = &"id_ed25519",
        extend("x-oscal-subid" = "exp.service.keypair.name.declare@v1")
    )]
    pub name: String,

    /// Public-key algorithm.
    #[schemars(
        description = "Public-key algorithm",
        example = &"ssh-ed25519",
        extend("x-oscal-subid" = "exp.service.keypair.algorithm.declare@v1")
    )]
    pub algorithm: String,

    /// Public key material.
    #[serde(default)]
    #[schemars(
        description = "Public key material",
        extend("x-oscal-subid" = "exp.service.keypair.public-key.declare@v1")
    )]
    pub public_key: Option<String>,

    /// Whether the keypair is present on the system.
    #[serde(default)]
    #[schemars(
        description = "Whether the keypair is present on the system",
        extend("x-oscal-subid" = "obs.service.keypair.present.detect@v1")
    )]
    pub present: bool,
}

pub struct KeypairPlugin;

impl Default for KeypairPlugin {
    fn default() -> Self {
        Self
    }
}

impl KeypairPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for KeypairPlugin {
    fn name(&self) -> &str {
        "keypair"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(keypair_schema())
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

/// Derived `keypair` schema from the `KeypairState` struct.
pub fn keypair_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(KeypairState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "keypair",
        "1.0.0",
        "Keypair declaration state",
        &root,
    );
    let method = super::plugin_schema_defs::cap_method(
        "list_keypairs",
        op_state_store::SideEffect::Read,
        true,
        "cap.service.keypair.list@v1",
        "obs.service.keypair.list@v1",
    );
    schema.methods.insert(method.name.clone(), method);
    schema
}

/// Frozen golden reference: the original hand-rolled schema, kept **test-only**
/// so `derived_schema_matches_hand_rolled` can prove the derived schema still
/// matches the contract this plugin shipped with. Production uses
/// [`keypair_schema`].
#[cfg(test)]
pub(crate) fn keypair_schema_golden() -> PluginSchema {
    let mut keypair_fields = HashMap::new();
    keypair_fields.insert(
        "name".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Keypair name".to_string(),
            default: None,
            example: Some(json!("id_ed25519")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    keypair_fields.insert(
        "algorithm".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Public-key algorithm".to_string(),
            default: None,
            example: Some(json!("ssh-ed25519")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    keypair_fields.insert(
        "public_key".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Public key material".to_string(),
            default: Some(json!(null)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    keypair_fields.insert(
        "present".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the keypair is present on the system".to_string(),
            default: Some(json!(false)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    PluginSchema::builder("keypair")
        .version("1.0.0")
        .description("Keypair declaration state")
        .subid("__schema__", "sch.software.plugin.keypair.schema@v1")
        .field(
            "keypairs",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(keypair_fields))),
                required: false,
                description: "Managed keypairs".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .subid("keypairs", "exp.service.keypair.keypairs.render@v1")
        .example(json!({
            "keypairs": [
                {
                    "name": "id_ed25519",
                    "algorithm": "ssh-ed25519",
                    "public_key": "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...",
                    "present": true
                }
            ]
        }))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schemars-derived schema must match the hand-rolled golden reference.
    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = keypair_schema_golden();
        let derived = keypair_schema();
        let diffs = crate::state_plugins::schemars_adapter::schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    /// Every `x-oscal-subid` annotation in the derived schema must be a valid
    /// OSCAL subid according to the canonical taxonomy.
    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(KeypairState)).unwrap();
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

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("keypair", |_ctx| std::sync::Arc::new(KeypairPlugin::new()))
}
