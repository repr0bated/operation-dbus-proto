//! Direct OVSDB JSON-RPC client - no wrappers, pure native protocol
//! Talks directly to /var/run/openvswitch/db.sock

use anyhow::{Context, Result};
use simd_json::prelude::*;
use simd_json::value::owned::Object;
use simd_json::{json, OwnedValue as Value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Direct OVSDB JSON-RPC client
pub struct OvsdbClient {
    socket_path: String,
}

impl OvsdbClient {
    /// Connect to OVSDB unix socket
    pub fn new() -> Self {
        let paths = ["/var/run/openvswitch/db.sock", "/run/openvswitch/db.sock"];
        let socket_path = paths
            .iter()
            .find(|p| Path::new(p).exists())
            .unwrap_or(&"/var/run/openvswitch/db.sock")
            .to_string();

        Self { socket_path }
    }

    /// Ensure OVSDB database is initialized (similar to ovs-vsctl init)
    /// This ensures the Open_vSwitch table exists and is properly set up
    pub async fn ensure_initialized(&self) -> Result<()> {
        // Check if we can list databases - this verifies the connection works
        let _ = self.list_dbs().await?;

        // Try to get the schema - this verifies the database is properly initialized
        let _ = self.get_schema().await?;

        // Check if Open_vSwitch table exists and has basic structure
        let dump = self.dump_open_vswitch().await?;
        if dump.as_array().is_none_or(|arr| arr.is_empty()) {
            log::warn!("OVSDB Open_vSwitch table appears empty - database may need initialization");
            // Note: We don't auto-initialize here as it should be done by systemd/ovs-vsctl init
        }

        Ok(())
    }

    /// Send JSON-RPC request and get response
    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        log::debug!(
            "Attempting to connect to OVSDB socket: {}",
            self.socket_path
        );
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to OVSDB socket")?;
        log::debug!("Successfully connected to OVSDB socket");

        // Build JSON-RPC request
        let request = json!({
            "method": method,
            "params": params,
            "id": 0
        });

        // Send request
        let request_str = simd_json::to_string(&request)?;
        log::debug!("Sending OVSDB request: {}", request_str);
        stream.write_all(request_str.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;
        log::debug!("OVSDB request sent, waiting for response");

        // Read response with timeout
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();

        let read_result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                response_line.clear();
                let n = reader.read_line(&mut response_line).await?;
                if n == 0 || response_line.trim().starts_with('{') {
                    return Ok::<usize, std::io::Error>(n);
                }
            }
        })
        .await;

        match read_result {
            Ok(Ok(bytes_read)) => {
                if bytes_read == 0 {
                    return Err(anyhow::anyhow!("OVSDB connection closed by server"));
                }
                log::debug!(
                    "Received OVSDB raw response ({} bytes): {}",
                    bytes_read,
                    response_line.trim()
                );
            }
            Ok(Err(e)) => {
                return Err(anyhow::anyhow!("Failed to read OVSDB response: {}", e));
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "OVSDB response timeout after sending: {}",
                    request_str
                ));
            }
        };

        let response: Value = unsafe { simd_json::from_str(response_line.as_mut_str())? };

        // Check for error (only if it's not null)
        if let Some(error) = response.get("error") {
            if !error.is_null() {
                return Err(anyhow::anyhow!("OVSDB error: {}", error));
            }
        }

        Ok(response["result"].clone())
    }

    /// List all databases
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        let result = self.rpc_call("list_dbs", json!([])).await?;
        Ok(simd_json::serde::from_owned_value(result)?)
    }

    /// Get schema for Open_vSwitch database
    #[allow(dead_code)]
    pub async fn get_schema(&self) -> Result<Value> {
        self.rpc_call("get_schema", json!(["Open_vSwitch"])).await
    }

    async fn ensure_tables_exist(&self, required_tables: &[&str]) -> Result<()> {
        let schema = self.get_schema().await?;
        let tables = schema
            .get("tables")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("Invalid OVSDB schema: missing tables"))?;

        for table in required_tables {
            if !tables.contains_key(*table) {
                return Err(anyhow::anyhow!(
                    "OVSDB schema does not contain required table '{}'",
                    table
                ));
            }
        }

        Ok(())
    }

    async fn validate_mutation_operations(&self, operations: &Value) -> Result<()> {
        let Some(ops) = operations.as_array() else {
            return Err(anyhow::anyhow!(
                "OVSDB transaction operations must be an array"
            ));
        };

        if !ops.iter().any(Self::operation_changes_rows) {
            return Ok(());
        }

        let schema = self.get_schema().await?;
        Self::validate_operations_against_schema(&schema, operations)
    }

    fn validate_operations_against_schema(schema: &Value, operations: &Value) -> Result<()> {
        let tables = schema
            .get("tables")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("Invalid OVSDB schema: missing tables"))?;

        let ops = operations
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("OVSDB transaction operations must be an array"))?;

        for (idx, op) in ops.iter().enumerate() {
            if !Self::operation_changes_rows(op) {
                continue;
            }

            let op_name = op
                .get("op")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("OVSDB operation {} is missing op", idx))?;
            let table_name = op.get("table").and_then(|v| v.as_str()).ok_or_else(|| {
                anyhow::anyhow!("OVSDB {} operation {} is missing table", op_name, idx)
            })?;
            let table_schema = tables.get(table_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "OVSDB schema does not contain table '{}' required by {} operation {}",
                    table_name,
                    op_name,
                    idx
                )
            })?;

            Self::validate_operation_columns(idx, op_name, table_name, table_schema, op)?;
        }

        Ok(())
    }

    fn operation_changes_rows(op: &Value) -> bool {
        matches!(
            op.get("op").and_then(|v| v.as_str()),
            Some("insert" | "update" | "mutate" | "delete")
        )
    }

    fn validate_operation_columns(
        idx: usize,
        op_name: &str,
        table_name: &str,
        table_schema: &Value,
        op: &Value,
    ) -> Result<()> {
        let columns = table_schema
            .get("columns")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("Invalid OVSDB schema for table '{}'", table_name))?;

        if let Some(row) = op.get("row").and_then(|v| v.as_object()) {
            for column in row.keys() {
                Self::ensure_column_exists(idx, op_name, table_name, columns, column)?;
            }
        }

        if let Some(conditions) = op.get("where").and_then(|v| v.as_array()) {
            for condition in conditions {
                if let Some(column) = condition
                    .as_array()
                    .and_then(|parts| parts.first())
                    .and_then(|v| v.as_str())
                {
                    Self::ensure_column_exists(idx, op_name, table_name, columns, column)?;
                }
            }
        }

        if let Some(mutations) = op.get("mutations").and_then(|v| v.as_array()) {
            for mutation in mutations {
                let column = mutation
                    .as_array()
                    .and_then(|parts| parts.first())
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "OVSDB mutate operation {} on table '{}' has a malformed mutation",
                            idx,
                            table_name
                        )
                    })?;
                Self::ensure_column_exists(idx, op_name, table_name, columns, column)?;
            }
        }

        if let Some(columns_list) = op.get("columns").and_then(|v| v.as_array()) {
            for column in columns_list.iter().filter_map(|v| v.as_str()) {
                Self::ensure_column_exists(idx, op_name, table_name, columns, column)?;
            }
        }

        Ok(())
    }

    fn ensure_column_exists(
        idx: usize,
        op_name: &str,
        table_name: &str,
        columns: &Object,
        column: &str,
    ) -> Result<()> {
        if column == "_uuid" || column == "_version" || columns.contains_key(column) {
            return Ok(());
        }

        Err(anyhow::anyhow!(
            "OVSDB schema table '{}' does not contain column '{}' required by {} operation {}",
            table_name,
            column,
            op_name,
            idx
        ))
    }

    /// Dump entire Open_vSwitch database: table -> rows (JSON)
    #[allow(dead_code)]
    pub async fn dump_open_vswitch(&self) -> Result<Value> {
        // Discover tables from schema
        let schema = self.get_schema().await?;
        let tables = schema
            .get("tables")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("Invalid OVSDB schema: missing tables"))?;

        // Build select ops for all tables
        let mut ops = Vec::new();
        let mut order = Vec::new();
        for (name, _def) in tables.iter() {
            ops.push(json!({
                "op": "select",
                "table": name,
                "where": []
            }));
            order.push(name.clone());
        }

        let result = self.transact(json!(ops)).await?;

        // Assemble into object
        let mut out = Object::new();
        for (i, name) in order.into_iter().enumerate() {
            let rows = result
                .get_idx(i)
                .and_then(|r| r.get("rows"))
                .cloned()
                .unwrap_or_else(|| json!([]));
            out.insert(name, rows);
        }

        Ok(Value::Object(Box::new(out)))
    }

    /// Select all rows from one Open_vSwitch table.
    pub async fn select_table(&self, table: &str) -> Result<Vec<Value>> {
        let result = self
            .transact(json!([{
                "op": "select",
                "table": table,
                "where": []
            }]))
            .await?;

        Ok(result
            .get_idx(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.as_array())
            .map(|rows| rows.to_vec())
            .unwrap_or_default())
    }

    /// Transact - execute OVSDB operations
    pub async fn transact(&self, operations: Value) -> Result<Value> {
        self.validate_mutation_operations(&operations).await?;

        let mut params = vec![json!("Open_vSwitch")];
        if let Some(ops_array) = operations.as_array() {
            for op in ops_array {
                params.push(op.clone());
            }
        }
        let result = self.rpc_call("transact", json!(params)).await?;

        // Check for per-operation errors in the result array
        // OVSDB returns an array of results, one per operation
        // Each result can be an error object like {"error": "...", "details": "..."}
        if let Some(results) = result.as_array() {
            for (i, op_result) in results.iter().enumerate() {
                if let Some(error) = op_result.get("error") {
                    if let Some(error_str) = error.as_str() {
                        let details = op_result
                            .get("details")
                            .and_then(|d| d.as_str())
                            .unwrap_or("no details");
                        return Err(anyhow::anyhow!(
                            "OVSDB operation {} failed: {} ({})",
                            i,
                            error_str,
                            details
                        ));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Create OVS bridge
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        // Validate against the native OVSDB schema before building the transact.
        self.ensure_tables_exist(&["Open_vSwitch", "Bridge", "Port", "Interface"])
            .await?;

        // Check if bridge already exists
        if self.bridge_exists(bridge_name).await? {
            log::info!("Bridge {} already exists, skipping creation", bridge_name);
            return Ok(());
        }

        // Generate stable temporary names for OVSDB row references.
        let safe_name: String = bridge_name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect();
        let bridge_ref = format!("bridge_{}", safe_name);
        let port_ref = format!("port_{}", safe_name);
        let iface_ref = format!("iface_{}", safe_name);

        // Create the bridge together with its local internal port. OVS does not
        // materialize a kernel-visible bridge interface on this host unless the
        // local internal interface row is created explicitly.
        let operations = json!([
            {
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "name": bridge_name,
                    "datapath_type": "system",
                    "stp_enable": false,
                    "ports": ["set", [["named-uuid", port_ref]]],
                    "other_config": ["map", []],
                    "external_ids": ["map", []]
                },
                "uuid-name": bridge_ref
            },
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": bridge_name,
                    "interfaces": ["set", [["named-uuid", iface_ref]]]
                },
                "uuid-name": port_ref
            },
            {
                "op": "insert",
                "table": "Interface",
                "row": {
                    "name": bridge_name,
                    "type": "internal"
                },
                "uuid-name": iface_ref
            },
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [
                    ["bridges", "insert", ["set", [["named-uuid", bridge_ref]]]]
                ]
            }
        ]);

        self.transact(operations).await?;

        // Verify bridge was created and persisted
        if self.bridge_exists(bridge_name).await? {
            log::info!("Bridge {} successfully created and persisted", bridge_name);
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Bridge {} creation failed - not found after creation",
                bridge_name
            ))
        }
    }

    /// Add port to bridge (system port - attaches existing interface)
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        self.add_port_with_type(bridge_name, port_name, None).await
    }

    /// Add port to bridge with optional type (e.g., "internal" for virtual ports)
    pub async fn add_port_with_type(
        &self,
        bridge_name: &str,
        port_name: &str,
        port_type: Option<&str>,
    ) -> Result<()> {
        // First, find the bridge UUID
        let bridge_uuid = self.find_bridge_uuid(bridge_name).await?;

        let safe_name: String = port_name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect();
        let port_uuid = format!("port_{}", safe_name);
        let iface_uuid = format!("iface_{}", safe_name);

        // Build interface row - add type if specified
        let interface_row = if let Some(iface_type) = port_type {
            json!({
                "name": port_name,
                "type": iface_type
            })
        } else {
            json!({
                "name": port_name
            })
        };

        let operations = json!([
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": port_name,
                    "interfaces": ["set", [["named-uuid", iface_uuid]]]
                },
                "uuid-name": port_uuid
            },
            {
                "op": "insert",
                "table": "Interface",
                "row": interface_row,
                "uuid-name": iface_uuid
            },
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
                "mutations": [
                    ["ports", "insert", ["set", [["named-uuid", port_uuid]]]]
                ]
            }
        ]);

        self.transact(operations).await?;
        log::info!(
            "Port {} (type: {:?}) added to bridge {}",
            port_name,
            port_type,
            bridge_name
        );
        Ok(())
    }

    /// Delete bridge
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        let bridge_uuid = self.find_bridge_uuid(bridge_name).await?;

        let operations = json!([
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [
                    ["bridges", "delete", ["uuid", &bridge_uuid]]
                ]
            },
            {
                "op": "delete",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", &bridge_uuid]]]
            }
        ]);

        self.transact(operations).await?;
        Ok(())
    }

    /// Check if bridge exists
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        match self.find_bridge_uuid(bridge_name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Find bridge UUID by name
    async fn find_bridge_uuid(&self, bridge_name: &str) -> Result<String> {
        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge_name]],
            "columns": ["_uuid"]
        }]);

        let result = self.transact(operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(uuid_array) = first_row["_uuid"].as_array() {
                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                        return Ok(uuid_array[1].as_str().unwrap().to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Bridge '{}' not found", bridge_name))
    }

    /// List all bridges
    pub async fn list_bridges(&self) -> Result<Vec<String>> {
        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [],
            "columns": ["name"]
        }]);

        let result = self.transact(operations).await?;

        let mut bridges = Vec::new();
        if let Some(rows) = result[0]["rows"].as_array() {
            for row in rows {
                if let Some(name) = row["name"].as_str() {
                    bridges.push(name.to_string());
                }
            }
        }

        Ok(bridges)
    }

    /// List ports on bridge
    pub async fn list_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        let bridge_uuid = self.find_bridge_uuid(bridge_name).await?;

        // Get the bridge with its ports
        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
            "columns": ["ports"]
        }]);

        let result = self.transact(operations).await?;

        let mut port_uuids = Vec::new();
        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(ports) = first_row["ports"].as_array() {
                    if ports.len() == 2 && ports[0] == "set" {
                        if let Some(port_set) = ports[1].as_array() {
                            for port in port_set {
                                if let Some(uuid_array) = port.as_array() {
                                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                                        port_uuids
                                            .push(uuid_array[1].as_str().unwrap().to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Now get port names
        let mut port_names = Vec::new();
        for port_uuid in port_uuids {
            let operations = json!([{
                "op": "select",
                "table": "Port",
                "where": [["_uuid", "==", ["uuid", &port_uuid]]],
                "columns": ["name"]
            }]);

            let result = self.transact(operations).await?;
            if let Some(rows) = result[0]["rows"].as_array() {
                if let Some(first_row) = rows.first() {
                    if let Some(name) = first_row["name"].as_str() {
                        port_names.push(name.to_string());
                    }
                }
            }
        }

        Ok(port_names)
    }

    /// Get bridge info
    pub async fn get_bridge_info(&self, bridge_name: &str) -> Result<String> {
        let bridge_uuid = self.find_bridge_uuid(bridge_name).await?;

        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
            "columns": []
        }]);

        let result = self.transact(operations).await?;
        Ok(simd_json::to_string_pretty(&result[0]["rows"][0])?)
    }

    /// Set interface type
    #[allow(dead_code)]
    pub async fn set_interface_type(
        &self,
        interface_name: &str,
        interface_type: &str,
    ) -> Result<()> {
        let operations = json!([
            {
                "op": "update",
                "table": "Interface",
                "where": [["name", "==", interface_name]],
                "row": {
                    "type": interface_type
                }
            }
        ]);

        let result = self.transact(operations).await?;
        // Check for errors in the response
        if let Some(errors) = result.as_array() {
            for error in errors {
                if error.get("error").is_some() {
                    return Err(anyhow::anyhow!("OVSDB transaction failed: {:?}", error));
                }
            }
        }

        Ok(())
    }

    /// Dump the entire database
    pub async fn dump_db(&self, _db: &str) -> Result<Value> {
        self.rpc_call("dump", json!(["Open_vSwitch"])).await
    }

    /// Monitor OVSDB for changes
    pub async fn monitor_db(&self, _db: &str) -> Result<tokio::sync::mpsc::Receiver<Value>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let socket_path = self.socket_path.clone();

        // Spawn a background task to monitor the socket
        tokio::spawn(async move {
            if let Ok(mut stream) = UnixStream::connect(&socket_path).await {
                // Send monitor request
                let monitor_req = json!({
                    "method": "monitor",
                    "params": ["Open_vSwitch", null, {
                        "Bridge": {"columns": ["name", "ports", "datapath_type"]},
                        "Port": {"columns": ["name", "interfaces"]},
                        "Interface": {"columns": ["name", "type", "options"]}
                    }],
                    "id": "monitor"
                });

                if let Ok(req_str) = simd_json::to_string(&monitor_req) {
                    let _ = stream.write_all(req_str.as_bytes()).await;
                    let _ = stream.write_all(b"\n").await;

                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match reader.read_line(&mut line).await {
                            Ok(0) => break, // Connection closed
                            Ok(_) => {
                                if line.trim().is_empty() {
                                    continue;
                                }
                                let mut data = line.clone().into_bytes();
                                if let Ok(update) = simd_json::from_slice::<Value>(&mut data) {
                                    if tx.send(update).await.is_err() {
                                        break;
                                    }
                                } else {
                                    log::warn!("Failed to parse OVSDB monitor update: {}", line);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Set bridge property (datapath_type, fail_mode, etc.)
    pub async fn set_bridge_property(
        &self,
        bridge_name: &str,
        property: &str,
        value: &str,
    ) -> Result<()> {
        // Build the row update based on property type
        let row = match property {
            "datapath_type" => json!({ "datapath_type": value }),
            "fail_mode" => json!({ "fail_mode": value }),
            "stp_enable" => json!({ "stp_enable": value == "true" }),
            "mcast_snooping_enable" => json!({ "mcast_snooping_enable": value == "true" }),
            _ => return Err(anyhow::anyhow!("Unknown bridge property: {}", property)),
        };

        let operations = json!([
            {
                "op": "update",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "row": row
            }
        ]);

        let result = self.transact(operations).await?;

        // Check for errors in the response
        if let Some(errors) = result.as_array() {
            for error in errors {
                if error.get("error").is_some() {
                    return Err(anyhow::anyhow!("OVSDB transaction failed: {:?}", error));
                }
            }
        }

        Ok(())
    }

    /// Delete a port from a bridge
    pub async fn delete_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        // First, get the port UUID
        let select_port = json!([{
            "op": "select",
            "table": "Port",
            "where": [["name", "==", port_name]],
            "columns": ["_uuid"]
        }]);

        let port_result = self.transact(select_port).await?;
        let port_uuid = port_result[0]["rows"][0]["_uuid"][1]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Port '{}' not found", port_name))?
            .to_string();

        // Get current bridge ports
        let select_bridge = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge_name]],
            "columns": ["_uuid", "ports"]
        }]);

        let bridge_result = self.transact(select_bridge).await?;
        let bridge_uuid = bridge_result[0]["rows"][0]["_uuid"][1]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge_name))?
            .to_string();

        // Remove port from bridge and delete port/interface
        let operations = json!([
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", bridge_uuid]]],
                "mutations": [["ports", "delete", ["uuid", port_uuid]]]
            },
            {
                "op": "delete",
                "table": "Port",
                "where": [["_uuid", "==", ["uuid", port_uuid]]]
            },
            {
                "op": "delete",
                "table": "Interface",
                "where": [["name", "==", port_name]]
            }
        ]);

        let result = self.transact(operations).await?;

        // Check for errors
        if let Some(errors) = result.as_array() {
            for error in errors {
                if error.get("error").is_some() {
                    return Err(anyhow::anyhow!("Failed to delete port: {:?}", error));
                }
            }
        }

        Ok(())
    }
}

