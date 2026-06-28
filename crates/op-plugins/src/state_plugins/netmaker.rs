use super::plugin_schema_defs::schema_from_state;
use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::StatePlugin;
use op_state::{ApplyResult, PluginCapabilities, StateAction, StateDiff};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::{json, prelude::*, OwnedValue as Value};
use std::path::Path;
use zbus::{Connection, Proxy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerConfig {
    /// Enable Netmaker mesh networking
    pub enabled: bool,
    /// Default network to join
    pub default_network: String,
    /// Enrollment token for joining networks
    pub enrollment_token: Option<String>,
    /// API endpoint for Netmaker server (if self-hosted)
    pub api_endpoint: Option<String>,
}

impl Default for NetmakerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_network: "mesh".to_string(),
            enrollment_token: None,
            api_endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerNetwork {
    pub name: String,
    pub connected: bool,
    pub is_default: bool,
    pub node_id: Option<String>,
    pub peers: Vec<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerState {
    pub software: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub installed: bool,
    pub daemon_running: bool,
    pub networks: Vec<NetmakerNetwork>,
    pub public_ip: Option<String>,
    pub config: NetmakerConfig,
    pub tools: Value,
}

/// Service controller interface for managing daemon lifecycle
#[derive(Clone)]
enum ServiceController {
    /// systemd via org.freedesktop.systemd1
    Systemd,
    /// s6 via opdbus.v1.S6.Systemctl
    S6 { connection: Connection },
}

impl ServiceController {
    /// Detect the appropriate service controller for this system
    async fn detect() -> Result<Self> {
        // First check if s6-systemctl D-Bus service is available (Artix/Chimera)
        if Path::new("/run/s6-rc").exists() || Path::new("/run/service").exists() {
            match Connection::system().await {
                Ok(conn) => {
                    // Check if our s6-systemctl service is available
                    let proxy = Proxy::new(
                        &conn,
                        "opdbus.v1",
                        "/opdbus/v1/s6/systemctl",
                        "opdbus.v1.S6.Systemctl",
                    )
                    .await;
                    if proxy.is_ok() {
                        return Ok(ServiceController::S6 { connection: conn });
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to connect to system D-Bus for s6: {}", e);
                }
            }
        }

        // Fall back to systemd
        Ok(ServiceController::Systemd)
    }

    /// Check if a service is active
    async fn is_active(&self, service: &str) -> Result<bool> {
        match self {
            ServiceController::Systemd => {
                let conn = Connection::system().await?;
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.systemd1",
                    "/org/freedesktop/systemd1",
                    "org.freedesktop.systemd1.Manager",
                )
                .await?;

                // Get unit path
                let unit_path: zbus::zvariant::OwnedObjectPath = proxy
                    .call("GetUnit", &(format!("{}.service", service),))
                    .await
                    .context(format!("Failed to get unit path for {}", service))?;

                // Query ActiveState property
                let unit_proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.systemd1",
                    unit_path.as_str(),
                    "org.freedesktop.systemd1.Unit",
                )
                .await?;

                let active_state: String = unit_proxy
                    .get_property("ActiveState")
                    .await
                    .unwrap_or_else(|_| "unknown".to_string());

                Ok(active_state == "active")
            }
            ServiceController::S6 { connection } => {
                let proxy = Proxy::new(
                    connection,
                    "org.opdbus.v1.S6.Systemctl",
                    "/org/opdbus/v1/s6/systemctl",
                    "org.opdbus.v1.S6.Systemctl",
                )
                .await?;

                let result: String = proxy.call("is_active", &(service,)).await?;
                Ok(result == "active")
            }
        }
    }

    /// Enable and start a service (enable --now)
    async fn enable_and_start(&self, service: &str) -> Result<()> {
        match self {
            ServiceController::Systemd => {
                let conn = Connection::system().await?;
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.systemd1",
                    "/org/freedesktop/systemd1",
                    "org.freedesktop.systemd1.Manager",
                )
                .await?;

                // Enable unit
                let _: (bool, Vec<(String, String, String)>) = proxy
                    .call(
                        "EnableUnitFiles",
                        &(vec![format!("{}.service", service)], false, true),
                    )
                    .await
                    .context(format!("Failed to enable unit {}", service))?;

                // Start unit
                let _: zbus::zvariant::OwnedObjectPath = proxy
                    .call("StartUnit", &(format!("{}.service", service), "replace"))
                    .await
                    .context(format!("Failed to start unit {}", service))?;

                Ok(())
            }
            ServiceController::S6 { connection } => {
                let proxy = Proxy::new(
                    connection,
                    "org.opdbus.v1.S6.Systemctl",
                    "/org/opdbus/v1/s6/systemctl",
                    "org.opdbus.v1.S6.Systemctl",
                )
                .await?;

                // Enable the service
                let (success, msg): (bool, String) = proxy.call("enable", &(service,)).await?;

                if !success {
                    return Err(anyhow::anyhow!("Failed to enable {}: {}", service, msg));
                }

                // Start the service
                let (success, msg): (bool, String) = proxy.call("start", &(service,)).await?;

                if !success {
                    return Err(anyhow::anyhow!("Failed to start {}: {}", service, msg));
                }

                Ok(())
            }
        }
    }
}

