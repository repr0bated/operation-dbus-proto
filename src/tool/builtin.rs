//! Built-in system tools.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::error::Result;
use op_execution_tracker::{ExecutionRecord, ExecutionTiming};

use super::{Tool, ToolContext, ToolResult};

// ============================================================================
// System Tools
// ============================================================================

/// Echo tool for testing
pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "system.echo"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"}
            },
            "required": ["message"]
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "echoed": {"type": "string"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Echo back the input message"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let output = serde_json::json!({
            "echoed": message
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// No-op tool for testing work stack mechanics
pub struct NoopTool;

#[async_trait]
impl Tool for NoopTool {
    fn name(&self) -> &'static str {
        "system.noop"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    fn description(&self) -> &'static str {
        "No-operation tool for testing"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();
        let output = serde_json::json!({"status": "noop"});
        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

// ============================================================================
// Network Tools
// ============================================================================

/// List network interfaces
pub struct NetworkListInterfacesTool;

#[async_trait]
impl Tool for NetworkListInterfacesTool {
    fn name(&self) -> &'static str {
        "network.interface.list"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filter": {"type": "string"}
            }
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "interfaces": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "mac": {"type": "string"},
                            "state": {"type": "string"}
                        }
                    }
                }
            }
        })
    }

    fn description(&self) -> &'static str {
        "List network interfaces"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        // Read from sysfs
        let mut interfaces = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                
                let mac = std::fs::read_to_string(entry.path().join("address"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                
                let state = std::fs::read_to_string(entry.path().join("operstate"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                interfaces.push(serde_json::json!({
                    "name": name,
                    "mac": mac,
                    "state": state
                }));
            }
        }

        let output = serde_json::json!({
            "interfaces": interfaces,
            "count": interfaces.len()
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// Get network interface details
pub struct NetworkGetInterfaceTool;

#[async_trait]
impl Tool for NetworkGetInterfaceTool {
    fn name(&self) -> &'static str {
        "network.interface.get"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "required": ["name"]
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "mac": {"type": "string"},
                "mtu": {"type": "integer"},
                "state": {"type": "string"},
                "statistics": {"type": "object"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Get detailed network interface information"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let name = input.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let path = std::path::Path::new("/sys/class/net").join(name);
        
        let output = if path.exists() {
            let mac = std::fs::read_to_string(path.join("address"))
                .unwrap_or_default().trim().to_string();
            let mtu: u32 = std::fs::read_to_string(path.join("mtu"))
                .unwrap_or_default().trim().parse().unwrap_or(0);
            let state = std::fs::read_to_string(path.join("operstate"))
                .unwrap_or_default().trim().to_string();

            // Read statistics
            let stats_path = path.join("statistics");
            let rx_bytes: u64 = std::fs::read_to_string(stats_path.join("rx_bytes"))
                .unwrap_or_default().trim().parse().unwrap_or(0);
            let tx_bytes: u64 = std::fs::read_to_string(stats_path.join("tx_bytes"))
                .unwrap_or_default().trim().parse().unwrap_or(0);

            serde_json::json!({
                "name": name,
                "mac": mac,
                "mtu": mtu,
                "state": state,
                "statistics": {
                    "rx_bytes": rx_bytes,
                    "tx_bytes": tx_bytes
                },
                "found": true
            })
        } else {
            serde_json::json!({
                "name": name,
                "found": false,
                "error": "Interface not found"
            })
        };

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

// ============================================================================
// Storage Tools
// ============================================================================

/// List storage volumes
pub struct StorageListVolumesTool;

#[async_trait]
impl Tool for StorageListVolumesTool {
    fn name(&self) -> &'static str {
        "storage.volume.list"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "volumes": {"type": "array"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "List storage volumes"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let mut volumes = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/sys/block") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("loop") || name.starts_with("ram") {
                    continue;
                }

                let size: u64 = std::fs::read_to_string(entry.path().join("size"))
                    .unwrap_or_default().trim().parse().unwrap_or(0) * 512;

                volumes.push(serde_json::json!({
                    "name": name,
                    "size_bytes": size
                }));
            }
        }

        let output = serde_json::json!({
            "volumes": volumes,
            "count": volumes.len()
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

// ============================================================================
// Service Tools
// ============================================================================

/// List systemd services
pub struct ServiceListTool;

#[async_trait]
impl Tool for ServiceListTool {
    fn name(&self) -> &'static str {
        "service.list"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "state": {"type": "string", "enum": ["running", "stopped", "all"]}
            }
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "services": {"type": "array"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "List systemd services"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        // Use systemctl to list services
        let result = tokio::process::Command::new("systemctl")
            .args(["list-units", "--type=service", "--no-pager", "--plain", "--no-legend"])
            .output()
            .await;

        let services: Vec<serde_json::Value> = match result {
            Ok(output) => {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 4 {
                            Some(serde_json::json!({
                                "name": parts[0].trim_end_matches(".service"),
                                "load": parts[1],
                                "active": parts[2],
                                "sub": parts[3]
                            }))
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            Err(_) => vec![],
        };

        let output = serde_json::json!({
            "services": services,
            "count": services.len()
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// Get service status
pub struct ServiceStatusTool;

#[async_trait]
impl Tool for ServiceStatusTool {
    fn name(&self) -> &'static str {
        "service.status"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "required": ["name"]
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "active": {"type": "boolean"},
                "enabled": {"type": "boolean"},
                "description": {"type": "string"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Get systemd service status"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let name = input.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let service_name = if name.ends_with(".service") {
            name.to_string()
        } else {
            format!("{}.service", name)
        };

        // Check if active
        let active = tokio::process::Command::new("systemctl")
            .args(["is-active", &service_name])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Check if enabled
        let enabled = tokio::process::Command::new("systemctl")
            .args(["is-enabled", &service_name])
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
            .unwrap_or(false);

        let output = serde_json::json!({
            "name": name,
            "active": active,
            "enabled": enabled
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

// ============================================================================
// Inspector Tools (One-shot only)
// ============================================================================

/// Discover system state (Inspector Gadget)
pub struct InspectorDiscoverTool;

#[async_trait]
impl Tool for InspectorDiscoverTool {
    fn name(&self) -> &'static str {
        "inspector.discover"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subsystems": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Subsystems to discover (network, storage, services)"
                }
            }
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "discovery_id": {"type": "string"},
                "objects": {"type": "object"},
                "stats": {"type": "object"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "One-shot system discovery (Inspector Gadget)"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        // This would delegate to InspectorGadget in production
        let output = serde_json::json!({
            "discovery_id": uuid::Uuid::new_v4().to_string(),
            "message": "Discovery would run here",
            "note": "ONE-SHOT ONLY - This tool does not run continuously"
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

// ============================================================================
// User Interaction Tools
// ============================================================================

/// Respond to user
pub struct UserRespondTool;

#[async_trait]
impl Tool for UserRespondTool {
    fn name(&self) -> &'static str {
        "user.respond"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"},
                "format": {"type": "string", "enum": ["text", "json", "markdown"], "default": "text"}
            },
            "required": ["message"]
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "delivered": {"type": "boolean"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Send a response message to the user"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let message = input.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
            
        let format = input.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let output = serde_json::json!({
            "delivered": true,
            "message_length": message.len(),
            "format": format
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// Cannot perform action
pub struct UserCannotPerformTool;

#[async_trait]
impl Tool for UserCannotPerformTool {
    fn name(&self) -> &'static str {
        "user.cannot_perform"
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reason": {"type": "string"},
                "alternatives": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["reason"]
        })
    }

    fn output_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "acknowledged": {"type": "boolean"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Report that an action cannot be performed"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let output = serde_json::json!({
            "acknowledged": true
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

/// Register all built-in tools
pub fn register_builtin_tools(registry: &op_tools::ToolRegistry) -> crate::error::Result<()> {
    use std::sync::Arc;

    // System tools
    registry.register(Arc::new(EchoTool), "system", "1.0.0")?;
    registry.register(Arc::new(NoopTool), "system", "1.0.0")?;

    // Network tools
    registry.register(Arc::new(NetworkListInterfacesTool), "network", "1.0.0")?;
    registry.register(Arc::new(NetworkGetInterfaceTool), "network", "1.0.0")?;

    // Storage tools
    registry.register(Arc::new(StorageListVolumesTool), "storage", "1.0.0")?;

    // Service tools
    registry.register(Arc::new(ServiceListTool), "service", "1.0.0")?;
    registry.register(Arc::new(ServiceStatusTool), "service", "1.0.0")?;

    // User interaction tools
    registry.register(Arc::new(UserRespondTool), "user", "1.0.0")?;
    registry.register(Arc::new(UserCannotPerformTool), "user", "1.0.0")?;

    // Inspector tools (one-shot only)
    registry.register(Arc::new(InspectorDiscoverTool), "inspector", "1.0.0")?;

    // D-Bus tools
    registry.register(Arc::new(super::dbus::DbusListServicesTool), "dbus", "1.0.0")?;
    registry.register(Arc::new(super::dbus::DbusIntrospectTool), "dbus", "1.0.0")?;

    // OVSDB protocol tools (RFC 7047)
    registry.register(Arc::new(super::ovs::OvsTool::new("ovsdb.list_dbs", "List OVSDB databases (RFC 7047 §4.1.1)")), "ovsdb", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("ovsdb.get_schema", "Get OVSDB schema (RFC 7047 §4.1.2)")), "ovsdb", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("ovsdb.transact", "Execute OVSDB transaction (RFC 7047 §4.1.3 — insert/select/update/mutate/delete/wait/commit/abort)")), "ovsdb", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("ovsdb.echo", "OVSDB echo liveness check (RFC 7047 §4.1.11)")), "ovsdb", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("ovsdb.dump", "Dump entire Open_vSwitch database")), "ovsdb", "1.0.0")?;

    // OVS high-level tools
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.bridge.list", "List OVS bridges")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.bridge.create", "Create OVS bridge")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.bridge.delete", "Delete OVS bridge")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.bridge.get", "Get OVS bridge details")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.bridge.set", "Set OVS bridge property")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.port.list", "List OVS bridge ports")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.port.add", "Add port to OVS bridge")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.port.delete", "Delete port from OVS bridge")), "network", "1.0.0")?;
    registry.register(Arc::new(super::ovs::OvsTool::new("network.ovs.interface.set_type", "Set OVS interface type")), "network", "1.0.0")?;

    // Incus container/VM management tools
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.list", "List all Incus instances (containers and VMs)")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.get", "Get Incus instance details by name")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.start", "Start an Incus instance")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.stop", "Stop an Incus instance")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.restart", "Restart an Incus instance")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.create", "Create a new Incus instance")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.delete", "Delete an Incus instance")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.instance.info", "Get detailed Incus instance info (state, memory, CPU, network, disk)")), "incus", "1.0.0")?;

    // Incus network tools
    registry.register(Arc::new(super::incus::IncusTool::new("incus.network.list", "List all Incus networks")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.network.get", "Get Incus network details")), "incus", "1.0.0")?;

    // Incus storage tools
    registry.register(Arc::new(super::incus::IncusTool::new("incus.storage.list", "List all Incus storage pools")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.storage.get", "Get Incus storage pool details")), "incus", "1.0.0")?;

    // Incus image tools
    registry.register(Arc::new(super::incus::IncusTool::new("incus.image.list", "List all Incus images")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.image.get", "Get Incus image details")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.image.delete", "Delete an Incus image")), "incus", "1.0.0")?;

    // Incus profile tools
    registry.register(Arc::new(super::incus::IncusTool::new("incus.profile.list", "List all Incus profiles")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.profile.get", "Get Incus profile details")), "incus", "1.0.0")?;

    // Incus snapshot tools
    registry.register(Arc::new(super::incus::IncusTool::new("incus.snapshot.list", "List snapshots for an Incus instance")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.snapshot.create", "Create a snapshot of an Incus instance")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.snapshot.delete", "Delete an Incus instance snapshot")), "incus", "1.0.0")?;
    registry.register(Arc::new(super::incus::IncusTool::new("incus.snapshot.restore", "Restore an Incus instance snapshot")), "incus", "1.0.0")?;

    Ok(())
}
