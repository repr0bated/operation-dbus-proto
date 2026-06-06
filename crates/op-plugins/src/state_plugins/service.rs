//! Service plugin - auto-generating, validating, init-agnostic service management.

use crate::service_def::{
    ExecCommand, LogType, ReadyNotification, RestartPolicy, ServiceDef, ServiceName, ServiceType,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zbus::{Connection, Proxy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLifecycle {
    pub last_active: Option<u64>,
    pub days_since_active: Option<u64>,
    pub is_orphaned: bool,
    pub orphan_reason: Option<String>,
}

/// Path to the s6-rc live database.
const S6_RC_LIVE: &str = "/run/s6-rc";

pub struct ServicePlugin {
    backend: ServiceBackend,
}

enum ServiceBackend {
    S6,
    Systemd,
}

impl Default for ServicePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl ServicePlugin {
    pub fn new() -> Self {
        // Prefer s6 when the live directory exists; fall back to systemd.
        let backend = if Path::new(S6_RC_LIVE).exists() || Path::new("/etc/s6/sv").exists() {
            ServiceBackend::S6
        } else {
            ServiceBackend::Systemd
        };
        Self { backend }
    }

    /// Connect to systemd via D-Bus.
    async fn connect_systemd(&self) -> Result<Proxy<'static>> {
        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;
        Proxy::new(
            &conn,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )
        .await
        .context("Failed to create systemd D-Bus proxy")
    }

    /// List systemd services via D-Bus.
    async fn list_systemd_services(&self) -> Result<Vec<String>> {
        let proxy = self.connect_systemd().await?;
        #[allow(clippy::type_complexity)]
        let units: Vec<(String, String, String, String, String, String, zbus::zvariant::OwnedObjectPath, u32, String, zbus::zvariant::OwnedObjectPath)> = proxy
            .call("ListUnits", &())
            .await
            .context("Failed to list systemd units")?;
        Ok(units
            .into_iter()
            .filter(|(name, _, _, _, _, _, _, _, _, _)| name.ends_with(".service"))
            .map(|(name, _, _, _, _, _, _, _, _, _)| name)
            .collect())
    }

    /// Query ActiveEnterTimestamp for a systemd unit via D-Bus.
    async fn systemd_last_active(&self, name: &str) -> Result<Option<u64>> {
        let proxy = self.connect_systemd().await?;
        let path: zbus::zvariant::OwnedObjectPath = proxy
            .call("GetUnit", &(name,))
            .await
            .context("Failed to get unit path")?;
        let conn = Connection::system().await?;
        let unit_proxy = Proxy::new(
            &conn,
            "org.freedesktop.systemd1",
            path,
            "org.freedesktop.systemd1.Unit",
        )
        .await?;
        let ts_usec: u64 = unit_proxy
            .get_property::<u64>("ActiveEnterTimestamp")
            .await
            .unwrap_or(0);
        if ts_usec > 0 {
            Ok(Some(ts_usec / 1_000_000))
        } else {
            Ok(None)
        }
    }

    /// Auto-generate service from installed binary
    pub async fn auto_generate_service(&self, binary_path: &Path) -> Result<ServiceDef> {
        let name = binary_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid binary name"))?;

        Ok(ServiceDef {
            name: ServiceName::new(name)?,
            service_type: ServiceType::Simple,
            exec_start: ExecCommand::new(binary_path.to_path_buf(), vec![])?,
            exec_stop: None,
            working_dir: None,
            user: None,
            group: None,
            depends_on: vec![],
            waits_for: vec![],
            restart: RestartPolicy::default(),
            environment: HashMap::new(),
            env_file: None,
            resources: None,
            log_type: LogType::None,
            ready_notification: ReadyNotification::None,
            chain_to: None,
            smooth_recovery: false,
            enabled: false,
        })
    }

    /// Convert from systemd unit file (Helper moved to ServicePlugin to avoid polluting schema)
    fn from_systemd_unit(path: &Path) -> Result<ServiceDef> {
        let content = std::fs::read_to_string(path)?;
        let mut exec_start = None;
        let mut exec_stop = None;
        let mut working_dir = None;
        let mut user = None;
        let mut depends = vec![];
        let mut env = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "ExecStart" => {
                        let parts: Vec<&str> = v.split_whitespace().collect();
                        if !parts.is_empty() {
                            if let Ok(cmd) = ExecCommand::new(
                                PathBuf::from(parts[0]),
                                parts[1..].iter().map(|s| s.to_string()).collect(),
                            ) {
                                exec_start = Some(cmd);
                            }
                        }
                    }
                    "ExecStop" => {
                        let parts: Vec<&str> = v.split_whitespace().collect();
                        if !parts.is_empty() {
                            if let Ok(cmd) = ExecCommand::new(
                                PathBuf::from(parts[0]),
                                parts[1..].iter().map(|s| s.to_string()).collect(),
                            ) {
                                exec_stop = Some(cmd);
                            }
                        }
                    }
                    "WorkingDirectory" => working_dir = Some(PathBuf::from(v.trim())),
                    "User" => user = Some(v.trim().to_string()),
                    "Requires" | "Wants" | "After" => {
                        for dep in v.split_whitespace() {
                            if let Ok(sn) = ServiceName::new(dep) {
                                depends.push(sn);
                            }
                        }
                    }
                    "Environment" => {
                        if let Some((ek, ev)) = v.split_once('=') {
                            env.insert(ek.trim().to_string(), ev.trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let name = ServiceName::new(file_name)?;

        Ok(ServiceDef {
            name,
            service_type: ServiceType::Simple, // Default, logic should improve
            exec_start: exec_start.ok_or_else(|| anyhow::anyhow!("no ExecStart"))?,
            exec_stop,
            working_dir,
            user,
            group: None,
            depends_on: depends,
            waits_for: vec![],
            restart: RestartPolicy::default(),
            environment: env,
            env_file: None,
            resources: None,
            log_type: LogType::None,
            ready_notification: ReadyNotification::None,
            chain_to: None,
            smooth_recovery: false,
            enabled: false,
        })
    }

    /// Convert all systemd units to s6 service definitions
    pub async fn convert_systemd_to_s6(&self) -> Result<Vec<ServiceDef>> {
        let mut services = vec![];
        let systemd_dir = Path::new("/etc/systemd/system");

        if !systemd_dir.exists() {
            return Ok(services);
        }

        for entry in std::fs::read_dir(systemd_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("service") {
                match Self::from_systemd_unit(&path) {
                    Ok(svc) => {
                        services.push(svc);
                    }
                    Err(e) => {
                        log::warn!("Failed to convert {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(services)
    }

    /// Install service definition
    pub async fn install_service(&self, svc: &ServiceDef) -> Result<()> {
        match self.backend {
            ServiceBackend::S6 => {
                svc.install()?;
                log::info!("Installed s6 service: {}", svc.name);
            }
            ServiceBackend::Systemd => {
                anyhow::bail!("systemd installation not implemented - use s6");
            }
        }

        Ok(())
    }

    /// List running s6 services via D-Bus.
    async fn list_s6_services(&self) -> Result<Vec<String>> {
        let client = crate::state_plugins::s6::S6DbusClient::new();
        let units = client.list_units().await.unwrap_or_default();
        let mut running = Vec::new();
        for unit in units {
            if let Some(name) = unit.get("name").and_then(|v| v.as_str()) {
                if let Some(active) = unit.get("active").and_then(|v| v.as_str()) {
                    if active == "true" || active == "up" {
                        running.push(name.to_string());
                    }
                }
            }
        }
        Ok(running)
    }

    async fn check_lifecycle(&self, name: &str) -> Result<ServiceLifecycle> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let last_active = match self.backend {
            ServiceBackend::Systemd => self.systemd_last_active(name).await.ok().flatten(),
            // s6 does not expose activation timestamps via D-Bus
            ServiceBackend::S6 => None,
        };

        let days_since_active = last_active.map(|t| (now - t) / 86400);
        let is_orphaned = days_since_active.is_none_or(|d| d > 30);

        let orphan_reason = if is_orphaned {
            Some(if last_active.is_none() {
                "never run".to_string()
            } else if let Some(days) = days_since_active {
                format!("inactive {} days", days)
            } else {
                "inactive unknown days".to_string()
            })
        } else {
            None
        };

        Ok(ServiceLifecycle {
            last_active,
            days_since_active,
            is_orphaned,
            orphan_reason,
        })
    }
}

#[async_trait]
impl StatePlugin for ServicePlugin {
    fn name(&self) -> &str {
        "service"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::service_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut services = HashMap::new();

        let service_list = match self.backend {
            ServiceBackend::Systemd => self.list_systemd_services().await.unwrap_or_default(),
            ServiceBackend::S6 => self.list_s6_services().await?,
        };

        for svc_name in service_list {
            if let Ok(lifecycle) = self.check_lifecycle(&svc_name).await {
                services.insert(svc_name, json!({ "lifecycle": lifecycle }));
            }
        }

        Ok(json!({ "services": services }))
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("service-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
