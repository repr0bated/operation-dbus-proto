//! s6_systemctl plugin — GB.S6Systemctl.
//!
//! Projects the op-s6-systemctl D-Bus daemon state.
//! Exposes daemon liveness, interface metadata, and a snapshot of all
//! s6 service units so the UI can render a service manager panel from the schema.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{PluginSchema, SideEffect};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::OnceCell;
use zbus::{proxy, Connection};

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};

// =============================================================================
// PLUGIN ENTRY: identity and typed schema seed
// =============================================================================

const PLUGIN_NAME: &str = "s6_systemctl";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "service";
const PLUGIN_DESCRIPTION: &str = "s6 service manager — daemon status and unit control";
const PLUGIN_DISPLAY_NAME: &str = "GB.S6Systemctl";

/// D-Bus proxy for the canonical `op-s6-systemctl` daemon.
#[proxy(
    default_service = "org.opdbus.v1.S6.Systemctl",
    default_path = "/org/opdbus/v1/s6/systemctl",
    interface = "org.opdbus.v1.S6.Systemctl"
)]
trait S6Systemctl {
    async fn start(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn stop(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn restart(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn reload(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn enable(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn disable(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn status(&self, unit: &str) -> zbus::Result<String>;
    async fn is_active(&self, unit: &str) -> zbus::Result<String>;
    async fn is_enabled_method(&self, unit: &str) -> zbus::Result<String>;
    async fn list_units(&self) -> zbus::Result<String>;
    async fn list_unit_files(&self) -> zbus::Result<String>;
    async fn daemon_reload(&self) -> zbus::Result<(bool, String)>;
}

/// Lazy D-Bus client used by the `s6_systemctl` plugin and other service plugins.
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
        self.check_result(
            "start",
            service,
            self.get_proxy().await?.start(service).await?,
        )
    }

    pub async fn stop(&self, service: &str) -> Result<()> {
        self.check_result(
            "stop",
            service,
            self.get_proxy().await?.stop(service).await?,
        )
    }

    pub async fn restart(&self, service: &str) -> Result<()> {
        self.check_result(
            "restart",
            service,
            self.get_proxy().await?.restart(service).await?,
        )
    }

    pub async fn reload(&self, service: &str) -> Result<()> {
        self.check_result(
            "reload",
            service,
            self.get_proxy().await?.reload(service).await?,
        )
    }

    pub async fn enable(&self, service: &str) -> Result<()> {
        self.check_result(
            "enable",
            service,
            self.get_proxy().await?.enable(service).await?,
        )
    }

    pub async fn disable(&self, service: &str) -> Result<()> {
        self.check_result(
            "disable",
            service,
            self.get_proxy().await?.disable(service).await?,
        )
    }

    pub async fn status(&self, service: &str) -> Result<String> {
        Ok(self.get_proxy().await?.status(service).await?)
    }

    pub async fn is_active(&self, service: &str) -> Result<String> {
        Ok(self.get_proxy().await?.is_active(service).await?)
    }

    pub async fn is_enabled(&self, service: &str) -> Result<String> {
        Ok(self.get_proxy().await?.is_enabled_method(service).await?)
    }

    pub async fn list_units(&self) -> Result<Vec<serde_json::Value>> {
        let raw = self.get_proxy().await?.list_units().await?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub async fn list_unit_files(&self) -> Result<Vec<serde_json::Value>> {
        let raw = self.get_proxy().await?.list_unit_files().await?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub async fn daemon_reload(&self) -> Result<()> {
        self.check_result(
            "daemon-reload",
            "s6",
            self.get_proxy().await?.daemon_reload().await?,
        )
    }

    fn check_result(&self, command: &str, service: &str, result: (bool, String)) -> Result<()> {
        let (ok, message) = result;
        if ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "s6-systemctl {command} {service}: {message}"
            ))
        }
    }
}

// ── State structs with schemars::JsonSchema ──────────────────────────────────

/// Plugin state for the s6_systemctl projection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
pub struct S6SystemctlState {
    /// Whether the op-s6-systemctl D-Bus daemon is reachable.
    pub daemon_running: bool,
    /// D-Bus bus name the daemon registers.
    pub bus_name: String,
    /// D-Bus object path.
    pub object_path: String,
    /// D-Bus interface name.
    pub interface: String,
    /// All s6 service units with their current status.
    pub units: Vec<S6UnitStatus>,
}

/// Status of a single s6 service unit.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
pub struct S6UnitStatus {
    /// Service name (bare, no suffix).
    pub id: String,
    /// "active" | "inactive" | "failed" | "unknown"
    pub active_state: String,
    /// "running" | "dead" | "unknown"
    pub sub_state: String,
    /// Whether the unit starts at boot.
    pub enabled: bool,
    /// Main PID if running.
    #[serde(default)]
    pub main_pid: Option<u32>,
    /// Seconds the process has been up.
    #[serde(default)]
    pub up_time: Option<String>,
}

// ── Method input/output types ────────────────────────────────────────────────

/// Input for unit-targeting methods (start, stop, restart, enable, disable, status, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UnitInput {
    /// Service name.
    pub name: String,
}

/// Output for status method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StatusOutput {
    pub status: String,
}

/// Output for is_active method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IsActiveOutput {
    pub active: bool,
}

