//! S6 state plugin — manages services via s6-rc on Artix/Chimera Linux.
//!
//! AGENTS.md §4 (D-Bus First): This plugin routes ALL control-plane calls
//! through the `org.opdbus.v1.S6.Systemctl` D-Bus object instead of spawning
//! s6-rc / s6-svc subprocesses directly.  The only exceptions are read-only
//! enumeration queries (`s6-rc -a list`) which are considered observations.
//!
//! OSCAL subids:
//!   obs.service.s6.list-units@v1    — query_current_state
//!   mut.service.s6.start-unit@v1    — start via D-Bus
//!   mut.service.s6.stop-unit@v1     — stop via D-Bus
//!   mut.service.s6.restart-unit@v1  — restart via D-Bus
//!   mut.service.s6.reload-unit@v1   — reload via D-Bus
//!   mut.service.s6.enable-unit@v1   — enable via D-Bus
//!   mut.service.s6.disable-unit@v1  — disable via D-Bus

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Path to the s6-rc live directory.
const S6_RC_LIVE: &str = "/run/s6-rc";

// ── State structs aligned with schema `units[]` ───────────────────────────────

/// Per-service configuration in the desired state.
/// Aligned with `s6_plugin_schema()` which declares a `units` **array**.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct S6UnitConfig {
    pub name: String,
    /// Desired state: "active" or "inactive"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Whether the unit should be enabled at boot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Top-level desired state for the s6 plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct S6Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<Vec<S6UnitConfig>>,
}

/// S6 state plugin — routes mutations through D-Bus, read-only via s6-rc CLI.
///
/// Automatically resolves systemd-style unit names to s6 primitives at runtime:
/// - "nginx.service" → "nginx"
/// - "backup.timer"  → s6-cron job
/// - "api.socket"    → s6-socketmux
///
/// Detects actual unit type by inspecting the s6 service directory structure.
pub struct S6StatePlugin {
    dbus: S6DbusClient,
}

/// Strip known systemd unit-type suffixes from a name.
fn normalize_unit_name(name: &str) -> &str {
    for suffix in [
        ".service", ".timer", ".socket", ".target", ".mount", ".device", ".path",
    ] {
        if let Some(base) = name.strip_suffix(suffix) {
            return base;
        }
    }
    name
}

/// Detect the systemd-compatible suffix for a bare s6 service by inspecting
/// its service-directory structure under `/run/service/<name>/`.
async fn detect_systemd_suffix(name: &str) -> &'static str {
    let base = normalize_unit_name(name);
    let svc_dir = format!("/run/service/{base}");
    let meta = tokio::fs::metadata(&svc_dir).await;
    if !meta.as_ref().is_ok_and(|m| m.is_dir()) {
        return ".service"; // default assumption
    }

    // Check for cron subdirectory → timer
    if tokio::fs::metadata(format!("{svc_dir}/cron")).await.is_ok() {
        return ".timer";
    }
    // Check for socket subdirectory or s6-socketmux
    if tokio::fs::metadata(format!("{svc_dir}/socket"))
        .await
        .is_ok()
        || tokio::fs::metadata(format!("{svc_dir}/env/SOCKET"))
            .await
            .is_ok()
    {
        return ".socket";
    }
    // Check for ftrig / path trigger
    if tokio::fs::metadata(format!("{svc_dir}/ftrig"))
        .await
        .is_ok()
        || tokio::fs::metadata(format!("{svc_dir}/env/PATH"))
            .await
            .is_ok()
    {
        return ".path";
    }
    // Check for mount-related env
    if tokio::fs::metadata(format!("{svc_dir}/env/MOUNTPOINT"))
        .await
        .is_ok()
    {
        return ".mount";
    }
    ".service"
}

impl S6StatePlugin {
    pub fn new() -> Self {
        Self {
            dbus: S6DbusClient::new(),
        }
    }

