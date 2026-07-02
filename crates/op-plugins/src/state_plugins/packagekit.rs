//! PackageKit Plugin for op-dbus
//!
//! Manages system packages via PackageKit D-Bus interface
//! Provides declarative package installation/removal

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use zbus::proxy;

use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema};

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

    /// Install package via PackageKit D-Bus (AGENTS.md §4: no subprocess bypasses)
    async fn install_via_direct(&self, package_name: &str) -> Result<()> {
        let conn = zbus::Connection::system()
            .await
            .context("Failed to connect to D-Bus for PackageKit")?;
        let pk_proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.PackageKit",
            "/org/freedesktop/PackageKit",
            "org.freedesktop.PackageKit",
        )
        .await
        .context("Failed to create PackageKit proxy")?;

        let tx_path: String = pk_proxy
            .call("CreateTransaction", &())
            .await
            .context("Failed to create PackageKit transaction")?;

        let tx_proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.PackageKit",
            tx_path.as_str(),
            "org.freedesktop.PackageKit.Transaction",
        )
        .await
        .context("Failed to create PackageKit transaction proxy")?;

        let flags: u64 = 0;
        let pkgs = vec![package_name.to_string()];
        tx_proxy
            .call::<_, _, ()>("InstallPackages", &(flags, pkgs))
            .await
            .context(format!("PackageKit install failed for {}", package_name))?;

        Ok(())
    }

    /// Remove package via PackageKit D-Bus (AGENTS.md §4: no subprocess bypasses)
    async fn remove_via_direct(&self, package_name: &str) -> Result<()> {
        let conn = zbus::Connection::system()
            .await
            .context("Failed to connect to D-Bus for PackageKit")?;
        let pk_proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.PackageKit",
            "/org/freedesktop/PackageKit",
            "org.freedesktop.PackageKit",
        )
        .await
        .context("Failed to create PackageKit proxy")?;

        let tx_path: String = pk_proxy
            .call("CreateTransaction", &())
            .await
            .context("Failed to create PackageKit transaction")?;

        let tx_proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.PackageKit",
            tx_path.as_str(),
            "org.freedesktop.PackageKit.Transaction",
        )
        .await
        .context("Failed to create PackageKit transaction proxy")?;

        let flags: u64 = 0;
        let pkgs = vec![package_name.to_string()];
        tx_proxy
            .call::<_, _, ()>("RemovePackages", &(flags, pkgs, false, false))
            .await
            .context(format!("PackageKit remove failed for {}", package_name))?;

        Ok(())
    }

    /// Check if package is installed by reading package databases directly (AGENTS.md §4: no subprocess bypasses)
    async fn package_installed(&self, package_name: &str) -> Result<bool> {
        // Check dpkg status (Debian/Ubuntu)
        if let Ok(content) = tokio::fs::read_to_string("/var/lib/dpkg/status").await {
            let mut current_name = String::new();
            let mut current_status = String::new();
            for line in content.lines() {
                if let Some(stripped) = line.strip_prefix("Package: ") {
                    current_name = stripped.trim().to_string();
                } else if let Some(stripped) = line.strip_prefix("Status: ") {
                    current_status = stripped.trim().to_string();
                } else if line.is_empty() {
                    if current_name == package_name && current_status.contains("installed") {
                        return Ok(true);
                    }
                    current_name.clear();
                    current_status.clear();
                }
            }
            if current_name == package_name && current_status.contains("installed") {
                return Ok(true);
            }
        }

        // Check pacman local directory (Arch)
        if let Ok(mut entries) = tokio::fs::read_dir("/var/lib/pacman/local").await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("{}-", package_name)) {
                    return Ok(true);
                }
            }
        }

        // Check rpm sqlite db presence (Fedora/RHEL heuristic)
        if std::path::Path::new("/var/lib/rpm/rpmdb.sqlite").exists()
            || std::path::Path::new("/usr/lib/sysimage/rpm/rpmdb.sqlite").exists()
        {
            // Rely on PackageKit D-Bus for rpm systems; direct rpm db query requires subprocess
            // or complex BDB/SQLite parsing. Return false here and let PackageKit handle it.
            return Ok(false);
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

    fn schema(&self) -> Option<PluginSchema> {
        Some(packagekit_schema())
    }

    async fn calculate_diff(&self, _current: &Value, desired: &Value) -> Result<StateDiff> {
        println!("PackageKit calculate_diff called with: {}", desired);
        let packages_obj = desired
            .get("packages")
            .ok_or_else(|| anyhow::anyhow!("missing packages field"))?;
        let packages: HashMap<String, PackageState> =
            simd_json::serde::from_owned_value(packages_obj.clone())?;
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
        let packages: HashMap<String, PackageState> =
            simd_json::serde::from_owned_value(packages_obj.clone())?;

        for (package_name, package_config) in &packages {
            let is_installed = self.package_installed(package_name).await?;

            match package_config.ensure.as_str() {
                "installed" if !is_installed => {
                    return Ok(false);
                }
                "removed" if is_installed => {
                    return Ok(false);
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

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("packagekit", |_ctx| std::sync::Arc::new(PackageKitPlugin::new()))
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolvePackagesInput {
    pub filter: u64,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InstallPackagesInput {
    pub transaction_flags: u64,
    pub package_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemovePackagesInput {
    pub transaction_flags: u64,
    pub package_ids: Vec<String>,
    pub allow_deps: bool,
    pub autoremove: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdatePackagesInput {
    pub transaction_flags: u64,
    pub package_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetUpdatesInput {
    pub filter: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchNamesInput {
    pub filter: u64,
    pub values: Vec<String>,
}

pub(crate) fn packagekit_schema() -> PluginSchema {
    let mut schema = PluginSchema::builder("packagekit")
        .version("1.0.0")
        .description("PackageKit package declarations")
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Schema version".to_string(),
                default: Some(simd_json::json!(1)),
                example: None,
                constraints: vec![Constraint::Min { value: 1.0 }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "packages",
            FieldSchema {
                field_type: FieldType::Any,
                required: true,
                description: "Package declaration map".to_string(),
                default: Some(simd_json::json!({})),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .build();

    // org.freedesktop.PackageKit methods
    // Spec: https://www.freedesktop.org/software/PackageKit/gtk-doc/api-reference.html
    schema.methods.insert(
        "resolve".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars::<ResolvePackagesInput>(
            "Resolve",
            op_state_store::SideEffect::Read,
            true,
            "packagekit.read",
            "obs.software.packagekit.package.resolve@v1",
        ),
    );
    schema.methods.insert(
        "install_packages".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars::<InstallPackagesInput>(
            "InstallPackages",
            op_state_store::SideEffect::Mutation,
            false,
            "packagekit.admin",
            "mut.software.packagekit.package.install@v1",
        ),
    );
    schema.methods.insert(
        "remove_packages".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars::<RemovePackagesInput>(
            "RemovePackages",
            op_state_store::SideEffect::Mutation,
            false,
            "packagekit.admin",
            "mut.software.packagekit.package.remove@v1",
        ),
    );
    schema.methods.insert(
        "update_packages".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars::<UpdatePackagesInput>(
            "UpdatePackages",
            op_state_store::SideEffect::Mutation,
            false,
            "packagekit.admin",
            "mut.software.packagekit.package.update@v1",
        ),
    );
    schema.methods.insert(
        "get_updates".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars::<GetUpdatesInput>(
            "GetUpdates",
            op_state_store::SideEffect::Read,
            true,
            "packagekit.read",
            "obs.software.packagekit.updates.get@v1",
        ),
    );
    schema.methods.insert(
        "search_names".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars::<SearchNamesInput>(
            "SearchNames",
            op_state_store::SideEffect::Read,
            true,
            "packagekit.read",
            "obs.software.packagekit.names.search@v1",
        ),
    );

    schema
}
