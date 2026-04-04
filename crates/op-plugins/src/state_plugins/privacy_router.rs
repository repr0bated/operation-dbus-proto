//! Privacy router system fabric.
//!
//! This plugin owns the base privacy fabric as system-managed Incus containers and
//! bridge/OpenFlow policy, separate from per-user privacy containers.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use op_network::{openflow::OpenFlowClient, OvsdbClient};
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::Path;
use tokio::process::Command;

use crate::state_plugins::incus::{IncusInstance, IncusPlugin, IncusState};
use crate::state_plugins::openflow::{
    BridgeFlowConfig, FlowAction, FlowEntry, OpenFlowConfig, OpenFlowPlugin,
};
use crate::state_plugins::privacy_routes::{PrivacyRoute, PrivacyRoutesPlugin, PrivacyRoutesState};

const DEFAULT_BRIDGE_NAME: &str = "ovsbr0";
const DEFAULT_UPLINK_PORT: &str = "o";
const DEFAULT_MGMT_PORT: &str = "ovsbr0-mgmt";
const DEFAULT_SOCKET_PORT: &str = "ovsbr0-sock";
const DEFAULT_MGMT_CIDR: &str = "10.200.0.1/24";
const DEFAULT_OPENFLOW_CONTROLLER: &str = "10.88.88.1:6653";
const DEFAULT_WARP_INTERFACE: &str = "wgcf";
const DEFAULT_WGCF_CONFIG: &str = "/etc/wireguard/wgcf.conf";
const SYSTEM_FLOW_COOKIE_PREFIX: u64 = 0x5053_0000_0000_0000;
const SYSTEM_FLOW_COOKIE_MASK: u64 = 0xFFFF_0000_0000_0000;

