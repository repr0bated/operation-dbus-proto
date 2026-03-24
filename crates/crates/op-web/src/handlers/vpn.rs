//! VPN Status and Connection Handlers

use axum::{extract::Extension, response::Json};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use tracing::{error, warn};

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct VpnStatus {
    pub running: bool,
    pub interface: String,
    pub active_connections: usize,
    pub peer_count: usize,
    pub bandwidth: Bandwidth,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bandwidth {
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VpnConnection {
    pub user_id: String,
    pub email: String,
    pub ip: String,
    pub connected_since: String,
    pub rx: u64,
    pub tx: u64,
    pub last_handshake: String,
    pub public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VpnConfig {
    pub server_public_key: String,
    pub endpoint: String,
    pub network: String,
    pub dns: String,
}

/// GET /api/vpn/status - Get VPN server status
pub async fn vpn_status_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<VpnStatus> {
    let interface = "wg0";

    // Check if WireGuard is running by trying to get interface info
    let running = Command::new("wg")
        .args(&["show", interface])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if !running {
        return Json(VpnStatus {
            running: false,
            interface: interface.to_string(),
            active_connections: 0,
            peer_count: 0,
            bandwidth: Bandwidth { up: 0, down: 0 },
        });
    }

    // Parse wg show output to get peer count and bandwidth
    let output = Command::new("wg")
        .args(&["show", interface, "dump"])
        .output();

    let (peer_count, total_rx, total_tx) = match output {
        Ok(output) if output.status.success() => {
            let data = String::from_utf8_lossy(&output.stdout);
            parse_wg_dump(&data)
        }
        Ok(output) => {
            warn!(
                "wg show dump failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            (0, 0, 0)
        }
        Err(e) => {
            error!("Failed to run wg command: {}", e);
            (0, 0, 0)
        }
    };

    Json(VpnStatus {
        running: true,
        interface: interface.to_string(),
        active_connections: peer_count,
        peer_count,
        bandwidth: Bandwidth {
            up: total_tx,
            down: total_rx,
        },
    })
}

/// GET /api/vpn/connections - Get active VPN connections
pub async fn vpn_connections_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<Vec<VpnConnection>> {
    // TODO: Match WireGuard peers with users from database
    Json(vec![])
}

/// GET /api/vpn/config - Get VPN server configuration
pub async fn vpn_config_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<VpnConfig> {
    let interface = "wg0";

    // Get server public key
    let server_public_key = Command::new("wg")
        .args(&["show", interface, "public-key"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Get endpoint from environment or default
    let endpoint =
        std::env::var("VPN_ENDPOINT").unwrap_or_else(|_| "148.113.204.83:51820".to_string());

    Json(VpnConfig {
        server_public_key,
        endpoint,
        network: "10.100.0.0/24".to_string(),
        dns: "1.1.1.1".to_string(),
    })
}

// Helper structs for parsing
#[derive(Debug)]
struct WgPeer {
    public_key: String,
    rx: u64,
    tx: u64,
    last_handshake: String,
}

/// Parse `wg show dump` output
/// Format: interface, private-key, public-key, listen-port, fwmark
///         public-key, preshared-key, endpoint, allowed-ips, latest-handshake, transfer-rx, transfer-tx, persistent-keepalive
fn parse_wg_dump(data: &str) -> (usize, u64, u64) {
    let mut peer_count = 0;
    let mut total_rx = 0u64;
    let mut total_tx = 0u64;

    for (i, line) in data.lines().enumerate() {
        if i == 0 {
            // Skip interface line
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 7 {
            peer_count += 1;

            // Parse transfer-rx (index 5)
            if let Ok(rx) = parts[5].parse::<u64>() {
                total_rx += rx;
            }

            // Parse transfer-tx (index 6)
            if let Ok(tx) = parts[6].parse::<u64>() {
                total_tx += tx;
            }
        }
    }

    (peer_count, total_rx, total_tx)
}

/// Parse `wg show dump` output into peer list
fn parse_wg_peers(data: &str) -> Vec<WgPeer> {
    let mut peers = Vec::new();

    for (i, line) in data.lines().enumerate() {
        if i == 0 {
            // Skip interface line
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 7 {
            let public_key = parts[0].to_string();
            let last_handshake_ts = parts[4].parse::<i64>().unwrap_or(0);
            let rx = parts[5].parse::<u64>().unwrap_or(0);
            let tx = parts[6].parse::<u64>().unwrap_or(0);

            // Convert timestamp to relative time
            let last_handshake = if last_handshake_ts > 0 {
                let now = chrono::Utc::now().timestamp();
                let seconds_ago = now - last_handshake_ts;
                format_duration(seconds_ago)
            } else {
                "never".to_string()
            };

            peers.push(WgPeer {
                public_key,
                rx,
                tx,
                last_handshake,
            });
        }
    }

    peers
}

fn format_duration(seconds: i64) -> String {
    if seconds < 60 {
        format!("{}s ago", seconds)
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h ago", seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86400)
    }
}
