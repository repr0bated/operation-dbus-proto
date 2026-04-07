use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardIdentity {
    pub pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub pubkey: String,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
}

/// Get the WireGuard public key for a given peer IP
pub async fn get_peer_pubkey(peer_ip: &str) -> Result<Option<String>> {
    // Run `wg show wg0 allowed-ips` (assuming wg0, could make configurable)
    // Output format: <public-key>\t<allowed-ips>
    // e.g. "AbC...123\t10.100.0.2/32"
    
    let output = Command::new("wg")
        .arg("show")
        .arg("wg0") // TODO: Make interface configurable
        .arg("allowed-ips")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn 'wg' command. Is WireGuard tools installed?")?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        warn!("wg command failed: {}", String::from_utf8_lossy(&output.stderr));
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        
        let pubkey = parts[0];
        let allowed_ips = &parts[1..];
        
        // simple check: if any allowed IP exactly matches the peer IP (or contains it? usually /32 for peers)
        // For now, we look for exact match of IP/32 or IP
        for ip_cidr in allowed_ips {
            if ip_cidr.starts_with(peer_ip) {
                // strict check: 10.100.0.2 should match 10.100.0.2/32 but not 10.100.0.20
                // simple prefix match is risky.
                // stripping /32
                let clean_ip = ip_cidr.split('/').next().unwrap_or("");
                if clean_ip == peer_ip {
                    debug!("Found pubkey {} for IP {}", pubkey, peer_ip);
                    return Ok(Some(pubkey.to_string()));
                }
            }
        }
    }

    debug!("No WireGuard peer found for IP {}", peer_ip);
    Ok(None)
}

/// Get the local device's public key
pub async fn get_local_pubkey() -> Result<String> {
    let output = Command::new("wg")
        .arg("show")
        .arg("wg0")
        .arg("public-key")
        .output()
        .await?;
        
    if !output.status.success() {
        anyhow::bail!("Failed to get local pubkey");
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
