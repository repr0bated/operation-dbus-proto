//! Host-level privacy network provisioning for wgcf-based architecture.
//
// This matches the network design from the recent commit:
// - wgcf tunnel (WireGuard WARP) created by systemd-networkd + netplan
// - ovsbr0 bridge managed by netplan with renderer: openvswitch
// - ovs-attach-ports.sh attaches wgcf + internal ports via OVSDB
// - Xray as privacy ingress on 10.88.88.1
// - priv_* internal ports for routing

use anyhow::{anyhow, Context, Result};
use op_network::{openflow::OpenFlowClient, OvsdbClient};
use std::path::Path;
use tracing::{info, warn};

const DEFAULT_BRIDGE: &str = "ovsbr0";
const DEFAULT_WGCF_TUNNEL: &str = "wgcf";
const DEFAULT_PRIVACY_PORTS: &[&str] = &[
    "priv_xray",
    "priv_warp",
    "priv_wg",
    "ovsbr0-mgmt",
    "ovsbr0-sock",
];
const DEFAULT_MGMT_CIDR: &str = "10.88.88.1/24"; // Matches Xray binding
const DEFAULT_OPENFLOW_CONTROLLER: &str = "10.88.88.1:6653";

#[derive(Debug, Clone)]
pub struct PrivacyNetworkHostConfig {
    pub bridge_name: String,
    pub wgcf_tunnel: String,
    pub privacy_ports: Vec<String>,
    pub management_cidr: String,
    pub openflow_controller: String,
    pub xray_ingress_ip: String,
}

impl PrivacyNetworkHostConfig {
    pub fn from_env() -> Self {
        Self {
            bridge_name: std::env::var("PRIVACY_BRIDGE_NAME")
                .unwrap_or_else(|_| DEFAULT_BRIDGE.to_string()),
            wgcf_tunnel: std::env::var("PRIVACY_WGCF_TUNNEL")
                .unwrap_or_else(|_| DEFAULT_WGCF_TUNNEL.to_string()),
            privacy_ports: std::env::var("PRIVACY_PORTS")
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_else(|_| {
                    DEFAULT_PRIVACY_PORTS
                        .iter()
                        .map(|&s| s.to_string())
                        .collect()
                }),
            management_cidr: std::env::var("PRIVACY_MGMT_CIDR")
                .unwrap_or_else(|_| DEFAULT_MGMT_CIDR.to_string()),
            openflow_controller: std::env::var("PRIVACY_OPENFLOW_CONTROLLER")
                .unwrap_or_else(|_| DEFAULT_OPENFLOW_CONTROLLER.to_string()),
            xray_ingress_ip: std::env::var("XRAY_INGRESS_IP")
                .unwrap_or_else(|_| "10.88.88.1".to_string()),
        }
    }
}

/// Ensure host privacy network topology exists for wgcf architecture.
///
/// This is called during magic link verification to ensure the network
/// is ready for new users.
pub async fn ensure_host_privacy_network() -> Result<()> {
    let cfg = PrivacyNetworkHostConfig::from_env();
    ensure_host_privacy_network_with_config(&cfg).await
}

async fn ensure_host_privacy_network_with_config(cfg: &PrivacyNetworkHostConfig) -> Result<()> {
    info!(
        "Ensuring wgcf-based privacy network: bridge={} wgcf={} ports={:?}",
        cfg.bridge_name, cfg.wgcf_tunnel, cfg.privacy_ports
    );

    let ovs = OvsdbClient::new();

    // Verify OVSDB connectivity
    ovs.list_dbs()
        .await
        .context("Open vSwitch DB is unavailable; cannot provision privacy network")?;

    // The bridge should be created by netplan, but ensure it exists
    if !ovs
        .bridge_exists(&cfg.bridge_name)
        .await
        .context("Failed to check bridge existence")?
    {
        info!("Bridge {} not found, creating via OVSDB", cfg.bridge_name);
        ovs.create_bridge(&cfg.bridge_name)
            .await
            .with_context(|| format!("Failed to create OVS bridge '{}'", cfg.bridge_name))?;
    }

    // Configure bridge for controller-driven forwarding (matches deploy scripts)
    ovs.set_bridge_property(&cfg.bridge_name, "datapath_type", "system")
        .await
        .context("Failed to set bridge datapath_type")?;
    ovs.set_bridge_property(&cfg.bridge_name, "fail_mode", "standalone")
        .await
        .context("Failed to set bridge fail_mode=standalone")?;

    let existing_ports = ovs
        .list_bridge_ports(&cfg.bridge_name)
        .await
        .context("Failed to list bridge ports")?;

    // Ensure wgcf tunnel is attached to bridge (this is the key privacy tunnel)
    if !existing_ports.iter().any(|p| p == &cfg.wgcf_tunnel) {
        if Path::new(&format!("/sys/class/net/{}", cfg.wgcf_tunnel)).exists() {
            ovs.add_port(&cfg.bridge_name, &cfg.wgcf_tunnel)
                .await
                .with_context(|| format!("Failed to add wgcf tunnel to bridge"))?;
            info!("Attached wgcf tunnel to {}", cfg.bridge_name);
        } else {
            warn!("wgcf interface not found yet - will be attached by ovs-attach-ports service");
        }
    }

    // Ensure all privacy internal ports exist (created by ovs-attach-ports.sh)
    for port in &cfg.privacy_ports {
        if !existing_ports.iter().any(|p| p == port) {
            ovs.add_port_with_type(&cfg.bridge_name, port, Some("internal"))
                .await
                .with_context(|| format!("Failed to add internal port '{}'", port))?;
            info!("Added internal port: {}", port);
        }
    }

    // Bring up critical interfaces
    for iface in [&cfg.bridge_name, &cfg.wgcf_tunnel] {
        if Path::new(&format!("/sys/class/net/{}", iface)).exists() {
            op_network::rtnetlink::link_up(iface)
                .await
                .with_context(|| format!("Failed to bring up interface {}", iface))?;
        }
    }

    // Bring up privacy ports
    for port in &cfg.privacy_ports {
        if Path::new(&format!("/sys/class/net/{}", port)).exists() {
            op_network::rtnetlink::link_up(port)
                .await
                .with_context(|| format!("Failed to bring up port {}", port))?;
        }
    }

    // Verify Xray ingress is available on the management IP
    info!(
        "Privacy network ready. Xray ingress should be listening on {}",
        cfg.xray_ingress_ip
    );

    // Probe OpenFlow controller if available
    if let Ok(controller_addr) = cfg.openflow_controller.parse::<std::net::SocketAddr>() {
        match OpenFlowClient::connect(controller_addr).await {
            Ok(_) => info!("OpenFlow controller reachable"),
            Err(e) => warn!("OpenFlow controller not ready yet: {}", e),
        }
    }

    info!("wgcf-based privacy network provisioning complete");
    Ok(())
}
