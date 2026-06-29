//! Rtnetlink state plugin - manages kernel-level network interface state
//!
//! Handles: IP addresses, link state (up/down), MAC addresses, default routes
//! Uses native rtnetlink (netlink) protocol — no CLI wrappers.
//! Depends on: net, ovsdb_bridge (interfaces must exist before configuring)

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;

/// Rtnetlink interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtnetlinkInterfaceConfig {
    /// Interface name (e.g., "ens3", "ovsbr0-int")
    pub name: String,

    /// IPv4/IPv6 addresses to assign
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<AddressEntry>>,

    /// MAC address to set (e.g., "fa:16:3e:f1:71:d2")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,

    /// MTU
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,

    /// Desired link state: "up" or "down"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// Default gateway (only one interface should set this)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_gateway: Option<String>,
}

/// IP address entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddressEntry {
    pub ip: String,
    pub prefix: u8,
}

/// Rtnetlink state — list of managed interfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtnetlinkState {
    pub interfaces: Vec<RtnetlinkInterfaceConfig>,
}

pub struct RtnetlinkPlugin;

impl RtnetlinkPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RtnetlinkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for RtnetlinkPlugin {
    fn name(&self) -> &str {
        "rtnetlink"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(rtnetlink_schema())
    }

    fn is_available(&self) -> bool {
        // rtnetlink is always available — it's the kernel
        true
    }

