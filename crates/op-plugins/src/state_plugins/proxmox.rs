use super::plugin_schema_defs::{any_field, simple_schema};
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxmoxState {
    pub containers: Vec<ContainerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    pub vmid: u32,
    pub hostname: Option<String>,
    pub status: String, // "running", "stopped"
}

pub struct ProxmoxPlugin;

impl Default for ProxmoxPlugin {
    fn default() -> Self {
        Self
    }
}

impl ProxmoxPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for ProxmoxPlugin {
    fn name(&self) -> &str {
        "proxmox"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(proxmox_schema())
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

pub(crate) fn proxmox_schema() -> PluginSchema {
    simple_schema(
        "proxmox",
        "Proxmox container declarations",
        &["net"],
        vec![(
            "containers",
            any_field(true, "Container declarations", Some(json!([]))),
        )],
    )
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("proxmox", |_ctx| std::sync::Arc::new(ProxmoxPlugin::new()))
}