impl Default for OvsdbClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        json!({
            "tables": {
                "Bridge": {
                    "columns": {
                        "name": {},
                        "ports": {}
                    }
                },
                "Port": {
                    "columns": {
                        "name": {},
                        "interfaces": {}
                    }
                }
            }
        })
    }

    #[test]
    fn validates_native_schema_for_mutation_table_and_columns() {
        let operations = json!([
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", "bridge-uuid"]]],
                "mutations": [
                    ["ports", "insert", ["set", [["uuid", "port-uuid"]]]]
                ]
            }
        ]);

        OvsdbClient::validate_operations_against_schema(&schema(), &operations)
            .expect("valid mutation schema");
    }

    #[test]
    fn rejects_mutation_for_unknown_native_table() {
        let operations = json!([
            {
                "op": "insert",
                "table": "Missing",
                "row": {
                    "name": "bad"
                }
            }
        ]);

        let err = OvsdbClient::validate_operations_against_schema(&schema(), &operations)
            .expect_err("unknown table must fail");
        assert!(err.to_string().contains("Missing"));
    }

    #[test]
    fn rejects_mutation_for_unknown_native_column() {
        let operations = json!([
            {
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "not_in_schema": "bad"
                }
            }
        ]);

        let err = OvsdbClient::validate_operations_against_schema(&schema(), &operations)
            .expect_err("unknown column must fail");
        assert!(err.to_string().contains("not_in_schema"));
    }
}
