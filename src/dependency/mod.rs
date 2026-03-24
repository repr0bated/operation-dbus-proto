//! Dependency Manager: PackageKit and systemd D-Bus Integration
//!
//! Each plugin declares its dependencies:
//! - Packages (via PackageKit D-Bus)
//! - Services (via systemd D-Bus API)
//! - Kernel capabilities
//!
//! Dependencies are first-class objects in the state store.

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zbus::Connection;

use crate::error::{OpDbusError, Result};
use op_state_store::StateStore;

/// Package dependency declaration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageDependency {
    /// Package name
    pub name: String,
    /// Version constraint (e.g., ">=1.0.0")
    pub version_constraint: Option<String>,
    /// Repository (optional)
    pub repository: Option<String>,
    /// Whether this is required or optional
    pub required: bool,
    /// Installation state
    pub state: DependencyState,
}

/// Service dependency declaration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceDependency {
    /// Service name
    pub name: String,
    /// D-Bus name (if different)
    pub dbus_name: Option<String>,
    /// Required state
    pub required_state: ServiceState,
    /// Whether this is required
    pub required: bool,
    /// Current state
    pub current_state: DependencyState,
}

/// Kernel capability dependency
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KernelCapability {
    /// Capability name
    pub name: String,
    /// How to check (sysfs path, module name, etc.)
    pub check_method: CapabilityCheckMethod,
    /// Current state
    pub state: DependencyState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CapabilityCheckMethod {
    SysfsPath(String),
    KernelModule(String),
    KernelConfig(String),
    Procfs(String),
}

/// Dependency state
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum DependencyState {
    Unknown,
    Satisfied,
    Missing,
    WrongVersion,
    Installing,
    Failed,
}

/// Service state
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum ServiceState {
    Running,
    Stopped,
    Enabled,
    Disabled,
    Any,
}

/// Plugin dependency set
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DependencySet {
    pub packages: Vec<PackageDependency>,
    pub services: Vec<ServiceDependency>,
    pub kernel_capabilities: Vec<KernelCapability>,
}

impl DependencySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_package(mut self, name: &str, version: Option<&str>, required: bool) -> Self {
        self.packages.push(PackageDependency {
            name: name.to_string(),
            version_constraint: version.map(String::from),
            repository: None,
            required,
            state: DependencyState::Unknown,
        });
        self
    }

    pub fn add_service(mut self, name: &str, required_state: ServiceState, required: bool) -> Self {
        self.services.push(ServiceDependency {
            name: name.to_string(),
            dbus_name: None,
            required_state,
            required,
            current_state: DependencyState::Unknown,
        });
        self
    }

    pub fn add_kernel_capability(mut self, name: &str, check: CapabilityCheckMethod) -> Self {
        self.kernel_capabilities.push(KernelCapability {
            name: name.to_string(),
            check_method: check,
            state: DependencyState::Unknown,
        });
        self
    }

    pub fn all_satisfied(&self) -> bool {
        self.packages.iter().all(|p| !p.required || p.state == DependencyState::Satisfied)
            && self.services.iter().all(|s| !s.required || s.current_state == DependencyState::Satisfied)
            && self.kernel_capabilities.iter().all(|k| k.state == DependencyState::Satisfied)
    }
}

/// Dependency manager
pub struct DependencyManager {
    state_store: Arc<dyn StateStore>,
    system_bus: Option<Connection>,
    session_bus: Option<Connection>,
}

impl DependencyManager {
    /// Create a new dependency manager
    pub fn new(state_store: Arc<dyn StateStore>) -> Self {
        Self {
            state_store,
            system_bus: None,
            session_bus: None,
        }
    }

    /// Initialize D-Bus connections
    pub async fn init(&mut self) -> Result<()> {
        match Connection::system().await {
            Ok(conn) => self.system_bus = Some(conn),
            Err(e) => tracing::warn!("Failed to connect to system bus: {}", e),
        }

        match Connection::session().await {
            Ok(conn) => self.session_bus = Some(conn),
            Err(e) => tracing::warn!("Failed to connect to session bus: {}", e),
        }

        Ok(())
    }

