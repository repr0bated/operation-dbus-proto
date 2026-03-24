//! Host-level privacy network provisioning.
//!
//! This ensures a single host bridge topology exists:
//! - OVS bridge (`ovsbr0`)
//! - Uplink port (default `o`)
//! - Management internal port
//! - Socket-network internal port for container attachment
//! - OpenFlow controller reachability probe

use anyhow::{anyhow, Context, Result};
use op_network::{openflow::OpenFlowClient, OvsdbClient};
use std::net::SocketAddr;
use std::path::Path;
use tracing::{info, warn};

const DEFAULT_BRIDGE: &str = "ovsbr0";
const DEFAULT_UPLINK_PORT: &str = "o";
const DEFAULT_MGMT_PORT: &str = "ovsbr0-mgmt";
const DEFAULT_SOCKET_PORT: &str = "ovsbr0-sock";
const DEFAULT_MGMT_CIDR: &str = "10.200.0.1/24";
const DEFAULT_OPENFLOW_CONTROLLER: &str = "127.0.0.1:6653";

#[derive(Debug, Clone)]
pub struct PrivacyNetworkHostConfig {
    pub bridge_name: String,
    pub uplink_port: String,
    pub management_port: String,
    pub socket_port: String,
    pub management_cidr: String,
    pub openflow_controller: String,
}

impl PrivacyNetworkHostConfig {
    pub fn from_env() -> Self {
        Self {
            bridge_name: std::env::var("PRIVACY_BRIDGE_NAME")
                .unwrap_or_else(|_| DEFAULT_BRIDGE.to_string()),
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

/// Ensure host privacy network topology exists and is configured.
///
/// Idempotent: safe to call on each registration.
pub async fn ensure_host_privacy_network() -> Result<()> {
    let cfg = PrivacyNetworkHostConfig::from_env();
    ensure_host_privacy_network_with_config(&cfg).await
}

async fn ensure_host_privacy_network_with_config(cfg: &PrivacyNetworkHostConfig) -> Result<()> {
    info!(
        "Ensuring host privacy network: bridge={} uplink={} mgmt_port={} socket_port={}",
        cfg.bridge_name, cfg.uplink_port, cfg.management_port, cfg.socket_port
    );

    let ovs = OvsdbClient::new();

    // Verify OVSDB connectivity first.
    ovs.list_dbs()
        .await
        .context("Open vSwitch DB is unavailable; cannot provision host privacy network")?;

    // Ensure bridge exists.
    if !ovs
        .bridge_exists(&cfg.bridge_name)
        .await
        .context("Failed to check bridge existence")?
    {
        ovs.create_bridge(&cfg.bridge_name)
            .await
            .with_context(|| format!("Failed to create OVS bridge '{}'", cfg.bridge_name))?;
    }

    // Bridge behavior expected for controller-driven forwarding.
    ovs.set_bridge_property(&cfg.bridge_name, "datapath_type", "system")
        .await
        .context("Failed to set bridge datapath_type")?;
    ovs.set_bridge_property(&cfg.bridge_name, "fail_mode", "secure")
        .await
        .context("Failed to set bridge fail_mode=secure")?;

    let existing_ports = ovs
        .list_bridge_ports(&cfg.bridge_name)
        .await
        .context("Failed to list bridge ports")?;

    // Ensure host uplink attachment (single host-level uplink).
    if !cfg.uplink_port.trim().is_empty() {
        let uplink_path = format!("/sys/class/net/{}", cfg.uplink_port);
        if !Path::new(&uplink_path).exists() {
            return Err(anyhow!(
                "Configured uplink '{}' not found on host ({})",
                cfg.uplink_port,
                uplink_path
            ));
        }
        if !existing_ports.iter().any(|p| p == &cfg.uplink_port) {
            ovs.add_port(&cfg.bridge_name, &cfg.uplink_port)
                .await
                .with_context(|| {
                    format!(
                        "Failed to add uplink port '{}' to '{}'",
                        cfg.uplink_port, cfg.bridge_name
                    )
                })?;
        }
    }

    // Ensure management and socket ports as internal OVS ports.
    if !existing_ports.iter().any(|p| p == &cfg.management_port) {
        ovs.add_port_with_type(&cfg.bridge_name, &cfg.management_port, Some("internal"))
            .await
            .with_context(|| {
                format!(
                    "Failed to add management internal port '{}' to '{}'",
                    cfg.management_port, cfg.bridge_name
                )
            })?;
    }

    if !existing_ports.iter().any(|p| p == &cfg.socket_port) {
        ovs.add_port_with_type(&cfg.bridge_name, &cfg.socket_port, Some("internal"))
            .await
            .with_context(|| {
                format!(
                    "Failed to add socket internal port '{}' to '{}'",
                    cfg.socket_port, cfg.bridge_name
                )
            })?;
    }

    // Bring up host bridge and management interfaces.
    op_network::rtnetlink::link_up(&cfg.bridge_name)
        .await
        .with_context(|| format!("Failed to bring bridge '{}' up", cfg.bridge_name))?;
    op_network::rtnetlink::link_up(&cfg.management_port)
        .await
        .with_context(|| {
            format!(
                "Failed to bring management port '{}' up",
                cfg.management_port
            )
        })?;
    op_network::rtnetlink::link_up(&cfg.socket_port)
        .await
        .with_context(|| format!("Failed to bring socket port '{}' up", cfg.socket_port))?;

    // Ensure management CIDR on management port.
    let (mgmt_ip, mgmt_prefix) = parse_cidr(&cfg.management_cidr)?;
    op_network::rtnetlink::flush_addresses(&cfg.management_port)
        .await
        .with_context(|| {
            format!(
                "Failed to flush existing addresses on '{}'",
                cfg.management_port
            )
        })?;
    op_network::rtnetlink::add_ipv4_address(&cfg.management_port, &mgmt_ip, mgmt_prefix)
        .await
        .with_context(|| {
            format!(
                "Failed to set management CIDR '{}' on '{}'",
                cfg.management_cidr, cfg.management_port
            )
        })?;

    // Probe OpenFlow controller connectivity using native Rust implementation.
    if let Ok(controller_addr) = cfg.openflow_controller.parse::<SocketAddr>() {
        match OpenFlowClient::connect(controller_addr).await {
            Ok(mut client) => {
                if let Err(e) = client.request_features().await {
                    warn!(
                        "OpenFlow controller probe connected but feature request failed: {}",
                        e
                    );
                } else {
                    info!(
                        "OpenFlow controller reachable at {}",
                        cfg.openflow_controller
                    );
                }
            }
            Err(e) => {
                warn!(
                    "OpenFlow controller '{}' is not reachable yet: {}",
                    cfg.openflow_controller, e
                );
            }
        }
    } else {
        warn!(
            "Invalid PRIVACY_OPENFLOW_CONTROLLER '{}'; skipping OpenFlow probe",
            cfg.openflow_controller
        );
    }

    info!("Host privacy network provisioning complete");
    Ok(())
}

fn parse_cidr(cidr: &str) -> Result<(String, u8)> {
    let mut parts = cidr.split('/');
    let ip = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid CIDR '{}': missing IP", cidr))?;
    let prefix = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid CIDR '{}': missing prefix", cidr))?
        .parse::<u8>()
        .with_context(|| format!("Invalid CIDR prefix in '{}'", cidr))?;
    if parts.next().is_some() {
        return Err(anyhow!("Invalid CIDR '{}': too many separators", cidr));
    }
    Ok((ip.to_string(), prefix))
}
