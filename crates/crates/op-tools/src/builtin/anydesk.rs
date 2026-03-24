//! AnyDesk remote desktop tools.
//!
//! These tools provide management and monitoring capabilities for AnyDesk,
//! including getting the AnyDesk ID, checking service status, and controlling
//! the AnyDesk service.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::Tool;

/// Register AnyDesk tools with the tool registry
pub async fn register_anydesk_tools(registry: &crate::ToolRegistry) -> Result<()> {
    registry
        .register_tool(std::sync::Arc::new(AnyDeskGetIdTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskGetStatusTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskServiceControlTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskGetConnectionsTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskCheckX11DisplayTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskDiagnoseX11AccessTool))
        .await?;

    Ok(())
}

/// Tool to get the AnyDesk ID
struct AnyDeskGetIdTool;

#[async_trait]
impl Tool for AnyDeskGetIdTool {
    fn name(&self) -> &str {
        "anydesk_get_id"
    }

    fn description(&self) -> &str {
        "Get the AnyDesk ID for remote connections"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        // Try to get AnyDesk ID from various sources
        // First check if AnyDesk is running and can provide the ID
        match get_anydesk_id() {
            Ok(id) => Ok(json!({
                "success": true,
                "anydesk_id": id
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Could not retrieve AnyDesk ID: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
        ]
    }
}

/// Tool to get AnyDesk service status
struct AnyDeskGetStatusTool;

#[async_trait]
impl Tool for AnyDeskGetStatusTool {
    fn name(&self) -> &str {
        "anydesk_get_status"
    }

    fn description(&self) -> &str {
        "Get the current status of the AnyDesk service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match get_anydesk_status() {
            Ok(status) => Ok(json!({
                "success": true,
                "status": status
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Could not get AnyDesk status: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "status".to_string(),
        ]
    }
}

/// Tool to control AnyDesk service (start/stop/restart)
struct AnyDeskServiceControlTool;

#[async_trait]
impl Tool for AnyDeskServiceControlTool {
    fn name(&self) -> &str {
        "anydesk_service_control"
    }

    fn description(&self) -> &str {
        "Control the AnyDesk service (start, stop, restart)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "stop", "restart"],
                    "description": "Action to perform on the AnyDesk service"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: action"))?;

        match control_anydesk_service(action) {
            Ok(result) => Ok(json!({
                "success": true,
                "action": action,
                "result": result
            })),
            Err(e) => Ok(json!({
                "success": false,
                "action": action,
                "error": format!("Failed to {} AnyDesk service: {}", action, e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "control".to_string(),
        ]
    }
}

/// Tool to get current AnyDesk connections
struct AnyDeskGetConnectionsTool;

/// Tool to check X11 display environment for AnyDesk
struct AnyDeskCheckX11DisplayTool;

/// Tool to diagnose X11 access issues for AnyDesk
struct AnyDeskDiagnoseX11AccessTool;

#[async_trait]
impl Tool for AnyDeskGetConnectionsTool {
    fn name(&self) -> &str {
        "anydesk_get_connections"
    }

    fn description(&self) -> &str {
        "Get information about current AnyDesk remote connections"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match get_anydesk_connections() {
            Ok(connections) => Ok(json!({
                "success": true,
                "connections": connections
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Could not get AnyDesk connections: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "connections".to_string(),
        ]
    }
}

#[async_trait]
impl Tool for AnyDeskCheckX11DisplayTool {
    fn name(&self) -> &str {
        "anydesk_check_x11_display"
    }

    fn description(&self) -> &str {
        "Check X11 display environment and configuration for AnyDesk screen sharing"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match check_x11_display_environment() {
            Ok(result) => Ok(json!({
                "success": true,
                "x11_environment": result
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Failed to check X11 display environment: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "x11".to_string(),
            "display".to_string(),
        ]
    }
}

#[async_trait]
impl Tool for AnyDeskDiagnoseX11AccessTool {
    fn name(&self) -> &str {
        "anydesk_diagnose_x11_access"
    }

    fn description(&self) -> &str {
        "Diagnose X11 access issues and provide fixes for AnyDesk screen sharing"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match diagnose_x11_access_issues() {
            Ok(result) => Ok(json!({
                "success": true,
                "diagnosis": result
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Failed to diagnose X11 access: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "x11".to_string(),
            "diagnostics".to_string(),
        ]
    }
}

/// Helper function to get AnyDesk ID
fn get_anydesk_id() -> Result<String> {
    // Try to get ID from AnyDesk configuration or command
    // First check if we can run anydesk command to get ID

    // Check for AnyDesk ID in various locations
    let config_paths = vec![
        "/etc/anydesk/anydesk.conf",
        "/home/jeremy/.anydesk/anydesk.conf",
        "/home/jeremy/.anydesk/user.conf",
    ];

    for path in config_paths {
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                // Look for ID in config file
                for line in content.lines() {
                    if line.contains("ad.anynet.id") || line.contains("id=") {
                        // Parse the ID from the line
                        if let Some(id_part) = line.split('=').nth(1) {
                            let id = id_part.trim().trim_matches('"');
                            if !id.is_empty() && id != "0" {
                                return Ok(id.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Try to run anydesk command if available
    match Command::new("anydesk").arg("--get-id").output() {
        Ok(output) if output.status.success() => {
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !id.is_empty() {
                return Ok(id);
            }
        }
        _ => {}
    }

    // Fallback: check systemd service and extract from logs or process
    match Command::new("systemctl")
        .args(&["show", "anydesk", "--property=MainPID"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let pid_str = String::from_utf8_lossy(&output.stdout);
            if let Some(pid_line) = pid_str.lines().next() {
                if let Some(pid) = pid_line.strip_prefix("MainPID=") {
                    if let Ok(pid_num) = pid.parse::<u32>() {
                        // Could potentially inspect process environment or memory
                        // For now, return a placeholder indicating AnyDesk is running
                        return Ok(format!("running_pid_{}", pid_num));
                    }
                }
            }
        }
        _ => {}
    }

    Err(anyhow!(
        "Could not determine AnyDesk ID. AnyDesk may not be properly configured or running."
    ))
}

/// Helper function to get AnyDesk service status
fn get_anydesk_status() -> Result<Value> {
    let mut status = json!({
        "service_running": false,
        "version": null,
        "connections": []
    });

    // Check systemd service status
    match Command::new("systemctl")
        .args(&["is-active", "anydesk"])
        .output()
    {
        Ok(output) => {
            let state_str = String::from_utf8_lossy(&output.stdout);
            let state = state_str.trim();
            status["service_running"] = json!(state == "active");
        }
        _ => {}
    }

    // Check if anydesk process is running
    match Command::new("pgrep").arg("anydesk").output() {
        Ok(output) if output.status.success() => {
            let pids: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
            status["process_pids"] = json!(pids);
        }
        _ => {}
    }

    // Try to get version
    match Command::new("anydesk").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            status["version"] = json!(version);
        }
        _ => {}
    }

    Ok(status)
}

/// Helper function to control AnyDesk service
fn control_anydesk_service(action: &str) -> Result<String> {
    let systemctl_action = match action {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        _ => return Err(anyhow!("Invalid action: {}", action)),
    };

    let output = Command::new("sudo")
        .args(&["systemctl", systemctl_action, "anydesk"])
        .output()?;

    if output.status.success() {
        Ok(format!("AnyDesk service {} successful", action))
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("Failed to {} AnyDesk service: {}", action, error))
    }
}

/// Helper function to get AnyDesk connections
fn get_anydesk_connections() -> Result<Vec<Value>> {
    // AnyDesk doesn't provide a direct way to list connections
    // This is a placeholder for future implementation
    // In a real implementation, this might parse logs or use AnyDesk's API

    let connections = Vec::new();

    // Check for any active connections by looking at network connections
    // or AnyDesk process status

    match Command::new("netstat").args(&["-tuln"]).output() {
        Ok(output) if output.status.success() => {
            let netstat_output = String::from_utf8_lossy(&output.stdout);
            // Look for AnyDesk-related ports (typically 7070, 6568, etc.)
            let anydesk_ports = ["7070", "6568", "80", "443"];
            for line in netstat_output.lines() {
                for port in &anydesk_ports {
                    if line.contains(&format!(":{} ", port))
                        || line.contains(&format!(":{}\n", port))
                    {
                        // Found a potential AnyDesk connection
                        // This is a simplified detection
                    }
                }
            }
        }
        _ => {}
    }

    Ok(connections)
}

/// Helper function to check X11 display environment
fn check_x11_display_environment() -> Result<Value> {
    let mut result = json!({
        "display_available": false,
        "display_variable": null,
        "xauthority_available": false,
        "xauthority_path": null,
        "anydesk_service_environment": {},
        "x11_server_running": false,
        "x11_auth_configured": false
    });

    // Check DISPLAY environment variable
    if let Ok(display) = std::env::var("DISPLAY") {
        result["display_variable"] = json!(display);
    }

    // Check XAUTHORITY environment variable
    if let Ok(xauthority) = std::env::var("XAUTHORITY") {
        result["xauthority_path"] = json!(xauthority);
        result["xauthority_available"] = json!(Path::new(&xauthority).exists());
    }

    // Check if X11 server is running by testing display access
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xdpyinfo").env("DISPLAY", &display).output() {
            Ok(output) if output.status.success() => {
                result["x11_server_running"] = json!(true);
                result["display_available"] = json!(true);
            }
            _ => {}
        }
    }

    // Check AnyDesk service environment
    match Command::new("systemctl")
        .args(&["show", "anydesk", "--property=Environment"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let env_str = String::from_utf8_lossy(&output.stdout);
            let env_vars: std::collections::HashMap<String, String> = env_str
                .strip_prefix("Environment=")
                .unwrap_or("")
                .split_whitespace()
                .filter_map(|kv| {
                    kv.split_once('=')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect();
            result["anydesk_service_environment"] = json!(env_vars);
        }
        _ => {}
    }

    // Check X11 authentication
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xauth").args(&["list", &display]).output() {
            Ok(output) if output.status.success() => {
                let auth_output = String::from_utf8_lossy(&output.stdout);
                if !auth_output.trim().is_empty() {
                    result["x11_auth_configured"] = json!(true);
                }
            }
            _ => {}
        }
    }

    Ok(result)
}

/// Helper function to diagnose X11 access issues
fn diagnose_x11_access_issues() -> Result<Value> {
    let mut issues = Vec::new();
    let mut recommendations = Vec::new();
    let mut fix_commands = Vec::new();

    // Check if AnyDesk service is running
    match Command::new("systemctl")
        .args(&["is-active", "anydesk"])
        .output()
    {
        Ok(output) => {
            let state_str = String::from_utf8_lossy(&output.stdout);
            let state = state_str.trim();
            if state != "active" {
                issues.push("AnyDesk service is not running".to_string());
                recommendations
                    .push("Start AnyDesk service with: sudo systemctl start anydesk".to_string());
                fix_commands.push("sudo systemctl start anydesk".to_string());
            }
        }
        _ => {
            issues.push("Cannot determine AnyDesk service status".to_string());
        }
    }

    // Check DISPLAY environment for AnyDesk service
    match Command::new("systemctl")
        .args(&["show", "anydesk", "--property=Environment"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let env_str = String::from_utf8_lossy(&output.stdout);
            let has_display = env_str.contains("DISPLAY=");
            let has_xauthority = env_str.contains("XAUTHORITY=");

            if !has_display {
                issues.push("AnyDesk service missing DISPLAY environment variable".to_string());
                recommendations.push("Add DISPLAY=:99 to AnyDesk service environment".to_string());
                fix_commands.push("sudo sed -i '/^Environment=/a Environment=DISPLAY=:99' /etc/systemd/system/anydesk.service && sudo systemctl daemon-reload && sudo systemctl restart anydesk".to_string());
            }

            if !has_xauthority {
                issues.push("AnyDesk service missing XAUTHORITY environment variable".to_string());
                recommendations.push(
                    "Add XAUTHORITY=/root/.Xauthority to AnyDesk service environment".to_string(),
                );
                fix_commands.push("sudo sed -i '/^Environment=/a Environment=XAUTHORITY=/root/.Xauthority' /etc/systemd/system/anydesk.service && sudo systemctl daemon-reload && sudo systemctl restart anydesk".to_string());
            }
        }
        _ => {
            issues.push("Cannot check AnyDesk service environment".to_string());
        }
    }

    // Check X11 server accessibility
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xdpyinfo").env("DISPLAY", &display).output() {
            Ok(output) if output.status.success() => {
                // X11 server is accessible
            }
            _ => {
                issues.push(format!("Cannot access X11 display {}", display));
                recommendations.push(
                    "Ensure Xvfb or X server is running on the specified display".to_string(),
                );
            }
        }
    } else {
        issues.push("DISPLAY environment variable not set".to_string());
        recommendations.push("Set DISPLAY=:99 for headless X11 server".to_string());
    }

    // Check X11 authentication
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xauth").args(&["list", &display]).output() {
            Ok(output) if output.status.success() => {
                let auth_output = String::from_utf8_lossy(&output.stdout);
                if auth_output.trim().is_empty() {
                    issues.push(format!(
                        "No X11 authentication configured for display {}",
                        display
                    ));
                    recommendations.push(
                        "Generate X11 authentication cookie with: xauth generate :99 . trusted"
                            .to_string(),
                    );
                    fix_commands.push("xauth generate :99 . trusted".to_string());
                }
            }
            _ => {
                issues.push("Cannot check X11 authentication".to_string());
            }
        }
    }

    // Check if Xauthority file exists for root
    if !Path::new("/root/.Xauthority").exists() {
        issues.push("Xauthority file missing for root user".to_string());
        recommendations.push(
            "Copy user Xauthority to root: sudo cp /home/user/.Xauthority /root/.Xauthority"
                .to_string(),
        );
        fix_commands.push("sudo cp /home/jeremy/.Xauthority /root/.Xauthority && sudo chown root:root /root/.Xauthority && sudo chmod 600 /root/.Xauthority".to_string());
    }

    let diagnosis = json!({
        "issues": issues,
        "recommendations": recommendations,
        "can_fix_automatically": !fix_commands.is_empty(),
        "fix_commands": fix_commands
    });

    Ok(diagnosis)
}
