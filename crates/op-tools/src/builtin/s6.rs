//! S6 service management tools via D-Bus.
//!
//! All operations target the s6-systemctl D-Bus service.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use zbus::{proxy, Connection};

use crate::{Tool, ToolRegistry};

#[proxy(
    interface = "org.opdbus.s6.Systemctl",
    default_service = "org.opdbus.s6.Systemctl",
    default_path = "/org/opdbus/s6/Systemctl"
)]
trait S6Systemctl {
    async fn start(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn stop(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn restart(&self, unit: &str) -> zbus::Result<(bool, String)>;
    async fn status(&self, unit: &str) -> zbus::Result<String>;
    async fn list_units(&self) -> zbus::Result<String>;
}

/// Lazy-initialising D-Bus client for the s6-systemctl service.
async fn get_proxy() -> Result<S6SystemctlProxy<'static>> {
    let conn = Connection::system()
        .await
        .context("connect to system D-Bus for S6Systemctl")?;
    S6SystemctlProxy::new(&conn)
        .await
        .context("create S6Systemctl D-Bus proxy")
}

/// Helper to run s6-rc via D-Bus fallback.
async fn s6rc(args: &[&str]) -> Result<std::process::Output> {
    // D-Bus is preferred; fallback to s6-rc only when D-Bus is unreachable.
    tokio::process::Command::new("s6-rc")
        .arg("-l")
        .arg("/run/s6-rc")
        .args(args)
        .output()
        .await
        .context("failed to run s6-rc")
}

// ─── Tool structs ────────────────────────────────────────────────────────────

pub struct S6StartServiceTool;
pub struct S6StopServiceTool;
pub struct S6StatusTool;
pub struct S6ListServicesTool;

// ─── s6_start_service ────────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6StartServiceTool {
    fn name(&self) -> &str {
        "s6_start_service"
    }

    fn description(&self) -> &str {
        "Start an s6 service via s6-rc"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "s6 service name (must exist under /etc/s6/sv/)"
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        match get_proxy().await {
            Ok(proxy) => {
                let (ok, msg) = proxy.start(service).await?;
                if ok {
                    return Ok(json!({
                        "started": true,
                        "service": service,
                        "manager": "s6"
                    }));
                }
                // Treat "already up" as success
                if msg.contains("already") {
                    return Ok(json!({
                        "started": true,
                        "service": service,
                        "manager": "s6",
                        "note": "service was already running"
                    }));
                }
                Err(anyhow!("s6 D-Bus start {service} failed: {msg}"))
            }
            Err(_) => {
                let out = s6rc(&["start", service]).await?;
                if out.status.success() {
                    return Ok(json!({
                        "started": true,
                        "service": service,
                        "manager": "s6"
                    }));
                }
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("already") {
                    return Ok(json!({
                        "started": true,
                        "service": service,
                        "manager": "s6",
                        "note": "service was already running"
                    }));
                }
                Err(anyhow!("s6-rc start {service} failed: {stderr}"))
            }
        }
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── s6_stop_service ─────────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6StopServiceTool {
    fn name(&self) -> &str {
        "s6_stop_service"
    }

    fn description(&self) -> &str {
        "Stop an s6 service via s6-rc"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "s6 service name"
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        match get_proxy().await {
            Ok(proxy) => {
                let (ok, msg) = proxy.stop(service).await?;
                if ok {
                    return Ok(json!({
                        "stopped": true,
                        "service": service,
                        "manager": "s6"
                    }));
                }
                if msg.contains("already") {
                    return Ok(json!({
                        "stopped": true,
                        "service": service,
                        "manager": "s6",
                        "note": "service was already stopped"
                    }));
                }
                Err(anyhow!("s6 D-Bus stop {service} failed: {msg}"))
            }
            Err(_) => {
                let out = s6rc(&["stop", service]).await?;
                if out.status.success() {
                    return Ok(json!({
                        "stopped": true,
                        "service": service,
                        "manager": "s6"
                    }));
                }
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("already") {
                    return Ok(json!({
                        "stopped": true,
                        "service": service,
                        "manager": "s6",
                        "note": "service was already stopped"
                    }));
                }
                Err(anyhow!("s6-rc stop {service} failed: {stderr}"))
            }
        }
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── s6_service_status ───────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6StatusTool {
    fn name(&self) -> &str {
        "s6_service_status"
    }

    fn description(&self) -> &str {
        "Get the current status of an s6-managed service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "s6 service name"
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        let svc_path = format!("/run/service/{service}");
        let svc_exists = std::path::Path::new(&svc_path).exists();
        let down_file = format!("{svc_path}/down");
        let is_down = std::path::Path::new(&down_file).exists();

        let is_running = match get_proxy().await {
            Ok(proxy) => {
                let status_str = proxy.status(service).await.unwrap_or_default();
                status_str.contains("up") || status_str.contains("active")
            }
            Err(_) => {
                let out = s6rc(&["-a", "list"]).await.ok();
                if let Some(out) = out {
                    let running_list = String::from_utf8_lossy(&out.stdout);
                    running_list.lines().any(|l| l.trim() == service)
                } else {
                    false
                }
            }
        };

        let status = if is_running {
            "active"
        } else if is_down {
            "inactive (down)"
        } else if svc_exists {
            "inactive"
        } else {
            "unknown"
        };

        Ok(json!({
            "service": service,
            "status": status,
            "running": is_running,
            "manager": "s6"
        }))
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── s6_list_services ────────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6ListServicesTool {
    fn name(&self) -> &str {
        "s6_list_services"
    }

    fn description(&self) -> &str {
        "List all running s6-managed services"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let services = match get_proxy().await {
            Ok(proxy) => {
                let raw = proxy.list_units().await.unwrap_or_default();
                let parsed: Vec<serde_json::Value> =
                    serde_json::from_str(&raw).unwrap_or_default();
                parsed
                    .into_iter()
                    .filter_map(|u| u.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            }
            Err(_) => {
                let out = s6rc(&["-a", "list"]).await?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(anyhow!("s6-rc list failed: {stderr}"));
                }
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            }
        };

        let count = services.len();
        Ok(json!({
            "services": services,
            "count": count,
            "manager": "s6"
        }))
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── Registration ─────────────────────────────────────────────────────────────

/// Register all s6 service management tools.
pub async fn register_s6_tools(registry: &ToolRegistry) -> Result<()> {
    registry.register_tool(Arc::new(S6StartServiceTool)).await?;
    registry.register_tool(Arc::new(S6StopServiceTool)).await?;
    registry.register_tool(Arc::new(S6StatusTool)).await?;
    registry.register_tool(Arc::new(S6ListServicesTool)).await?;
    Ok(())
}
