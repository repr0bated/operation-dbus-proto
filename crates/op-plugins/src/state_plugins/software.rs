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
pub struct SoftwareState {
    #[serde(default)]
    pub packages: Vec<PackageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub manager: String, // "dpkg", "rpm", "cargo", etc.
}

/// Schema-only state for the `software` plugin. The runtime state is fully typed
/// (`SoftwareState`), but the published schema contract preserves the original
/// opaque `packages` field so the migration does not change the downstream UI.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.software.schema@v1"))]
pub struct SoftwareSchemaState {
    #[schemars(
        description = "Package list",
        extend("default" = serde_json::json!([]), "x-oscal-subid" = "obs.software.plugin.software.packages@v1")
    )]
    pub packages: JsonValue,
}

pub struct SoftwarePlugin;

impl Default for SoftwarePlugin {
    fn default() -> Self {
        Self
    }
}

impl SoftwarePlugin {
    pub fn new() -> Self {
        Self
    }

    async fn scan_dpkg() -> Vec<PackageInfo> {
        let mut packages = Vec::new();
        let content = tokio::fs::read_to_string("/var/lib/dpkg/status")
            .await
            .unwrap_or_default();

        let mut current_name = None;
        let mut current_version = None;

        for line in content.lines() {
            if let Some(stripped) = line.strip_prefix("Package: ") {
                current_name = Some(stripped.trim().to_string());
            } else if let Some(stripped) = line.strip_prefix("Version: ") {
                current_version = Some(stripped.trim().to_string());
            } else if line.is_empty() {
                if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                    packages.push(PackageInfo {
                        name,
                        version,
                        manager: "dpkg".to_string(),
                    });
                }
            }
        }
        // Handle last stanza if file doesn't end with blank line
        if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
            packages.push(PackageInfo {
                name,
                version,
                manager: "dpkg".to_string(),
            });
        }
        packages
    }
}

#[async_trait]
impl StatePlugin for SoftwarePlugin {
    fn name(&self) -> &str {
        "software"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(software_schema())
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

pub(crate) fn software_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(SoftwareSchemaState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "software",
        "1.0.0",
        "Software package inventory",
        &root,
    )
}

/// Frozen golden reference: the original hand-rolled `software` schema, kept
/// test-only so the schemars-derived contract can be verified against drift.
#[cfg(test)]
pub(crate) fn software_schema_golden() -> PluginSchema {
    PluginSchema::builder("software")
        .version("1.0.0")
        .description("Software package inventory")
        .subid("__schema__", "sch.software.plugin.software.schema@v1")
        .field(
            "packages",
            FieldSchema {
                field_type: FieldType::Any,
                required: true,
                description: "Package list".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .subid("packages", "obs.software.plugin.software.packages@v1")
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let diffs = crate::state_plugins::schemars_adapter::schema_diffs(
            &software_schema_golden(),
            &software_schema(),
        );
        assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(SoftwareSchemaState)).unwrap();
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
    crate::default_registry::PluginReg::new("software", |_ctx| std::sync::Arc::new(SoftwarePlugin::new()))
}
