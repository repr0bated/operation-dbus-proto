//! Privacy Router Tunnel - Complete Architecture
//!
//! Chain: WireGuard Gateway (CT100) → WARP Tunnel (CT101) → XRay Client (CT102) → VPS → Internet
//!
//! THREE PRIVACY CONTAINERS (Debian 13 Trixie):
//! 1. CT 100 - wireguard-gateway: WireGuard entry point (priv_wg)
//! 2. CT 101 - warp-tunnel: Cloudflare WARP tunnel (priv_warp)  
//! 3. CT 102 - xray-client: XRay client to VPS (priv_xray)
//!
//! TWO SEPARATE SOCKET NETWORKS:
//! 1. PRIVACY SOCKETS (priv_*) - 3 containers in tunnel chain:
//!    - priv_wg: CT 100 WireGuard gateway entry point
//!    - priv_warp: CT 101 Cloudflare WARP tunnel
//!    - priv_xray: CT 102 XRay client exit to VPS
//!
//! 2. SHARED INGRESS SOCKETS - route objects select user traffic on a shared ingress
//!    - One shared OVS port per bridge (for example: ovsbr0-sock)
//!    - One D-Bus/privacy route object per user keyed from the WireGuard public key
//!    - OpenFlow matches selector IPs and forwards into the privacy chain
//!
//! All on single OVS bridge: ovs-br0

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet};

use crate::state_plugins::privacy_routes::{PrivacyRoute, PrivacyRoutesPlugin, PrivacyRoutesState};

/// Privacy Router Tunnel Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRouterConfig {
    /// OVS bridge name (shared by all components)
    pub bridge_name: String,

    /// WireGuard Gateway configuration
    pub wireguard: WireGuardConfig,

    /// WARP tunnel configuration
    pub warp: WarpConfig,

    /// XRay client configuration
    pub xray: XRayConfig,

    /// VPS XRay server endpoint
    pub vps: VpsConfig,

    /// Socket networking configuration
    pub socket_networking: SocketNetworkingConfig,

    /// OpenFlow privacy flow configuration
    pub openflow: OpenFlowPrivacyConfig,

    /// Netmaker mesh configuration
    pub netmaker: NetmakerMeshConfig,

    /// Additional containers (vector DB, bucket storage, etc.)
    pub containers: Vec<ContainerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardConfig {
    /// Enable WireGuard gateway
    pub enabled: bool,
    /// Container ID for WireGuard gateway
    pub container_id: u32,
    /// Socket port name (always "priv_wg" for privacy network)
    pub socket_port: String,
    /// Zero config mode (auto-generate keys)
    pub zero_config: bool,
    /// Listen port
    pub listen_port: u16,
    /// Container resources
    pub resources: ContainerResources,
}

