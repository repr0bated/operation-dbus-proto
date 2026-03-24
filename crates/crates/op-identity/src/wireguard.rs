//! WireGuard identity detection and peer management.

use std::process::Command;
use tracing::{debug, warn};

/// WireGuard identity provider
#[derive(Debug, Clone)]
pub struct WireGuardIdentity {
    /// Interface name (default: wg0)
    interface: String,
}

impl WireGuardIdentity {
    pub fn new() -> Self {
        Self::with_interface("wg0")
    }

    pub fn with_interface(interface: &str) -> Self {
        Self {
            interface: interface.to_string(),
        }
    }

    /// Get the local WireGuard public key (this machine's identity)
    pub fn get_local_pubkey(&self) -> anyhow::Result<String> {
        // Try environment variable first
        if let Ok(pubkey) = std::env::var("WG_PUBKEY") {
            debug!("Using WG_PUBKEY from environment");
            return Ok(pubkey);
        }

        // Try to read from wg interface
        let output = Command::new("wg")
            .args(["show", &self.interface, "public-key"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let pubkey = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !pubkey.is_empty() {
                    debug!("Got pubkey from wg interface {}", self.interface);
                    return Ok(pubkey);
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                debug!("wg show failed: {}", stderr);
            }
            Err(e) => {
                debug!("Failed to run wg command: {}", e);
            }
        }

        // Fallback: generate a deterministic ID from hostname
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        warn!("Could not get WireGuard pubkey, using hostname-based ID");
        Ok(format!("local:{}", hostname))
    }

    /// Get peer's pubkey from their IP address
    pub fn get_pubkey_for_ip(&self, peer_ip: &str) -> anyhow::Result<Option<String>> {
        let output = Command::new("wg")
            .args(["show", &self.interface, "allowed-ips"])
            .output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Format: pubkey\tallowed_ip1, allowed_ip2
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let pubkey = parts[0];
                let ips = parts[1];

                if ips.contains(peer_ip) {
                    return Ok(Some(pubkey.to_string()));
                }
            }
        }

        Ok(None)
    }

    /// Get all connected peers with their latest handshake times
    pub fn get_connected_peers(&self) -> anyhow::Result<Vec<PeerInfo>> {
        let output = Command::new("wg")
            .args(["show", &self.interface, "latest-handshakes"])
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut peers = Vec::new();

        // Format: pubkey\ttimestamp
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let pubkey = parts[0].to_string();
                let timestamp: u64 = parts[1].parse().unwrap_or(0);

                // Only include peers with recent handshakes (within 3 minutes)
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                if timestamp > 0 && now - timestamp < 180 {
                    peers.push(PeerInfo {
                        pubkey,
                        last_handshake: timestamp,
                        allowed_ips: self.get_allowed_ips_for_peer(parts[0]).unwrap_or_default(),
                    });
                }
            }
        }

        Ok(peers)
    }

    /// Get allowed IPs for a specific peer
    fn get_allowed_ips_for_peer(&self, pubkey: &str) -> anyhow::Result<Vec<String>> {
        let output = Command::new("wg")
            .args(["show", &self.interface, "allowed-ips"])
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 && parts[0] == pubkey {
                return Ok(parts[1].split(',').map(|s| s.trim().to_string()).collect());
            }
        }

        Ok(Vec::new())
    }
}

impl Default for WireGuardIdentity {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a WireGuard peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub pubkey: String,
    pub last_handshake: u64,
    pub allowed_ips: Vec<String>,
}
