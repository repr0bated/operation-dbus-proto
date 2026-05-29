//! S6 service management tools via the s6-rc CLI.
//!
//! All operations target the live s6-rc database at /run/s6-rc.
//! No D-Bus is involved — s6 is a purely CLI-driven init system.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

use crate::{Tool, ToolRegistry};

/// Path to the s6-rc live state directory.
const S6_RC_LIVE: &str = "/run/s6-rc";

/// Run `s6-rc -l /run/s6-rc <args…>` and return the output.
async fn s6rc(args: &[&str]) -> Result<std::process::Output> {
    tokio::process::Command::new("s6-rc")
        .arg("-l")
        .arg(S6_RC_LIVE)
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

        let out = s6rc(&["start", service]).await?;

        if out.status.success() {
            return Ok(json!({
                "started": true,
                "service": service,
                "manager": "s6"
            }));
        }

        let stderr = String::from_utf8_lossy(&out.stderr);
        // Treat "already up" as success
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

        // Check whether the service directory exists in the live run area
        let svc_path = format!("/run/service/{service}");
        let svc_exists = std::path::Path::new(&svc_path).exists();

        // A "down" file inside the service directory means the service is intentionally stopped
        let down_file = format!("{svc_path}/down");
        let is_down = std::path::Path::new(&down_file).exists();

        // Ask s6-rc for the authoritative list of running services
        let out = s6rc(&["-a", "list"]).await?;
        let running_list = String::from_utf8_lossy(&out.stdout);
        let is_running = running_list.lines().any(|l| l.trim() == service);

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
        // -a = only active (running) services
        let out = s6rc(&["-a", "list"]).await?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(anyhow!("s6-rc list failed: {stderr}"));
        }

        let services: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

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
