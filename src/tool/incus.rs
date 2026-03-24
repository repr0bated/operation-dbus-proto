//! Incus Tools - CLI-based Incus/LXC management
//!
//! Provides tools for managing Incus instances, networks, storage,
//! images, profiles, and snapshots via the incus CLI.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use crate::error::Result;
use op_execution_tracker::{ExecutionRecord, ExecutionTiming};

use super::{Tool, ToolContext, ToolResult};

pub struct IncusTool {
    name: String,
    description: String,
}

impl IncusTool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }

    fn run_incus_command(&self, args: &[&str]) -> Result<Value> {
        let output = Command::new("incus").args(args).output().map_err(|e| {
            crate::error::OpDbusError::ToolExecution(format!(
                "Failed to execute incus command: {}",
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::OpDbusError::ToolExecution(format!(
                "incus command failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Try to parse as JSON
        if let Ok(parsed) = serde_json::from_str::<Value>(&stdout) {
            Ok(parsed)
        } else {
            // Return as text if not JSON
            Ok(json!({ "output": stdout.trim().to_string() }))
        }
    }

    fn run_incus_command_with_input(&self, args: &[&str], input: Option<&str>) -> Result<Value> {
        let mut cmd = Command::new("incus");
        cmd.args(args);

        if let Some(input_str) = input {
            cmd.arg(input_str);
        }

        let output = cmd.output().map_err(|e| {
            crate::error::OpDbusError::ToolExecution(format!(
                "Failed to execute incus command: {}",
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::OpDbusError::ToolExecution(format!(
                "incus command failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        if let Ok(parsed) = serde_json::from_str::<Value>(&stdout) {
            Ok(parsed)
        } else {
            Ok(json!({ "output": stdout.trim().to_string() }))
        }
    }
}

#[async_trait]
impl Tool for IncusTool {
    fn name(&self) -> &'static str {
        Box::leak(self.name.clone().into_boxed_str())
    }

    fn description(&self) -> &'static str {
        Box::leak(self.description.clone().into_boxed_str())
    }

    fn input_schema(&self) -> Value {
        match self.name.as_str() {
            // Instance tools
            "incus.instance.list" => json!({
                "type": "object",
                "properties": {
                    "remote": {"type": "string", "description": "Remote server (default: local)", "default": "local"}
                }
            }),
            "incus.instance.get" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Instance name"}
                },
                "required": ["name"]
            }),
            "incus.instance.start" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Instance name"},
                    "force": {"type": "boolean", "description": "Force start", "default": false}
                },
                "required": ["name"]
            }),
            "incus.instance.stop" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Instance name"},
                    "force": {"type": "boolean", "description": "Force stop", "default": false},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 30}
                },
                "required": ["name"]
            }),
            "incus.instance.restart" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Instance name"},
                    "force": {"type": "boolean", "description": "Force restart", "default": false},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 30}
                },
                "required": ["name"]
            }),
            "incus.instance.create" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Instance name"},
                    "image": {"type": "string", "description": "Image alias or fingerprint"},
                    "profile": {"type": "array", "items": {"type": "string"}, "description": "Profile names"},
                    "config": {"type": "object", "description": "Instance config keypairs"},
                    "device": {"type": "object", "description": "Device configuration"}
                },
                "required": ["name", "image"]
            }),
            "incus.instance.delete" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Instance name"},
                    "force": {"type": "boolean", "description": "Force delete", "default": false}
                },
                "required": ["name"]
            }),
            "incus.instance.info" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Instance name"}
                },
                "required": ["name"]
            }),

            // Network tools
            "incus.network.list" => json!({
                "type": "object",
                "properties": {
                    "remote": {"type": "string", "description": "Remote server (default: local)", "default": "local"}
                }
            }),
            "incus.network.get" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Network name"}
                },
                "required": ["name"]
            }),

            // Storage tools
            "incus.storage.list" => json!({
                "type": "object",
                "properties": {
                    "remote": {"type": "string", "description": "Remote server (default: local)", "default": "local"}
                }
            }),
            "incus.storage.get" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Storage pool name"}
                },
                "required": ["name"]
            }),

            // Image tools
            "incus.image.list" => json!({
                "type": "object",
                "properties": {
                    "remote": {"type": "string", "description": "Remote server (default: local)", "default": "local"}
                }
            }),
            "incus.image.get" => json!({
                "type": "object",
                "properties": {
                    "fingerprint": {"type": "string", "description": "Image fingerprint or alias"}
                },
                "required": ["fingerprint"]
            }),
            "incus.image.delete" => json!({
                "type": "object",
                "properties": {
                    "fingerprint": {"type": "string", "description": "Image fingerprint or alias"}
                },
                "required": ["fingerprint"]
            }),

            // Profile tools
            "incus.profile.list" => json!({
                "type": "object",
                "properties": {
                    "remote": {"type": "string", "description": "Remote server (default: local)", "default": "local"}
                }
            }),
            "incus.profile.get" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Profile name"}
                },
                "required": ["name"]
            }),

            // Snapshot tools
            "incus.snapshot.list" => json!({
                "type": "object",
                "properties": {
                    "instance": {"type": "string", "description": "Instance name"}
                },
                "required": ["instance"]
            }),
            "incus.snapshot.create" => json!({
                "type": "object",
                "properties": {
                    "instance": {"type": "string", "description": "Instance name"},
                    "name": {"type": "string", "description": "Snapshot name"},
                    "stateful": {"type": "boolean", "description": "Include runtime state", "default": false}
                },
                "required": ["instance", "name"]
            }),
            "incus.snapshot.delete" => json!({
                "type": "object",
                "properties": {
                    "instance": {"type": "string", "description": "Instance name"},
                    "name": {"type": "string", "description": "Snapshot name"}
                },
                "required": ["instance", "name"]
            }),
            "incus.snapshot.restore" => json!({
                "type": "object",
                "properties": {
                    "instance": {"type": "string", "description": "Instance name"},
                    "name": {"type": "string", "description": "Snapshot name"}
                },
                "required": ["instance", "name"]
            }),

            _ => json!({"type": "object", "properties": {}}),
        }
    }

    fn output_schema(&self) -> Value {
        json!({"type": "object"})
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let map_err = |e: crate::error::OpDbusError| e;

        let result_val = match self.name.as_str() {
            // ==================== Instance Tools ====================
            "incus.instance.list" => {
                let remote = input
                    .get("remote")
                    .and_then(|r| r.as_str())
                    .unwrap_or("local");
                self.run_incus_command(&["list", "--format", "json", remote])
                    .map(|list| json!({ "instances": list }))
                    .map_err(map_err)
            }

            "incus.instance.get" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                })?;
                self.run_incus_command(&["info", name, "--format", "json"])
                    .map(|info| json!({ "instance": name, "details": info }))
                    .map_err(map_err)
            }

            "incus.instance.start" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                })?;
                let force = input
                    .get("force")
                    .and_then(|f| f.as_bool())
                    .unwrap_or(false);

                let mut args = vec!["start"];
                if force {
                    args.push("--force");
                }
                args.push(name);
                args.push("--format");
                args.push("json");

                self.run_incus_command(&args)
                    .map(|_| json!({ "started": name }))
                    .map_err(map_err)
            }

            "incus.instance.stop" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                })?;
                let force = input
                    .get("force")
                    .and_then(|f| f.as_bool())
                    .unwrap_or(false);
                let timeout = input.get("timeout").and_then(|t| t.as_i64()).unwrap_or(30);

                let mut args = vec!["stop"];
                if force {
                    args.push("--force");
                }
                args.push(name);
                args.push("--timeout");
                args.push(&timeout.to_string());
                args.push("--format");
                args.push("json");

                self.run_incus_command(&args)
                    .map(|_| json!({ "stopped": name }))
                    .map_err(map_err)
            }

            "incus.instance.restart" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                })?;
                let force = input
                    .get("force")
                    .and_then(|f| f.as_bool())
                    .unwrap_or(false);
                let timeout = input.get("timeout").and_then(|t| t.as_i64()).unwrap_or(30);

                let mut args = vec!["restart"];
                if force {
                    args.push("--force");
                }
                args.push(name);
                args.push("--timeout");
                args.push(&timeout.to_string());
                args.push("--format");
                args.push("json");

                self.run_incus_command(&args)
                    .map(|_| json!({ "restarted": name }))
                    .map_err(map_err)
            }

            "incus.instance.create" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                })?;
                let image = input.get("image").and_then(|i| i.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing image".into())
                })?;

                let profiles = input
                    .get("profile")
                    .and_then(|p| p.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>());

                let config = input.get("config").and_then(|c| c.as_object());
                let device = input.get("device").and_then(|d| d.as_object());

                let mut args = vec!["launch", image, name, "--format", "json"];

                if let Some(profs) = profiles {
                    if !profs.is_empty() {
                        args.push("--profile");
                        for prof in &profs {
                            args.push(prof);
                        }
                    }
                }

                if let Some(cfg) = config {
                    for (k, v) in cfg {
                        if let Some(vstr) = v.as_str() {
                            args.push("--config");
                            args.push(&format!("{}={}", k, vstr));
                        }
                    }
                }

                if let Some(devs) = device {
                    for (k, v) in devs {
                        if let Some(vobj) = v.as_object() {
                            let mut dev_str = format!("{}:", k);
                            let props: Vec<String> = vobj
                                .iter()
                                .filter_map(|(key, val)| {
                                    val.as_str().map(|v| format!("{}={}", key, v))
                                })
                                .collect();
                            dev_str.push_str(&props.join(","));
                            args.push("--device");
                            args.push(&dev_str);
                        }
                    }
                }

                self.run_incus_command(&args)
                    .map(|_| json!({ "created": name, "image": image }))
                    .map_err(map_err)
            }

            "incus.instance.delete" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                })?;
                let force = input
                    .get("force")
                    .and_then(|f| f.as_bool())
                    .unwrap_or(false);

                let mut args = vec!["delete"];
                if force {
                    args.push("--force");
                }
                args.push(name);
                args.push("--format");
                args.push("json");

                self.run_incus_command(&args)
                    .map(|_| json!({ "deleted": name }))
                    .map_err(map_err)
            }

            "incus.instance.info" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                })?;

                let info = self.run_incus_command(&["info", name, "--format", "json"])?;

                // Parse the detailed info for state, memory, CPU, network, disk
                let state = info.get("status").cloned().unwrap_or(json!("Unknown"));
                let memory = info.get("memory").cloned().unwrap_or(json!({}));
                let cpu = info.get("cpu").cloned().unwrap_or(json!({}));
                let networks = info.get("networks").cloned().unwrap_or(json!({}));
                let devices = info.get("devices").cloned().unwrap_or(json!({}));

                Ok(json!({
                    "instance": name,
                    "status": state,
                    "memory": memory,
                    "cpu": cpu,
                    "networks": networks,
                    "devices": devices,
                    "raw_info": info
                }))
            }

            // ==================== Network Tools ====================
            "incus.network.list" => {
                let remote = input
                    .get("remote")
                    .and_then(|r| r.as_str())
                    .unwrap_or("local");
                self.run_incus_command(&["network", "list", "--format", "json", remote])
                    .map(|list| json!({ "networks": list }))
                    .map_err(map_err)
            }

            "incus.network.get" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing network name".into())
                })?;
                self.run_incus_command(&["network", "info", name, "--format", "json"])
                    .map(|info| json!({ "network": name, "details": info }))
                    .map_err(map_err)
            }

            // ==================== Storage Tools ====================
            "incus.storage.list" => {
                let remote = input
                    .get("remote")
                    .and_then(|r| r.as_str())
                    .unwrap_or("local");
                self.run_incus_command(&["storage", "list", "--format", "json", remote])
                    .map(|list| json!({ "storage_pools": list }))
                    .map_err(map_err)
            }

            "incus.storage.get" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing storage pool name".into())
                })?;
                self.run_incus_command(&["storage", "info", name, "--format", "json"])
                    .map(|info| json!({ "storage_pool": name, "details": info }))
                    .map_err(map_err)
            }

            // ==================== Image Tools ====================
            "incus.image.list" => {
                let remote = input
                    .get("remote")
                    .and_then(|r| r.as_str())
                    .unwrap_or("local");
                self.run_incus_command(&["image", "list", "--format", "json", remote])
                    .map(|list| json!({ "images": list }))
                    .map_err(map_err)
            }

            "incus.image.get" => {
                let fingerprint = input
                    .get("fingerprint")
                    .and_then(|f| f.as_str())
                    .ok_or_else(|| {
                        crate::error::OpDbusError::ToolExecution("Missing image fingerprint".into())
                    })?;
                self.run_incus_command(&["image", "info", fingerprint, "--format", "json"])
                    .map(|info| json!({ "image": fingerprint, "details": info }))
                    .map_err(map_err)
            }

            "incus.image.delete" => {
                let fingerprint = input
                    .get("fingerprint")
                    .and_then(|f| f.as_str())
                    .ok_or_else(|| {
                        crate::error::OpDbusError::ToolExecution("Missing image fingerprint".into())
                    })?;
                self.run_incus_command(&["image", "delete", fingerprint, "--format", "json"])
                    .map(|_| json!({ "deleted": fingerprint }))
                    .map_err(map_err)
            }

            // ==================== Profile Tools ====================
            "incus.profile.list" => {
                let remote = input
                    .get("remote")
                    .and_then(|r| r.as_str())
                    .unwrap_or("local");
                self.run_incus_command(&["profile", "list", "--format", "json", remote])
                    .map(|list| json!({ "profiles": list }))
                    .map_err(map_err)
            }

            "incus.profile.get" => {
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing profile name".into())
                })?;
                self.run_incus_command(&["profile", "show", name, "--format", "json"])
                    .map(|info| json!({ "profile": name, "details": info }))
                    .map_err(map_err)
            }

            // ==================== Snapshot Tools ====================
            "incus.snapshot.list" => {
                let instance = input
                    .get("instance")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| {
                        crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                    })?;
                self.run_incus_command(&["snapshot", "list", instance, "--format", "json"])
                    .map(|list| json!({ "instance": instance, "snapshots": list }))
                    .map_err(map_err)
            }

            "incus.snapshot.create" => {
                let instance = input
                    .get("instance")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| {
                        crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                    })?;
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing snapshot name".into())
                })?;
                let stateful = input
                    .get("stateful")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);

                let mut args = vec!["snapshot", "create"];
                if stateful {
                    args.push("--stateful");
                }
                args.push(instance);
                args.push(name);
                args.push("--format");
                args.push("json");

                self.run_incus_command(&args)
                    .map(|_| json!({ "created": name, "instance": instance }))
                    .map_err(map_err)
            }

            "incus.snapshot.delete" => {
                let instance = input
                    .get("instance")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| {
                        crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                    })?;
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing snapshot name".into())
                })?;

                let snapshot_name = format!("{}/{}", instance, name);
                self.run_incus_command(&["snapshot", "delete", &snapshot_name, "--format", "json"])
                    .map(|_| json!({ "deleted": name, "instance": instance }))
                    .map_err(map_err)
            }

            "incus.snapshot.restore" => {
                let instance = input
                    .get("instance")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| {
                        crate::error::OpDbusError::ToolExecution("Missing instance name".into())
                    })?;
                let name = input.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                    crate::error::OpDbusError::ToolExecution("Missing snapshot name".into())
                })?;

                let snapshot_name = format!("{}/{}", instance, name);
                self.run_incus_command(&["snapshot", "restore", &snapshot_name, "--format", "json"])
                    .map(|_| json!({ "restored": name, "instance": instance }))
                    .map_err(map_err)
            }

            _ => Ok(json!({"error": "unknown tool", "name": self.name})),
        }?;

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(result_val.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output: result_val,
            execution_record: record,
        })
    }
}