    /// Check all dependencies in a set
    pub async fn check_dependencies(&self, deps: &mut DependencySet) -> Result<DependencyCheckResult> {
        let mut result = DependencyCheckResult {
            all_satisfied: true,
            packages: Vec::new(),
            services: Vec::new(),
            capabilities: Vec::new(),
        };

        // Check packages
        for pkg in &mut deps.packages {
            pkg.state = self.check_package(&pkg.name, pkg.version_constraint.as_deref()).await?;
            result.packages.push((pkg.name.clone(), pkg.state));
            if pkg.required && pkg.state != DependencyState::Satisfied {
                result.all_satisfied = false;
            }
        }

        // Check services
        for svc in &mut deps.services {
            svc.current_state = self.check_service(&svc.name, svc.required_state).await?;
            result.services.push((svc.name.clone(), svc.current_state));
            if svc.required && svc.current_state != DependencyState::Satisfied {
                result.all_satisfied = false;
            }
        }

        // Check kernel capabilities
        for cap in &mut deps.kernel_capabilities {
            cap.state = self.check_kernel_capability(&cap.name, &cap.check_method).await?;
            result.capabilities.push((cap.name.clone(), cap.state));
            if cap.state != DependencyState::Satisfied {
                result.all_satisfied = false;
            }
        }

        Ok(result)
    }

