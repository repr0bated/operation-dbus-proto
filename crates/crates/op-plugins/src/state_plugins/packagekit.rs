//! PackageKit Plugin for op-dbus
//!
//! Manages system packages via PackageKit D-Bus interface
//! Provides declarative package installation/removal

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::collections::HashMap;
use std::process::Command;
use zbus::proxy;

use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};

// PackageKit D-Bus interface
#[proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
trait PackageKit {
    /// Get transaction list
    async fn get_transaction_list(&self) -> zbus::Result<Vec<String>>;

    /// Create transaction
    async fn create_transaction(&self) -> zbus::Result<String>;
}

// Transaction interface
#[proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
trait PackageKitTransaction {
    /// Install packages
    async fn install_packages(
        &self,
        transaction_flags: u64,
        package_ids: Vec<String>,
    ) -> zbus::Result<()>;

    /// Remove packages
    async fn remove_packages(
        &self,
        transaction_flags: u64,
        package_ids: Vec<String>,
        allow_deps: bool,
        autoremove: bool,
    ) -> zbus::Result<()>;

    /// Resolve packages
    async fn resolve(&self, filters: u64, packages: Vec<String>) -> zbus::Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageState {
    pub ensure: String,           // "installed", "removed", "latest"
    pub provider: Option<String>, // "apt", "dnf", "pacman", etc.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageKitState {
    pub version: u32,
    pub packages: HashMap<String, PackageState>,
}

#[derive(Debug, Clone)]
pub struct PackageKitPlugin;

impl Default for PackageKitPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageKitPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Install package via direct package manager
    async fn install_via_direct(&self, package_name: &str) -> Result<()> {
        // Try apt
        if Command::new("apt-get")
            .args(["install", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try dnf
        if Command::new("dnf")
            .args(["install", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try pacman
        if Command::new("pacman")
            .args(["-S", "--noconfirm", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        Err(anyhow::anyhow!("No suitable package manager found"))
    }

    /// Remove package via direct package manager
    async fn remove_via_direct(&self, package_name: &str) -> Result<()> {
        // Try apt
        if Command::new("apt-get")
            .args(["remove", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try dnf
        if Command::new("dnf")
            .args(["remove", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try pacman
        if Command::new("pacman")
            .args(["-R", "--noconfirm", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        Err(anyhow::anyhow!("No suitable package manager found"))
    }

    /// Check if package is installed
    async fn package_installed(&self, package_name: &str) -> Result<bool> {
        // Try dpkg (Debian/Ubuntu)
        if Command::new("dpkg")
            .args(["-l", package_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(true);
        }

        // Try rpm (Fedora/RHEL)
        if Command::new("rpm")
            .args(["-q", package_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(true);
        }

        // Try pacman (Arch)
        if Command::new("pacman")
            .args(["-Q", package_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(true);
        }

        Ok(false)
    }
}

#[async_trait]
impl StatePlugin for PackageKitPlugin {
    fn name(&self) -> &str {
        "packagekit"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::json!({
            "version": 1,
            "packages": {}
        }))
    }

    async fn calculate_diff(&self, _current: &Value, desired: &Value) -> Result<StateDiff> {
        println!("PackageKit calculate_diff called with: {}", desired);
        let packages_obj = desired
            .get("packages")
            .ok_or_else(|| anyhow::anyhow!("missing packages field"))?;
        let packages: HashMap<String, PackageState> = simd_json::serde::from_owned_value(packages_obj.clone())?;
        let desired_state = PackageKitState {
            version: 1,
            packages,
        };

        let mut actions = Vec::new();

        for (package_name, package_config) in &desired_state.packages {
            let is_installed = self.package_installed(package_name).await?;

            match package_config.ensure.as_str() {
                "installed" if !is_installed => {
                    actions.push(StateAction::Create {
                        resource: package_name.clone(),
                        config: simd_json::json!({
                            "ensure": "installed",
                            "provider": package_config.provider,
                            "version": package_config.version
                        }),
                    });
                }
                "removed" if is_installed => {
                    actions.push(StateAction::Delete {
                        resource: package_name.clone(),
                    });
                }
                _ => {
                    actions.push(StateAction::NoOp {
                        resource: package_name.clone(),
                    });
                }
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "placeholder".to_string(),
                desired_hash: "placeholder".to_string(),
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
                } => match self.install_via_direct(resource).await {
                    Ok(()) => {
                        changes_applied.push(format!("✅ Installed package: {}", resource));
                    }
                    Err(e) => {
                        errors.push(format!("❌ Failed to install {}: {}", resource, e));
                    }
                },
                StateAction::Delete { resource } => match self.remove_via_direct(resource).await {
                    Ok(()) => {
                        changes_applied.push(format!("✅ Removed package: {}", resource));
                    }
                    Err(e) => {
                        errors.push(format!("❌ Failed to remove {}: {}", resource, e));
                    }
                },
                StateAction::Modify {
                    resource,
                    changes: _,
                } => {
                    changes_applied.push(format!(
                        "⚠️  Package {} modification not implemented",
                        resource
                    ));
                }
                StateAction::NoOp { resource } => {
                    changes_applied.push(format!("📦 Package {}: no action required", resource));
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

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let packages_obj = desired
            .get("packages")
            .ok_or_else(|| anyhow::anyhow!("missing packages field"))?;
        let packages: HashMap<String, PackageState> = simd_json::serde::from_owned_value(packages_obj.clone())?;

        for (package_name, package_config) in &packages {
            let is_installed = self.package_installed(package_name).await?;

            match package_config.ensure.as_str() {
                "installed" => {
                    if !is_installed {
                        return Ok(false);
                    }
                }
                "removed" => {
                    if is_installed {
                        return Ok(false);
                    }
                }
                _ => {}
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("{}-{}", self.name(), chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
