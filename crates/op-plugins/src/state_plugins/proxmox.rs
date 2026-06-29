use super::plugin_schema_defs::{any_field, simple_schema, method_decl_from_schemars};
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{FieldSchema, FieldType, PluginSchema, SideEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProxmoxState {
    pub containers: Vec<ContainerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContainerState {
    pub vmid: u32,
    pub hostname: Option<String>,
    pub status: String, // "running", "stopped"
}

// D-Bus method input types for Proxmox VM operations
// Reference: https://pve.proxmox.com/wiki/Developer_Workspace/QEMU/KVM_virtual_machines

/// CreateVM method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateVmInput {
    /// VM ID (vmid)
    pub vmid: u32,
    /// VM name
    pub name: String,
    /// VM type (qemu or lxc)
    #[serde(default)]
    pub vm_type: String,
    /// Number of cores
    #[serde(default)]
    pub cores: u32,
    /// Amount of memory in MB
    #[serde(default)]
    pub memory: u32,
}

/// StartVM method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartVmInput {
    /// VM ID (vmid)
    pub vmid: u32,
}

/// StopVM method input  
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopVmInput {
    /// VM ID (vmid)
    pub vmid: u32,
    /// Force stop (skip shutdown)
    #[serde(default)]
    pub force: bool,
}

/// DeleteVM method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteVmInput {
    /// VM ID (vmid)
    pub vmid: u32,
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
    let mut schema = simple_schema(
        "proxmox",
        "Proxmox container/VM declarations",
        &["net"],
        vec![(
            "containers",
            any_field(true, "Container declarations", Some(json!([]))),
        )],
    );
    
    // Add D-Bus methods for VM lifecycle management
    // Reference: https://pve.proxmox.com/wiki/Developer_Workspace/QEMU/KVM_virtual_machines
    schema.methods.insert("create_vm".to_string(), method_decl_from_schemars::<CreateVmInput>(
        "create_vm",
        SideEffect::Mutation,
        false,
        "cap.virtualization.proxmox.vm.create@v1",
        "mut.virtualization.proxmox.vm.create@v1",
    ));
    schema.methods.insert("start_vm".to_string(), method_decl_from_schemars::<StartVmInput>(
        "start_vm",
        SideEffect::Mutation,
        false,
        "cap.virtualization.proxmox.vm.start@v1",
        "mut.virtualization.proxmox.vm.start@v1",
    ));
    schema.methods.insert("stop_vm".to_string(), method_decl_from_schemars::<StopVmInput>(
        "stop_vm",
        SideEffect::Mutation,
        false,
        "cap.virtualization.proxmox.vm.stop@v1",
        "mut.virtualization.proxmox.vm.stop@v1",
    ));
    schema.methods.insert("delete_vm".to_string(), method_decl_from_schemars::<DeleteVmInput>(
        "delete_vm",
        SideEffect::Mutation,
        false,
        "cap.virtualization.proxmox.vm.delete@v1",
        "mut.virtualization.proxmox.vm.delete@v1",
    ));
    
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("proxmox", |_ctx| std::sync::Arc::new(ProxmoxPlugin::new()))
}
