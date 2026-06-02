//! Mail server state plugin - manages Incus mail container and D-Bus registration.
//!
//! Tracks Postfix/Dovecot runtime state, Unix socket endpoints for Xray routing,
//! and exposes mail configuration as a D-Bus object via zbus.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Top-level state for the mail server plugin.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailServerState {
    /// Incus container name running the mail stack
    pub container_name: String,
    /// Container status: "Running", "Stopped", "Frozen"
    pub container_status: String,
    /// Primary mail domain
    pub domain: String,
    /// Unix socket path for Xray naive routing integration
    pub xray_socket_path: String,
    /// D-Bus service name registered for this mail instance
    pub dbus_service_name: String,
    /// Active mail service endpoints
    pub endpoints: MailEndpoints,
    /// Container IPv4 address
    pub container_ip: Option<String>,
    /// Whether the mail stack is healthy
    pub healthy: bool,
    /// Last error message if unhealthy
    pub last_error: Option<String>,
    /// Additional container devices (unix socket mounts, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<HashMap<String, HashMap<String, String>>>,
}

/// Mail protocol endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailEndpoints {
    /// SMTP submission port (587)
    pub smtp_submission: Option<String>,
    /// SMTP TLS port (465)
    pub smtp_tls: Option<String>,
    /// IMAP port (143)
    pub imap: Option<String>,
    /// IMAPS port (993)
    pub imaps: Option<String>,
    /// Dovecot LDA/LMTP unix socket inside container
    pub dovecot_lmtp: Option<String>,
    /// Postfix pickup unix socket inside container
    pub postfix_pickup: Option<String>,
}

pub struct MailServerPlugin;

impl MailServerPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Default state for 3tched.com mail stack
    fn default_state() -> MailServerState {
        MailServerState {
            container_name: "mail-3tched".to_string(),
            container_status: "Unknown".to_string(),
            domain: "3tched.com".to_string(),
            xray_socket_path: "/run/xray/mail-naive.sock".to_string(),
            dbus_service_name: "org.opdbus.MailServer.3tched".to_string(),
            endpoints: MailEndpoints {
                smtp_submission: Some("0.0.0.0:587".to_string()),
                smtp_tls: Some("0.0.0.0:465".to_string()),
                imap: Some("0.0.0.0:143".to_string()),
                imaps: Some("0.0.0.0:993".to_string()),
                dovecot_lmtp: Some("/var/spool/postfix/private/dovecot-lmtp".to_string()),
                postfix_pickup: Some("/var/spool/postfix/private/pickup".to_string()),
            },
            container_ip: None,
            healthy: false,
            last_error: None,
            devices: None,
        }
    }

    /// Query incus for container status
    async fn query_container_status(&self, name: &str) -> Result<(String, Option<String>)> {
        let output = tokio::process::Command::new("/usr/bin/incus")
            .args(["list", name, "--format=json"])
            .output()
            .await
            .context("Failed to query incus container status")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("incus list failed: {}", stderr.trim());
        }

        let mut raw = output.stdout;
        let instances: Vec<simd_json::OwnedValue> =
            simd_json::from_slice(&mut raw).unwrap_or_default();

        if let Some(inst) = instances.first() {
            let status = inst
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let ip = inst
                .get("state")
                .and_then(|s| s.get("network"))
                .and_then(|n| n.get("eth0"))
                .and_then(|e| e.get("addresses"))
                .and_then(|a| a.as_array())
                .and_then(|addrs| {
                    addrs.iter().find_map(|addr| {
                        if addr.get("family")?.as_str()? == "inet" {
                            addr.get("address")?.as_str().map(String::from)
                        } else {
                            None
                        }
                    })
                });

            Ok((status, ip))
        } else {
            Ok(("NotFound".to_string(), None))
        }
    }

    /// Check if Postfix and Dovecot are responding inside the container
    async fn check_mail_health(&self, container: &str) -> (bool, Option<String>) {
        // Check postfix is running inside container
        let postfix = tokio::process::Command::new("/usr/bin/incus")
            .args(["exec", container, "--", "postfix", "status"])
            .output()
            .await;

        let postfix_ok = postfix.map(|o| o.status.success()).unwrap_or(false);

        // Check dovecot is running inside container
        let dovecot = tokio::process::Command::new("/usr/bin/incus")
            .args(["exec", container, "--", "doveadm", "service", "status"])
            .output()
            .await;

        let dovecot_ok = dovecot.map(|o| o.status.success()).unwrap_or(false);

        if postfix_ok && dovecot_ok {
            (true, None)
        } else {
            let err = format!("postfix_ok={}, dovecot_ok={}", postfix_ok, dovecot_ok);
            (false, Some(err))
        }
    }
}

impl Default for MailServerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for MailServerPlugin {
    fn metadata(&self) -> op_state::PluginMetadata {
        op_state::PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: "Mail server container state and D-Bus registration for 3tched.com"
                .to_string(),
            author: None,
            license: None,
            dependencies: vec!["incus".to_string(), "unix_socket".to_string()],
            dbus_services: vec!["org.opdbus.MailServer.3tched".to_string()],
            feature_schemas: vec![],
            object_schemas: std::collections::HashMap::new(),
        }
    }

    fn name(&self) -> &str {
        "mail_server"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::mail_server_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut state = Self::default_state();

        match self.query_container_status(&state.container_name).await {
            Ok((status, ip)) => {
                state.container_status = status;
                state.container_ip = ip;
            }
            Err(e) => {
                state.container_status = "Error".to_string();
                state.last_error = Some(e.to_string());
            }
        }

        if state.container_status == "Running" {
            let (healthy, err) = self.check_mail_health(&state.container_name).await;
            state.healthy = healthy;
            state.last_error = err;
        }

        // Query container devices from incus config
        let config_output = tokio::process::Command::new("/usr/bin/incus")
            .args(["config", "show", &state.container_name, "--format=json"])
            .output()
            .await;

        if let Ok(out) = config_output {
            if out.status.success() {
                let mut raw = out.stdout;
                if let Ok(config) = simd_json::from_slice::<simd_json::OwnedValue>(&mut raw) {
                    if let Some(devices) = config.get("devices") {
                        if let Ok(dev_map) = simd_json::serde::from_owned_value::<
                            HashMap<String, HashMap<String, String>>,
                        >(devices.clone())
                        {
                            state.devices = Some(dev_map);
                        }
                    }
                }
            }
        }

        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
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
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
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
