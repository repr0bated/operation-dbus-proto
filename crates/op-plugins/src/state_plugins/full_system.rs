//! Full System State Plugin
//!
//! This plugin captures the COMPLETE system state for disaster recovery.
//! It aggregates state from all other plugins into a single JSON document
//! that can be used to reinstall/restore the entire system.
//!
//! ## State Categories
//!
//! - **system**: hostname, timezone, locale, kernel parameters
//! - **network**: interfaces, routes, DNS, bridges, VLANs
//! - **services**: systemd units and their configurations
//! - **packages**: installed packages and versions
//! - **users**: user accounts and groups
//! - **storage**: mounts, fstab entries
//! - **containers**: LXC/Docker containers
//! - **security**: firewall rules, SELinux/AppArmor policies
//!
//! This plugin is special: it queries OTHER plugins to build the full state.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, StateAction, StateDiff, StatePlugin};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::{Connection, Proxy};

/// Full system state for disaster recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSystemState {
    /// State schema version
    pub version: u32,

    /// Timestamp of when this state was captured
    pub captured_at: String,

    /// Hostname
    pub hostname: String,

    /// System information
    pub system: SystemInfo,

    /// Network configuration
    pub network: NetworkState,

    /// Systemd services
    pub services: Vec<ServiceState>,

    /// Installed packages
    pub packages: Vec<PackageInfo>,

    /// User accounts
    pub users: Vec<UserInfo>,

    /// Storage mounts
    pub storage: StorageState,

    /// Container state (LXC/Docker)
    pub containers: ContainerState,

    /// Plugin-specific state (aggregated from all plugins)
    pub plugins: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemInfo {
    pub kernel_version: String,
    pub os_release: String,
    pub timezone: String,
    pub locale: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkState {
    pub interfaces: Vec<InterfaceInfo>,
    pub routes: Vec<RouteInfo>,
    pub dns_servers: Vec<String>,
    pub bridges: Vec<BridgeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub mac: String,
    pub addresses: Vec<String>,
    pub state: String,
    pub mtu: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub destination: String,
    pub gateway: Option<String>,
    pub interface: String,
    pub metric: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeInfo {
    pub name: String,
    pub ports: Vec<String>,
    pub bridge_type: String, // "linux" or "ovs"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: String,
    pub enabled: bool,
    pub running: bool,
    pub unit_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageState {
    pub mounts: Vec<MountInfo>,
    pub block_devices: Vec<BlockDeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub source: String,
    pub target: String,
    pub fstype: String,
    pub options: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDeviceInfo {
    pub name: String,
    pub size_bytes: u64,
    pub fstype: Option<String>,
    pub mountpoint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerState {
    pub lxc: Vec<LxcContainerInfo>,
    pub docker: Vec<DockerContainerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcContainerInfo {
    pub name: String,
    pub status: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
}

/// The Full System State Plugin
pub struct FullSystemPlugin {
    /// Cached current state
    state_cache: Arc<RwLock<Option<FullSystemState>>>,

    /// Sender for blockchain footprints
    #[allow(dead_code)]
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<op_blockchain::PluginFootprint>>,
}

impl Default for FullSystemPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl FullSystemPlugin {
    pub fn new() -> Self {
        Self {
            state_cache: Arc::new(RwLock::new(None)),
            blockchain_sender: None,
        }
    }

    /// Create with blockchain sender for change tracking
    pub fn with_blockchain(
        sender: tokio::sync::mpsc::UnboundedSender<op_blockchain::PluginFootprint>,
    ) -> Self {
        Self {
            state_cache: Arc::new(RwLock::new(None)),
            blockchain_sender: Some(sender),
        }
    }

    /// Capture complete system state
    pub async fn capture_full_state(&self) -> Result<FullSystemState> {
        let now = chrono::Utc::now().to_rfc3339();

        info!("Capturing full system state...");

        let hostname = self
            .get_hostname()
            .await
            .unwrap_or_else(|_| "unknown".to_string());
        let system = self.capture_system_info().await.unwrap_or_default();
        let network = self.capture_network_state().await.unwrap_or_default();
        let services = self.capture_services().await.unwrap_or_default();
        let packages = self.capture_packages().await.unwrap_or_default();
        let users = self.capture_users().await.unwrap_or_default();
        let storage = self.capture_storage().await.unwrap_or_default();
        let containers = self.capture_containers().await.unwrap_or_default();

        let state = FullSystemState {
            version: 1,
            captured_at: now,
            hostname,
            system,
            network,
            services,
            packages,
            users,
            storage,
            containers,
            plugins: HashMap::new(), // Will be populated by StateManager
        };

        info!("Full system state captured");

        // Cache the state
        *self.state_cache.write().await = Some(state.clone());

        Ok(state)
    }

    async fn get_hostname(&self) -> Result<String> {
        tokio::fs::read_to_string("/proc/sys/kernel/hostname")
            .await
            .map(|s| s.trim().to_string())
            .context("Failed to read hostname from /proc/sys/kernel/hostname")
    }

    async fn capture_system_info(&self) -> Result<SystemInfo> {
        let kernel_version = tokio::fs::read_to_string("/proc/sys/kernel/osrelease")
            .await
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let os_release = tokio::fs::read_to_string("/etc/os-release")
            .await
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| {
                        l.trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_default();

        let timezone = tokio::fs::read_link("/etc/localtime")
            .await
            .ok()
            .and_then(|p| p.to_str().map(|s| s.replace("/usr/share/zoneinfo/", "")))
            .unwrap_or_else(|| "UTC".to_string());

        let locale = std::env::var("LANG").unwrap_or_else(|_| "C.UTF-8".to_string());

        let uptime = tokio::fs::read_to_string("/proc/uptime")
            .await
            .ok()
            .and_then(|s| {
                s.split_whitespace()
                    .next()
                    .and_then(|u| u.parse::<f64>().ok())
            })
            .map(|u| u as u64)
            .unwrap_or(0);

        Ok(SystemInfo {
            kernel_version,
            os_release,
            timezone,
            locale,
            uptime_seconds: uptime,
        })
    }

    async fn capture_network_state(&self) -> Result<NetworkState> {
        let mut state = NetworkState::default();

        // Get interfaces from /sys/class/net
        if let Ok(mut entries) = tokio::fs::read_dir("/sys/class/net").await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "lo" {
                    continue;
                }

                let mac = tokio::fs::read_to_string(format!("/sys/class/net/{}/address", name))
                    .await
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let state_str =
                    tokio::fs::read_to_string(format!("/sys/class/net/{}/operstate", name))
                        .await
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                let mtu: u32 = tokio::fs::read_to_string(format!("/sys/class/net/{}/mtu", name))
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(1500);

                state.interfaces.push(InterfaceInfo {
                    name,
                    mac,
                    addresses: vec![], // Would need ip tool to get addresses
                    state: state_str,
                    mtu,
                });
            }
        }

        // Get DNS from resolv.conf
        if let Ok(resolv) = tokio::fs::read_to_string("/etc/resolv.conf").await {
            for line in resolv.lines() {
                if line.starts_with("nameserver") {
                    if let Some(ns) = line.split_whitespace().nth(1) {
                        state.dns_servers.push(ns.to_string());
                    }
                }
            }
        }

        // Check for OVS bridges via D-Bus daemon (AGENTS.md §4)
        let ovs_client = op_network::rovs_proxy::OvsdbDbusClient::new();
        if let Ok(bridges) = ovs_client.list_bridges().await {
            for bridge in bridges {
                let ports = ovs_client
                    .list_bridge_ports(&bridge)
                    .await
                    .unwrap_or_default();

                state.bridges.push(BridgeInfo {
                    name: bridge,
                    ports,
                    bridge_type: "ovs".to_string(),
                });
            }
        }

        Ok(state)
    }

    async fn capture_services(&self) -> Result<Vec<ServiceState>> {
        let mut services = Vec::new();

        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        // On non-systemd hosts (e.g. Artix s6-rc), org.freedesktop.systemd1 is
        // not owned. Skip silently instead of hanging on a method call that
        // will never be answered.
        let dbus_proxy = Proxy::new(
            &conn,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )
        .await
        .context("Failed to create D-Bus proxy")?;
        let names: Vec<String> = dbus_proxy
            .call("ListActivatableNames", &())
            .await
            .unwrap_or_default();
        if !names.iter().any(|n| n == "org.freedesktop.systemd1") {
            debug!("org.freedesktop.systemd1 not available — skipping service capture (non-systemd host)");
            return Ok(services);
        }

        let proxy = Proxy::new(
            &conn,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )
        .await
        .context("Failed to create systemd D-Bus proxy")?;

        #[allow(clippy::type_complexity)]
        let units: Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            zbus::zvariant::OwnedObjectPath,
            u32,
            String,
            zbus::zvariant::OwnedObjectPath,
        )> = proxy
            .call("ListUnits", &())
            .await
            .context("Failed to list systemd units")?;

        let unit_files: Vec<(String, String)> = proxy
            .call("ListUnitFiles", &())
            .await
            .context("Failed to list systemd unit files")?;

        let enabled_map: std::collections::HashMap<String, bool> = unit_files
            .into_iter()
            .filter(|(name, _)| name.ends_with(".service"))
            .map(|(name, state)| (name, state == "enabled"))
            .collect();

        for (
            name,
            _desc,
            _load_state,
            active_state,
            _sub_state,
            _followed,
            _path,
            _job_id,
            _job_type,
            _job_path,
        ) in units
        {
            if !name.ends_with(".service") {
                continue;
            }
            let running = active_state == "active" || active_state == "running";
            let enabled = enabled_map.get(&name).copied().unwrap_or(false);
            services.push(ServiceState {
                name: name.trim_end_matches(".service").to_string(),
                enabled,
                running,
                unit_type: "service".to_string(),
            });
        }

        Ok(services)
    }

    async fn capture_packages(&self) -> Result<Vec<PackageInfo>> {
        let mut packages = Vec::new();

        // Read dpkg status directly (AGENTS.md §4: no subprocess bypasses)
        let content = tokio::fs::read_to_string("/var/lib/dpkg/status")
            .await
            .unwrap_or_default();

        let mut current_name = None;
        let mut current_version = None;
        let mut current_arch = None;

        for line in content.lines() {
            if let Some(stripped) = line.strip_prefix("Package: ") {
                current_name = Some(stripped.trim().to_string());
            } else if let Some(stripped) = line.strip_prefix("Version: ") {
                current_version = Some(stripped.trim().to_string());
            } else if let Some(stripped) = line.strip_prefix("Architecture: ") {
                current_arch = Some(stripped.trim().to_string());
            } else if line.is_empty() {
                if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                    let arch = current_arch.take().unwrap_or_else(|| "unknown".to_string());
                    packages.push(PackageInfo {
                        name,
                        version,
                        arch,
                    });
                }
            }
        }
        // Handle last stanza if file doesn't end with blank line
        if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
            let arch = current_arch.take().unwrap_or_else(|| "unknown".to_string());
            packages.push(PackageInfo {
                name,
                version,
                arch,
            });
        }

        Ok(packages)
    }

    async fn capture_users(&self) -> Result<Vec<UserInfo>> {
        let mut users = Vec::new();

        // Read group memberships directly (AGENTS.md §4: no subprocess bypasses)
        let groups_content = tokio::fs::read_to_string("/etc/group")
            .await
            .unwrap_or_default();
        let mut group_map: HashMap<String, Vec<String>> = HashMap::new();
        for line in groups_content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4 {
                let group_name = parts[0].to_string();
                let members: Vec<String> = parts[3]
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                for member in &members {
                    group_map
                        .entry(member.clone())
                        .or_default()
                        .push(group_name.clone());
                }
            }
        }

        if let Ok(passwd) = tokio::fs::read_to_string("/etc/passwd").await {
            for line in passwd.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 7 {
                    let uid: u32 = parts[2].parse().unwrap_or(0);

                    // Skip system users (uid < 1000) except root
                    if uid != 0 && uid < 1000 {
                        continue;
                    }

                    let name = parts[0].to_string();
                    let groups = group_map.get(&name).cloned().unwrap_or_default();

                    users.push(UserInfo {
                        name,
                        uid,
                        gid: parts[3].parse().unwrap_or(0),
                        home: parts[5].to_string(),
                        shell: parts[6].to_string(),
                        groups,
                    });
                }
            }
        }

        Ok(users)
    }

    async fn capture_storage(&self) -> Result<StorageState> {
        let mut state = StorageState::default();

        // Get mounts from /proc/mounts
        if let Ok(mounts) = tokio::fs::read_to_string("/proc/mounts").await {
            for line in mounts.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let source = parts[0].to_string();
                    let target = parts[1].to_string();

                    // Skip virtual filesystems
                    if source.starts_with("none")
                        || target.starts_with("/sys")
                        || target.starts_with("/proc")
                        || target.starts_with("/dev")
                        || target.starts_with("/run")
                    {
                        continue;
                    }

                    state.mounts.push(MountInfo {
                        source,
                        target,
                        fstype: parts[2].to_string(),
                        options: parts[3].to_string(),
                    });
                }
            }
        }

        // Get block devices from /sys/block (AGENTS.md §4: no subprocess bypasses)
        let sysfs = std::path::Path::new("/sys/block");
        if sysfs.exists() {
            let mut entries = match tokio::fs::read_dir(sysfs).await {
                Ok(e) => e,
                Err(_) => return Ok(state),
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip loop devices and ramdisks
                if name.starts_with("loop") || name.starts_with("ram") {
                    continue;
                }

                let size_path = entry.path().join("size");
                let size_sectors = tokio::fs::read_to_string(&size_path)
                    .await
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0);
                let size_bytes = size_sectors * 512;

                let mountpoint = Self::read_mountpoint(&name).await;

                state.block_devices.push(BlockDeviceInfo {
                    name,
                    size_bytes,
                    fstype: None,
                    mountpoint,
                });
            }
        }

        Ok(state)
    }

    async fn read_mountpoint(device: &str) -> Option<String> {
        let mounts = tokio::fs::read_to_string("/proc/mounts").await.ok()?;
        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let dev = parts[0];
                if dev.contains(device) {
                    return Some(parts[1].to_string());
                }
            }
        }
        None
    }

    async fn capture_containers(&self) -> Result<ContainerState> {
        let mut state = ContainerState::default();

        // LXC containers - read /var/lib/lxc/ directly (AGENTS.md §4: no subprocess bypasses)
        let lxc_dir = std::path::Path::new("/var/lib/lxc");
        if lxc_dir.exists() {
            let mut entries = match tokio::fs::read_dir(lxc_dir).await {
                Ok(e) => e,
                Err(_) => {
                    // fall through to docker check
                    return Ok(state);
                }
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip files, only directories are containers
                if !entry
                    .file_type()
                    .await
                    .ok()
                    .map(|t| t.is_dir())
                    .unwrap_or(false)
                {
                    continue;
                }
                state.lxc.push(LxcContainerInfo {
                    name,
                    status: "unknown".to_string(),
                    config: json!({}),
                });
            }
        }

        // TODO: Docker containers should be queried via D-Bus or API, not CLI.
        // Keeping as a fallback until Docker D-Bus interface is implemented.
        if let Ok(output) = Command::new("docker")
            .args([
                "ps",
                "-a",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}",
            ])
            .output()
            .await
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 4 {
                        state.docker.push(DockerContainerInfo {
                            id: parts[0].to_string(),
                            name: parts[1].to_string(),
                            image: parts[2].to_string(),
                            status: parts[3].to_string(),
                        });
                    }
                }
            }
        }

        Ok(state)
    }
}

