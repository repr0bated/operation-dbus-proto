// Net state plugin - authoritative OVS state management via D-Bus
// Handles: interfaces, bridges, IPs, basic connectivity via plugin schema
// Integrates with systemd-networkd as subordinate service for L3 configuration
use op_blockchain::PluginFootprint;

// Use D-Bus introspection instead of CLI commands
use anyhow::{Context, Result};
use async_trait::async_trait;
use log;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateAction, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
// use std::net::Ipv4Addr; // not needed currently

/// Network configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub interfaces: Vec<InterfaceConfig>,
}

/// Interface configuration with immutable identity and tunable config
/// Pattern matches LXC plugin: immutable core + tunable properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    // IMMUTABLE - Core identity (set once, never changes)
    /// Interface name (e.g., "ovsbr0", "mesh")
    pub name: String,

    /// Interface type (e.g., "ovs-bridge", "ethernet")
    #[serde(rename = "type")]
    pub if_type: InterfaceType,

    /// L2 driver to use (e.g., "openvswitch", "linux-bridge")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,

    // TUNABLE - Configuration that can change (blockchain tracks all changes)
    /// All tunable configuration in a single object
    #[serde(flatten)]
    pub tunable: TunableConfig,
}

/// Tunable configuration - can be changed, each change tracked in blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunableConfig {
    /// Ports attached to this interface
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,

    /// L3 driver for IP configuration (e.g., "rtnetlink", "ovs-rpc", "systemd-networkd")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l3_driver: Option<String>,

    /// IPv4 configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4: Option<Ipv4Config>,

    /// IPv6 configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<Ipv6Config>,

    /// SDN controller (for OpenFlow)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,

    /// Dynamic properties - introspection captures ALL hardware properties here
    /// Examples: mtu, mac_addresses (array), speed, duplex, txqueuelen, etc.
    ///
    /// APPEND-ONLY: Field names are permanent once added (by introspection or user)
    /// Values are mutable (ledger tracks all changes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,

    /// Property schema - tracks which fields exist (append-only set)
    /// Used for validation: new fields can be added, existing fields cannot be removed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_schema: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceType {
    Ethernet,
    OvsBridge,
    OvsPort,
    Bridge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ipv4Config {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<AddressConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ipv6Config {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressConfig {
    pub ip: String,
    pub prefix: u8,
}

/// Net state plugin implementation - authoritative OVS state via D-Bus
pub struct NetStatePlugin {
    #[allow(dead_code)]
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<PluginFootprint>>,
}

#[allow(dead_code)]
impl NetStatePlugin {
    pub fn new() -> Self {
        Self {
            blockchain_sender: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_blockchain_sender(
        blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
    ) -> Self {
        Self {
            blockchain_sender: Some(blockchain_sender),
        }
    }

    /// Validate interface configuration
    pub fn validate_interface_config(&self, _config: &InterfaceConfig) -> Result<()> {
        // TODO: Implement validation logic
        Ok(())
    }

    /// Check if OVS is available via JSON-RPC
    pub async fn check_ovs_available(&self) -> Result<bool> {
        // Try to connect to OVSDB unix socket
        let client = op_network::ovsdb::OvsdbClient::new();
        match client.list_dbs().await {
            Ok(_) => Ok(true),
            Err(_) => {
                log::info!("OVSDB socket not available - skipping OVS operations");
                Ok(false)
            }
        }
    }

    /// Query current network state via D-Bus (OVS bridges only)
    pub async fn query_current_state_dbus(&self) -> Result<NetworkConfig> {
        let mut network_interfaces = Vec::new();

        // Query OVS bridges via D-Bus
        let ovs_bridges = self.query_ovs_bridges().await?;
        network_interfaces.extend(ovs_bridges);

        Ok(NetworkConfig {
            interfaces: network_interfaces,
        })
    }

    /// Parse IPv4 configuration from ip addr show output
    fn parse_ipv4_config(output: &str) -> Option<Ipv4Config> {
        let mut ipv4_config = Ipv4Config {
            enabled: false,
            dhcp: None,
            address: Some(Vec::new()),
            gateway: None,
            dns: Some(Vec::new()),
        };

        let mut found_ipv4 = false;

        for line in output.lines() {
            let line = line.trim();

            // Look for inet lines (IPv4 addresses)
            if line.starts_with("inet ") {
                found_ipv4 = true;
                ipv4_config.enabled = true;

                // Parse inet 192.168.1.100/24 brd 192.168.1.255 scope global dynamic ens1
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let addr_part = parts[1]; // e.g., "192.168.1.100/24"
                    if let Some((ip, prefix)) = Self::parse_cidr(addr_part) {
                        if let Some(ref mut addresses) = ipv4_config.address {
                            addresses.push(AddressConfig {
                                ip,
                                prefix: prefix as u8,
                            });
                        }
                    }
                }
            }
        }

        if found_ipv4 {
            Some(ipv4_config)
        } else {
            None
        }
    }

    /// Parse CIDR notation like "192.168.1.100/24" into (ip, prefix)
    fn parse_cidr(cidr: &str) -> Option<(String, u32)> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() == 2 {
            if let Ok(prefix) = parts[1].parse::<u32>() {
                return Some((parts[0].to_string(), prefix));
            }
        }
        None
    }

    /// Query OVS bridges directly via JSON-RPC
    pub async fn query_ovs_bridges(&self) -> Result<Vec<InterfaceConfig>> {
        // Use OVSDB JSON-RPC client - native protocol
        let client = op_network::ovsdb::OvsdbClient::new();

        // Check if OVSDB is available
        if client.list_dbs().await.is_err() {
            log::info!("OVSDB socket not available - skipping OVS operations");
            return Ok(Vec::new());
        }

        let mut bridges = Vec::new();

        // Get all bridge names via JSON-RPC
        let bridge_names = match client.list_bridges().await {
            Ok(names) => names,
            Err(_) => {
                log::info!("Failed to list OVS bridges via JSON-RPC");
                return Ok(Vec::new());
            }
        };

        for bridge_name in bridge_names {
            // Get bridge information via JSON-RPC
            let bridge_info_json = match client.get_bridge_info(&bridge_name).await {
                Ok(info) => info,
                Err(_) => {
                    log::debug!("Failed to get info for bridge: {}", bridge_name);
                    continue;
                }
            };

            // Parse JSON string to HashMap
            let mut bridge_info: HashMap<String, Value> = match unsafe {
                let mut bridge_info_json_mut = bridge_info_json;
                simd_json::from_str::<HashMap<String, Value>>(&mut bridge_info_json_mut)
            } {
                Ok(info) => info,
                Err(_) => {
                    log::debug!("Failed to parse bridge info JSON for: {}", bridge_name);
                    continue;
                }
            };

            // Enrich with routing info (via rtnetlink) for this bridge
            if let Ok(routes) = op_network::rtnetlink::list_routes_for_interface(&bridge_name).await
            {
                bridge_info.insert(
                    "routing".to_string(),
                    simd_json::json!({
                        "ipv4_routes": routes
                    }),
                );
            }

            // Get ports for this bridge via JSON-RPC
            let ports = match client.list_bridge_ports(&bridge_name).await {
                Ok(ports) => Some(ports),
                Err(_) => {
                    log::debug!("Failed to get ports for bridge: {}", bridge_name);
                    None
                }
            };

            // Derive simple role tags for ports (best-effort heuristics)
            if let Some(ref port_list) = ports {
                let mut tags: HashMap<String, String> = HashMap::new();
                for p in port_list {
                    let role = if p == "wgcf" {
                        "warp"
                    } else if p.starts_with("wg") {
                        "wireguard"
                    } else if p.starts_with("vi") {
                        // vi{VMID}
                        "container"
                    } else if p.starts_with("nm") {
                        "netmaker"
                    } else if p.starts_with("eth") || p.starts_with("en") {
                        "uplink"
                    } else if p == &bridge_name {
                        "internal"
                    } else {
                        "unknown"
                    };
                    tags.insert(p.clone(), role.to_string());
                }
                bridge_info.insert(
                    "port_tags".to_string(),
                    simd_json::serde::to_owned_value(tags).unwrap_or(Value::null()),
                );
            }

            bridges.push(InterfaceConfig {
                name: bridge_name,
                if_type: InterfaceType::OvsBridge,
                driver: Some("openvswitch".to_string()),
                tunable: TunableConfig {
                    ports,
                    l3_driver: None, // Bridges typically don't need L3 config
                    ipv4: None,      // OVS bridges don't have IP config directly
                    ipv6: None,
                    controller: None,
                    properties: Some(bridge_info),
                    property_schema: Some(vec!["ovsdb".to_string()]),
                },
            });
        }

        Ok(bridges)
    }

    /// Apply OVS bridge configuration via JSON-RPC and rtnetlink
    pub async fn apply_ovs_config(&self, config: &InterfaceConfig) -> Result<()> {
        let client = op_network::ovsdb::OvsdbClient::new();
        log::info!("Starting apply_ovs_config for {}", config.name);

        // Ensure bridge exists via OVSDB JSON-RPC
        if !client
            .bridge_exists(&config.name)
            .await
            .context("Failed to check bridge existence")?
        {
            client
                .create_bridge(&config.name)
                .await
                .context("Failed to create OVS bridge via JSON-RPC")?;
            log::info!("Created OVS bridge via JSON-RPC: {}", config.name);
        }

        // Add ports to bridge if specified via OVSDB JSON-RPC
        // Skip netmaker interfaces (nm-*) - they are managed by netclient
        if let Some(ref ports) = config.tunable.ports {
            // Get current ports via JSON-RPC instead of ovs-vsctl
            let current_ports = client
                .list_bridge_ports(&config.name)
                .await
                .context("Failed to list ports via JSON-RPC")?;

            for port in ports {
                // Skip netmaker/wireguard interfaces - netclient manages them
                if port.starts_with("nm-") || port.starts_with("wg") {
                    log::info!(
                        "Skipping netmaker interface {} (managed by netclient)",
                        port
                    );
                    continue;
                }

                if !current_ports.contains(port) {
                    client.add_port(&config.name, port).await.context(format!(
                        "Failed to add port {} to bridge {} via JSON-RPC",
                        port, config.name
                    ))?;
                    log::info!("Added port {} to bridge {} via JSON-RPC", port, config.name);
                }
            }
        }

        // Update /etc/network/interfaces with bridge and IP configuration
        self.update_interfaces_file(&config.name, None, &config.tunable.ipv4)
            .await?;

        // Bring bridge up via rtnetlink (native netlink)
        if let Err(e) = op_network::rtnetlink::link_up(&config.name).await {
            log::warn!("Failed to bring bridge up via netlink: {}", e);
        }

        // Configure IPv4 if specified via rtnetlink (native netlink)
        if let Some(ref ipv4) = config.tunable.ipv4 {
            if ipv4.enabled {
                if let Some(ref addresses) = ipv4.address {
                    for addr in addresses {
                        match op_network::rtnetlink::add_ipv4_address(
                            &config.name,
                            &addr.ip,
                            addr.prefix,
                        )
                        .await
                        {
                            Ok(_) => {
                                log::info!(
                                    "Added IP {}/{} to {} via rtnetlink",
                                    addr.ip,
                                    addr.prefix,
                                    config.name
                                );
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to add IP {} (may already exist): {}",
                                    addr.ip,
                                    e
                                );
                            }
                        }
                    }
                }

                // Configure gateway if specified via rtnetlink (native netlink)
                if let Some(ref gateway) = ipv4.gateway {
                    // Delete existing default route (ignore errors)
                    let _ = op_network::rtnetlink::del_default_route().await;

                    // Add new default route
                    match op_network::rtnetlink::add_default_route(&config.name, gateway).await {
                        Ok(_) => {
                            log::info!(
                                "Added default route via {} on {} via rtnetlink",
                                gateway,
                                config.name
                            );
                        }
                        Err(e) => {
                            log::warn!("Failed to add default route: {}", e);
                        }
                    }
                }
            }
        }

        log::info!("Finished apply_ovs_config for {}", config.name);
        Ok(())
    }

    /// Apply OVS internal port configuration
    pub async fn apply_ovs_port_config(&self, config: &InterfaceConfig) -> Result<()> {
        log::info!("Starting apply_ovs_port_config for {}", config.name);

        // Internal ports are created as part of their parent bridge
        // This function handles IP configuration only

        // Determine L3 driver (default to rtnetlink)
        let l3_driver = config.tunable.l3_driver.as_deref().unwrap_or("rtnetlink");

        if l3_driver == "rtnetlink" {
            // Bring interface up via native rtnetlink
            if let Err(e) = op_network::rtnetlink::link_up(&config.name).await {
                log::warn!("Failed to bring port up: {}", e);
            }

            // Configure IPv4 if specified via rtnetlink
            if let Some(ref ipv4) = config.tunable.ipv4 {
                if ipv4.enabled {
                    if let Some(ref addresses) = ipv4.address {
                        for addr in addresses {
                            match op_network::rtnetlink::add_ipv4_address(
                                &config.name,
                                &addr.ip,
                                addr.prefix,
                            )
                            .await
                            {
                                Ok(_) => {
                                    log::info!(
                                        "Added IP {}/{} to {} via rtnetlink",
                                        addr.ip,
                                        addr.prefix,
                                        config.name
                                    );
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Failed to add IP {} (may already exist): {}",
                                        addr.ip,
                                        e
                                    );
                                }
                            }
                        }
                    }

                    // Configure gateway if specified
                    if let Some(ref gateway) = ipv4.gateway {
                        let _ = op_network::rtnetlink::del_default_route().await;
                        match op_network::rtnetlink::add_default_route(&config.name, gateway).await
                        {
                            Ok(_) => {
                                log::info!("Added default route via {} via rtnetlink", gateway);
                            }
                            Err(e) => {
                                log::warn!("Failed to add default route: {}", e);
                            }
                        }
                    }
                }
            }

            // Update /etc/network/interfaces for persistence
            self.update_interfaces_file(&config.name, None, &config.tunable.ipv4)
                .await?;
        } else {
            log::warn!("Unsupported L3 driver '{}' for {}", l3_driver, config.name);
        }

        log::info!("Finished apply_ovs_port_config for {}", config.name);
        Ok(())
    }

    /// Delete OVS bridge via JSON-RPC
    pub async fn delete_ovs_bridge(&self, name: &str) -> Result<()> {
        let client = op_network::ovsdb::OvsdbClient::new();

        client
            .delete_bridge(name)
            .await
            .context("Failed to delete OVS bridge via JSON-RPC")?;

        Ok(())
    }

    /// Update /etc/network/interfaces with bridge configuration
    async fn update_interfaces_file(
        &self,
        bridge: &str,
        uplink: Option<&str>,
        ipv4: &Option<Ipv4Config>,
    ) -> Result<()> {
        let interfaces_path = std::path::Path::new("/etc/network/interfaces");
        let tag = "op-dbus-managed";
        let begin_marker = format!("# BEGIN {}\n", tag);
        let end_marker = format!("# END {}\n", tag);

        // Build the managed block
        let mut block = String::new();
        block.push_str(&begin_marker);
        block.push_str(&format!("# Managed by {}. Do not edit manually.\n\n", tag));

        // OVS Bridge with IP configuration
        // Use allow-ovs instead of auto to prevent ifupdown hang
        block.push_str(&format!("allow-ovs {}\n", bridge));
        block.push_str(&format!("iface {} inet ", bridge));

        if let Some(ref ipv4_cfg) = ipv4 {
            if ipv4_cfg.enabled {
                if ipv4_cfg.dhcp == Some(true) {
                    block.push_str("dhcp\n");
                } else if let Some(ref addresses) = ipv4_cfg.address {
                    if let Some(addr) = addresses.first() {
                        block.push_str("static\n");
                        block.push_str(&format!("    address {}\n", addr.ip));
                        block.push_str(&format!(
                            "    netmask {}\n",
                            Self::prefix_to_netmask(addr.prefix)
                        ));

                        if let Some(ref gateway) = ipv4_cfg.gateway {
                            block.push_str(&format!("    gateway {}\n", gateway));
                        }
                    } else {
                        block.push_str("manual\n");
                    }
                } else {
                    block.push_str("manual\n");
                }
            } else {
                block.push_str("manual\n");
            }
        } else {
            block.push_str("manual\n");
        }

        block.push_str("    ovs_type OVSBridge\n");

        // Add uplink to ovs_ports if specified
        if let Some(uplink_iface) = uplink {
            block.push_str(&format!("    ovs_ports {}\n", uplink_iface));
        }
        block.push('\n');

        // Physical uplink (if specified)
        if let Some(uplink_iface) = uplink {
            block.push_str(&format!("allow-{} {}\n", bridge, uplink_iface));
            block.push_str(&format!("iface {} inet manual\n", uplink_iface));
            block.push_str(&format!("    ovs_bridge {}\n", bridge));
            block.push_str("    ovs_type OVSPort\n");
            block.push('\n');
        }

        block.push_str(&end_marker);

        // Read current file content
        let content = tokio::fs::read_to_string(interfaces_path)
            .await
            .unwrap_or_else(|_| String::from("# network interfaces file\n"));

        // Replace or append the managed block
        let new_content = Self::replace_block(&content, &begin_marker, &end_marker, &block);

        // Write back if changed
        if new_content != content {
            tokio::fs::write(interfaces_path, new_content)
                .await
                .context("Failed to write /etc/network/interfaces")?;
            log::info!("Updated /etc/network/interfaces");
        }

        Ok(())
    }

    /// Convert CIDR prefix to netmask string
    fn prefix_to_netmask(prefix: u8) -> String {
        let mask: u32 = !0u32 << (32 - prefix);
        format!(
            "{}.{}.{}.{}",
            (mask >> 24) & 0xFF,
            (mask >> 16) & 0xFF,
            (mask >> 8) & 0xFF,
            mask & 0xFF
        )
    }

    /// Replace a marked block in text content
    fn replace_block(
        content: &str,
        begin_marker: &str,
        end_marker: &str,
        new_block: &str,
    ) -> String {
        if let Some(start) = content.find(begin_marker) {
            if let Some(end) = content[start..].find(end_marker) {
                let end_idx = start + end + end_marker.len();
                let mut result = String::with_capacity(content.len() + new_block.len());
                result.push_str(&content[..start]);
                result.push_str(new_block);
                result.push_str(&content[end_idx..]);
                return result;
            }
        }

        // Block not found, append it
        let mut result = String::with_capacity(content.len() + new_block.len() + 1);
        result.push_str(content);
        if !content.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(new_block);
        result
    }
}

