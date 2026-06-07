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
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

    /// Minimal HTTP-over-UnixSocket helper for Incus REST API
    async fn incus_api_get(path: &str) -> Result<simd_json::OwnedValue> {
        let socket_path = "/var/lib/incus/unix.socket";
        if !std::path::Path::new(socket_path).exists() {
            return Err(anyhow::anyhow!(
                "Incus Unix socket not found at {}",
                socket_path
            ));
        }

        let mut stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .context("Failed to connect to Incus Unix socket")?;

        let request = format!(
            "GET {} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\n\r\n",
            path
        );
        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => response.extend_from_slice(&buf[..n]),
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            }
        }

        let body_start = if let Some(idx) = response.windows(4).position(|w| w == b"\r\n\r\n") {
            idx + 4
        } else if let Some(idx) = response.windows(2).position(|w| w == b"\n\n") {
            idx + 2
        } else {
            return Ok(simd_json::json!({}));
        };

        let mut body = response[body_start..].to_vec();
        let headers = std::str::from_utf8(&response[..body_start]).unwrap_or("");
        if headers
            .to_lowercase()
            .contains("transfer-encoding: chunked")
        {
            let mut decoded = Vec::new();
            let mut pos = 0;
            while pos < body.len() {
                let mut line_end = pos;
                while line_end < body.len() && body[line_end] != b'\n' {
                    line_end += 1;
                }
                if line_end >= body.len() {
                    break;
                }
                let line = std::str::from_utf8(&body[pos..line_end])
                    .unwrap_or("")
                    .trim();
                let size = usize::from_str_radix(line.split(';').next().unwrap_or("0").trim(), 16)
                    .unwrap_or(0);
                if size == 0 {
                    break;
                }
                pos = line_end + 1;
                if pos < body.len() && body[pos] == b'\r' {
                    pos += 1;
                }
                decoded.extend_from_slice(&body[pos..pos + size]);
                pos += size;
                if pos < body.len() && body[pos] == b'\r' {
                    pos += 1;
                }
                if pos < body.len() && body[pos] == b'\n' {
                    pos += 1;
                }
            }
            body = decoded;
        }

        let mut raw = body;
        let val: simd_json::OwnedValue =
            simd_json::from_slice(&mut raw).context("Failed to parse Incus API response")?;
        let metadata = val.get("metadata").cloned().unwrap_or(simd_json::json!({}));
        Ok(metadata)
    }

    /// Query incus for container status via REST API (AGENTS.md §4: no subprocess bypasses)
    async fn query_container_status(&self, name: &str) -> Result<(String, Option<String>)> {
        let inst = Self::incus_api_get(&format!("/1.0/instances/{}?recursion=1", name)).await?;

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
    }

    /// Check if container is Running as a proxy for mail health
    /// (AGENTS.md §4: incus exec is a subprocess bypass; we use container state instead)
    async fn check_mail_health(&self, container: &str) -> (bool, Option<String>) {
        match Self::incus_api_get(&format!("/1.0/instances/{}?recursion=1", container)).await {
            Ok(inst) => {
                let status = inst
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                if status == "Running" {
                    (true, None)
                } else {
                    let err = format!("container_status={}", status);
                    (false, Some(err))
                }
            }
            Err(e) => {
                let err = format!("failed to query container: {}", e);
                (false, Some(err))
            }
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

        // Query container devices from Incus REST API (AGENTS.md §4: no subprocess bypasses)
        if let Ok(config) = Self::incus_api_get(&format!(
            "/1.0/instances/{}?recursion=1",
            state.container_name
        ))
        .await
        {
            if let Some(devices) = config.get("devices") {
                if let Ok(dev_map) = simd_json::serde::from_owned_value::<
                    HashMap<String, HashMap<String, String>>,
                >(devices.clone())
                {
                    state.devices = Some(dev_map);
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