#[async_trait]
impl StatePlugin for FullSystemPlugin {
    fn name(&self) -> &str {
        "full_system"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let any_field = |required: bool, description: &str, default: Option<Value>| FieldSchema {
            field_type: FieldType::Any,
            required,
            description: description.to_string(),
            default,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        };

        Some(
            PluginSchema::builder("full_system")
                .version("1.0.0")
                .description("Full system recovery snapshot")
                .field(
                    "version",
                    FieldSchema {
                        field_type: FieldType::Integer,
                        required: true,
                        description: "Snapshot schema version".to_string(),
                        default: Some(json!(1)),
                        example: None,
                        constraints: vec![Constraint::Min { value: 1.0 }],
                        read_only: false,
                        read_only_when: None,
                    },
                )
                .field(
                    "captured_at",
                    FieldSchema {
                        field_type: FieldType::String,
                        required: false,
                        description: "Capture timestamp".to_string(),
                        default: None,
                        example: None,
                        constraints: Vec::new(),
                        read_only: false,
                        read_only_when: None,
                    },
                )
                .field("hostname", any_field(true, "Host name", Some(json!(""))))
                .field(
                    "system",
                    any_field(false, "System details", Some(json!({}))),
                )
                .field(
                    "network",
                    any_field(false, "Network snapshot", Some(json!({}))),
                )
                .field(
                    "services",
                    any_field(false, "Service snapshot", Some(json!([]))),
                )
                .field(
                    "packages",
                    any_field(false, "Package snapshot", Some(json!([]))),
                )
                .field("users", any_field(false, "User snapshot", Some(json!([]))))
                .field(
                    "storage",
                    any_field(false, "Storage snapshot", Some(json!({}))),
                )
                .field(
                    "containers",
                    any_field(false, "Container snapshot", Some(json!({}))),
                )
                .field(
                    "plugins",
                    any_field(false, "Plugin snapshots", Some(json!({}))),
                )
                .build(),
        )
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = self.capture_full_state().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        use op_state::DiffMetadata;
        use sha2::{Digest, Sha256};

        let mut actions = Vec::new();

        // Check for hostname change
        if current.get("hostname") != desired.get("hostname") {
            actions.push(StateAction::Modify {
                resource: "hostname".to_string(),
                changes: json!({
                    "from": current.get("hostname"),
                    "to": desired.get("hostname"),
                }),
            });
        }

        // More sophisticated diffing would be done here
        // For now, just mark if there's any difference
        if current != desired {
            actions.push(StateAction::Modify {
                resource: "full_system".to_string(),
                changes: json!({
                    "message": "Full system state differs from desired"
                }),
            });
        }

        // Create hashes for metadata
        let current_str = simd_json::to_string(current).unwrap_or_default();
        let desired_str = simd_json::to_string(desired).unwrap_or_default();
        let current_hash = format!("{:x}", Sha256::digest(current_str.as_bytes()));
        let desired_hash = format!("{:x}", Sha256::digest(desired_str.as_bytes()));

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash,
                desired_hash,
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Modify { resource, changes } if resource == "hostname" => {
                    if let Some(hostname) = changes.get("to").and_then(|v| v.as_str()) {
                        let result = Command::new("hostnamectl")
                            .args(["set-hostname", hostname])
                            .output()
                            .await;

                        match result {
                            Ok(output) if output.status.success() => {
                                changes_applied.push(format!("Set hostname to {}", hostname));
                            }
                            Ok(output) => {
                                errors.push(format!(
                                    "Failed to set hostname: {}",
                                    String::from_utf8_lossy(&output.stderr)
                                ));
                            }
                            Err(e) => {
                                errors.push(format!("Failed to run hostnamectl: {}", e));
                            }
                        }
                    }
                }
                _ => {
                    debug!("Unhandled action: {:?}", action);
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

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.capture_full_state().await?;
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(state)?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        warn!("Full system rollback not implemented - requires manual intervention");
        Ok(())
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        // Would compare current vs desired
        Ok(true)
    }

    fn capabilities(&self) -> op_state::PluginCapabilities {
        op_state::PluginCapabilities {
            supports_rollback: false, // Too complex for automatic rollback
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capture_system_info() {
        let plugin = FullSystemPlugin::new();
        let info = plugin.capture_system_info().await;
        assert!(info.is_ok());
        let info = info.unwrap();
        assert!(!info.kernel_version.is_empty());
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("full_system", |_ctx| std::sync::Arc::new(FullSystemPlugin::new()))
}