impl Default for NetStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}
#[async_trait]
impl StatePlugin for NetStatePlugin {
    fn name(&self) -> &str {
        "net"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        // Check if OVSDB socket is available
        std::path::Path::new("/var/run/openvswitch/db.sock").exists()
    }

    fn unavailable_reason(&self) -> String {
        "OpenVSwitch OVSDB socket not found at /var/run/openvswitch/db.sock - install with: apt install openvswitch-switch".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        // Query current OVS state via D-Bus exclusively
        let network_config = self.query_current_state_dbus().await?;
        Ok(simd_json::serde::to_owned_value(network_config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: NetworkConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: NetworkConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        // Build maps for quick lookup - avoid cloning strings unnecessarily
        let current_map: HashMap<&String, &InterfaceConfig> = current_config
            .interfaces
            .iter()
            .map(|i| (&i.name, i))
            .collect();

        let desired_map: HashMap<&String, &InterfaceConfig> = desired_config
            .interfaces
            .iter()
            .map(|i| (&i.name, i))
            .collect();

        // Find interfaces to create or modify
        for (name, desired_iface) in &desired_map {
            if let Some(current_iface) = current_map.get(name) {
                // Check if modification needed
                if simd_json::serde::to_owned_value(current_iface)?
                    != simd_json::serde::to_owned_value(desired_iface)?
                {
                    actions.push(StateAction::Modify {
                        resource: (*name).clone(),
                        changes: simd_json::serde::to_owned_value(desired_iface)?,
                    });
                }
            } else {
                actions.push(StateAction::Create {
                    resource: (*name).clone(),
                    config: simd_json::serde::to_owned_value(desired_iface)?,
                });
            }
        }

        // Find interfaces to delete
        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                actions.push(StateAction::Delete {
                    resource: (*name).clone(),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: op_state::DiffMetadata {
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
            match action {
                StateAction::Create { resource, config }
                | StateAction::Modify {
                    resource,
                    changes: config,
                } => {
                    let iface_config: InterfaceConfig =
                        simd_json::serde::from_owned_value(config.clone())?;

                    match self.apply_ovs_config(&iface_config).await {
                        Ok(_) => {
                            changes_applied.push(format!("Applied OVS config for: {}", resource));
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Failed to apply OVS config for {}: {}",
                                resource, e
                            ));
                        }
                    }
                }
                StateAction::Delete { resource } => {
                    // Delete OVS bridge via D-Bus
                    if resource.starts_with("ovsbr") || resource.starts_with("br") {
                        match self.delete_ovs_bridge(resource).await {
                            Ok(_) => {
                                changes_applied.push(format!("Deleted OVS bridge: {}", resource));
                            }
                            Err(e) => {
                                errors.push(format!(
                                    "Failed to delete OVS bridge {}: {}",
                                    resource, e
                                ));
                            }
                        }
                    } else {
                        changes_applied.push(format!("Skipped non-OVS interface: {}", resource));
                    }
                }
                StateAction::NoOp { .. } => {}
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let desired_config: NetworkConfig = simd_json::serde::from_owned_value(desired.clone())?;
        let current = self.query_current_state().await?;
        let current_config: NetworkConfig = simd_json::serde::from_owned_value(current)?;

        // Simple verification: check if desired interfaces exist
        let current_names: std::collections::HashSet<_> =
            current_config.interfaces.iter().map(|i| &i.name).collect();

        for iface in &desired_config.interfaces {
            if !current_names.contains(&iface.name) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current_state = self.query_current_state().await?;

        Ok(Checkpoint {
            id: format!("network-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current_state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_config: NetworkConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;

        // Restore old OVS configuration via D-Bus
        for iface in &old_config.interfaces {
            match iface.if_type {
                InterfaceType::OvsBridge => {
                    self.apply_ovs_config(iface).await?;
                }
                InterfaceType::OvsPort => {
                    self.apply_ovs_port_config(iface).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true, // D-Bus operations are atomic
        }
    }
}

// impl Default for NetStatePlugin {
//     fn default() -> Self {
//         Self::new()
//     }
// }
