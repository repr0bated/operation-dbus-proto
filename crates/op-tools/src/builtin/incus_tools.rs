//! Incus Container Management Tools
//!
//! These tools expose Incus instance operations (containers and VMs) to the
//! LLM chat system using the `incus` CLI. Mirrors the LXC tools pattern but
//! targets the Incus container manager instead of Proxmox API.

use crate::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::process::Command;

// ---------------------------------------------------------------------------
// Helper: run an incus command and return (stdout, stderr, success)
// ---------------------------------------------------------------------------

async fn run_incus(args: &[&str]) -> Result<(String, String, bool)> {
    let output = Command::new("incus")
        .args(args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to execute incus command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((stdout, stderr, output.status.success()))
}

// ---------------------------------------------------------------------------
// 1. IncusCheckAvailableTool
// ---------------------------------------------------------------------------

/// Tool to check if incusd is running and available
pub struct IncusCheckAvailableTool;

#[async_trait]
impl Tool for IncusCheckAvailableTool {
    fn name(&self) -> &str {
        "incus_check_available"
    }

    fn description(&self) -> &str {
        "Check if incusd is running and available. Returns version info if connected. Use this first to verify Incus operations will work."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["incus".into(), "check".into(), "status".into()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match run_incus(&["version"]).await {
            Ok((stdout, _stderr, true)) => {
                let version = stdout.trim().to_string();
                Ok(json!({
                    "available": true,
                    "version": version,
                    "message": format!("Incus {} is available", version)
                }))
            }
            Ok((_stdout, stderr, false)) => Ok(json!({
                "available": false,
                "error": stderr.trim(),
                "message": "Incus is not available or incusd is not running"
            })),
            Err(e) => Ok(json!({
                "available": false,
                "error": e.to_string(),
                "message": "Incus CLI is not installed or not in PATH"
            })),
        }
    }
}

// ---------------------------------------------------------------------------
// 2. IncusListInstancesTool
// ---------------------------------------------------------------------------

/// Tool to list all Incus instances (containers and VMs) with status
pub struct IncusListInstancesTool;

#[async_trait]
impl Tool for IncusListInstancesTool {
    fn name(&self) -> &str {
        "incus_list_instances"
    }

    fn description(&self) -> &str {
        "List all Incus instances (containers and VMs) with status. Optionally filter by type ('container' or 'virtual-machine')."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "description": "Filter by instance type: 'container' or 'virtual-machine'",
                    "enum": ["container", "virtual-machine"]
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["incus".into(), "containers".into(), "list".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let type_filter = input.get("type").and_then(|v| v.as_str());

        let (stdout, stderr, success) = run_incus(&["list", "--format=json"]).await?;
        if !success {
            return Err(anyhow::anyhow!("incus list failed: {}", stderr.trim()));
        }

        // Parse the JSON array output from incus
        let mut json_bytes = stdout.into_bytes();
        let instances: Vec<Value> = simd_json::from_slice(&mut json_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse incus list JSON: {}", e))?;

        // Apply type filter if provided
        let filtered: Vec<&Value> = if let Some(filter) = type_filter {
            instances
                .iter()
                .filter(|inst| {
                    inst.get("type")
                        .and_then(|t| t.as_str())
                        .map(|t| t == filter)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            instances.iter().collect()
        };

        let count = filtered.len();
        Ok(json!({
            "instances": filtered,
            "count": count,
            "filter": type_filter
        }))
    }
}

// ---------------------------------------------------------------------------
// 3. IncusGetInstanceTool
// ---------------------------------------------------------------------------

/// Tool to get detailed info about a specific Incus instance
pub struct IncusGetInstanceTool;

#[async_trait]
impl Tool for IncusGetInstanceTool {
    fn name(&self) -> &str {
        "incus_get_instance"
    }

    fn description(&self) -> &str {
        "Get detailed info about a specific Incus instance including its full expanded configuration."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["incus".into(), "containers".into(), "info".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let (stdout, stderr, success) =
            run_incus(&["config", "show", name, "--expanded"]).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus config show {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "name": name,
            "config": stdout.trim(),
            "message": format!("Configuration for instance '{}'", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 4. IncusLaunchInstanceTool
// ---------------------------------------------------------------------------

/// Tool to launch a new Incus instance from an image (creates and starts it)
pub struct IncusLaunchInstanceTool;

#[async_trait]
impl Tool for IncusLaunchInstanceTool {
    fn name(&self) -> &str {
        "incus_launch_instance"
    }

    fn description(&self) -> &str {
        "Launch a new Incus instance from an image (creates and starts it). For example, 'images:debian/13' or 'images:ubuntu/24.04'."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "image": {
                    "type": "string",
                    "description": "Image to launch from (e.g. 'images:debian/13', 'images:ubuntu/24.04')"
                },
                "name": {
                    "type": "string",
                    "description": "Name for the new instance"
                },
                "type": {
                    "type": "string",
                    "description": "Instance type: 'container' (default) or 'virtual-machine'",
                    "enum": ["container", "virtual-machine"]
                },
                "profile": {
                    "type": "string",
                    "description": "Profile to apply to the instance"
                }
            },
            "required": ["image", "name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "create".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let image = input
            .get("image")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: image"))?;

        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let instance_type = input.get("type").and_then(|v| v.as_str());
        let profile = input.get("profile").and_then(|v| v.as_str());

        // Build command arguments
        let mut args: Vec<&str> = vec!["launch", image, name];

        if instance_type == Some("virtual-machine") {
            args.push("--vm");
        }

        // We need to own the profile string for the borrow checker
        let profile_flag;
        if let Some(p) = profile {
            args.push("--profile");
            profile_flag = p.to_string();
            args.push(&profile_flag);
        }

        let (stdout, stderr, success) = run_incus(&args).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus launch failed: {}",
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "image": image,
            "type": instance_type.unwrap_or("container"),
            "profile": profile,
            "output": stdout.trim(),
            "message": format!("Instance '{}' launched successfully from {}", name, image)
        }))
    }
}

// ---------------------------------------------------------------------------
// 5. IncusStartInstanceTool
// ---------------------------------------------------------------------------

/// Tool to start a stopped Incus instance
pub struct IncusStartInstanceTool;

#[async_trait]
impl Tool for IncusStartInstanceTool {
    fn name(&self) -> &str {
        "incus_start_instance"
    }

    fn description(&self) -> &str {
        "Start a stopped Incus instance."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to start"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "start".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let (stdout, stderr, success) = run_incus(&["start", name]).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus start {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "output": stdout.trim(),
            "message": format!("Instance '{}' started successfully", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 6. IncusStopInstanceTool
// ---------------------------------------------------------------------------

/// Tool to stop a running Incus instance
pub struct IncusStopInstanceTool;

#[async_trait]
impl Tool for IncusStopInstanceTool {
    fn name(&self) -> &str {
        "incus_stop_instance"
    }

    fn description(&self) -> &str {
        "Stop a running Incus instance. Use force=true for immediate stop."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to stop"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force stop immediately (default: false for graceful shutdown)"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "stop".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let force = input
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let args: Vec<&str> = if force {
            vec!["stop", name, "--force"]
        } else {
            vec!["stop", name]
        };

        let (stdout, stderr, success) = run_incus(&args).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus stop {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "force": force,
            "output": stdout.trim(),
            "message": format!("Instance '{}' stopped successfully", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 7. IncusDeleteInstanceTool
// ---------------------------------------------------------------------------

/// Tool to delete an Incus instance permanently
pub struct IncusDeleteInstanceTool;

#[async_trait]
impl Tool for IncusDeleteInstanceTool {
    fn name(&self) -> &str {
        "incus_delete_instance"
    }

    fn description(&self) -> &str {
        "Delete an Incus instance permanently. WARNING: Destroys the instance and its data. Use force=true to delete a running instance."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to delete"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force delete even if running (default: false)"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "delete".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let force = input
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let args: Vec<&str> = if force {
            vec!["delete", name, "--force"]
        } else {
            vec!["delete", name]
        };

        let (stdout, stderr, success) = run_incus(&args).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus delete {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "force": force,
            "output": stdout.trim(),
            "message": format!("Instance '{}' deleted successfully", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 8. IncusExecTool
// ---------------------------------------------------------------------------

/// Tool to execute a command inside an Incus instance
pub struct IncusExecTool;

#[async_trait]
impl Tool for IncusExecTool {
    fn name(&self) -> &str {
        "incus_exec"
    }

    fn description(&self) -> &str {
        "Execute a command inside an Incus instance. The command can be a single string (run via sh -c) or an array of strings (run directly)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to execute the command in"
                },
                "command": {
                    "description": "Command to execute. String (run via 'sh -c') or array of strings (run directly).",
                    "oneOf": [
                        { "type": "string" },
                        { "type": "array", "items": { "type": "string" } }
                    ]
                }
            },
            "required": ["name", "command"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "exec".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let command_value = input
            .get("command")
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: command"))?;

        // Build the command parts depending on whether command is a string or array
        let cmd_parts: Vec<String> = if let Some(cmd_str) = command_value.as_str() {
            // String command: wrap in sh -c
            vec!["sh".into(), "-c".into(), cmd_str.to_string()]
        } else if let Some(cmd_array) = command_value.as_array() {
            // Array command: use directly
            cmd_array
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            return Err(anyhow::anyhow!(
                "command must be a string or array of strings"
            ));
        };

        if cmd_parts.is_empty() {
            return Err(anyhow::anyhow!("command must not be empty"));
        }

        // Build: incus exec <name> -- <cmd_parts...>
        let mut args: Vec<String> = vec!["exec".into(), name.to_string(), "--".into()];
        args.extend(cmd_parts);

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr, success) = run_incus(&args_refs).await?;

        let exit_code = if success { 0 } else { 1 };

        Ok(json!({
            "name": name,
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "success": success,
            "message": if success {
                format!("Command executed successfully in '{}'", name)
            } else {
                format!("Command failed in '{}': {}", name, stderr.trim())
            }
        }))
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all Incus tools with the tool registry
pub async fn register_incus_tools(registry: &ToolRegistry) -> Result<()> {
    registry
        .register_tool(Arc::new(IncusCheckAvailableTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusListInstancesTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusGetInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusLaunchInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusStartInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusStopInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusDeleteInstanceTool))
        .await?;
    registry.register_tool(Arc::new(IncusExecTool)).await?;
    tracing::info!("Registered 8 Incus container tools");
    Ok(())
}

/// Create all Incus tools as a vector
pub fn create_incus_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(IncusCheckAvailableTool),
        Arc::new(IncusListInstancesTool),
        Arc::new(IncusGetInstanceTool),
        Arc::new(IncusLaunchInstanceTool),
        Arc::new(IncusStartInstanceTool),
        Arc::new(IncusStopInstanceTool),
        Arc::new(IncusDeleteInstanceTool),
        Arc::new(IncusExecTool),
    ]
}