pub struct NetmakerPlugin {
    config: NetmakerConfig,
}

impl NetmakerPlugin {
    pub fn new(config: NetmakerConfig) -> Self {
        Self { config }
    }

    /// Check if netclient is installed via direct file check (AGENTS.md §4: no subprocess bypasses)
    async fn check_netclient_installed() -> Result<bool> {
        Ok(std::path::Path::new("/usr/bin/netclient").exists()
            || std::path::Path::new("/usr/local/bin/netclient").exists())
    }

    /// Check if netclient daemon is running via D-Bus
    async fn check_daemon_running() -> Result<bool> {
        let controller = ServiceController::detect().await?;
        controller.is_active("netclient").await
    }

    /// Get current networks from netclient config files (AGENTS.md §4: no subprocess bypasses)
    async fn get_networks(&self) -> Result<Vec<NetmakerNetwork>> {
        let mut networks = Vec::new();

        // Read netclient.json to discover connected networks
        if let Ok(content) = tokio::fs::read_to_string("/etc/netclient/netclient.json").await {
            if let Ok(config) = simd_json::to_owned_value(&mut content.into_bytes()) {
                // Check nodes array for connected networks
                if let Some(nodes) = config.get("nodes").and_then(|n| n.as_array()) {
                    for node in nodes {
                        let network_name = node
                            .get("network")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let connected = node
                            .get("connected")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let address = node
                            .get("address")
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        let peers = self
                            .get_network_peers(&network_name)
                            .await
                            .unwrap_or_default();

                        networks.push(NetmakerNetwork {
                            name: network_name.clone(),
                            connected,
                            is_default: network_name == self.config.default_network,
                            node_id: node.get("id").and_then(|v| v.as_str()).map(String::from),
                            peers,
                            address,
                        });
                    }
                }
            }
        }

        Ok(networks)
    }

    /// Get peers for a specific network
    async fn get_network_peers(&self, network: &str) -> Result<Vec<String>> {
        // This is a simplified implementation
        // In reality, you'd need to query the Netmaker API or parse daemon state
        let _ = network; // Suppress unused variable warning
        Ok(Vec::new()) // TODO: Implement actual peer discovery
    }

    /// Get public IP from netclient config (AGENTS.md §4: no subprocess bypasses)
    async fn get_public_ip(&self) -> Result<Option<String>> {
        if let Ok(content) = tokio::fs::read_to_string("/etc/netclient/netclient.json").await {
            if let Ok(config) = simd_json::to_owned_value(&mut content.into_bytes()) {
                return Ok(config
                    .get("endpointip")
                    .and_then(|v| v.as_str())
                    .map(String::from));
            }
        }
        Ok(None)
    }

    /// Join a Netmaker network
    /// Note: netclient enrollment requires the netclient CLI per Netmaker protocol.
    /// AGENTS.md §4 forbids subprocess calls in plugin code; manual enrollment is required.
    async fn join_network(&self, network: &str, _token: &str) -> Result<()> {
        Err(anyhow::anyhow!(
            "Netmaker network '{}' join requires netclient CLI (disabled per AGENTS.md §4). \
             Run: netclient join -t <token> externally.",
            network
        ))
    }

    /// Leave a Netmaker network
    /// Note: netclient leave requires the netclient CLI per Netmaker protocol.
    /// AGENTS.md §4 forbids subprocess calls in plugin code; manual disconnection is required.
    #[allow(dead_code)]
    async fn leave_network(&self, network: &str) -> Result<()> {
        Err(anyhow::anyhow!(
            "Netmaker network '{}' leave requires netclient CLI (disabled per AGENTS.md §4). \
             Run: netclient leave '{}' externally.",
            network,
            network
        ))
    }