    /// Return the names of all currently-up services.
    ///
    /// Tries D-Bus first; falls back to `s6-rc -a list` when the daemon isn't running.
    /// subid: `obs.service.s6.list-units@v1`
    async fn list_running(&self) -> Result<Vec<String>> {
        if let Ok(units) = self.dbus.list_units().await {
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
            return Ok(running);
        }

        // Fallback: direct s6-rc query
        let output = tokio::process::Command::new("s6-rc")
            .args(["-a", "list"])
            .output()
            .await
            .context("s6-rc -a list")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Return *all* available service names.
    ///
    /// Tries D-Bus first; falls back to scanning `/etc/s6/sv/`.
    async fn list_all(&self) -> Result<Vec<String>> {
        if let Ok(files) = self.dbus.list_unit_files().await {
            let mut all = Vec::new();
            for file in files {
                if let Some(name) = file.get("name").and_then(|v| v.as_str()) {
                    all.push(name.to_string());
                }
            }
            if !all.is_empty() {
                return Ok(all);
            }
        }

        // Fallback: scan the s6 service directory
        let mut all = Vec::new();
        if let Ok(mut dir) = tokio::fs::read_dir("/etc/s6/sv").await {
            while let Ok(Some(entry)) = dir.next_entry().await {
                if let Ok(name) = entry.file_name().into_string() {
                    all.push(name);
                }
            }
        }
        Ok(all)
    }

    /// Check if a service is enabled by looking at the bundle.
    async fn is_enabled(&self, name: &str) -> bool {
        let bundle_path = format!("/etc/s6-rc/bundle/default/contents.d/{name}");
        tokio::fs::metadata(&bundle_path).await.is_ok()
    }
}

impl Default for S6StatePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for S6StatePlugin {
    fn name(&self) -> &str {
        "s6"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(s6_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new(S6_RC_LIVE).exists()
    }

    fn unavailable_reason(&self) -> String {
        format!("s6-rc live directory not found at {S6_RC_LIVE}")
    }

    async fn query_current_state(&self) -> Result<Value> {
        let running = self.list_running().await?;
        let all = self.list_all().await?;

        let mut units = Vec::new();
        for name in all {
            let is_active = running.contains(&name);
            let is_enabled = self.is_enabled(&name).await;
            // Auto-detect systemd-compatible suffix from s6 directory structure
            let suffix = detect_systemd_suffix(&name).await;
            let display_name = format!("{}{}", normalize_unit_name(&name), suffix);
            units.push(S6UnitConfig {
                name: display_name,
                state: Some(if is_active {
                    "active".to_string()
                } else {
                    "inactive".to_string()
                }),
                enabled: Some(is_enabled),
            });
        }
        Ok(simd_json::serde::to_owned_value(S6Config {
            units: Some(units),
        })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: S6Config = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: S6Config = simd_json::serde::from_owned_value(desired.clone())?;
        let mut actions = Vec::new();

        // Normalize names (strip .service/.timer etc.) so "nginx" and "nginx.service" match
        let current_map: std::collections::HashMap<String, S6UnitConfig> = current_config
            .units
            .unwrap_or_default()
            .into_iter()
            .map(|u| (normalize_unit_name(&u.name).to_string(), u))
            .collect();

        if let Some(desired_units) = desired_config.units {
            for mut desired in desired_units {
                let normalized = normalize_unit_name(&desired.name).to_string();
                desired.name = normalized.clone();
                let current = current_map.get(&normalized);
                if current != Some(&desired) {
                    actions.push(StateAction::Modify {
                        resource: normalized,
                        changes: simd_json::serde::to_owned_value(&desired)?,
                    });
                }
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let unit: S6UnitConfig = simd_json::serde::from_owned_value(changes.clone())?;
                // Normalize resource name so "nginx.service" becomes "nginx"
                let base = normalize_unit_name(resource).to_string();

                // State transition via D-Bus (AGENTS.md §4)
                match unit.state.as_deref() {
                    Some("active") => {
                        if let Err(e) = self.dbus.start(&base).await {
                            errors.push(format!("Failed to start {base}: {e}"));
                        } else {
                            changes_applied.push(format!("Started {base}"));
                        }
                    }
                    Some("inactive") => {
                        if let Err(e) = self.dbus.stop(&base).await {
                            errors.push(format!("Failed to stop {base}: {e}"));
                        } else {
                            changes_applied.push(format!("Stopped {base}"));
                        }
                    }
                    _ => {}
                }

                // Enable / disable via D-Bus (independent of start/stop result)
                match unit.enabled {
                    Some(true) => {
                        if let Err(e) = self.dbus.enable(&base).await {
                            errors.push(format!("Failed to enable {base}: {e}"));
                        } else {
                            changes_applied.push(format!("Enabled {base}"));
                        }
                    }
                    Some(false) => {
                        if let Err(e) = self.dbus.disable(&base).await {
                            errors.push(format!("Failed to disable {base}: {e}"));
                        } else {
                            changes_applied.push(format!("Disabled {base}"));
                        }
                    }
                    None => {}
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
        let current = self.query_current_state().await?;
        let mut current_config: S6Config = simd_json::serde::from_owned_value(current)?;
        let mut desired_config: S6Config = simd_json::serde::from_owned_value(desired.clone())?;
        // Normalize names on both sides so "nginx" == "nginx.service"
        if let Some(ref mut units) = current_config.units {
            for u in units.iter_mut() {
                u.name = normalize_unit_name(&u.name).to_string();
            }
        }
        if let Some(ref mut units) = desired_config.units {
            for u in units.iter_mut() {
                u.name = normalize_unit_name(&u.name).to_string();
            }
        }
        Ok(current_config == desired_config)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("s6-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: S6Config = simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        if let Some(units) = old.units {
            for unit in units {
                let base = normalize_unit_name(&unit.name).to_string();
                match unit.state.as_deref() {
                    Some("active") => {
                        let _ = self.dbus.start(&base).await;
                    }
                    Some("inactive") => {
                        let _ = self.dbus.stop(&base).await;
                    }
                    _ => {}
                }
            }
        }
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

// ── D-Bus client wrapper ──────────────────────────────────────────────────────

use op_state_store::{FieldSchema, FieldType, PluginSchema};
use simd_json::json;
use std::sync::Arc;
use tokio::sync::OnceCell;
use zbus::{proxy, Connection};

/// D-Bus proxy for `org.opdbus.v1.S6.Systemctl`.
#[proxy(
    default_service = "org.opdbus.v1.S6.Systemctl",
    default_path = "/org/opdbus/v1/s6/systemctl",
    interface = "org.opdbus.v1.S6.Systemctl"
)]
trait S6Systemctl {
    /// Start a unit (resolves type at runtime).
    async fn start(&self, unit: &str) -> zbus::Result<(bool, String)>;
    /// Stop a unit (resolves type at runtime).
    async fn stop(&self, unit: &str) -> zbus::Result<(bool, String)>;
    /// Restart a unit.
    async fn restart(&self, unit: &str) -> zbus::Result<(bool, String)>;
    /// Reload a unit.
    async fn reload(&self, unit: &str) -> zbus::Result<(bool, String)>;
    /// Enable a unit.
    async fn enable(&self, unit: &str) -> zbus::Result<(bool, String)>;
    /// Disable a unit.
    async fn disable(&self, unit: &str) -> zbus::Result<(bool, String)>;
    /// Get full status JSON.
    async fn status(&self, unit: &str) -> zbus::Result<String>;
    /// One-word active state.
    async fn is_active(&self, unit: &str) -> zbus::Result<String>;
    /// One-word enabled state.
    async fn is_enabled_method(&self, unit: &str) -> zbus::Result<String>;
    /// Show all properties (systemctl show).
    async fn show(&self, unit: &str) -> zbus::Result<String>;
    /// List active units.
    async fn list_units(&self) -> zbus::Result<String>;
    /// List available unit files.
    async fn list_unit_files(&self) -> zbus::Result<String>;
    /// Get logs (journalctl equivalent via s6-log).
    async fn journalctl(&self, unit: &str, lines: u32) -> zbus::Result<String>;
    /// Recompile s6-rc database (daemon-reload).
    async fn daemon_reload(&self) -> zbus::Result<(bool, String)>;
    /// Daemon status.
    async fn daemon_status(&self) -> zbus::Result<String>;
    /// Get runtime unit type resolution.
    async fn get_unit_type(&self, unit: &str) -> zbus::Result<String>;
}

/// Lazy-initialising D-Bus client for the s6-systemctl service.
#[derive(Clone)]
pub struct S6DbusClient {
    proxy: Arc<OnceCell<S6SystemctlProxy<'static>>>,
}

impl Default for S6DbusClient {
    fn default() -> Self {
        Self::new()
    }
}

impl S6DbusClient {
    pub fn new() -> Self {
        Self {
            proxy: Arc::new(OnceCell::new()),
        }
    }

    async fn get_proxy(&self) -> Result<&S6SystemctlProxy<'static>> {
        self.proxy
            .get_or_try_init(|| async {
                let conn = Connection::system()
                    .await
                    .context("connect to system D-Bus for S6Systemctl")?;
                let proxy: S6SystemctlProxy<'static> = S6SystemctlProxy::new(&conn).await?;
                Ok::<_, anyhow::Error>(proxy)
            })
            .await
            .context("initialise S6Systemctl D-Bus proxy")
    }

    pub async fn start(&self, service: &str) -> Result<()> {
        let proxy = self.get_proxy().await?;
        let (ok, msg) = proxy.start(service).await?;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!("s6-systemctl start {service}: {msg}"))
        }
    }

    pub async fn stop(&self, service: &str) -> Result<()> {
        let proxy = self.get_proxy().await?;
        let (ok, msg) = proxy.stop(service).await?;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!("s6-systemctl stop {service}: {msg}"))
        }
    }

