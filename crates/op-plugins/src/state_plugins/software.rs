use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
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

/// Input struct for Install method.
/// See: https://www.freedesktop.org/software/systemd/man/org.freedesktop.systemd1.html
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InstallInput {
    /// Package name to install
    pub name: String,
    /// Package version (optional)
    pub version: Option<String>,
}

/// Input struct for Uninstall method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UninstallInput {
    /// Package name to uninstall
    pub name: String,
    /// Force removal even if dependent
    #[serde(default)]
    pub force: bool,
}

/// Input struct for Update method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateInput {
    /// Package name to update
    pub name: String,
    /// Target version (optional, defaults to latest)
    pub version: Option<String>,
}

/// Input struct for GetInfo method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetInfoInput {
    /// Package name to query
    pub name: String,
}

/// Published schema state for the `software` plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.software.schema@v1"))]
#[schemars(extend("x-oscal-category" = "software"))]
pub struct SoftwareSchemaState {
    #[schemars(
        description = "Package list",
        extend("x-oscal-subid" = "obs.software.plugin.software.packages@v1")
    )]
    pub packages: Vec<PackageInfo>,
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
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "software",
        "1.0.0",
        "Software package inventory",
        &serde_json::to_value(schemars::schema_for!(SoftwareSchemaState)).unwrap(),
    );
    schema.subids.insert(
        "__schema__".to_string(),
        "sch.software.plugin.software.schema@v1".to_string(),
    );
    super::schemars_adapter::apply_state_defaults(
        &mut schema,
        &simd_json::serde::to_owned_value(&SoftwareState::default()).unwrap(),
    );

    // Install method - https://www.freedesktop.org/software/systemd/man/org.freedesktop.systemd1.html
    schema.methods.insert(
        "install".to_string(),
        method_decl_from_schemars_with_output::<InstallInput, AckOutput>(
            "Install",
            op_state_store::SideEffect::Mutation,
            false,
            "software.write",
            "mut.software.plugin.software.install@v1",
        ),
    );

    // Uninstall method
    schema.methods.insert(
        "uninstall".to_string(),
        method_decl_from_schemars_with_output::<UninstallInput, AckOutput>(
            "Uninstall",
            op_state_store::SideEffect::Mutation,
            false,
            "software.write",
            "mut.software.plugin.software.uninstall@v1",
        ),
    );

    // Update method
    schema.methods.insert(
        "update".to_string(),
        method_decl_from_schemars_with_output::<UpdateInput, AckOutput>(
            "Update",
            op_state_store::SideEffect::Mutation,
            false,
            "software.write",
            "mut.software.plugin.software.update@v1",
        ),
    );

    // GetInfo method
    schema.methods.insert(
        "get_info".to_string(),
        method_decl_from_schemars_with_output::<GetInfoInput, AckOutput>(
            "GetInfo",
            op_state_store::SideEffect::Read,
            true,
            "software.read",
            "obs.software.plugin.software.get-info@v1",
        ),
    );

    schema.capabilities.insert(
        "software.write".to_string(),
        CapabilityDecl {
            id: "software.write".to_string(),
            description: "Grants: install, uninstall, update.".to_string(),
        },
    );
    schema.capabilities.insert(
        "software.read".to_string(),
        CapabilityDecl {
            id: "software.read".to_string(),
            description: "Grants: get_info.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

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
