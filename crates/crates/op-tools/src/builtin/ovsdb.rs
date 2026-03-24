//! OVSDB Tools - Native JSON-RPC protocol for Open vSwitch
//!
//! These tools communicate directly with OVSDB via JSON-RPC over Unix socket.
//! NO CLI TOOLS (ovs-vsctl, ovs-ofctl) are used.
//!
//! Protocol: RFC 7047 - The Open vSwitch Database Management Protocol
//! Socket: /var/run/openvswitch/db.sock

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, info, warn};

use crate::tool::{BoxedTool, Tool};

/// Default OVSDB socket path
pub const OVSDB_SOCKET: &str = "/var/run/openvswitch/db.sock";

/// OVSDB JSON-RPC client
pub struct OvsdbClient {
    socket_path: String,
}

impl OvsdbClient {
    /// Create new client with default socket
    pub fn new() -> Self {
        Self {
            socket_path: OVSDB_SOCKET.to_string(),
        }
    }

    /// Create with custom socket path
    pub fn with_socket(path: &str) -> Self {
        Self {
            socket_path: path.to_string(),
        }
    }

    /// Send JSON-RPC request and get response
    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .context(format!("Failed to connect to OVSDB socket: {}", self.socket_path))?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Build JSON-RPC request
        let request = json!({
            "method": method,
            "params": params,
            "id": 1
        });

        let request_str = simd_json::to_string(&request)? + "\n";
        debug!("OVSDB request: {}", request_str.trim());

        writer.write_all(request_str.as_bytes()).await?;
        writer.flush().await?;

        // Read response
        let mut response_str = String::new();
        reader.read_line(&mut response_str).await?;
        debug!("OVSDB response: {}", response_str.trim());

        let response: Value = simd_json::from_str(&response_str)
            .context("Failed to parse OVSDB response")?;

        // Check for error
        if let Some(error) = response.get("error") {
            if !error.is_null() {
                return Err(anyhow::anyhow!("OVSDB error: {}", error));
            }
        }

        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Execute OVSDB transaction
    pub async fn transact(&self, operations: Vec<Value>) -> Result<Value> {
        let params = json!(["Open_vSwitch", operations]);
        self.rpc_call("transact", params).await
    }