    pub async fn restart(&self, service: &str) -> Result<()> {
        let proxy = self.get_proxy().await?;
        let (ok, msg) = proxy.restart(service).await?;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!("s6-systemctl restart {service}: {msg}"))
        }
    }

    pub async fn reload(&self, service: &str) -> Result<()> {
        let proxy = self.get_proxy().await?;
        let (ok, msg) = proxy.reload(service).await?;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!("s6-systemctl reload {service}: {msg}"))
        }
    }

    pub async fn enable(&self, service: &str) -> Result<()> {
        let proxy = self.get_proxy().await?;
        let (ok, msg) = proxy.enable(service).await?;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!("s6-systemctl enable {service}: {msg}"))
        }
    }

    pub async fn disable(&self, service: &str) -> Result<()> {
        let proxy = self.get_proxy().await?;
        let (ok, msg) = proxy.disable(service).await?;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!("s6-systemctl disable {service}: {msg}"))
        }
    }

    pub async fn status(&self, service: &str) -> Result<String> {
        let proxy = self.get_proxy().await?;
        Ok(proxy.status(service).await?)
    }

    pub async fn is_active(&self, service: &str) -> Result<String> {
        let proxy = self.get_proxy().await?;
        Ok(proxy.is_active(service).await?)
    }

    pub async fn is_enabled(&self, service: &str) -> Result<String> {
        let proxy = self.get_proxy().await?;
        Ok(proxy.is_enabled_method(service).await?)
    }

    pub async fn show(&self, unit: &str) -> Result<HashMap<String, String>> {
        let proxy = self.get_proxy().await?;
        let raw = proxy.show(unit).await?;
        let parsed: HashMap<String, String> = serde_json::from_str(&raw).unwrap_or_default();
        Ok(parsed)
    }

    pub async fn list_unit_files(&self) -> Result<Vec<serde_json::Value>> {
        let proxy = self.get_proxy().await?;
        let raw = proxy.list_unit_files().await?;
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap_or_default();
        Ok(parsed)
    }

    pub async fn list_units(&self) -> Result<Vec<serde_json::Value>> {
        let proxy = self.get_proxy().await?;
        let raw = proxy.list_units().await?;
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap_or_default();
        Ok(parsed)
    }

    pub async fn journalctl(&self, unit: &str, lines: u32) -> Result<Vec<serde_json::Value>> {
        let proxy = self.get_proxy().await?;
        let raw = proxy.journalctl(unit, lines).await?;
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap_or_default();
        Ok(parsed)
    }

    pub async fn daemon_reload(&self) -> Result<()> {
        let proxy = self.get_proxy().await?;
        let (ok, msg) = proxy.daemon_reload().await?;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!("daemon-reload: {msg}"))
        }
    }

    pub async fn get_unit_type(&self, unit: &str) -> Result<String> {
        let proxy = self.get_proxy().await?;
        Ok(proxy.get_unit_type(unit).await?)
    }
}

pub(crate) fn s6_schema() -> PluginSchema {
    let unit_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unit name".to_string(),
                default: None,
                example: Some(json!("nginx.service")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "active".to_string(),
                    "inactive".to_string(),
                    "failed".to_string(),
                ]),
                required: false,
                description: "Desired unit state".to_string(),
                default: Some(json!("active")),
                example: Some(json!("active")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether unit is enabled at boot".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("s6")
        .version("1.0.0")
        .description("s6 service management")
        .array_field("units", FieldType::Object(unit_fields), true, "s6 services")
        .example(json!({
            "units": [
                {
                    "name": "nginx",
                    "state": "active",
                    "enabled": true
                }
            ]
        }))
        .build()
}
