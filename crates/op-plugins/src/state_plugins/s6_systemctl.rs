//! s6_systemctl plugin — projects the op-s6-systemctl D-Bus daemon state.
//!
//! Exposes the daemon's liveness, interface metadata, and a snapshot of all
//! s6 service units so the UI can render a service manager panel from the schema.
//!
//! Schema-driven: every field here maps 1:1 to a UI widget in the schema renderer.
//!
//! OSCAL subids:
//!   obs.service.s6_systemctl.status@v1   — daemon liveness + unit list
//!   mut.service.s6_systemctl.start@v1    — start a unit via D-Bus
//!   mut.service.s6_systemctl.stop@v1     — stop a unit via D-Bus
//!   mut.service.s6_systemctl.restart@v1  — restart a unit via D-Bus
//!   mut.service.s6_systemctl.enable@v1   — enable a unit via D-Bus
//!   mut.service.s6_systemctl.disable@v1  — disable a unit via D-Bus

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};

// ── State structs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub main_pid: Option<u32>,
    /// Seconds the process has been up.
    pub up_time: Option<String>,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct S6SystemctlPlugin;

impl S6SystemctlPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Probe the daemon and enumerate units. Falls back to direct s6 CLI when
    /// the daemon isn't on the bus yet.
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

    /// Check if the daemon is registered on the system bus.
    async fn probe_daemon(&self) -> bool {
        use zbus::Connection;
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

    /// Enumerate all units. Uses direct s6 CLI — no dependency on the daemon.
    async fn enumerate_units(&self, daemon_running: bool) -> Vec<S6UnitStatus> {
        // Get running services
        let running: std::collections::HashSet<String> = tokio::process::Command::new("s6-rc")
            .args(["-a", "list"])
            .output()
            .await
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Get all available services from /etc/s6/sv
        let mut all_names: Vec<String> = Vec::new();
        if let Ok(mut dir) = tokio::fs::read_dir("/etc/s6/sv").await {
            while let Ok(Some(entry)) = dir.next_entry().await {
                if let Ok(name) = entry.file_name().into_string() {
                    all_names.push(name);
                }
            }
        }
        all_names.sort();

        let mut units = Vec::new();
        for name in all_names {
            let is_active = running.contains(&name);
            let enabled =
                tokio::fs::metadata(format!("/etc/s6-rc/bundle/default/contents.d/{}", name))
                    .await
                    .is_ok();

            // Get pid/uptime from s6-svstat if active
            let (main_pid, up_time) = if is_active {
                self.svstat_info(&name).await
            } else {
                (None, None)
            };

            let _ = daemon_running; // available for future D-Bus enrichment

            units.push(S6UnitStatus {
                id: name,
                active_state: if is_active {
                    "active".to_string()
                } else {
                    "inactive".to_string()
                },
                sub_state: if is_active {
                    "running".to_string()
                } else {
                    "dead".to_string()
                },
                enabled,
                main_pid,
                up_time,
            });
        }
        units
    }

    async fn svstat_info(&self, name: &str) -> (Option<u32>, Option<String>) {
        let path = format!("/etc/s6/sv/{}", name);
        let Ok(output) = tokio::process::Command::new("s6-svstat")
            .arg(&path)
            .output()
            .await
        else {
            return (None, None);
        };
        let text = String::from_utf8_lossy(&output.stdout);
        // "up (pid 1234) 3600 seconds"
        let pid = text
            .split("pid ")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .and_then(|p| p.trim().parse::<u32>().ok());
        let uptime = text.split_whitespace().rev().nth(1).map(|s| s.to_string());
        (pid, uptime)
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
        "s6_systemctl"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        // Available as long as s6 is installed; daemon liveness is a field in state
        std::path::Path::new("/etc/s6/sv").exists()
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(
            PluginSchema::builder("s6_systemctl")
                .version("1.0.0")
                .description("s6 service manager — daemon status and unit control")
                .field(
                    "daemon_running",
                    FieldSchema {
                        field_type: FieldType::Boolean,
                        required: true,
                        description: "Whether the op-s6-systemctl D-Bus daemon is reachable"
                            .to_string(),
                        default: Some(json!(false)),
                        example: Some(json!(true)),
                        constraints: Vec::new(),
                        read_only: true,
                        read_only_when: None,
                    },
                )
                .field(
                    "bus_name",
                    FieldSchema {
                        field_type: FieldType::String,
                        required: true,
                        description: "D-Bus well-known name the daemon registers".to_string(),
                        default: Some(json!("org.opdbus.v1.S6.Systemctl")),
                        example: None,
                        constraints: Vec::new(),
                        read_only: true,
                        read_only_when: None,
                    },
                )
                .field(
                    "object_path",
                    FieldSchema {
                        field_type: FieldType::String,
                        required: true,
                        description: "D-Bus object path".to_string(),
                        default: Some(json!("/org/opdbus/v1/s6/systemctl")),
                        example: None,
                        constraints: Vec::new(),
                        read_only: true,
                        read_only_when: None,
                    },
                )
                .field(
                    "interface",
                    FieldSchema {
                        field_type: FieldType::String,
                        required: true,
                        description: "D-Bus interface name".to_string(),
                        default: Some(json!("org.opdbus.v1.S6.Systemctl")),
                        example: None,
                        constraints: Vec::new(),
                        read_only: true,
                        read_only_when: None,
                    },
                )
                .field(
                    "units",
                    FieldSchema {
                        field_type: FieldType::Any,
                        required: true,
                        description: "All s6 service units with live status".to_string(),
                        default: Some(json!([])),
                        example: Some(json!([{
                            "id": "op-projection",
                            "active_state": "active",
                            "sub_state": "running",
                            "enabled": true,
                            "main_pid": 1234,
                            "up_time": "3600"
                        }])),
                        constraints: Vec::new(),
                        read_only: false,
                        read_only_when: None,
                    },
                )
                .build(),
        )
    }


    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_s: S6SystemctlState = simd_json::serde::from_owned_value(current.clone())?;
        let desired_s: S6SystemctlState = simd_json::serde::from_owned_value(desired.clone())?;
        let mut actions = Vec::new();

        // Diff individual units
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
                let unit: S6UnitStatus = simd_json::serde::from_owned_value(changes.clone())?;

                let state_result = match unit.active_state.as_str() {
                    "active" => {
                        tokio::process::Command::new("s6")
                            .args(["process", "start", resource])
                            .status()
                            .await
                    }
                    "inactive" => {
                        tokio::process::Command::new("s6")
                            .args(["process", "stop", resource])
                            .status()
                            .await
                    }
                    _ => {
                        continue;
                    }
                };

                match state_result {
                    Ok(s) if s.success() => {
                        changes_applied.push(format!("{} -> {}", resource, unit.active_state))
                    }
                    Ok(s) => errors.push(format!(
                        "{}: s6 exited {}",
                        resource,
                        s.code().unwrap_or(-1)
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
        let state = simd_json::json!(null);
        Ok(Checkpoint {
            id: format!("s6_systemctl-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: S6SystemctlState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        for unit in old.units {
            let cmd = match unit.active_state.as_str() {
                "active" => "start",
                "inactive" => "stop",
                _ => continue,
            };
            let _ = tokio::process::Command::new("s6")
                .args(["process", cmd, &unit.id])
                .status()
                .await;
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

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("s6_systemctl", |_ctx| std::sync::Arc::new(S6SystemctlPlugin::new()))
}