    /// List all databases
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        let result = self.rpc_call("list_dbs", json!([])).await?;
        let dbs: Vec<String> = simd_json::serde::from_owned_value(result)?;
        Ok(dbs)
    }

    /// Get database schema
    pub async fn get_schema(&self, db: &str) -> Result<Value> {
        self.rpc_call("get_schema", json!([db])).await
    }

    /// Create a bridge
    pub async fn create_bridge(&self, name: &str) -> Result<Value> {
        info!("Creating OVS bridge '{}' via OVSDB JSON-RPC", name);

        // First, get the Open_vSwitch row UUID
        let select_ovs = json!({
            "op": "select",
            "table": "Open_vSwitch",
            "where": []
        });

        let result = self.transact(vec![select_ovs]).await?;
        let ovs_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Could not find Open_vSwitch row"))?;

        // Insert bridge and update Open_vSwitch.bridges
        let operations = vec![
            // Insert new bridge
            json!({
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "name": name,
                    "protocols": ["set", ["OpenFlow10", "OpenFlow13"]]
                },
                "uuid-name": "new_bridge"
            }),
            // Add bridge to Open_vSwitch.bridges set
            json!({
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [["_uuid", "==", ["uuid", ovs_uuid]]],
                "mutations": [
                    ["bridges", "insert", ["named-uuid", "new_bridge"]]
                ]
            }),
        ];

        self.transact(operations).await
    }

    /// Delete a bridge
    pub async fn delete_bridge(&self, name: &str) -> Result<Value> {
        info!("Deleting OVS bridge '{}' via OVSDB JSON-RPC", name);

        // Get bridge UUID
        let select_bridge = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", name]]
        });

        let result = self.transact(vec![select_bridge]).await?;
        let bridge_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))?;

        // Get Open_vSwitch UUID
        let select_ovs = json!({
            "op": "select",
            "table": "Open_vSwitch",
            "where": []
        });

        let result = self.transact(vec![select_ovs]).await?;
        let ovs_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Could not find Open_vSwitch row"))?;

        // Remove bridge from Open_vSwitch and delete it
        let operations = vec![
            json!({
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [["_uuid", "==", ["uuid", ovs_uuid]]],
                "mutations": [
                    ["bridges", "delete", ["uuid", bridge_uuid]]
                ]
            }),
            json!({
                "op": "delete",
                "table": "Bridge",
                "where": [["name", "==", name]]
            }),
        ];

        self.transact(operations).await
    }

    /// List all bridges
    pub async fn list_bridges(&self) -> Result<Vec<String>> {
        let select = json!({
            "op": "select",
            "table": "Bridge",
            "where": [],
            "columns": ["name"]
        });

        let result = self.transact(vec![select]).await?;
        let mut bridges = Vec::new();

        if let Some(rows) = result.get(0).and_then(|r| r.get("rows")).and_then(|r| r.as_array()) {
            for row in rows {
                if let Some(name) = row.get("name").and_then(|n| n.as_str()) {
                    bridges.push(name.to_string());
                }
            }
        }

        Ok(bridges)
    }

    /// Add port to bridge
    pub async fn add_port(&self, bridge: &str, port: &str) -> Result<Value> {
        info!("Adding port '{}' to bridge '{}' via OVSDB JSON-RPC", port, bridge);

        // Get bridge UUID
        let select_bridge = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]]
        });

        let result = self.transact(vec![select_bridge]).await?;
        let bridge_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge))?;

        let operations = vec![
            // Create interface
            json!({
                "op": "insert",
                "table": "Interface",
                "row": {
                    "name": port,
                    "type": ""
                },
                "uuid-name": "new_interface"
            }),
            // Create port with interface
            json!({
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": port,
                    "interfaces": ["named-uuid", "new_interface"]
                },
                "uuid-name": "new_port"
            }),
            // Add port to bridge
            json!({
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", bridge_uuid]]],
                "mutations": [
                    ["ports", "insert", ["named-uuid", "new_port"]]
                ]
            }),
        ];

        self.transact(operations).await
    }

    /// Delete port from bridge
    pub async fn delete_port(&self, bridge: &str, port: &str) -> Result<Value> {
        info!("Deleting port '{}' from bridge '{}' via OVSDB JSON-RPC", port, bridge);

        // Get port UUID
        let select_port = json!({
            "op": "select",
            "table": "Port",
            "where": [["name", "==", port]]
        });

        let result = self.transact(vec![select_port]).await?;
        let port_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Port '{}' not found", port))?;

        // Get bridge UUID
        let select_bridge = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]]
        });

        let result = self.transact(vec![select_bridge]).await?;
        let bridge_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge))?;

        let operations = vec![
            // Remove port from bridge
            json!({
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", bridge_uuid]]],
                "mutations": [
                    ["ports", "delete", ["uuid", port_uuid]]
                ]
            }),
            // Delete port (interface is deleted by cascade)
            json!({
                "op": "delete",
                "table": "Port",
                "where": [["name", "==", port]]
            }),
        ];

        self.transact(operations).await
    }

    /// List ports on a bridge
    pub async fn list_ports(&self, bridge: &str) -> Result<Vec<String>> {
        let select = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]],
            "columns": ["ports"]
        });

        let result = self.transact(vec![select]).await?;
        let mut ports = Vec::new();

        // Get port UUIDs from bridge
        let port_uuids = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("ports"));

        if let Some(port_refs) = port_uuids {
            // port_refs is either ["set", [...]] or ["uuid", "..."]
            let uuids: Vec<&str> = if let Some(arr) = port_refs.get(1).and_then(|v| v.as_array()) {
                arr.iter()
                    .filter_map(|u| u.get(1).and_then(|v| v.as_str()))
                    .collect()
            } else if let Some(uuid) = port_refs.get(1).and_then(|v| v.as_str()) {
                vec![uuid]
            } else {
                vec![]
            };

            // Get port names
            for uuid in uuids {
                let select_port = json!({
                    "op": "select",
                    "table": "Port",
                    "where": [["_uuid", "==", ["uuid", uuid]]],
                    "columns": ["name"]
                });

                if let Ok(result) = self.transact(vec![select_port]).await {
                    if let Some(name) = result
                        .get(0)
                        .and_then(|r| r.get("rows"))
                        .and_then(|rows| rows.get(0))
                        .and_then(|row| row.get("name"))
                        .and_then(|n| n.as_str())
                    {
                        ports.push(name.to_string());
                    }
                }
            }
        }

        Ok(ports)
    }

    /// Get bridge info
    pub async fn get_bridge(&self, name: &str) -> Result<Value> {
        let select = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", name]]
        });

        let result = self.transact(vec![select]).await?;

        result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))
    }
}

impl Default for OvsdbClient {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TOOL IMPLEMENTATIONS
// =============================================================================

/// Tool: Create OVS Bridge
pub struct OvsCreateBridgeTool {
    client: OvsdbClient,
}

impl OvsCreateBridgeTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsCreateBridgeTool {
    fn name(&self) -> &str {
        "ovs_create_bridge"
    }