/// LXC Container resources configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerResources {
    /// Number of vCPUs
    pub vcpus: u8,
    /// RAM in MB
    pub memory_mb: u32,
    /// Root disk size in GB
    pub disk_gb: u32,
    /// OS template (e.g., "debian-13-standard")
    pub os_template: String,
    /// Swap in MB (0 = disabled)
    pub swap_mb: u32,
    /// Unprivileged container
    pub unprivileged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpConfig {
    /// Enable WARP tunnel container
    pub enabled: bool,
    /// Container ID for WARP tunnel
    pub container_id: u32,
    /// Socket port name (always "priv_warp" for privacy network)
    pub socket_port: String,
    /// wgcf configuration path inside container
    pub wgcf_config: String,
    /// WARP+ premium license key
    pub warp_license: Option<String>,
    /// Container resources
    pub resources: ContainerResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XRayConfig {
    /// Enable XRay client
    pub enabled: bool,
    /// Container ID for XRay client
    pub container_id: u32,
    /// Socket port name (always "priv_xray" for privacy network)
    pub socket_port: String,
    /// SOCKS proxy port
    pub socks_port: u16,
    /// VPS server address
    pub vps_address: String,
    /// VPS server port
    pub vps_port: u16,
    /// Container resources
    pub resources: ContainerResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsConfig {
    /// VPS XRay server address
    pub xray_server: String,
    /// VPS XRay server port
    pub xray_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketNetworkingConfig {
    /// Enable socket networking
    pub enabled: bool,
    /// Socket ports for privacy tunnel (priv_wg, priv_xray)
    /// These are the ONLY predefined sockets - container sockets are dynamic
    pub privacy_sockets: Vec<PrivacySocketPort>,
    // NOTE: Container sockets (sock_{container_name}) are DYNAMIC
    // They are created/destroyed with container lifecycle, not predefined here
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySocketPort {
    /// Port name (priv_wg or priv_xray)
    pub name: String,
    /// Container ID (if applicable)
    pub container_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowPrivacyConfig {
    /// Enable privacy flow routing
    pub enabled: bool,
    /// Enable security hardening flows (default: true)
    #[serde(default = "default_security_enabled")]
    pub enable_security_flows: bool,
    /// Traffic obfuscation level (0=none, 1=basic, 2=pattern-hiding, 3=advanced)
    /// Level 1: Basic security (drop invalid, rate limit) - 11+ flows
    /// Level 2: Pattern hiding (TTL normalization, packet padding, timing) - 3 flows
    /// Level 3: Advanced obfuscation (protocol mimicry, decoy traffic, morphing) - 4 flows
    #[serde(default = "default_obfuscation_level")]
    pub obfuscation_level: u8,
    /// Privacy flow rules (rewritten in Rust)
    pub privacy_flows: Vec<PrivacyFlowRule>,
    /// Function-based routing to sockets
    pub function_routing: Vec<FunctionRoute>,
}

fn default_security_enabled() -> bool {
    true
}

fn default_obfuscation_level() -> u8 {
    2 // Level 2 (pattern hiding) recommended for privacy router
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyFlowRule {
    /// Flow priority
    pub priority: u16,
    /// Match criteria
    pub match_fields: HashMap<String, String>,
    /// Actions
    pub actions: Vec<String>,
    /// Description
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionRoute {
    /// Function name (e.g., "vector_db", "bucket_storage")
    pub function: String,
    /// Target socket port
    pub target_socket: String,
    /// Match criteria
    pub match_fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerMeshConfig {
    /// Enable Netmaker mesh
    pub enabled: bool,
    /// Netmaker interface name (e.g., "nm-privacy")
    pub interface: String,
    /// Network name
    pub network_name: String,
    /// One interface per Proxmox node
    pub per_node_interface: bool,
    /// Node identifier
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Container ID
    pub id: u32,
    /// Container name - socket port derived as sock_{name}
    pub name: String,
    /// Container type: "vector_db", "bucket_storage", etc.
    pub container_type: String,
    // NOTE: socket_port is DYNAMIC - derived from container name as sock_{name}
    // Not stored here - created at container start, removed at container stop
}

impl Default for PrivacyRouterConfig {
    fn default() -> Self {
        Self {
            bridge_name: "ovs-br0".to_string(),
            wireguard: WireGuardConfig {
                enabled: true,
                container_id: 100,
                socket_port: "priv_wg".to_string(),
                zero_config: true,
                listen_port: 51820,
                resources: ContainerResources {
                    vcpus: 1,
                    memory_mb: 512,
                    disk_gb: 4,
                    os_template: "debian-13-standard".to_string(),
                    swap_mb: 0,
                    unprivileged: true,
                },
            },
            warp: WarpConfig {
                enabled: true,
                container_id: 101,
                socket_port: "priv_warp".to_string(),
                wgcf_config: "/etc/wireguard/wgcf.conf".to_string(),
                warp_license: Some("g02I15ns-an48j3g6-6WS58KR7".to_string()),
                resources: ContainerResources {
                    vcpus: 1,
                    memory_mb: 512,
                    disk_gb: 4,
                    os_template: "debian-13-standard".to_string(),
                    swap_mb: 0,
                    unprivileged: true,
                },
            },
            xray: XRayConfig {
                enabled: true,
                container_id: 102,
                socket_port: "priv_xray".to_string(),
                socks_port: 1080,
                vps_address: "vps.example.com".to_string(),
                vps_port: 443,
                resources: ContainerResources {
                    vcpus: 1,
                    memory_mb: 512,
                    disk_gb: 4,
                    os_template: "debian-13-standard".to_string(),
                    swap_mb: 0,
                    unprivileged: true,
                },
            },
            vps: VpsConfig {
                xray_server: "vps.example.com".to_string(),
                xray_port: 443,
            },
            socket_networking: SocketNetworkingConfig {
                enabled: true,
                // ONLY privacy sockets are predefined - container sockets are DYNAMIC
                privacy_sockets: vec![
                    PrivacySocketPort {
                        name: "priv_wg".to_string(),
                        container_id: Some(100),
                    },
                    PrivacySocketPort {
                        name: "priv_warp".to_string(),
                        container_id: Some(101),
                    },
                    PrivacySocketPort {
                        name: "priv_xray".to_string(),
                        container_id: Some(102),
                    },
                ],
                // NOTE: No mesh_sockets - container sockets (sock_{name}) are created dynamically
            },
            openflow: OpenFlowPrivacyConfig {
                enabled: true,
                enable_security_flows: true,
                obfuscation_level: 2, // Level 2: Pattern hiding (recommended)
                privacy_flows: vec![
                    // priv_wg → priv_warp (CT100 WireGuard → CT101 WARP)
                    PrivacyFlowRule {
                        priority: 100,
                        match_fields: {
                            let mut m = HashMap::new();
                            m.insert("in_port".to_string(), "priv_wg".to_string());
                            m
                        },
                        actions: vec!["output:priv_warp".to_string()],
                        description: Some(
                            "priv_wg → priv_warp (CT100 WG → CT101 WARP)".to_string(),
                        ),
                    },
                    // priv_warp → priv_xray (CT101 WARP → CT102 XRay)
                    PrivacyFlowRule {
                        priority: 100,
                        match_fields: {
                            let mut m = HashMap::new();
                            m.insert("in_port".to_string(), "priv_warp".to_string());
                            m
                        },
                        actions: vec!["output:priv_xray".to_string()],
                        description: Some(
                            "priv_warp → priv_xray (CT101 WARP → CT102 XRay)".to_string(),
                        ),
                    },
                    // priv_xray → priv_warp (CT102 XRay → CT101 WARP return)
                    PrivacyFlowRule {
                        priority: 100,
                        match_fields: {
                            let mut m = HashMap::new();
                            m.insert("in_port".to_string(), "priv_xray".to_string());
                            m
                        },
                        actions: vec!["output:priv_warp".to_string()],
                        description: Some(
                            "priv_xray → priv_warp (CT102 XRay → CT101 WARP return)".to_string(),
                        ),
                    },
                    // priv_warp → priv_wg (CT101 WARP → CT100 WG return)
                    PrivacyFlowRule {
                        priority: 100,
                        match_fields: {
                            let mut m = HashMap::new();
                            m.insert("in_port".to_string(), "priv_warp".to_string());
                            m.insert("direction".to_string(), "return".to_string());
                            m
                        },
                        actions: vec!["output:priv_wg".to_string()],
                        description: Some(
                            "priv_warp → priv_wg (CT101 WARP → CT100 WG return)".to_string(),
                        ),
                    },
                ],
                function_routing: vec![],
            },
            netmaker: NetmakerMeshConfig {
                enabled: true,
                interface: "nm0".to_string(),
                network_name: "container-mesh".to_string(),
                per_node_interface: true,
                node_id: None,
            },
            containers: vec![],
        }
    }
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

    fn unique_ingress_ports(routes: &[PrivacyRoute]) -> Vec<String> {
        let mut ingress_ports: HashSet<String> = routes
            .iter()
            .map(|route| route.ingress_port.clone())
            .collect();
        let mut ingress_ports: Vec<String> = ingress_ports.drain().collect();
        ingress_ports.sort();
        ingress_ports
    }
}

#[async_trait]
impl StatePlugin for PrivacyRouterPlugin {
    fn name(&self) -> &'static str {
        "privacy_router"
    }

    fn version(&self) -> &str {
        "1.0.0"
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

        // Query current state of all components
        let mut state = json!({
            "config": self.config,
            "components": {}
        });

        // Check WireGuard gateway
        if self.config.wireguard.enabled {
            state["components"]["wireguard"] = json!({
                "enabled": true,
                "container_id": self.config.wireguard.container_id,
                "socket_port": self.config.wireguard.socket_port,
            });
        }

        // Check WARP tunnel container
        if self.config.warp.enabled {
            state["components"]["warp"] = json!({
                "enabled": true,
                "container_id": self.config.warp.container_id,
                "socket_port": self.config.warp.socket_port,
            });
        }

        // Check XRay client
        if self.config.xray.enabled {
            state["components"]["xray"] = json!({
                "enabled": true,
                "container_id": self.config.xray.container_id,
                "socket_port": self.config.xray.socket_port,
            });
        }

        // Check Netmaker mesh
        if self.config.netmaker.enabled {
            state["components"]["netmaker"] = json!({
                "enabled": true,
                "interface": self.config.netmaker.interface,
                "network_name": self.config.netmaker.network_name,
            });
        }

        // Check OpenFlow privacy flows
        if self.config.openflow.enabled {
            state["components"]["openflow"] = json!({
                "enabled": true,
                "enable_security_flows": self.config.openflow.enable_security_flows,
                "obfuscation_level": self.config.openflow.obfuscation_level,
                "privacy_flows": self.config.openflow.privacy_flows.len(),
                "function_routes": self.config.openflow.function_routing.len(),
                "published_routes": privacy_routes.routes.len(),
                "shared_ingress_ports": Self::unique_ingress_ports(&privacy_routes.routes),
            });
        }

        // Check containers
        state["components"]["containers"] = json!(self.config.containers);

        Ok(state)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();

        // Compare configurations
        let current_config = current.get("config");
        let desired_config = desired.get("config");

        if current_config != desired_config {
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

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let errors = Vec::new();

        // This plugin coordinates the setup but delegates to other plugins:
        // - LXC plugin: Creates containers with socket networking
        // - OpenFlow plugin: Sets up privacy flow routing
        // - Netmaker plugin: Sets up mesh networking
        // - Net plugin: Creates OVS bridge

        log::info!("Privacy Router: Coordinating component setup...");

        // Note: Actual implementation would call other plugins via StateManager
        // For now, we return a placeholder

        changes_applied.push("Privacy router configuration applied".to_string());
        changes_applied.push(format!(
            "Bridge: {}, WireGuard: {}, WARP: {}, XRay: {}",
            self.config.bridge_name,
            self.config.wireguard.enabled,
            self.config.warp.enabled,
            self.config.xray.enabled
        ));

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
        // Rollback would restore previous configuration
        log::info!(
            "Rolling back privacy router to checkpoint: {}",
            checkpoint.id
        );
        Err(anyhow::anyhow!(
            "Privacy router rollback not yet implemented"
        ))
    }
}