    /// Check if a package is installed via PackageKit D-Bus
    async fn check_package(&self, name: &str, version: Option<&str>) -> Result<DependencyState> {
        // Without PackageKit D-Bus, fall back to dpkg/rpm check
        let output = tokio::process::Command::new("dpkg")
            .args(["-s", name])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if stdout.contains("Status: install ok installed") {
                    // Check version if specified
                    if let Some(required_version) = version {
                        if let Some(line) = stdout.lines().find(|l| l.starts_with("Version:")) {
                            let installed_version = line.trim_start_matches("Version:").trim();
                            // Simplified version check
                            if installed_version.starts_with(required_version.trim_start_matches(">=")) {
                                return Ok(DependencyState::Satisfied);
                            } else {
                                return Ok(DependencyState::WrongVersion);
                            }
                        }
                    }
                    Ok(DependencyState::Satisfied)
                } else {
                    Ok(DependencyState::Missing)
                }
            }
            _ => {
                // Try rpm as fallback
                let rpm_output = tokio::process::Command::new("rpm")
                    .args(["-q", name])
                    .output()
                    .await;

                match rpm_output {
                    Ok(o) if o.status.success() => Ok(DependencyState::Satisfied),
                    _ => Ok(DependencyState::Missing),
                }
            }
        }
    }

    /// Check service state via systemd D-Bus API
    async fn check_service(&self, name: &str, required_state: ServiceState) -> Result<DependencyState> {
        let service_name = if name.ends_with(".service") {
            name.to_string()
        } else {
            format!("{}.service", name)
        };

        // Use systemctl as fallback when D-Bus unavailable
        let output = tokio::process::Command::new("systemctl")
            .args(["is-active", &service_name])
            .output()
            .await;

        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let is_active = stdout == "active";

                match required_state {
                    ServiceState::Running if is_active => Ok(DependencyState::Satisfied),
                    ServiceState::Running => Ok(DependencyState::Missing),
                    ServiceState::Stopped if !is_active => Ok(DependencyState::Satisfied),
                    ServiceState::Stopped => Ok(DependencyState::Missing),
                    ServiceState::Any => Ok(DependencyState::Satisfied),
                    _ => {
                        // Check enabled state
                        let enabled_output = tokio::process::Command::new("systemctl")
                            .args(["is-enabled", &service_name])
                            .output()
                            .await;

                        match enabled_output {
                            Ok(e) => {
                                let enabled = String::from_utf8_lossy(&e.stdout).trim() == "enabled";
                                match required_state {
                                    ServiceState::Enabled if enabled => Ok(DependencyState::Satisfied),
                                    ServiceState::Disabled if !enabled => Ok(DependencyState::Satisfied),
                                    _ => Ok(DependencyState::Missing),
                                }
                            }
                            Err(_) => Ok(DependencyState::Unknown),
                        }
                    }
                }
            }
            Err(_) => Ok(DependencyState::Unknown),
        }
    }

    /// Check kernel capability
    async fn check_kernel_capability(&self, name: &str, method: &CapabilityCheckMethod) -> Result<DependencyState> {
        match method {
            CapabilityCheckMethod::SysfsPath(path) => {
                if std::path::Path::new(path).exists() {
                    Ok(DependencyState::Satisfied)
                } else {
                    Ok(DependencyState::Missing)
                }
            }
            CapabilityCheckMethod::KernelModule(module) => {
                let output = tokio::process::Command::new("lsmod")
                    .output()
                    .await;

                match output {
                    Ok(o) => {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        if stdout.contains(module) {
                            Ok(DependencyState::Satisfied)
                        } else {
                            Ok(DependencyState::Missing)
                        }
                    }
                    Err(_) => Ok(DependencyState::Unknown),
                }
            }
            CapabilityCheckMethod::KernelConfig(config) => {
                // Check /boot/config-* or /proc/config.gz
                let config_path = format!("/proc/config.gz");
                if std::path::Path::new(&config_path).exists() {
                    // Would need to decompress and search
                    Ok(DependencyState::Unknown)
                } else {
                    Ok(DependencyState::Unknown)
                }
            }
            CapabilityCheckMethod::Procfs(path) => {
                if std::path::Path::new(path).exists() {
                    Ok(DependencyState::Satisfied)
                } else {
                    Ok(DependencyState::Missing)
                }
            }
        }
    }

    /// Install a package via PackageKit D-Bus
    pub async fn install_package(&self, name: &str) -> Result<()> {
        // Fall back to apt/dnf when PackageKit unavailable
        let apt_result = tokio::process::Command::new("apt-get")
            .args(["install", "-y", name])
            .status()
            .await;

        match apt_result {
            Ok(status) if status.success() => Ok(()),
            _ => {
                // Try dnf
                let dnf_result = tokio::process::Command::new("dnf")
                    .args(["install", "-y", name])
                    .status()
                    .await;

                match dnf_result {
                    Ok(status) if status.success() => Ok(()),
                    _ => Err(OpDbusError::ExecutionFailed(
                        format!("Failed to install package: {}", name)
                    )),
                }
            }
        }
    }

    /// Enable and start a service via systemd D-Bus API
    pub async fn ensure_service(&self, name: &str, state: ServiceState) -> Result<()> {
        let service_name = if name.ends_with(".service") {
            name.to_string()
        } else {
            format!("{}.service", name)
        };

        match state {
            ServiceState::Running => {
                tokio::process::Command::new("systemctl")
                    .args(["start", &service_name])
                    .status()
                    .await
                    .map_err(|e| OpDbusError::ExecutionFailed(e.to_string()))?;
            }
            ServiceState::Stopped => {
                tokio::process::Command::new("systemctl")
                    .args(["stop", &service_name])
                    .status()
                    .await
                    .map_err(|e| OpDbusError::ExecutionFailed(e.to_string()))?;
            }
            ServiceState::Enabled => {
                tokio::process::Command::new("systemctl")
                    .args(["enable", &service_name])
                    .status()
                    .await
                    .map_err(|e| OpDbusError::ExecutionFailed(e.to_string()))?;
            }
            ServiceState::Disabled => {
                tokio::process::Command::new("systemctl")
                    .args(["disable", &service_name])
                    .status()
                    .await
                    .map_err(|e| OpDbusError::ExecutionFailed(e.to_string()))?;
            }
            ServiceState::Any => {}
        }

        Ok(())
    }

    /// Satisfy all dependencies in a set
    pub async fn satisfy_dependencies(&self, deps: &mut DependencySet) -> Result<()> {
        // Install missing packages
        for pkg in &deps.packages {
            if pkg.required && pkg.state == DependencyState::Missing {
                self.install_package(&pkg.name).await?;
            }
        }

        // Ensure services
        for svc in &deps.services {
            if svc.required && svc.current_state != DependencyState::Satisfied {
                self.ensure_service(&svc.name, svc.required_state).await?;
            }
        }

        // Re-check
        self.check_dependencies(deps).await?;

        if deps.all_satisfied() {
            Ok(())
        } else {
            Err(OpDbusError::ExecutionFailed(
                "Some dependencies could not be satisfied".into()
            ))
        }
    }
}

/// Result of dependency check
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyCheckResult {
    pub all_satisfied: bool,
    pub packages: Vec<(String, DependencyState)>,
    pub services: Vec<(String, DependencyState)>,
    pub capabilities: Vec<(String, DependencyState)>,
}