    fn description(&self) -> &str {
        "Create an Open vSwitch bridge using native OVSDB JSON-RPC protocol. \
         NO CLI tools are used. Communicates directly with ovsdb-server."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge to create (e.g., 'ovsbr0')"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let result = self.client.create_bridge(name).await?;

        Ok(json!({
            "success": true,
            "operation": "create_bridge",
            "bridge": name,
            "protocol": "OVSDB JSON-RPC",
            "socket": OVSDB_SOCKET,
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Delete OVS Bridge
pub struct OvsDeleteBridgeTool {
    client: OvsdbClient,
}

impl OvsDeleteBridgeTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsDeleteBridgeTool {
    fn name(&self) -> &str {
        "ovs_delete_bridge"
    }

    fn description(&self) -> &str {
        "Delete an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge to delete"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let result = self.client.delete_bridge(name).await?;

        Ok(json!({
            "success": true,
            "operation": "delete_bridge",
            "bridge": name,
            "protocol": "OVSDB JSON-RPC",
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: List OVS Bridges
pub struct OvsListBridgesTool {
    client: OvsdbClient,
}

impl OvsListBridgesTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsListBridgesTool {
    fn name(&self) -> &str {
        "ovs_list_bridges"
    }

    fn description(&self) -> &str {
        "List all Open vSwitch bridges using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let bridges = self.client.list_bridges().await?;

        Ok(json!({
            "success": true,
            "operation": "list_bridges",
            "bridges": bridges,
            "count": bridges.len(),
            "protocol": "OVSDB JSON-RPC"
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Add Port to Bridge
pub struct OvsAddPortTool {
    client: OvsdbClient,
}

impl OvsAddPortTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsAddPortTool {
    fn name(&self) -> &str {
        "ovs_add_port"
    }

    fn description(&self) -> &str {
        "Add a port to an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                },
                "port": {
                    "type": "string",
                    "description": "Name of the port to add"
                }
            },
            "required": ["bridge", "port"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let port = input
            .get("port")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: port"))?;

        let result = self.client.add_port(bridge, port).await?;

        Ok(json!({
            "success": true,
            "operation": "add_port",
            "bridge": bridge,
            "port": port,
            "protocol": "OVSDB JSON-RPC",
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "port".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Delete Port from Bridge
pub struct OvsDeletePortTool {
    client: OvsdbClient,
}

impl OvsDeletePortTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsDeletePortTool {
    fn name(&self) -> &str {
        "ovs_delete_port"
    }

    fn description(&self) -> &str {
        "Delete a port from an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                },
                "port": {
                    "type": "string",
                    "description": "Name of the port to delete"
                }
            },
            "required": ["bridge", "port"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let port = input
            .get("port")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: port"))?;

        let result = self.client.delete_port(bridge, port).await?;

        Ok(json!({
            "success": true,
            "operation": "delete_port",
            "bridge": bridge,
            "port": port,
            "protocol": "OVSDB JSON-RPC",
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "port".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: List Ports on Bridge
pub struct OvsListPortsTool {
    client: OvsdbClient,
}

impl OvsListPortsTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsListPortsTool {
    fn name(&self) -> &str {
        "ovs_list_ports"
    }

    fn description(&self) -> &str {
        "List all ports on an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                }
            },
            "required": ["bridge"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let ports = self.client.list_ports(bridge).await?;

        Ok(json!({
            "success": true,
            "operation": "list_ports",
            "bridge": bridge,
            "ports": ports,
            "count": ports.len(),
            "protocol": "OVSDB JSON-RPC"
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "port".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Get Bridge Info
pub struct OvsGetBridgeTool {
    client: OvsdbClient,
}

impl OvsGetBridgeTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsGetBridgeTool {
    fn name(&self) -> &str {
        "ovs_get_bridge"
    }

    fn description(&self) -> &str {
        "Get detailed information about an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let bridge = self.client.get_bridge(name).await?;
        let ports = self.client.list_ports(name).await.unwrap_or_default();

        Ok(json!({
            "success": true,
            "operation": "get_bridge",
            "bridge": name,
            "info": bridge,
            "ports": ports,
            "protocol": "OVSDB JSON-RPC"
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

// =============================================================================
// TOOL REGISTRATION
// =============================================================================

/// Create all OVS tools
pub fn create_ovs_tools() -> Vec<BoxedTool> {
    vec![
        Arc::new(OvsCreateBridgeTool::new()),
        Arc::new(OvsDeleteBridgeTool::new()),
        Arc::new(OvsListBridgesTool::new()),
        Arc::new(OvsAddPortTool::new()),
        Arc::new(OvsDeletePortTool::new()),
        Arc::new(OvsListPortsTool::new()),
        Arc::new(OvsGetBridgeTool::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_names() {
        let tools = create_ovs_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();

        assert!(names.contains(&"ovs_create_bridge"));
        assert!(names.contains(&"ovs_delete_bridge"));
        assert!(names.contains(&"ovs_list_bridges"));
        assert!(names.contains(&"ovs_add_port"));
        assert!(names.contains(&"ovs_delete_port"));
        assert!(names.contains(&"ovs_list_ports"));
        assert!(names.contains(&"ovs_get_bridge"));
    }

    #[test]
    fn test_tool_schemas() {
        let tool = OvsCreateBridgeTool::new();
        let schema = tool.input_schema();

        assert!(schema.get("properties").is_some());
        assert!(schema.get("properties").unwrap().get("name").is_some());
    }
}