/// Privacy Router Tunnel Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRouterConfig {
    /// OVS bridge name (shared by all components)
    pub bridge_name: String,

    /// WireGuard ingress container configuration
    pub wireguard: WireGuardConfig,

    /// WARP tunnel configuration
    pub warp: WarpConfig,

    /// XRay REALITY outbound client configuration
    pub xray: XRayConfig,

    /// VPS XRay server endpoint
    pub vps: VpsConfig,

    /// Socket networking configuration
    pub socket_networking: SocketNetworkingConfig,

    /// OpenFlow privacy flow configuration
    pub openflow: OpenFlowPrivacyConfig,

    /// Additional containers (vector DB, bucket storage, etc.)
    pub containers: Vec<ContainerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardConfig {
    pub enabled: bool,
    pub container_id: u32,
    pub socket_port: String,
    pub zero_config: bool,
    pub listen_port: u16,
    pub resources: ContainerResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerResources {
    pub vcpus: u8,
    pub memory_mb: u32,
    pub disk_gb: u32,
    /// Incus image reference, e.g. images:debian/13
    pub os_template: String,
    pub swap_mb: u32,
    pub unprivileged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpConfig {
    pub enabled: bool,
    pub bridge_interface: String,
    pub wgcf_config: String,
    pub warp_license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XRayConfig {
    pub enabled: bool,
    pub container_id: u32,
    pub socket_port: String,
    pub socks_port: u16,
    pub vps_address: String,
    pub vps_port: u16,
    pub resources: ContainerResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsConfig {
    pub xray_server: String,
    pub xray_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketNetworkingConfig {
    pub enabled: bool,
    pub privacy_sockets: Vec<PrivacySocketPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySocketPort {
    pub name: String,
    pub container_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowPrivacyConfig {
    pub enabled: bool,
    #[serde(default = "default_security_enabled")]
    pub enable_security_flows: bool,
    #[serde(default = "default_obfuscation_level")]
    pub obfuscation_level: u8,
    pub privacy_flows: Vec<PrivacyFlowRule>,
    pub function_routing: Vec<FunctionRoute>,
}

fn default_security_enabled() -> bool {
    true
}

fn default_obfuscation_level() -> u8 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyFlowRule {
    pub priority: u16,
    pub match_fields: HashMap<String, String>,
    pub actions: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionRoute {
    pub function: String,
    pub target_socket: String,
    pub match_fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub id: u32,
    pub name: String,
    pub container_type: String,
}

#[derive(Debug, Clone)]
struct PrivacyHostBootstrapConfig {
    bridge_name: String,
    uplink_port: String,
    management_port: String,
    socket_port: String,
    management_cidr: String,
    openflow_controller: String,
}

impl PrivacyHostBootstrapConfig {
    fn from_env(bridge_name: &str) -> Self {
        Self {
            bridge_name: std::env::var("PRIVACY_BRIDGE_NAME")
                .unwrap_or_else(|_| bridge_name.to_string()),
            uplink_port: std::env::var("PRIVACY_UPLINK_PORT")
                .unwrap_or_else(|_| DEFAULT_UPLINK_PORT.to_string()),
            management_port: std::env::var("PRIVACY_MGMT_PORT")
                .unwrap_or_else(|_| DEFAULT_MGMT_PORT.to_string()),
            socket_port: std::env::var("PRIVACY_SOCKET_PORT")
                .unwrap_or_else(|_| DEFAULT_SOCKET_PORT.to_string()),
            management_cidr: std::env::var("PRIVACY_MGMT_CIDR")
                .unwrap_or_else(|_| DEFAULT_MGMT_CIDR.to_string()),
            openflow_controller: std::env::var("PRIVACY_OPENFLOW_CONTROLLER")
                .unwrap_or_else(|_| DEFAULT_OPENFLOW_CONTROLLER.to_string()),
        }
    }
}

impl Default for PrivacyRouterConfig {
    fn default() -> Self {
        Self {
            bridge_name: DEFAULT_BRIDGE_NAME.to_string(),
            wireguard: WireGuardConfig {
                enabled: true,
                container_id: 100,
                socket_port: "priv_wg".to_string(),
                zero_config: true,
                listen_port: 51820,
                resources: default_resources(),
            },
            warp: WarpConfig {
                enabled: true,
                bridge_interface: DEFAULT_WARP_INTERFACE.to_string(),
                wgcf_config: DEFAULT_WGCF_CONFIG.to_string(),
                warp_license: None,
            },
            xray: XRayConfig {
                enabled: true,
                container_id: 101,
                socket_port: "priv_xray".to_string(),
                socks_port: 1080,
                vps_address: "vps.example.com".to_string(),
                vps_port: 443,
                resources: default_resources(),
            },
            vps: VpsConfig {
                xray_server: "vps.example.com".to_string(),
                xray_port: 443,
            },
            socket_networking: SocketNetworkingConfig {
                enabled: true,
                privacy_sockets: vec![
                    PrivacySocketPort {
                        name: "priv_wg".to_string(),
                        container_id: Some(100),
                    },
                    PrivacySocketPort {
                        name: "priv_xray".to_string(),
                        container_id: Some(101),
                    },
                ],
            },
            openflow: OpenFlowPrivacyConfig {
                enabled: true,
                enable_security_flows: true,
                obfuscation_level: 2,
                privacy_flows: default_privacy_flows(),
                function_routing: vec![],
            },
            containers: vec![],
        }
    }
}

fn default_resources() -> ContainerResources {
    ContainerResources {
        vcpus: 1,
        memory_mb: 512,
        disk_gb: 4,
        os_template: "images:debian/13".to_string(),
        swap_mb: 0,
        unprivileged: false,
    }
}

fn default_privacy_flows() -> Vec<PrivacyFlowRule> {
    vec![
        PrivacyFlowRule {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "priv_wg".to_string())]),
            actions: vec!["output:wgcf".to_string()],
            description: Some("priv_wg -> wgcf".to_string()),
        },
        PrivacyFlowRule {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "wgcf".to_string())]),
            actions: vec!["output:priv_xray".to_string()],
            description: Some("wgcf -> priv_xray".to_string()),
        },
        PrivacyFlowRule {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "priv_xray".to_string())]),
            actions: vec!["output:wgcf".to_string()],
            description: Some("priv_xray -> wgcf".to_string()),
        },
    ]
}

pub struct PrivacyRouterPlugin {
    config: PrivacyRouterConfig,
    routes_store: PrivacyRoutesPlugin,
}

impl PrivacyRouterPlugin {
    pub fn new(config: PrivacyRouterConfig) -> Self {
        Self {
            config,
            routes_store: PrivacyRoutesPlugin::default(),
        }
    }

    async fn query_privacy_routes(&self) -> Result<PrivacyRoutesState> {
        let state = self.routes_store.query_current_state().await?;
        Ok(simd_json::serde::from_owned_value(state)?)
    }

    async fn query_incus_state(&self) -> Result<IncusState> {
        let state = IncusPlugin::new().query_current_state().await?;
        Ok(simd_json::serde::from_owned_value(state)?)
    }

