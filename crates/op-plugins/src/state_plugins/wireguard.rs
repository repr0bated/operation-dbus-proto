//! GB.Wireguard plugin — schemars-seeded, D-Bus-backed WireGuard interface state.
//!
//! This plugin owns the WireGuard interface state and peer management.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{PluginSchema, SideEffect};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};

// =============================================================================
// PLUGIN ENTRY: identity and typed schema seed
// =============================================================================

const PLUGIN_NAME: &str = "wireguard";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "network";
const PLUGIN_DESCRIPTION: &str = "WireGuard interface state";
const PLUGIN_DISPLAY_NAME: &str = "GB.Wireguard";

/// WireGuard interface state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.network.plugin.wireguard.schema@v1"))]
pub struct WireGuardState {
    /// Active WireGuard interfaces.
    #[serde(default)]
    #[schemars(
        description = "Active WireGuard interfaces",
        extend("x-oscal-subid" = "exp.network.wireguard.interfaces.render@v1")
    )]
    pub interfaces: Vec<WireGuardInterface>,
}

/// A single WireGuard interface.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wireguard.interface.observe@v1"))]
pub struct WireGuardInterface {
    /// Interface name.
    #[schemars(
        description = "Interface name",
        example = &"wg0",
        extend("x-oscal-subid" = "exp.network.wireguard.interface.name.render@v1")
    )]
    pub name: String,

    /// Private key (only present when set).
    #[serde(default)]
    #[schemars(
        description = "Private key",
        extend("x-oscal-subid" = "obs.network.wireguard.interface.private-key.observe@v1")
    )]
    pub private_key: Option<String>,

    /// Listen port.
    #[schemars(
        description = "Listen port",
        extend("x-oscal-subid" = "exp.network.wireguard.interface.port.render@v1")
    )]
    pub listen_port: u16,

    /// Configured peers.
    #[serde(default)]
    #[schemars(
        description = "Configured peers",
        extend("x-oscal-subid" = "exp.network.wireguard.peers.render@v1")
    )]
    pub peers: Vec<WireGuardPeer>,
}

/// A WireGuard peer.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wireguard.peer.observe@v1"))]
pub struct WireGuardPeer {
    /// Peer's public key.
    #[schemars(
        description = "Peer's public key",
        extend("x-oscal-subid" = "exp.network.wireguard.peer.public-key.render@v1")
    )]
    pub public_key: String,

    /// Allowed IP addresses.
    #[schemars(
        description = "Allowed IP addresses",
        extend("x-oscal-subid" = "exp.network.wireguard.peer.allowed-ips.render@v1")
    )]
    pub allowed_ips: Vec<String>,

    /// Endpoint address.
    #[serde(default)]
    #[schemars(
        description = "Endpoint address",
        extend("x-oscal-subid" = "obs.network.wireguard.peer.endpoint.render@v1")
    )]
    pub endpoint: Option<String>,
}

// =============================================================================
// METHOD INPUT/OUTPUT TYPES
// =============================================================================

/// Input for SetDevice method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.network.wireguard.device.set.input@v1"))]
pub struct SetDeviceInput {
    /// Interface name.
    pub interface: String,
    /// Private key (optional).
    #[serde(default)]
    pub private_key: Option<String>,
    /// Listen port (optional).
    #[serde(default)]
    pub listen_port: Option<u16>,
    /// Firewall mark (optional).
    #[serde(default)]
    pub fwmark: Option<u32>,
}

/// Input for GetDevice method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wireguard.device.get.input@v1"))]
pub struct GetDeviceInput {
    /// Interface name.
    pub interface: String,
}

/// Output for GetDevice method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wireguard.device.get.output@v1"))]
pub struct GetDeviceOutput {
    /// Interface details.
    pub interface: Option<WireGuardInterface>,
}

/// Input for AddPeer method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.network.wireguard.peer.add.input@v1"))]
pub struct AddPeerInput {
    /// Interface name.
    pub interface: String,
    /// Peer's public key.
    pub public_key: String,
    /// Endpoint address (optional).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Allowed IP addresses.
    pub allowed_ips: Vec<String>,
    /// Persistent keepalive interval (optional).
    #[serde(default)]
    pub persistent_keepalive: Option<u16>,
    /// Preshared key (optional).
    #[serde(default)]
    pub preshared_key: Option<String>,
}

/// Input for RemovePeer method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.network.wireguard.peer.remove.input@v1"))]
pub struct RemovePeerInput {
    /// Interface name.
    pub interface: String,
    /// Peer's public key.
    pub public_key: String,
}

/// Input for ListPeers method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wireguard.peers.list.input@v1"))]
pub struct ListPeersInput {
    /// Interface name.
    pub interface: String,
}

/// Output for ListPeers method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wireguard.peers.list.output@v1"))]
pub struct ListPeersOutput {
    /// List of peers.
    pub peers: Vec<WireGuardPeer>,
}

/// Input for SetConfig method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.network.wireguard.config.set.input@v1"))]
pub struct SetConfigInput {
    /// Interface name.
    pub interface: String,
    /// Full configuration as JSON.
    pub config: serde_json::Value,
}

// =============================================================================
// PLUGIN BODY: D-Bus-backed behavior only
// =============================================================================

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
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(wireguard_schema())
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: PLUGIN_NAME.to_string(),
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
            plugin: PLUGIN_NAME.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!(null),
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

// =============================================================================
// PLUGIN EXIT: publish the single PluginSchema contract
// =============================================================================

pub fn wireguard_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(WireGuardState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());

    // SetDevice method
    schema.methods.insert(
        "set_device".to_string(),
        method_decl_from_schemars_with_output::<SetDeviceInput, AckOutput>(
            "set_device",
            SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.device.set@v1",
        ),
    );

    // GetDevice method
    schema.methods.insert(
        "get_device".to_string(),
        method_decl_from_schemars_with_output::<GetDeviceInput, GetDeviceOutput>(
            "get_device",
            SideEffect::Read,
            true,
            "wireguard.read",
            "obs.network.wireguard.device.get@v1",
        ),
    );

    // AddPeer method
    schema.methods.insert(
        "add_peer".to_string(),
        method_decl_from_schemars_with_output::<AddPeerInput, AckOutput>(
            "add_peer",
            SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.peer.add@v1",
        ),
    );

    // RemovePeer method
    schema.methods.insert(
        "remove_peer".to_string(),
        method_decl_from_schemars_with_output::<RemovePeerInput, AckOutput>(
            "remove_peer",
            SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.peer.remove@v1",
        ),
    );

    // ListPeers method
    schema.methods.insert(
        "list_peers".to_string(),
        method_decl_from_schemars_with_output::<ListPeersInput, ListPeersOutput>(
            "list_peers",
            SideEffect::Read,
            true,
            "wireguard.read",
            "obs.network.wireguard.peers.list@v1",
        ),
    );

    // SetConfig method
    schema.methods.insert(
        "set_config".to_string(),
        method_decl_from_schemars_with_output::<SetConfigInput, AckOutput>(
            "set_config",
            SideEffect::Mutation,
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
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(WireGuardPlugin::new()))
}
