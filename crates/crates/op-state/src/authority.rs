/// Network Authority Enforcement
///
/// This module ensures the plugin system remains the ultimate authoritative source
/// for all network configuration, preventing interference from legacy systems.
use anyhow::Result;
use std::process::Command;

pub struct NetworkAuthority;

impl NetworkAuthority {
    /// Ensure no competing network managers are active
    pub fn enforce_authority() -> Result<()> {
        // Disable NetworkManager if running
        let _ = Command::new("systemctl")
            .args(["stop", "NetworkManager"])
            .output();

        let _ = Command::new("systemctl")
            .args(["disable", "NetworkManager"])
            .output();

        // Disable systemd-networkd if running
        let _ = Command::new("systemctl")
            .args(["stop", "systemd-networkd"])
            .output();

        let _ = Command::new("systemctl")
            .args(["disable", "systemd-networkd"])
            .output();

        log::info!("Network authority enforced - plugin system is sole controller");
        Ok(())
    }

    /// Check for authority violations
    pub fn check_authority() -> Result<Vec<String>> {
        let mut violations = Vec::new();

        // Check if NetworkManager is active
        if let Ok(output) = Command::new("systemctl")
            .args(["is-active", "NetworkManager"])
            .output()
        {
            if output.stdout == b"active\n" {
                violations.push("NetworkManager is active".to_string());
            }
        }

        // Check if systemd-networkd is active
        if let Ok(output) = Command::new("systemctl")
            .args(["is-active", "systemd-networkd"])
            .output()
        {
            if output.stdout == b"active\n" {
                violations.push("systemd-networkd is active".to_string());
            }
        }

        Ok(violations)
    }
}