    async fn query_openflow_state(&self) -> Result<OpenFlowConfig> {
        let state = OpenFlowPlugin::new().query_current_state().await?;
        Ok(simd_json::serde::from_owned_value(state)?)
    }

    async fn query_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        OvsdbClient::new()
            .list_bridge_ports(bridge_name)
            .await
            .with_context(|| format!("list ports on {}", bridge_name))
    }

    fn unique_ingress_ports(routes: &[PrivacyRoute]) -> Vec<String> {
        let mut ingress_ports: HashSet<String> = routes
            .iter()
            .map(|route| route.ingress_port.clone())
            .collect();
        let mut ingress_ports: Vec<String> = ingress_ports.drain().collect();
        ingress_ports.sort();
        ingress_ports
    }

    fn desired_config_from_diff(&self, diff: &StateDiff) -> Result<PrivacyRouterConfig> {
        let mut merged = simd_json::serde::to_owned_value(self.config.clone())?;
        for action in &diff.actions {
            if let StateAction::Modify { changes, .. } = action {
                if let Some(config) = changes.get("config") {
                    Self::deep_merge(&mut merged, config);
                } else {
                    Self::deep_merge(&mut merged, changes);
                }
            }
        }
        Ok(simd_json::serde::from_owned_value(merged)?)
    }

    fn expected_system_container_names(config: &PrivacyRouterConfig) -> Vec<&'static str> {
        let mut names = Vec::new();
        if config.wireguard.enabled {
            names.push("privacy-wireguard-ingress");
        }
        if config.xray.enabled {
            names.push("privacy-xray-egress");
        }
        names
    }

    fn actual_system_containers(
        &self,
        config: &PrivacyRouterConfig,
        incus: &IncusState,
    ) -> Vec<String> {
        let expected: HashSet<&str> = Self::expected_system_container_names(config)
            .into_iter()
            .collect();
        let mut containers = incus
            .instances
            .iter()
            .filter(|instance| {
                expected.contains(instance.name.as_str())
                    && instance.status.eq_ignore_ascii_case("running")
            })
            .map(|instance| instance.name.clone())
            .collect::<Vec<_>>();
        containers.sort();
        containers
    }

    fn required_system_flow_count(&self, config: &PrivacyRouterConfig) -> usize {
        self.chain_ports(config).windows(2).count() * 2
    }

    async fn runtime_needs_reconcile(&self, config: &PrivacyRouterConfig) -> Result<bool> {
        if config.warp.enabled {
            match self.query_bridge_ports(&config.bridge_name).await {
                Ok(ports) => {
                    if !ports
                        .iter()
                        .any(|port| port == &config.warp.bridge_interface)
                    {
                        return Ok(true);
                    }
                }
                Err(_) => {
                    // Treat a missing bridge as drift so apply_state can build it.
                    return Ok(true);
                }
            }
        }

        let incus_state = self.query_incus_state().await?;
        let actual_containers = self.actual_system_containers(config, &incus_state);
        if actual_containers.len() != Self::expected_system_container_names(config).len() {
            return Ok(true);
        }

        let openflow_state = self.query_openflow_state().await?;
        let actual_flow_count = openflow_state
            .bridges
            .iter()
            .find(|bridge| bridge.name == config.bridge_name)
            .map(|bridge| {
                bridge
                    .flows
                    .iter()
                    .filter(|flow| flow.cookie.is_some_and(is_system_cookie))
                    .count()
            })
            .unwrap_or_default();

        Ok(config.openflow.enabled && actual_flow_count < self.required_system_flow_count(config))
    }

    fn deep_merge(target: &mut Value, source: &Value) {
        match (target, source) {
            (Value::Object(target_obj), Value::Object(source_obj)) => {
                for (key, value) in source_obj.iter() {
                    match target_obj.get_mut(key) {
                        Some(existing) => Self::deep_merge(existing, value),
                        None => {
                            target_obj.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
            (target_value, source_value) => {
                *target_value = source_value.clone();
            }
        }
    }

    async fn ensure_warp_interface_on_bridge(&self, config: &PrivacyRouterConfig) -> Result<()> {
        if !config.warp.enabled {
            return Ok(());
        }

        let ovs = op_network::OvsdbClient::new();
        let ports = ovs
            .list_bridge_ports(&config.bridge_name)
            .await
            .with_context(|| format!("list ports on {}", config.bridge_name))?;
        if ports
            .iter()
            .any(|port| port == &config.warp.bridge_interface)
        {
            let _ = op_network::rtnetlink::link_up(&config.warp.bridge_interface).await;
            return Ok(());
        }

        let interfaces = op_network::rtnetlink::list_interfaces()
            .await
            .context("list interfaces for warp attach")?;
        if !interfaces
            .iter()
            .any(|iface| iface.name == config.warp.bridge_interface)
        {
            if !std::path::Path::new(&config.warp.wgcf_config).exists() {
                bail!(
                    "warp interface '{}' missing and wgcf config '{}' not found",
                    config.warp.bridge_interface,
                    config.warp.wgcf_config
                );
            }
            self.ensure_wg_quick_interface(&config.warp.bridge_interface, &config.warp.wgcf_config)
                .await?;
        }

        ovs.add_port(&config.bridge_name, &config.warp.bridge_interface)
            .await
            .with_context(|| {
                format!(
                    "attach '{}' to '{}'",
                    config.warp.bridge_interface, config.bridge_name
                )
            })?;
        op_network::rtnetlink::link_up(&config.warp.bridge_interface)
            .await
            .with_context(|| format!("bring '{}' up", config.warp.bridge_interface))?;
        Ok(())
    }

    async fn ensure_host_bridge_topology(&self, config: &PrivacyRouterConfig) -> Result<()> {
        let host = PrivacyHostBootstrapConfig::from_env(&config.bridge_name);
        let ovs = OvsdbClient::new();

        ovs.list_dbs()
            .await
            .context("Open vSwitch DB is unavailable; cannot provision privacy bridge")?;

        if !ovs
            .bridge_exists(&host.bridge_name)
            .await
            .context("check privacy bridge existence")?
        {
            ovs.create_bridge(&host.bridge_name)
                .await
                .with_context(|| format!("create bridge '{}'", host.bridge_name))?;
        }

        ovs.set_bridge_property(&host.bridge_name, "datapath_type", "system")
            .await
            .context("set bridge datapath_type=system")?;
        ovs.set_bridge_property(&host.bridge_name, "fail_mode", "secure")
            .await
            .context("set bridge fail_mode=secure")?;

        let existing_ports = ovs
            .list_bridge_ports(&host.bridge_name)
            .await
            .with_context(|| format!("list bridge ports on '{}'", host.bridge_name))?;

        if !host.uplink_port.trim().is_empty() {
            let uplink_path = format!("/sys/class/net/{}", host.uplink_port);
            if !Path::new(&uplink_path).exists() {
                bail!(
                    "configured uplink '{}' not found on host ({})",
                    host.uplink_port,
                    uplink_path
                );
            }
            if !existing_ports.iter().any(|port| port == &host.uplink_port) {
                ovs.add_port(&host.bridge_name, &host.uplink_port)
                    .await
                    .with_context(|| {
                        format!(
                            "attach uplink '{}' to '{}'",
                            host.uplink_port, host.bridge_name
                        )
                    })?;
            }
        }

        if !existing_ports
            .iter()
            .any(|port| port == &host.management_port)
        {
            ovs.add_port_with_type(&host.bridge_name, &host.management_port, Some("internal"))
                .await
                .with_context(|| {
                    format!(
                        "add management port '{}' to '{}'",
                        host.management_port, host.bridge_name
                    )
                })?;
        }

        if !existing_ports.iter().any(|port| port == &host.socket_port) {
            ovs.add_port_with_type(&host.bridge_name, &host.socket_port, Some("internal"))
                .await
                .with_context(|| {
                    format!(
                        "add socket port '{}' to '{}'",
                        host.socket_port, host.bridge_name
                    )
                })?;
        }

        op_network::rtnetlink::link_up(&host.bridge_name)
            .await
            .with_context(|| format!("bring '{}' up", host.bridge_name))?;
        op_network::rtnetlink::link_up(&host.management_port)
            .await
            .with_context(|| format!("bring '{}' up", host.management_port))?;
        op_network::rtnetlink::link_up(&host.socket_port)
            .await
            .with_context(|| format!("bring '{}' up", host.socket_port))?;

        let (management_ip, management_prefix) = parse_cidr(&host.management_cidr)?;
        op_network::rtnetlink::flush_addresses(&host.management_port)
            .await
            .with_context(|| format!("flush addresses on '{}'", host.management_port))?;
        op_network::rtnetlink::add_ipv4_address(
            &host.management_port,
            &management_ip,
            management_prefix,
        )
        .await
        .with_context(|| {
            format!(
                "assign management CIDR '{}' to '{}'",
                host.management_cidr, host.management_port
            )
        })?;

        if let Ok(controller_addr) = host.openflow_controller.parse::<SocketAddr>() {
            match OpenFlowClient::connect(controller_addr).await {
                Ok(mut client) => {
                    if let Err(e) = client.request_features().await {
                        log::warn!(
                            "OpenFlow controller probe connected but feature request failed: {}",
                            e
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "OpenFlow controller '{}' is not reachable yet: {}",
                        host.openflow_controller,
                        e
                    );
                }
            }
        } else {
            log::warn!(
                "Invalid PRIVACY_OPENFLOW_CONTROLLER '{}'; skipping OpenFlow probe",
                host.openflow_controller
            );
        }

        Ok(())
    }

    async fn ensure_wg_quick_interface(&self, name: &str, config_path: &str) -> Result<()> {
        self.validate_wg_quick_config(name, config_path)?;
        self.run_command("/usr/bin/wg-quick", &["up", config_path])
            .await?;
        self.run_command("/usr/bin/ip", &["link", "set", "up", "dev", name])
            .await?;
        Ok(())
    }

    fn validate_wg_quick_config(&self, interface_name: &str, config_path: &str) -> Result<()> {
        let config = std::fs::read_to_string(config_path)
            .with_context(|| format!("read wg-quick config '{}'", config_path))?;
        let normalized = config
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();

        if !normalized
            .iter()
            .any(|line| line.eq_ignore_ascii_case("[Interface]"))
        {
            bail!(
                "wg-quick config '{}' for '{}' is missing [Interface]",
                config_path,
                interface_name
            );
        }
        if !normalized.iter().any(|line| {
            line.split_once('=')
                .map(|(key, value)| {
                    key.trim().eq_ignore_ascii_case("PrivateKey") && !value.trim().is_empty()
                })
                .unwrap_or(false)
        }) {
            bail!(
                "wg-quick config '{}' for '{}' is missing PrivateKey",
                config_path,
                interface_name
            );
        }
        if !normalized.iter().any(|line| {
            line.split_once('=')
                .map(|(key, value)| {
                    key.trim().eq_ignore_ascii_case("Table")
                        && value.trim().eq_ignore_ascii_case("off")
                })
                .unwrap_or(false)
        }) {
            bail!(
                "wg-quick config '{}' for '{}' must set 'Table = off' before bridging to OVS",
                config_path,
                interface_name
            );
        }

        Ok(())
    }

    async fn run_command(&self, binary: &str, args: &[&str]) -> Result<()> {
        let output = Command::new(binary)
            .args(args)
            .output()
            .await
            .with_context(|| format!("execute {}", binary))?;
        if !output.status.success() {
            bail!(
                "{} {} failed (exit {}): {}",
                binary,
                args.join(" "),
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn system_container_specs<'a>(
        &'a self,
        config: &'a PrivacyRouterConfig,
    ) -> Vec<SystemContainerSpec<'a>> {
        let mut specs = Vec::new();
        if config.wireguard.enabled {
            specs.push(SystemContainerSpec {
                name: "privacy-wireguard-ingress",
                role: "wireguard_ingress",
                socket_port: &config.wireguard.socket_port,
                resources: &config.wireguard.resources,
            });
        }
        if config.xray.enabled {
            specs.push(SystemContainerSpec {
                name: "privacy-xray-egress",
                role: "xray_reality_client",
                socket_port: &config.xray.socket_port,
                resources: &config.xray.resources,
            });
        }
        specs
    }

    fn desired_system_instance(
        &self,
        config: &PrivacyRouterConfig,
        spec: &SystemContainerSpec<'_>,
    ) -> IncusInstance {
        let devices = HashMap::from([(
            "fabric0".to_string(),
            HashMap::from([
                ("type".to_string(), "nic".to_string()),
                ("nictype".to_string(), "bridged".to_string()),
                ("parent".to_string(), config.bridge_name.clone()),
                ("name".to_string(), "eth0".to_string()),
                ("host_name".to_string(), spec.socket_port.to_string()),
            ]),
        )]);

        IncusInstance {
            name: spec.name.to_string(),
            status: "Running".to_string(),
            instance_type: "container".to_string(),
            image: Some(spec.resources.os_template.clone()),
            storage_pool: Some(
                std::env::var("PRIVACY_SYSTEM_STORAGE_POOL")
                    .or_else(|_| std::env::var("INCUS_STORAGE_POOL"))
                    .unwrap_or_else(|_| "default".to_string()),
            ),
            profiles: Vec::new(),
            config: Some(HashMap::from([
                ("boot.autostart".to_string(), "true".to_string()),
                ("security.nesting".to_string(), "true".to_string()),
                (
                    "security.privileged".to_string(),
                    (!spec.resources.unprivileged).to_string(),
                ),
                ("user.opdbus.scope".to_string(), "system".to_string()),
                (
                    "user.opdbus.component".to_string(),
                    "privacy_router".to_string(),
                ),
                ("user.opdbus.role".to_string(), spec.role.to_string()),
                (
                    "user.opdbus.host_port".to_string(),
                    spec.socket_port.to_string(),
                ),
            ])),
            devices: Some(devices),
        }
    }

    fn upsert_instance(instances: &mut Vec<IncusInstance>, desired: IncusInstance) {
        match instances
            .iter_mut()
            .find(|existing| existing.name == desired.name)
        {
            Some(existing) => *existing = desired,
            None => instances.push(desired),
        }
        instances.sort_by(|a, b| a.name.cmp(&b.name));
    }

    async fn apply_incus_system_containers(
        &self,
        config: &PrivacyRouterConfig,
    ) -> Result<ApplyResult> {
        let plugin = IncusPlugin::new();
        let current_state = plugin.query_current_state().await?;
        let mut desired_state: IncusState =
            simd_json::serde::from_owned_value(current_state.clone())
                .context("deserialize current incus state")?;

        for spec in self.system_container_specs(config) {
            Self::upsert_instance(
                &mut desired_state.instances,
                self.desired_system_instance(config, &spec),
            );
        }

        let desired_value = simd_json::serde::to_owned_value(desired_state)?;
        let diff = plugin
            .calculate_diff(&current_state, &desired_value)
            .await?;
        if diff.actions.is_empty() {
            return Ok(ApplyResult {
                success: true,
                changes_applied: vec!["System privacy containers already in sync".to_string()],
                errors: Vec::new(),
                checkpoint: None,
            });
        }
        plugin.apply_state(&diff).await
    }

    fn chain_ports(&self, config: &PrivacyRouterConfig) -> Vec<String> {
        let mut ports = Vec::new();
        if config.wireguard.enabled {
            ports.push(config.wireguard.socket_port.clone());
        }
        if config.warp.enabled {
            ports.push(config.warp.bridge_interface.clone());
        }
        if config.xray.enabled {
            ports.push(config.xray.socket_port.clone());
        }
        ports
    }

    fn merge_openflow_config(
        &self,
        mut current: OpenFlowConfig,
        config: &PrivacyRouterConfig,
    ) -> OpenFlowConfig {
        let bridge_index = current
            .bridges
            .iter()
            .position(|bridge| bridge.name == config.bridge_name);
        let mut bridge = bridge_index
            .map(|index| current.bridges.remove(index))
            .unwrap_or(BridgeFlowConfig {
                name: config.bridge_name.clone(),
                flows: Vec::new(),
                socket_ports: None,
            });

        bridge
            .flows
            .retain(|flow| !flow.cookie.is_some_and(is_system_cookie));

        let ports = self.chain_ports(config);
        for (index, path) in ports.windows(2).enumerate() {
            bridge.flows.push(chain_flow(index, &path[0], &path[1]));
            bridge
                .flows
                .push(chain_flow(index + 1000, &path[1], &path[0]));
        }
        bridge.flows.sort_by_key(flow_sort_key);

        current.bridges.push(bridge);
        current.bridges.sort_by(|a, b| a.name.cmp(&b.name));
        current.auto_discover_containers = false;
        current.enable_security_flows =
            current.enable_security_flows || config.openflow.enable_security_flows;
        current.obfuscation_level = current
            .obfuscation_level
            .max(config.openflow.obfuscation_level);
        current
    }

    async fn apply_openflow_system_chain(
        &self,
        config: &PrivacyRouterConfig,
    ) -> Result<ApplyResult> {
        let plugin = OpenFlowPlugin::new();
        let current_state = plugin.query_current_state().await?;
        let current_config: OpenFlowConfig =
            simd_json::serde::from_owned_value(current_state.clone())?;
        let desired_config = self.merge_openflow_config(current_config, config);
        let desired_value = simd_json::serde::to_owned_value(desired_config)?;
        let diff = plugin
            .calculate_diff(&current_state, &desired_value)
            .await?;
        if diff.actions.is_empty() {
            return Ok(ApplyResult {
                success: true,
                changes_applied: vec!["Privacy router OpenFlow chain already in sync".to_string()],
                errors: Vec::new(),
                checkpoint: None,
            });
        }
        plugin.apply_state(&diff).await
    }
}

struct SystemContainerSpec<'a> {
    name: &'a str,
    role: &'a str,
    socket_port: &'a str,
    resources: &'a ContainerResources,
}

fn chain_flow(index: usize, in_port: &str, out_port: &str) -> FlowEntry {
    FlowEntry {
        table: 0,
        priority: 21000,
        match_fields: HashMap::from([
            ("in_port".to_string(), in_port.to_string()),
            ("ip".to_string(), "".to_string()),
        ]),
        actions: vec![FlowAction::Output {
            port: out_port.to_string(),
        }],
        cookie: Some(SYSTEM_FLOW_COOKIE_PREFIX | index as u64),
        idle_timeout: 0,
        hard_timeout: 0,
    }
}

fn is_system_cookie(cookie: u64) -> bool {
    cookie & SYSTEM_FLOW_COOKIE_MASK == SYSTEM_FLOW_COOKIE_PREFIX
}

fn flow_sort_key(flow: &FlowEntry) -> (u8, u16, u64) {
    (flow.table, flow.priority, flow.cookie.unwrap_or_default())
}

#[async_trait]
impl StatePlugin for PrivacyRouterPlugin {
    fn name(&self) -> &'static str {
        "privacy_router"
    }

    fn version(&self) -> &str {
        "1.2.0"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }

    async fn query_current_state(&self) -> Result<Value> {
        let privacy_routes = self
            .query_privacy_routes()
            .await
            .unwrap_or(PrivacyRoutesState { routes: Vec::new() });
        let incus_state = self.query_incus_state().await.unwrap_or(IncusState {
            instances: Vec::new(),
        });
        let openflow_state = self.query_openflow_state().await.unwrap_or(OpenFlowConfig {
            bridges: Vec::new(),
            controller_endpoint: None,
            flow_policies: None,
            auto_discover_containers: false,
            enable_security_flows: false,
            obfuscation_level: 0,
        });

        let mut components = simd_json::owned::Object::new();

        if self.config.wireguard.enabled {
            components.insert(
                "wireguard".to_string(),
                json!({
                    "enabled": true,
                    "container_id": self.config.wireguard.container_id,
                    "socket_port": self.config.wireguard.socket_port,
                }),
            );
        }
        if self.config.warp.enabled {
            components.insert(
                "warp".to_string(),
                json!({
                    "enabled": true,
                    "bridge_interface": self.config.warp.bridge_interface,
                    "wgcf_config": self.config.warp.wgcf_config,
                }),
            );
        }
        if self.config.xray.enabled {
            components.insert(
                "xray".to_string(),
                json!({
                    "enabled": true,
                    "container_id": self.config.xray.container_id,
                    "socket_port": self.config.xray.socket_port,
                    "upstream_server": self.config.vps.xray_server,
                    "upstream_port": self.config.vps.xray_port,
                }),
            );
        }
        if self.config.openflow.enabled {
            let system_flow_count = openflow_state
                .bridges
                .iter()
                .find(|bridge| bridge.name == self.config.bridge_name)
                .map(|bridge| {
                    bridge
                        .flows
                        .iter()
                        .filter(|flow| flow.cookie.is_some_and(is_system_cookie))
                        .count()
                })
                .unwrap_or_default();
            components.insert(
                "openflow".to_string(),
                json!({
                    "enabled": true,
                    "enable_security_flows": self.config.openflow.enable_security_flows,
                    "obfuscation_level": self.config.openflow.obfuscation_level,
                    "privacy_flows": system_flow_count,
                    "function_routes": self.config.openflow.function_routing.len(),
                    "published_routes": privacy_routes.routes.len(),
                    "shared_ingress_ports": Self::unique_ingress_ports(&privacy_routes.routes),
                }),
            );
        }
        components.insert(
            "containers".to_string(),
            json!(self.actual_system_containers(&self.config, &incus_state)),
        );

        Ok(json!({
            "config": self.config,
            "components": components
        }))
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();
        let current_config = current.get("config").unwrap_or(current);
        let desired_config = desired.get("config").unwrap_or(desired);
        let desired_runtime: PrivacyRouterConfig =
            simd_json::serde::from_owned_value(desired_config.clone())?;

        if current_config != desired_config
            || self.runtime_needs_reconcile(&desired_runtime).await?
        {
            actions.push(StateAction::Modify {
                resource: "privacy_router_config".to_string(),
                changes: desired.clone(),
            });
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let config = self.desired_config_from_diff(diff)?;
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        self.ensure_host_bridge_topology(&config).await?;
        self.ensure_warp_interface_on_bridge(&config).await?;

        let incus_result = self.apply_incus_system_containers(&config).await?;
        changes_applied.extend(incus_result.changes_applied);
        errors.extend(incus_result.errors);

        if !errors.is_empty() {
            return Ok(ApplyResult {
                success: false,
                changes_applied,
                errors,
                checkpoint: None,
            });
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let openflow_result = self.apply_openflow_system_chain(&config).await?;
        changes_applied.extend(openflow_result.changes_applied);
        errors.extend(openflow_result.errors);

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        Ok(self
            .calculate_diff(&current, desired)
            .await?
            .actions
            .is_empty())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!(
                "privacy_router_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
            ),
            plugin: self.name().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        log::info!(
            "Rolling back privacy router to checkpoint: {}",
            checkpoint.id
        );
        Err(anyhow::anyhow!(
            "Privacy router rollback not yet implemented"
        ))
    }
}

fn parse_cidr(cidr: &str) -> Result<(String, u8)> {
    let mut parts = cidr.split('/');
    let ip = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid CIDR '{}': missing IP", cidr))?;
    let prefix = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid CIDR '{}': missing prefix", cidr))?
        .parse::<u8>()
        .with_context(|| format!("invalid CIDR prefix in '{}'", cidr))?;
    if parts.next().is_some() {
        bail!("invalid CIDR '{}': too many separators", cidr);
    }
    Ok((ip.to_string(), prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_config_merges_partial_overlay() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let diff = StateDiff {
            plugin: "privacy_router".to_string(),
            actions: vec![StateAction::Modify {
                resource: "privacy_router_config".to_string(),
                changes: json!({
                    "xray": {
                        "vps_address": "xray.example.com"
                    }
                }),
            }],
            metadata: DiffMetadata {
                timestamp: 0,
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        };

        let config = plugin.desired_config_from_diff(&diff).expect("config");
        assert_eq!(config.xray.vps_address, "xray.example.com");
        assert_eq!(config.bridge_name, DEFAULT_BRIDGE_NAME);
    }

    #[test]
    fn chain_ports_follow_enabled_system_components() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let config = PrivacyRouterConfig::default();
        assert_eq!(
            plugin.chain_ports(&config),
            vec![
                config.wireguard.socket_port.clone(),
                config.warp.bridge_interface.clone(),
                config.xray.socket_port.clone(),
            ]
        );
    }

    #[test]
    fn desired_system_instance_sets_privileged_system_container_flags() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let config = PrivacyRouterConfig::default();
        let spec = SystemContainerSpec {
            name: "privacy-wireguard-ingress",
            role: "wireguard_ingress",
            socket_port: &config.wireguard.socket_port,
            resources: &config.wireguard.resources,
        };

        let instance = plugin.desired_system_instance(&config, &spec);
        let config = instance.config.expect("instance config");

        assert_eq!(config.get("security.nesting"), Some(&"true".to_string()));
        assert_eq!(config.get("security.privileged"), Some(&"true".to_string()));
    }

    #[test]
    fn actual_system_containers_require_running_status() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let config = PrivacyRouterConfig::default();
        let instances = vec![
            IncusInstance {
                name: "privacy-wireguard-ingress".to_string(),
                status: "Stopped".to_string(),
                instance_type: "container".to_string(),
                image: None,
                storage_pool: None,
                profiles: Vec::new(),
                config: None,
                devices: None,
            },
            IncusInstance {
                name: "privacy-xray-egress".to_string(),
                status: "Running".to_string(),
                instance_type: "container".to_string(),
                image: None,
                storage_pool: None,
                profiles: Vec::new(),
                config: None,
                devices: None,
            },
        ];

        let actual = plugin.actual_system_containers(&config, &IncusState { instances });
        assert_eq!(actual, vec!["privacy-xray-egress".to_string()]);
    }
}
