use op_state::StatePlugin;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateDiff, StateAction};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::collections::HashMap;
use tokio::process::Command;

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
    pub installed: bool,
    pub daemon_running: bool,
    pub networks: Vec<NetmakerNetwork>,
    pub public_ip: Option<String>,
    pub config: NetmakerConfig,
}

pub struct NetmakerPlugin {
    config: NetmakerConfig,
}

impl NetmakerPlugin {
    pub fn new(config: NetmakerConfig) -> Self {
        Self { config }
    }

    /// Check if netclient is installed
    async fn check_netclient_installed() -> Result<bool> {
        let output = Command::new("which").arg("netclient").output().await?;
        Ok(output.status.success())
    }

    /// Check if netclient daemon is running
    async fn check_daemon_running() -> Result<bool> {
        let output = Command::new("systemctl")
            .args(["is-active", "netclient"])
            .output()
            .await;
        Ok(output.is_ok() && output.unwrap().status.success())
    }

    /// Get current networks from netclient
    async fn get_networks(&self) -> Result<Vec<NetmakerNetwork>> {
        let output = Command::new("netclient").arg("list").output().await?;

        if !output.status.success() {
            return Ok(Vec::new()); // No networks or not connected
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut networks = Vec::new();

        // Parse netclient output
        // Format: "NETWORK NAME | CONNECTED | ADDRESS"
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                let network_name = parts[0].to_string();
                let connected =
                    parts[1].to_lowercase() == "yes" || parts[1].to_lowercase() == "true";
                let address = if parts.len() > 2 && !parts[2].is_empty() {
                    Some(parts[2].to_string())
                } else {
                    None
                };

                // Get peers for this network
                let peers = self
                    .get_network_peers(&network_name)
                    .await
                    .unwrap_or_default();

                networks.push(NetmakerNetwork {
                    name: network_name.clone(),
                    connected,
                    is_default: network_name == self.config.default_network,
                    node_id: None, // Would need to parse from daemon logs
                    peers,
                    address,
                });
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

    /// Get public IP (for NAT traversal info)
    async fn get_public_ip(&self) -> Result<Option<String>> {
        // Try to get public IP for Netmaker status
        let output = Command::new("curl")
            .args(["-s", "--max-time", "5", "https://api.ipify.org"])
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(Some(ip));
            }
        }

        Ok(None)
    }

    /// Join a Netmaker network
    async fn join_network(&self, network: &str, token: &str) -> Result<()> {
        let output = Command::new("netclient")
            .args(["join", "-t", token])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to join network {}: {}",
                network,
                stderr
            ));
        }

        Ok(())
    }

    /// Leave a Netmaker network
    #[allow(dead_code)]
    async fn leave_network(&self, network: &str) -> Result<()> {
        let output = Command::new("netclient")
            .args(["leave", network])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to leave network {}: {}",
                network,
                stderr
            ));
        }

        Ok(())
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

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }

    async fn query_current_state(&self) -> Result<Value> {
        let installed = Self::check_netclient_installed().await?;
        let daemon_running = if installed {
            Self::check_daemon_running().await.unwrap_or(false)
        } else {
            false
        };

        let networks = if daemon_running {
            self.get_networks().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let public_ip = self.get_public_ip().await.unwrap_or(None);

        let state = NetmakerState {
            installed,
            daemon_running,
            networks,
            public_ip,
            config: self.config.clone(),
        };

        Ok(simd_json::serde::to_owned_value(state)?)
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

        for action in &diff.actions {
            match action {
                StateAction::Create {
                    resource,
                    config: _,
                } => {
                    if resource == "netmaker_installation" {
                        // Install netclient
                        let install_result = Command::new("apt")
                            .args(["update", "&&", "apt", "install", "-y", "netclient"])
                            .status()
                            .await;

                        match install_result {
                            Ok(_) => {
                                changes_applied.push("Installed netclient package".to_string());
                                // Enable and start service
                                let _ = Command::new("systemctl")
                                    .args(["enable", "--now", "netclient"])
                                    .status()
                                    .await;
                            }
                            Err(e) => errors.push(format!("Failed to install netclient: {}", e)),
                        }
                    } else if resource.starts_with("netmaker_network_") {
                        let network = resource.strip_prefix("netmaker_network_").unwrap_or("");
                        if let Some(token) = &self.config.enrollment_token {
                            match self.join_network(network, token).await {
                                Ok(_) => changes_applied
                                    .push(format!("Joined Netmaker network {}", network)),
                                Err(e) => errors
                                    .push(format!("Failed to join network {}: {}", network, e)),
                            }
                        } else {
                            errors.push(format!(
                                "No enrollment token configured for network {}",
                                network
                            ));
                        }
                    }
                }
                _ => {} // Other actions not implemented yet
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
        let current = self.query_current_state().await?;
        Ok(self
            .calculate_diff(&current, desired)
            .await?
            .actions
            .is_empty())
    }

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        let state = self.query_current_state().await?;
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