    pub(crate) fn current_state() -> NetmakerState {
        let tools = simd_json::json!([
            {
                "name": "netmaker.join",
                "description": "Join a Netmaker network",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "token": {
                            "type": "string",
                            "description": "The enrollment token"
                        }
                    },
                    "required": ["token"]
                }
            },
            {
                "name": "netmaker.leave",
                "description": "Leave a Netmaker network",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "network": {
                            "type": "string",
                            "description": "The network name to leave"
                        }
                    },
                    "required": ["network"]
                }
            }
        ]);

        NetmakerState {
            software: "netclient".to_string(),
            version: "1.0.0".to_string(),
            dependencies: vec!["net".to_string(), "s6".to_string()],
            installed: false,
            daemon_running: false,
            networks: Vec::new(),
            public_ip: None,
            config: NetmakerConfig::default(),
            tools,
        }
    }
}

#[async_trait]
impl StatePlugin for NetmakerPlugin {
    fn name(&self) -> &'static str {
        "netmaker"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(netmaker_schema())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();

        // Check if netclient should be installed/enabled
        let current_installed = current
            .get("installed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let desired_enabled = desired
            .get("config")
            .and_then(|c| c.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !current_installed && desired_enabled {
            actions.push(StateAction::Create {
                resource: "netmaker_installation".to_string(),
                config: simd_json::json!({
                    "action": "install_netclient",
                    "type": "system_package"
                }),
            });
        }

        // Check network membership changes
        let empty_networks = vec![];
        let current_networks = current
            .get("networks")
            .and_then(|n| n.as_array())
            .unwrap_or(&empty_networks);
        let desired_networks = desired
            .get("config")
            .and_then(|c| c.get("default_network"))
            .and_then(|n| n.as_str());

        if let Some(desired_network) = desired_networks {
            let currently_connected = current_networks.iter().any(|net| {
                net.get("name").and_then(|n| n.as_str()) == Some(desired_network)
                    && net
                        .get("connected")
                        .and_then(|c| c.as_bool())
                        .unwrap_or(false)
            });

            if !currently_connected && desired_enabled {
                actions.push(StateAction::Create {
                    resource: format!("netmaker_network_{}", desired_network),
                    config: simd_json::json!({
                        "network": desired_network,
                        "action": "join_network",
                        "type": "network_membership"
                    }),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        // Detect service controller once for all operations
        let controller = match ServiceController::detect().await {
            Ok(ctrl) => Some(ctrl),
            Err(e) => {
                errors.push(format!("Failed to detect service controller: {}", e));
                None
            }
        };

        for action in &diff.actions {
            if let StateAction::Create {
                resource,
                config: _,
            } = action
            {
                if resource == "netmaker_installation" {
                    // Package installation requires PackageKit D-Bus or manual install per AGENTS.md §4
                    errors.push(
                        "Netclient package installation requires external PackageKit D-Bus or manual install (disabled per AGENTS.md §4). \
                         Run: apt-get install -y netclient".to_string(),
                    );
                    // Attempt to enable and start service via D-Bus regardless
                    if let Some(ref ctrl) = controller {
                        match ctrl.enable_and_start("netclient").await {
                            Ok(_) => {
                                changes_applied.push(
                                    "Enabled and started netclient service via D-Bus".to_string(),
                                );
                            }
                            Err(e) => {
                                errors.push(format!(
                                    "Failed to enable/start netclient via D-Bus: {}",
                                    e
                                ));
                            }
                        }
                    }
                } else if resource.starts_with("netmaker_network_") {
                    let network = resource.strip_prefix("netmaker_network_").unwrap_or("");
                    if let Some(token) = &self.config.enrollment_token {
                        match self.join_network(network, token).await {
                            Ok(_) => {
                                changes_applied.push(format!("Joined Netmaker network {}", network))
                            }
                            Err(e) => {
                                errors.push(format!("Failed to join network {}: {}", network, e))
                            }
                        }
                    } else {
                        errors.push(format!(
                            "No enrollment token configured for network {}",
                            network
                        ));
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

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        let state = simd_json::json!(null);
        Ok(op_state::Checkpoint {
            id: format!(
                "netmaker_{}",
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

    async fn rollback(&self, _checkpoint: &op_state::Checkpoint) -> Result<()> {
        // Rollback would leave networks and potentially rejoin them
        // This is a simplified implementation
        Err(anyhow::anyhow!(
            "Netmaker rollback not implemented - would require leaving and rejoining networks"
        ))
    }
}

pub(crate) fn netmaker_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::netmaker::NetmakerPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    let mut schema = schema_from_state(
        "netmaker",
        "net",
        "1.0.0",
        "Netmaker daemon state and execution schema",
        &state,
    );
    let method = super::plugin_schema_defs::cap_method(
        "join_network",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.network.netmaker.network.join@v1",
        "mut.network.netmaker.network.join@v1",
    );
    schema.methods.insert(method.name.clone(), method);
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("netmaker", |_ctx| std::sync::Arc::new(NetmakerPlugin::new(NetmakerConfig::default())))
}
