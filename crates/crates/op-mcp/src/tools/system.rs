//! System Information Tools

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(ProcFsTool)).await?;
    registry.register(Arc::new(ListNetworkInterfacesTool)).await?;
    Ok(2)
}

pub struct ProcFsTool;

#[async_trait]
impl Tool for ProcFsTool {
    fn name(&self) -> &str { "procfs_read" }
    fn description(&self) -> &str { "Read system information from /proc filesystem." }
    fn category(&self) -> &str { "system" }
    fn tags(&self) -> Vec<String> { vec!["system".into(), "procfs".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "info_type": {
                    "type": "string",
                    "enum": ["cpuinfo", "meminfo", "loadavg", "uptime", "version", "mounts"]
                }
            },
            "required": ["info_type"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let info_type = input.get("info_type").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing info_type"))?;
        
        let path = match info_type {
            "cpuinfo" => "/proc/cpuinfo",
            "meminfo" => "/proc/meminfo",
            "loadavg" => "/proc/loadavg",
            "uptime" => "/proc/uptime",
            "version" => "/proc/version",
            "mounts" => "/proc/mounts",
            _ => return Ok(json!({"success": false, "error": "Unknown info_type"}))
        };
        
        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(json!({"success": true, "info_type": info_type, "content": content})),
            Err(e) => Ok(json!({"success": false, "error": e.to_string()}))
        }
    }
}

pub struct ListNetworkInterfacesTool;

#[async_trait]
impl Tool for ListNetworkInterfacesTool {
    fn name(&self) -> &str { "list_network_interfaces" }
    fn description(&self) -> &str { "List all network interfaces." }
    fn category(&self) -> &str { "network" }
    fn tags(&self) -> Vec<String> { vec!["network".into(), "interfaces".into()] }
    
    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let mut interfaces = Vec::new();
        let mut dir = tokio::fs::read_dir("/sys/class/net").await?;
        
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let state = tokio::fs::read_to_string(format!("/sys/class/net/{}/operstate", name))
                .await.unwrap_or_else(|_| "unknown".into()).trim().to_string();
            let mac = tokio::fs::read_to_string(format!("/sys/class/net/{}/address", name))
                .await.unwrap_or_else(|_| "unknown".into()).trim().to_string();
            
            interfaces.push(json!({"name": name, "state": state, "mac": mac}));
        }
        
        Ok(json!({"success": true, "interfaces": interfaces}))
    }
}
