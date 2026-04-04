//! Dinit tools.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::process::Command;

use crate::{Tool, ToolRegistry};

pub struct DbusDinitStartServiceTool;
pub struct DbusDinitStopServiceTool;
pub struct DbusDinitStatusTool;
pub struct DbusDinitListServicesTool;

#[async_trait]
impl Tool for DbusDinitStartServiceTool {
    fn name(&self) -> &str {
        "dbus_dinit_start_service"
    }

    fn description(&self) -> &str {
        "Start a dinit service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string", "description": "Dinit service name" }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;
        run_dinitctl(["start", service]).await?;
        Ok(
            json!({ "started": true, "service": service, "manager": "dinit", "protocol": "dinitctl" }),
        )
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

#[async_trait]
impl Tool for DbusDinitStopServiceTool {
    fn name(&self) -> &str {
        "dbus_dinit_stop_service"
    }

    fn description(&self) -> &str {
        "Stop a dinit service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string", "description": "Dinit service name" }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;
        run_dinitctl(["stop", service]).await?;
        Ok(
            json!({ "stopped": true, "service": service, "manager": "dinit", "protocol": "dinitctl" }),
        )
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

#[async_trait]
impl Tool for DbusDinitStatusTool {
    fn name(&self) -> &str {
        "dbus_dinit_get_service_status"
    }

    fn description(&self) -> &str {
        "Get dinit service status"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string", "description": "Dinit service name" }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;
        let status = run_dinitctl(["status", service]).await?;
        Ok(
            json!({ "service": service, "status": status, "manager": "dinit", "protocol": "dinitctl" }),
        )
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

#[async_trait]
impl Tool for DbusDinitListServicesTool {
    fn name(&self) -> &str {
        "dbus_dinit_list_services"
    }

    fn description(&self) -> &str {
        "List dinit services"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let raw = run_dinitctl(["list"]).await?;
        let services: Vec<String> = raw
            .lines()
            .filter_map(|line| line.split_whitespace().last().map(|s| s.to_string()))
            .collect();
        Ok(json!({
            "services": services.clone(),
            "count": services.len(),
            "manager": "dinit",
            "protocol": "dinitctl"
        }))
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

pub async fn register_dinit_tools(registry: &ToolRegistry) -> Result<()> {
    registry
        .register_tool(Arc::new(DbusDinitStartServiceTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusDinitStopServiceTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusDinitStatusTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusDinitListServicesTool))
        .await?;
    Ok(())
}

async fn run_dinitctl<const N: usize>(args: [&str; N]) -> Result<String> {
    let out = Command::new("dinitctl").args(args).output().await?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!(
            "dinitctl {} failed: {}",
            args.join(" "),
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