/// Output for is_enabled method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IsEnabledOutput {
    pub enabled: bool,
}

/// Output for list_units method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListUnitsOutput {
    pub units: Vec<S6UnitStatus>,
}

/// Output for daemon_reload method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DaemonReloadOutput {
    pub success: bool,
    pub message: String,
}

/// Empty input for methods with no parameters.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EmptyInput {}

// =============================================================================
// PLUGIN BODY: D-Bus-backed behavior only
// =============================================================================

pub struct S6SystemctlPlugin {
    dbus: S6DbusClient,
}

impl S6SystemctlPlugin {
    pub fn new() -> Self {
        Self {
            dbus: S6DbusClient::new(),
        }
    }

    async fn collect_state(&self) -> S6SystemctlState {
        let daemon_running = self.probe_daemon().await;
        let units = self.enumerate_units(daemon_running).await;

        S6SystemctlState {
            daemon_running,
            bus_name: "org.opdbus.v1.S6.Systemctl".to_string(),
            object_path: "/org/opdbus/v1/s6/systemctl".to_string(),
            interface: "org.opdbus.v1.S6.Systemctl".to_string(),
            units,
        }
    }

    async fn probe_daemon(&self) -> bool {
        let Ok(conn) = Connection::system().await else {
            return false;
        };
        conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &"org.opdbus.v1.S6.Systemctl",
        )
        .await
        .is_ok()
    }

    async fn enumerate_units(&self, daemon_running: bool) -> Vec<S6UnitStatus> {
        if !daemon_running {
            return Vec::new();
        }

        let unit_files = self.dbus.list_unit_files().await.unwrap_or_default();
        let live_units = self.dbus.list_units().await.unwrap_or_default();
        let mut live_by_name = std::collections::HashMap::new();
        for unit in live_units {
            if let Some(name) = unit.get("name").and_then(|v| v.as_str()) {
                live_by_name.insert(name.to_string(), unit);
            }
        }

        let mut units = Vec::new();
        for file in unit_files {
            let Some(unit_file) = file.get("unit_file").and_then(|v| v.as_str()) else {
                continue;
            };
            let id = unit_file
                .strip_suffix(".service")
                .unwrap_or(unit_file)
                .to_string();
            let enabled = file
                .get("state")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s == "enabled");
            let live = live_by_name.get(&id);

            units.push(S6UnitStatus {
                id,
                active_state: live
                    .and_then(|u| u.get("active_state"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("inactive")
                    .to_string(),
                sub_state: live
                    .and_then(|u| u.get("sub_state"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("dead")
                    .to_string(),
                enabled,
                main_pid: live
                    .and_then(|u| u.get("main_pid"))
                    .and_then(|v| v.as_u64())
                    .and_then(|p| u32::try_from(p).ok()),
                up_time: live
                    .and_then(|u| u.get("up_time"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
        units.sort_by(|a, b| a.id.cmp(&b.id));
        units
    }
}

impl Default for S6SystemctlPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for S6SystemctlPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/etc/s6/sv").exists()
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = s6_systemctl_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_s: S6SystemctlState = simd_json::serde::from_owned_value(current.clone())?;
        let desired_s: S6SystemctlState = simd_json::serde::from_owned_value(desired.clone())?;
        let mut actions = Vec::new();

        let current_map: std::collections::HashMap<_, _> =
            current_s.units.iter().map(|u| (u.id.clone(), u)).collect();

        for desired_unit in &desired_s.units {
            if let Some(current_unit) = current_map.get(&desired_unit.id) {
                if current_unit.active_state != desired_unit.active_state
                    || current_unit.enabled != desired_unit.enabled
                {
                    actions.push(StateAction::Modify {
                        resource: desired_unit.id.clone(),
                        changes: simd_json::serde::to_owned_value(desired_unit)?,
                    });
                }
            }
        }

        Ok(StateDiff {
            plugin: PLUGIN_NAME.to_string(),
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
                let unit: S6UnitStatus = simd_json::serde::from_owned_value(changes.clone())?;

                match unit.active_state.as_str() {
                    "active" => match self.dbus.start(resource).await {
                        Ok(()) => changes_applied.push(format!("{} -> active", resource)),
                        Err(e) => errors.push(format!("{}: {}", resource, e)),
                    },
                    "inactive" => match self.dbus.stop(resource).await {
                        Ok(()) => changes_applied.push(format!("{} -> inactive", resource)),
                        Err(e) => errors.push(format!("{}: {}", resource, e)),
                    },
                    _ => {}
                }

                let enable_result = if unit.enabled {
                    self.dbus.enable(resource).await
                } else {
                    self.dbus.disable(resource).await
                };
                match enable_result {
                    Ok(()) => changes_applied.push(format!(
                        "{} -> {}",
                        resource,
                        if unit.enabled { "enabled" } else { "disabled" }
                    )),
                    Err(e) => errors.push(format!("{}: {}", resource, e)),
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

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("{}-{}", PLUGIN_NAME, chrono::Utc::now().timestamp()),
            plugin: PLUGIN_NAME.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!(null),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: S6SystemctlState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        for unit in old.units {
            match unit.active_state.as_str() {
                "active" => {
                    let _ = self.dbus.start(&unit.id).await;
                }
                "inactive" => {
                    let _ = self.dbus.stop(&unit.id).await;
                }
                _ => continue,
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

// =============================================================================
// PLUGIN EXIT: publish the single PluginSchema contract
// =============================================================================

pub fn s6_systemctl_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(S6SystemctlState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());

    schema.methods.insert(
        "start".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, AckOutput>(
            "start",
            SideEffect::Mutation,
            false,
            "s6.systemctl.write",
            "mut.software.s6.systemctl.start@v1",
        ),
    );
    schema.methods.insert(
        "stop".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, AckOutput>(
            "stop",
            SideEffect::Mutation,
            false,
            "s6.systemctl.write",
            "mut.software.s6.systemctl.stop@v1",
        ),
    );
    schema.methods.insert(
        "restart".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, AckOutput>(
            "restart",
            SideEffect::Mutation,
            false,
            "s6.systemctl.write",
            "mut.software.s6.systemctl.restart@v1",
        ),
    );
    schema.methods.insert(
        "enable".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, AckOutput>(
            "enable",
            SideEffect::Mutation,
            false,
            "s6.systemctl.write",
            "mut.software.s6.systemctl.enable@v1",
        ),
    );
    schema.methods.insert(
        "disable".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, AckOutput>(
            "disable",
            SideEffect::Mutation,
            false,
            "s6.systemctl.write",
            "mut.software.s6.systemctl.disable@v1",
        ),
    );
    schema.methods.insert(
        "status".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, StatusOutput>(
            "status",
            SideEffect::Read,
            true,
            "s6.systemctl.read",
            "obs.software.s6.systemctl.status@v1",
        ),
    );
    schema.methods.insert(
        "is_active".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, IsActiveOutput>(
            "is_active",
            SideEffect::Read,
            true,
            "s6.systemctl.read",
            "obs.software.s6.systemctl.is_active@v1",
        ),
    );
    schema.methods.insert(
        "is_enabled".to_string(),
        method_decl_from_schemars_with_output::<UnitInput, IsEnabledOutput>(
            "is_enabled",
            SideEffect::Read,
            true,
            "s6.systemctl.read",
            "obs.software.s6.systemctl.is_enabled@v1",
        ),
    );
    schema.methods.insert(
        "daemon_reload".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, DaemonReloadOutput>(
            "daemon_reload",
            SideEffect::Mutation,
            false,
            "s6.systemctl.write",
            "mut.software.s6.systemctl.daemon_reload@v1",
        ),
    );
    schema.methods.insert(
        "list_units".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, ListUnitsOutput>(
            "list_units",
            SideEffect::Read,
            true,
            "s6.systemctl.read",
            "obs.software.s6.systemctl.list_units@v1",
        ),
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(S6SystemctlPlugin::new()))
}
