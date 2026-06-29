use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;

use simd_json::OwnedValue as Value;

use super::plugin_schema_defs::{any_field, simple_schema};
use op_state_store::PluginSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardState {
    pub interfaces: Vec<WireGuardInterface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardInterface {
    pub name: String,
    pub private_key: Option<String>,
    pub listen_port: u16,
    pub peers: Vec<WireGuardPeer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardPeer {
    pub public_key: String,
    pub allowed_ips: Vec<String>,
    pub endpoint: Option<String>,
}

pub struct WireGuardPlugin;

impl Default for WireGuardPlugin {
    fn default() -> Self {
        Self
    }
}

impl WireGuardPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for WireGuardPlugin {
    fn name(&self) -> &str {
        "wireguard"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(wireguard_schema())
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetDeviceInput {
    pub interface: String,
    pub private_key: Option<String>,
    pub listen_port: Option<u16>,
    pub fwmark: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetDeviceInput {
    pub interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AddPeerInput {
    pub interface: String,
    pub public_key: String,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: Option<u16>,
    pub preshared_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemovePeerInput {
    pub interface: String,
    pub public_key: String,
}

/// Input struct for ListPeers method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListPeersInput {
    /// Interface name
    pub interface: String,
}

/// Input struct for SetConfig method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetConfigInput {
    /// Interface name
    pub interface: String,
    /// Full configuration as JSON
    pub config: serde_json::Value,
}

pub(crate) fn wireguard_schema() -> PluginSchema {
    let mut schema = simple_schema(
        "wireguard",
        "WireGuard interface state",
        &["net"],
        vec![(
            "interfaces",
            any_field(true, "WireGuard interfaces", Some(json!([]))),
        )],
    );

    schema.methods.insert(
        "set_device".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<SetDeviceInput>(
            "SetDevice",
            op_state_store::SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.device.set@v1",
        ),
    );
    schema.methods.insert(
        "get_device".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<GetDeviceInput>(
            "GetDevice",
            op_state_store::SideEffect::Read,
            true,
            "wireguard.read",
            "obs.network.wireguard.device.get@v1",
        ),
    );
    schema.methods.insert(
        "add_peer".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<AddPeerInput>(
            "AddPeer",
            op_state_store::SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.peer.add@v1",
        ),
    );
    schema.methods.insert(
        "remove_peer".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<RemovePeerInput>(
            "RemovePeer",
            op_state_store::SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.peer.remove@v1",
        ),
    );

    // ListPeers method
    schema.methods.insert(
        "list_peers".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<ListPeersInput>(
            "ListPeers",
            op_state_store::SideEffect::Read,
            true,
            "wireguard.read",
            "obs.network.wireguard.peers.list@v1",
        ),
    );

    // SetConfig method
    schema.methods.insert(
        "set_config".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<SetConfigInput>(
            "SetConfig",
            op_state_store::SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.config.set@v1",
        ),
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("wireguard", |_ctx| std::sync::Arc::new(WireGuardPlugin::new()))
}
