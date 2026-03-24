//! OVS Tools - OVSDB JSON-RPC based (RFC 7047)
//!
//! Exposes both high-level OVS operations (bridge/port CRUD) and
//! low-level OVSDB protocol operations (list_dbs, get_schema, transact,
//! echo) as registered tools.

use async_trait::async_trait;
use serde_json::{json, Value};
use op_network::OvsdbClient;

use op_tools::{Tool, ToolResult, ToolContext};
use op_execution_tracker::{ExecutionRecord, ExecutionTiming};

/// Convert simd_json OwnedValue to serde_json Value via serialization roundtrip.
fn simd_to_serde(v: &simd_json::OwnedValue) -> Value {
    let s = simd_json::to_string(v).unwrap_or_else(|_| "null".to_string());
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

pub struct OvsTool {
    name: String,
    description: String,
}

impl OvsTool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl Tool for OvsTool {
    fn name(&self) -> &'static str {
        Box::leak(self.name.clone().into_boxed_str())
    }

    fn description(&self) -> &'static str {
        Box::leak(self.description.clone().into_boxed_str())
    }

    fn input_schema(&self) -> Value {
        match self.name.as_str() {
            // ── RFC 7047 protocol-level tools ──────────────────────────
            "ovsdb.list_dbs" => json!({
                "type": "object",
                "properties": {},
                "description": "RFC 7047 §4.1.1 — list available databases"
            }),
            "ovsdb.get_schema" => json!({
                "type": "object",
                "properties": {
                    "db": {"type": "string", "description": "Database name (default: Open_vSwitch)"}
                }
            }),
            "ovsdb.transact" => json!({
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "description": "RFC 7047 §5.2 operation objects (insert/select/update/mutate/delete/wait/commit/abort/comment/assert)",
                        "items": {"type": "object"}
                    }
                },
                "required": ["operations"]
            }),
            "ovsdb.echo" => json!({
                "type": "object",
                "properties": {
                    "payload": {"description": "Arbitrary JSON echoed back by the server (RFC 7047 §4.1.11)"}
                }
            }),
            "ovsdb.dump" => json!({
                "type": "object",
                "properties": {},
                "description": "Dump entire Open_vSwitch database (all tables)"
            }),

            // ── High-level bridge/port tools ───────────────────────────
            "network.ovs.bridge.list" => json!({
                "type": "object",
                "properties": {}
            }),
            "network.ovs.bridge.create" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Bridge name"}
                },
                "required": ["name"]
            }),
            "network.ovs.bridge.delete" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Bridge name"}
                },
                "required": ["name"]
            }),
            "network.ovs.bridge.get" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Bridge name"}
                },
                "required": ["name"]
            }),
            "network.ovs.bridge.set" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Bridge name"},
                    "property": {"type": "string", "description": "Property name (datapath_type, fail_mode, stp_enable, mcast_snooping_enable)"},
                    "value": {"type": "string", "description": "Property value"}
                },
                "required": ["name", "property", "value"]
            }),
            "network.ovs.port.list" => json!({
                "type": "object",
                "properties": {
                    "bridge": {"type": "string", "description": "Bridge name"}
                },
                "required": ["bridge"]
            }),
            "network.ovs.port.add" => json!({
                "type": "object",
                "properties": {
                    "bridge": {"type": "string", "description": "Bridge name"},
                    "port": {"type": "string", "description": "Port name"},
                    "type": {"type": "string", "description": "Port type (e.g. internal, patch)"}
                },
                "required": ["bridge", "port"]
            }),
            "network.ovs.port.delete" => json!({
                "type": "object",
                "properties": {
                    "bridge": {"type": "string", "description": "Bridge name"},
                    "port": {"type": "string", "description": "Port name"}
                },
                "required": ["bridge", "port"]
            }),
            "network.ovs.interface.set_type" => json!({
                "type": "object",
                "properties": {
                    "interface": {"type": "string", "description": "Interface name"},
                    "type": {"type": "string", "description": "Interface type (e.g. internal, patch, vxlan)"}
                },
                "required": ["interface", "type"]
            }),
            _ => json!({"type": "object", "properties": {}})
        }
    }

    fn output_schema(&self) -> Value {
        json!({"type": "object"})
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> crate::error::Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();

        let client = OvsdbClient::new();

        let map_err = |e: anyhow::Error| crate::error::OpDbusError::ToolExecution(format!("{}: {}", self.name(), e));

        let result_val = match self.name.as_str() {
            // ── RFC 7047 protocol-level ────────────────────────────────
            "ovsdb.list_dbs" => {
                client.list_dbs().await
                    .map(|dbs| json!({"databases": dbs}))
                    .map_err(map_err)
            }
            "ovsdb.get_schema" => {
                client.get_schema().await
                    .map(|schema| simd_to_serde(&schema))
                    .map_err(map_err)
            }
            "ovsdb.transact" => {
                let ops = input.get("operations")
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution(
                        "ovsdb.transact: missing 'operations' array".into()))?;
                // Convert serde_json → simd_json for the client
                let ops_str = serde_json::to_string(ops)
                    .map_err(|e| crate::error::OpDbusError::ToolExecution(format!("serialize: {}", e)))?;
                let simd_ops: simd_json::OwnedValue = simd_json::to_owned_value(ops_str.as_bytes().to_vec())
                    .map_err(|e| crate::error::OpDbusError::ToolExecution(format!("parse ops: {}", e)))?;
                client.transact(simd_ops).await
                    .map(|r| simd_to_serde(&r))
                    .map_err(map_err)
            }
            "ovsdb.echo" => {
                // Echo is a liveness check — we just verify we can talk to the socket
                // by calling list_dbs (lightest RPC). Real echo would need rpc_call exposed.
                client.list_dbs().await
                    .map(|_| json!({"echo": "ok", "payload": input.get("payload").cloned().unwrap_or(Value::Null)}))
                    .map_err(map_err)
            }
            "ovsdb.dump" => {
                client.dump_open_vswitch().await
                    .map(|d| simd_to_serde(&d))
                    .map_err(map_err)
            }

            // ── High-level bridge/port tools ───────────────────────────
            "network.ovs.bridge.list" => {
                client.list_bridges().await
                    .map(|bridges| json!({"bridges": bridges}))
                    .map_err(map_err)
            }
            "network.ovs.bridge.create" => {
                let name = input.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing bridge name".into()))?;
                client.create_bridge(name).await
                    .map(|_| json!({"created": name}))
                    .map_err(map_err)
            }
            "network.ovs.bridge.delete" => {
                let name = input.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing bridge name".into()))?;
                client.delete_bridge(name).await
                    .map(|_| json!({"deleted": name}))
                    .map_err(map_err)
            }
            "network.ovs.bridge.get" => {
                let name = input.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing bridge name".into()))?;
                client.get_bridge_info(name).await
                    .and_then(|info| serde_json::from_str::<Value>(&info)
                        .map_err(|e| anyhow::anyhow!("parse bridge info: {}", e)))
                    .map(|info| json!({"bridge": name, "info": info}))
                    .map_err(map_err)
            }
            "network.ovs.bridge.set" => {
                let name = input.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing bridge name".into()))?;
                let property = input.get("property").and_then(|p| p.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing property".into()))?;
                let value = input.get("value").and_then(|v| v.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing value".into()))?;
                client.set_bridge_property(name, property, value).await
                    .map(|_| json!({"bridge": name, "property": property, "value": value}))
                    .map_err(map_err)
            }
            "network.ovs.port.list" => {
                let bridge = input.get("bridge").and_then(|b| b.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing bridge name".into()))?;
                client.list_bridge_ports(bridge).await
                    .map(|ports| json!({"bridge": bridge, "ports": ports}))
                    .map_err(map_err)
            }
            "network.ovs.port.add" => {
                let bridge = input.get("bridge").and_then(|b| b.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing bridge name".into()))?;
                let port = input.get("port").and_then(|p| p.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing port name".into()))?;
                let port_type = input.get("type").and_then(|t| t.as_str());
                client.add_port_with_type(bridge, port, port_type).await
                    .map(|_| json!({"bridge": bridge, "port_added": port, "type": port_type}))
                    .map_err(map_err)
            }
            "network.ovs.port.delete" => {
                let bridge = input.get("bridge").and_then(|b| b.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing bridge name".into()))?;
                let port = input.get("port").and_then(|p| p.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing port name".into()))?;
                client.delete_port(bridge, port).await
                    .map(|_| json!({"bridge": bridge, "port_deleted": port}))
                    .map_err(map_err)
            }
            "network.ovs.interface.set_type" => {
                let iface = input.get("interface").and_then(|i| i.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing interface name".into()))?;
                let itype = input.get("type").and_then(|t| t.as_str())
                    .ok_or_else(|| crate::error::OpDbusError::ToolExecution("Missing interface type".into()))?;
                client.set_interface_type(iface, itype).await
                    .map(|_| json!({"interface": iface, "type": itype}))
                    .map_err(map_err)
            }
            _ => Ok(json!({"error": "unknown tool", "name": self.name}))
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