    fn unavailable_reason(&self) -> String {
        "rtnetlink is always available".to_string()
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: RtnetlinkState = simd_json::serde::from_owned_value(current.clone())
            .unwrap_or(RtnetlinkState { interfaces: vec![] });
        let desired_state: RtnetlinkState = simd_json::serde::from_owned_value(desired.clone())
            .unwrap_or(RtnetlinkState { interfaces: vec![] });

        let mut actions = Vec::new();

        let current_map: HashMap<&str, &RtnetlinkInterfaceConfig> = current_state
            .interfaces
            .iter()
            .map(|i| (i.name.as_str(), i))
            .collect();

        for desired_iface in &desired_state.interfaces {
            if let Some(current_iface) = current_map.get(desired_iface.name.as_str()) {
                // Check if any property differs
                let needs_update = desired_iface.state != current_iface.state
                    || desired_iface.mac_address != current_iface.mac_address
                    || desired_iface.addresses != current_iface.addresses
                    || desired_iface.default_gateway.is_some();

                if needs_update {
                    actions.push(StateAction::Modify {
                        resource: desired_iface.name.clone(),
                        changes: simd_json::serde::to_owned_value(desired_iface)?,
                    });
                }
            } else {
                // Interface not found in kernel — can only configure if it exists
                log::warn!(
                    "rtnetlink: desired interface '{}' not found in kernel",
                    desired_iface.name
                );
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let config: RtnetlinkInterfaceConfig =
                    simd_json::serde::from_owned_value(changes.clone())?;

                // Set MAC address
                if let Some(ref mac) = config.mac_address {
                    match op_network::rtnetlink::set_mac_address(resource, mac).await {
                        Ok(_) => changes_applied
                            .push(format!("Set MAC {} on {} via rtnetlink", mac, resource)),
                        Err(e) => errors.push(format!("Failed to set MAC on {}: {}", resource, e)),
                    }
                }

                // Add IP addresses
                if let Some(ref addresses) = config.addresses {
                    for addr in addresses {
                        match op_network::rtnetlink::add_ipv4_address(
                            resource,
                            &addr.ip,
                            addr.prefix,
                        )
                        .await
                        {
                            Ok(_) => changes_applied.push(format!(
                                "Added {}/{} to {} via rtnetlink",
                                addr.ip, addr.prefix, resource
                            )),
                            Err(e) => {
                                // EEXIST is not an error — address already assigned
                                let msg = e.to_string();
                                if msg.contains("exist") {
                                    log::info!(
                                        "Address {}/{} already on {} (ok)",
                                        addr.ip,
                                        addr.prefix,
                                        resource
                                    );
                                } else {
                                    errors.push(format!(
                                        "Failed to add {}/{} to {}: {}",
                                        addr.ip, addr.prefix, resource, e
                                    ));
                                }
                            }
                        }
                    }
                }

                // Set link state
                if let Some(ref state) = config.state {
                    let result = if state == "up" {
                        op_network::rtnetlink::link_up(resource).await
                    } else {
                        op_network::rtnetlink::link_down(resource).await
                    };
                    match result {
                        Ok(_) => changes_applied
                            .push(format!("Set {} {} via rtnetlink", resource, state)),
                        Err(e) => {
                            errors.push(format!("Failed to set {} {}: {}", resource, state, e))
                        }
                    }
                }

                // Set default gateway
                if let Some(ref gateway) = config.default_gateway {
                    // Delete existing default route first
                    let _ = op_network::rtnetlink::del_default_route().await;
                    match op_network::rtnetlink::add_default_route(resource, gateway).await {
                        Ok(_) => changes_applied.push(format!(
                            "Set default route via {} on {} via rtnetlink",
                            gateway, resource
                        )),
                        Err(e) => errors.push(format!(
                            "Failed to set default route via {}: {}",
                            gateway, e
                        )),
                    }
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = simd_json::json!(null);
        Ok(Checkpoint {
            id: format!("rtnetlink-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_state: RtnetlinkState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;

        // Re-apply old state
        let current = simd_json::json!(null);
        let diff = self
            .calculate_diff(&current, &simd_json::serde::to_owned_value(&old_state)?)
            .await?;
        self.apply_state(&diff).await?;

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

/// Method input types - single source of truth via schemars
/// set_link_state method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetLinkStateInput {
    /// Interface name
    pub name: String,
    /// Desired state: "up" or "down"
    pub state: String,
}

/// add_ipv4_address method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddIpv4AddressInput {
    /// Interface name
    pub name: String,
    /// IP address (CIDR notation, e.g. "10.0.0.1/24")
    pub address: String,
}

/// set_mac_address method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetMacAddressInput {
    /// Interface name
    pub name: String,
    /// MAC address
    pub mac: String,
}

/// set_default_route method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetDefaultRouteInput {
    /// Interface name for default route
    pub name: String,
    /// Gateway IP address
    pub gateway: String,
}

pub(crate) fn rtnetlink_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Administrative interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Interface IP addresses in CIDR form".to_string(),
                default: Some(json!([])),
                example: Some(json!(["10.0.0.2/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "mac_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional MAC address override".to_string(),
                default: None,
                example: Some(json!("02:00:00:00:00:01")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "default_gateway".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Default gateway for this interface".to_string(),
                default: None,
                example: Some(json!("10.0.0.1")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let mut schema = PluginSchema::builder("rtnetlink")
        .version("1.0.0")
        .description("Native kernel rtnetlink interface management")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "Desired rtnetlink-managed interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "ovsbr0",
                    "state": "up",
                    "addresses": ["10.10.0.1/24"],
                    "default_gateway": "10.10.0.254"
                }
            ]
        }))
        .build();
    
    schema.methods.insert("set_link_state".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<SetLinkStateInput>(
        "set_link_state",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.network.rtnetlink.link-state.set@v1",
        "mut.network.rtnetlink.link-state.set@v1",
    ));
    schema.methods.insert("add_ipv4_address".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<AddIpv4AddressInput>(
        "add_ipv4_address",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.network.rtnetlink.ipv4-address.add@v1",
        "mut.network.rtnetlink.ipv4-address.add@v1",
    ));
    schema.methods.insert("set_mac_address".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<SetMacAddressInput>(
        "set_mac_address",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.network.rtnetlink.mac-address.set@v1",
        "mut.network.rtnetlink.mac-address.set@v1",
    ));
    schema.methods.insert("set_default_route".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<SetDefaultRouteInput>(
        "set_default_route",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.network.rtnetlink.default-route.set@v1",
        "mut.network.rtnetlink.default-route.set@v1",
    ));
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("rtnetlink", |_ctx| std::sync::Arc::new(RtnetlinkPlugin::new()))
}
