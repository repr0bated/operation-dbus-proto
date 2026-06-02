This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-jsonrpc/
              src/
                lib.rs
                nonnet_staging.rs
                nonnet.rs
                ovsdb_jsonrpc.rs
                ovsdb_rpc_call.rs
                ovsdb.rs
                protocol.rs
                server.rs
              Cargo.toml
              compare-op-jsonrpc.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/lib.rs">
//! op-jsonrpc: JSON-RPC server with OVSDB and NonNet support
//!
//! This crate provides:
//! - JSON-RPC 2.0 server over Unix sockets
//! - OVSDB client for Open vSwitch integration
//! - NonNet database for non-network plugin state

pub mod nonnet;
pub mod ovsdb;
pub mod protocol;
pub mod server;

pub use nonnet::NonNetDb;
pub use ovsdb::OvsdbClient;
pub use server::JsonRpcServer;

/// Prelude for convenient imports
pub mod prelude {
    pub use super::nonnet::NonNetDb;
    pub use super::ovsdb::OvsdbClient;
    pub use super::protocol::{JsonRpcRequest, JsonRpcResponse};
    pub use super::server::JsonRpcServer;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/nonnet_staging.rs">
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use simd_json::{json, OwnedValue as Value};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::state::StateManager;

// Minimal JSON-RPC handler for a read-only, OVSDB-like interface over a unix socket.
// Methods: list_dbs() -> ["OpNonNet"], get_schema(db) -> { tables: {...} }, transact([db, ops]) with select ops only.

pub async fn run_unix_jsonrpc(state: Arc<StateManager>, socket_path: &str) -> Result<()> {
    let p = Path::new(socket_path);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).await.ok();
    }
    if p.exists() {
        let _ = fs::remove_file(p).await;
    }

    let listener = UnixListener::bind(p).context("bind nonnet DB socket")?;
    loop {
        let (stream, _) = listener.accept().await?;
        let st = Arc::clone(&state);
        tokio::spawn(async move {
            let _ = handle_connection(st, stream).await;
        });
    }
}

async fn handle_connection(state: Arc<StateManager>, stream: UnixStream) -> Result<()> {
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);
    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        let response = match simd_json::from_str::<Value>(&line) {
            Ok(req) => handle_request(&state, req)
                .await
                .unwrap_or_else(|e| json!({"error": e.to_string()})),
            Err(e) => json!({"error": format!("invalid json: {}", e)}),
        };
        let s = simd_json::to_string(&response)?;
        w.write_all(s.as_bytes()).await?;
        w.write_all(b"\n").await?;
        line.clear();
    }
    Ok(())
}

async fn handle_request(state: &Arc<StateManager>, req: Value) -> Result<Value> {
    let id = req.get("id").cloned().unwrap_or(json!(null));
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(json!([]));

    let result = match method {
        "list_dbs" => json!(["OpNonNet"]),
        "get_schema" => {
            // params: [db]
            let _db = params.get(0).and_then(|v| v.as_str()).unwrap_or("OpNonNet");
            let current = state.query_current_state().await?;
            json!({"tables": build_tables_schema(&current.plugins) })
        }
        "transact" => {
            // params: [db, ops]
            let db = params.get(0).and_then(|v| v.as_str()).unwrap_or("OpNonNet");
            let ops = params.get(1).cloned().unwrap_or(json!([]));
            if db != "OpNonNet" {
                json!([{ "error": "unknown db" }])
            } else {
                handle_transact_select(state, ops).await?
            }
        }
        _ => json!({"error": format!("unknown method: {}", method)}),
    };

    Ok(json!({"result": result, "id": id}))
}

fn build_tables_schema(plugins: &HashMap<String, Value>) -> Value {
    let mut tables = simd_json::value::owned::Object::new();
    for (name, val) in plugins {
        if name == "net" {
            continue;
        }
        let columns = infer_columns(val);
        tables.insert(name.clone(), json!({"columns": columns}));
    }
    Value::Object(tables)
}

fn infer_columns(val: &Value) -> Value {
    match val {
        Value::Object(map) => {
            let mut cols = simd_json::value::owned::Object::new();
            for (k, v) in map {
                cols.insert(k.clone(), json!(infer_type(v)));
            }
            Value::Object(cols)
        }
        _ => json!({"value": infer_type(val)}),
    }
}

fn infer_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

async fn handle_transact_select(state: &Arc<StateManager>, ops: Value) -> Result<Value> {
    let mut out = Vec::new();
    let current = state.query_current_state().await?;
    let plugins = &current.plugins;
    if let Some(arr) = ops.as_array() {
        for op in arr {
            let table = op.get("table").and_then(|v| v.as_str()).unwrap_or("");
            if table == "net" {
                out.push(json!({"rows": []}));
                continue;
            }
            let val = plugins.get(table).cloned().unwrap_or(json!(null));
            let rows = rows_from_plugin_value(&val);
            out.push(json!({"rows": rows}));
        }
    }
    Ok(json!(out))
}

fn rows_from_plugin_value(val: &Value) -> Value {
    // Heuristics: if object with a single array field -> rows from that array; if array -> rows = items; else single row.
    match val {
        Value::Object(map) => {
            // find first array member
            if let Some((_, Value::Array(arr))) =
                map.iter().find(|(_, v)| matches!(v, Value::Array(_)))
            {
                let rows: Vec<Value> = arr.clone();
                Value::Array(rows)
            } else {
                Value::Array(vec![val.clone()])
            }
        }
        Value::Array(arr) => Value::Array(arr.clone()),
        _ => Value::Array(vec![val.clone()]),
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/nonnet.rs">
//! NonNet database - OVSDB-like interface for non-network plugin state
//!
//! Provides a read-only, OVSDB-compatible JSON-RPC interface over Unix socket
//! for querying non-network plugin state.

use anyhow::{Context, Result};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

use crate::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse};

const NONNET_DB_NAME: &str = "OpNonNet";

/// NonNet update event
#[derive(Debug, Clone)]
pub struct NonNetUpdate {
    pub db_name: String,
    pub table: String,
    pub rows: Vec<Value>,
}

/// NonNet changed event for watch() method
#[derive(Debug, Clone)]
pub struct NonNetChanged {
    pub key: String,
    pub operation: NonNetOperation,
}

/// NonNet operation type
#[derive(Debug, Clone)]
pub enum NonNetOperation {
    Insert,
    Update,
    Delete,
}

/// NonNet database state
pub struct NonNetDb {
    state: Arc<RwLock<NonNetState>>,
    update_tx: broadcast::Sender<NonNetUpdate>,
    /// Broadcast sender for watch() method
    watch_tx: broadcast::Sender<NonNetChanged>,
}

/// Internal state structure
struct NonNetState {
    tables: HashMap<String, Vec<Value>>,
    schema: Value,
}

fn empty_nonnet_schema() -> Value {
    json!({
        "name": NONNET_DB_NAME,
        "tables": {}
    })
}

impl Default for NonNetState {
    fn default() -> Self {
        Self {
            tables: HashMap::new(),
            schema: empty_nonnet_schema(),
        }
    }
}

impl NonNetDb {
    /// Create a new NonNet database
    pub fn new() -> Self {
        let (update_tx, _) = broadcast::channel(100);
        let (watch_tx, _) = broadcast::channel(100);
        Self {
            state: Arc::new(RwLock::new(NonNetState::default())),
            update_tx,
            watch_tx,
        }
    }

    /// Subscribe to database updates
    pub fn subscribe(&self) -> broadcast::Receiver<NonNetUpdate> {
        self.update_tx.subscribe()
    }

    /// Watch for database changes
    pub fn watch(&self) -> broadcast::Receiver<NonNetChanged> {
        self.watch_tx.subscribe()
    }

    /// Set the tables/schema from plugin state
    pub async fn load_from_plugins(&self, plugins: &HashMap<String, Value>) {
        let mut state = self.state.write().await;

        // Build schema and tables from plugin state
        let mut schema_tables = simd_json::value::owned::Object::new();
        let mut tables = HashMap::new();

        for (name, value) in plugins {
            // Skip network plugin
            if name == "net" {
                continue;
            }

            // Infer columns from the value structure
            let columns = infer_columns(value);
            schema_tables.insert(name.clone(), json!({"columns": columns}));

            // Convert value to rows
            let rows = value_to_rows(value);
            tables.insert(name.clone(), rows.clone());

            // Broadcast initial load as update
            let _ = self.update_tx.send(NonNetUpdate {
                db_name: NONNET_DB_NAME.to_string(),
                table: name.clone(),
                rows: rows.clone(),
            });

            // Fire watch broadcast
            let _ = self.watch_tx.send(NonNetChanged {
                key: name.clone(),
                operation: NonNetOperation::Insert,
            });
        }

        state.schema = json!({
            "name": NONNET_DB_NAME,
            "tables": Value::Object(Box::new(schema_tables))
        });
        state.tables = tables;

        debug!("NonNet DB loaded {} tables", state.tables.len());
    }

    /// Update a specific table
    pub async fn update_table(&self, name: &str, rows: Vec<Value>) {
        let mut state = self.state.write().await;
        state.tables.insert(name.to_string(), rows.clone());

        // Keep schema in sync with updated rows.
        let mut schema_tables = simd_json::value::owned::Object::new();
        for (table_name, table_rows) in state.tables.iter() {
            let columns = infer_columns(&Value::Array(table_rows.clone()));
            schema_tables.insert(table_name.clone(), json!({"columns": columns}));
        }
        state.schema = json!({
            "name": NONNET_DB_NAME,
            "tables": Value::Object(Box::new(schema_tables))
        });

        let _ = self.update_tx.send(NonNetUpdate {
            db_name: NONNET_DB_NAME.to_string(),
            table: name.to_string(),
            rows: rows.clone(),
        });

        // Fire watch broadcast
        let _ = self.watch_tx.send(NonNetChanged {
            key: name.to_string(),
            operation: NonNetOperation::Update,
        });
    }

    /// Insert a new table with rows
    pub async fn insert_table(&self, name: &str, rows: Vec<Value>) {
        let mut state = self.state.write().await;
        state.tables.insert(name.to_string(), rows.clone());

        // Update schema
        let mut schema_tables = simd_json::value::owned::Object::new();
        for (table_name, table_rows) in state.tables.iter() {
            let columns = infer_columns(&Value::Array(table_rows.clone()));
            schema_tables.insert(table_name.clone(), json!({"columns": columns}));
        }
        state.schema = json!({
            "name": NONNET_DB_NAME,
            "tables": Value::Object(Box::new(schema_tables))
        });

        let _ = self.update_tx.send(NonNetUpdate {
            db_name: NONNET_DB_NAME.to_string(),
            table: name.to_string(),
            rows: rows.clone(),
        });

        // Fire watch broadcast
        let _ = self.watch_tx.send(NonNetChanged {
            key: name.to_string(),
            operation: NonNetOperation::Insert,
        });
    }

    /// Delete a table
    pub async fn delete_table(&self, name: &str) {
        let mut state = self.state.write().await;
        state.tables.remove(name);

        // Update schema
        let mut schema_tables = simd_json::value::owned::Object::new();
        for (table_name, table_rows) in state.tables.iter() {
            let columns = infer_columns(&Value::Array(table_rows.clone()));
            schema_tables.insert(table_name.clone(), json!({"columns": columns}));
        }
        state.schema = json!({
            "name": NONNET_DB_NAME,
            "tables": Value::Object(Box::new(schema_tables))
        });

        let _ = self.update_tx.send(NonNetUpdate {
            db_name: NONNET_DB_NAME.to_string(),
            table: name.to_string(),
            rows: vec![],
        });

        // Fire watch broadcast
        let _ = self.watch_tx.send(NonNetChanged {
            key: name.to_string(),
            operation: NonNetOperation::Delete,
        });
    }

    /// Run the JSON-RPC server on a Unix socket
    pub async fn run_server(&self, socket_path: &str) -> Result<()> {
        let path = Path::new(socket_path);

        // Create parent directory if needed
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await.ok();
        }

        // Remove existing socket
        if path.exists() {
            tokio::fs::remove_file(path).await.ok();
        }

        let listener = UnixListener::bind(path).context("Failed to bind NonNet socket")?;

        info!("NonNet JSON-RPC server listening on {}", socket_path);

        loop {
            let (stream, _) = listener.accept().await?;
            let state = Arc::clone(&self.state);

            tokio::spawn(async move {
                if let Err(e) = handle_connection(state, stream).await {
                    warn!("NonNet connection error: {}", e);
                }
            });
        }
    }

    /// Handle a single JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let state = self.state.read().await;
        handle_method(&state, request)
    }
}

impl Default for NonNetDb {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a client connection
async fn handle_connection(state: Arc<RwLock<NonNetState>>, stream: UnixStream) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
            Ok(value) => {
                let state = state.read().await;
                match simd_json::serde::from_owned_value::<JsonRpcRequest>(value.clone()) {
                    Ok(request) => handle_method(&state, request),
                    Err(e) => JsonRpcResponse::error(
                        value.get("id").cloned().unwrap_or(Value::null()),
                        error_codes::INVALID_REQUEST,
                        format!("Invalid request: {}", e),
                    ),
                }
            }
            Err(e) => JsonRpcResponse::error(
                Value::null(),
                error_codes::PARSE_ERROR,
                format!("Parse error: {}", e),
            ),
        };

        let response_str = simd_json::to_string(&response)?;
        writer.write_all(response_str.as_bytes()).await?;
        writer.write_all(b"\n").await?;

        line.clear();
    }

    Ok(())
}

/// Handle a JSON-RPC method call
fn handle_method(state: &NonNetState, request: JsonRpcRequest) -> JsonRpcResponse {
    let result = match request.method.as_str() {
        "list_dbs" => json!([NONNET_DB_NAME]),

        "get_schema" => {
            let db = request
                .params
                .as_array()
                .and_then(|params| params.first())
                .and_then(|v| v.as_str())
                .unwrap_or(NONNET_DB_NAME);

            if db != NONNET_DB_NAME {
                return JsonRpcResponse::error(
                    request.id,
                    error_codes::NOT_FOUND,
                    format!("Unknown database: {}", db),
                );
            }

            state.schema.clone()
        }

        "transact" => {
            // params: [db, ops...]
            let params = request.params.as_array();
            if let Some(params) = params {
                if params.is_empty() {
                    return JsonRpcResponse::error(
                        request.id,
                        error_codes::INVALID_PARAMS,
                        "Missing database name",
                    );
                }

                let db = params[0].as_str().unwrap_or("");
                if db != NONNET_DB_NAME {
                    return JsonRpcResponse::error(
                        request.id,
                        error_codes::NOT_FOUND,
                        format!("Unknown database: {}", db),
                    );
                }

                // Process operations
                let ops = &params[1..];
                let mut results = Vec::new();

                for op in ops {
                    let op_type = op.get("op").and_then(|v| v.as_str()).unwrap_or("");

                    match op_type {
                        "select" => {
                            let table = op.get("table").and_then(|v| v.as_str()).unwrap_or("");
                            let rows = state.tables.get(table).cloned().unwrap_or_default();
                            results.push(json!({"rows": rows}));
                        }
                        "insert" | "update" | "delete" | "mutate" => {
                            // Read-only database
                            results.push(json!({"error": "Read-only database"}));
                        }
                        _ => {
                            results
                                .push(json!({"error": format!("Unknown operation: {}", op_type)}));
                        }
                    }
                }

                json!(results)
            } else {
                json!({"error": "Invalid params"})
            }
        }

        "echo" => request.params.clone(),

        _ => {
            return JsonRpcResponse::error(
                request.id,
                error_codes::METHOD_NOT_FOUND,
                format!("Unknown method: {}", request.method),
            );
        }
    };

    JsonRpcResponse::success(request.id, result)
}

/// Infer column types from a value
fn infer_columns(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut cols = simd_json::value::owned::Object::new();
            for (k, v) in map.iter() {
                cols.insert(k.clone(), json!({"type": infer_type(v)}));
            }
            Value::Object(Box::new(cols))
        }
        Value::Array(arr) => {
            if let Some(first) = arr.first() {
                infer_columns(first)
            } else {
                json!({})
            }
        }
        _ => json!({"value": {"type": infer_type(value)}}),
    }
}

/// Infer the type of a value
fn infer_type(value: &Value) -> &'static str {
    if value.is_null() {
        return "null";
    }
    if value.is_bool() {
        return "boolean";
    }
    if value.is_number() {
        return "integer";
    }
    if value.is_str() {
        return "string";
    }
    if value.is_array() {
        return "set";
    }
    if value.is_object() {
        return "map";
    }
    "unknown"
}

/// Convert a value to table rows
fn value_to_rows(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(arr) => arr.clone(),
        Value::Object(map) => {
            // Check if there's an array field
            for (_, v) in map.iter() {
                if let Value::Array(arr) = v {
                    return arr.clone();
                }
            }
            // Return single row
            vec![value.clone()]
        }
        _ => vec![value.clone()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nonnet_db_creation() {
        let db = NonNetDb::new();
        let mut plugins = HashMap::new();
        plugins.insert(
            "test_plugin".to_string(),
            json!({
                "items": ["item1", "item2"]
            }),
        );

        db.load_from_plugins(&plugins).await;

        let request = JsonRpcRequest::new("list_dbs", json!([]));
        let response = db.handle_request(request).await;

        assert!(response.result.is_some());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/ovsdb_jsonrpc.rs">
//! Direct OVSDB JSON-RPC client - no wrappers, pure native protocol
//! Talks directly to /var/run/openvswitch/db.sock

use anyhow::{Context, Result};
use simd_json::{json, OwnedValue as Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Direct OVSDB JSON-RPC client
pub struct OvsdbClient {
    socket_path: String,
}

impl OvsdbClient {
    /// Connect to OVSDB unix socket
    pub fn new() -> Self {
        Self {
            socket_path: "/var/run/openvswitch/db.sock".to_string(),
        }
    }

    /// Send JSON-RPC request and get response
    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to OVSDB socket")?;

        // Build JSON-RPC request
        let request = json!({
            "method": method,
            "params": params,
            "id": 0
        });

        // Send request
        let request_str = simd_json::to_string(&request)?;
        stream.write_all(request_str.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        // Read response with timeout
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            reader.read_line(&mut response_line),
        )
        .await
        .context("OVSDB response timeout")??;

        let response: Value = simd_json::from_str(&response_line)?;

        // Check for error
        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("OVSDB error: {}", error));
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
        let mut out = simd_json::value::owned::Object::new();
        for (i, name) in order.into_iter().enumerate() {
            let rows = result
                .get(i)
                .and_then(|r| r.get("rows"))
                .cloned()
                .unwrap_or_else(|| json!([]));
            out.insert(name, rows);
        }

        Ok(Value::Object(Box::new(out)))
    }

    /// Transact - execute OVSDB operations
    pub async fn transact(&self, operations: Value) -> Result<Value> {
        let mut params = vec![json!("Open_vSwitch")];
        if let Some(ops_array) = operations.as_array() {
            for op in ops_array {
                params.push(op.clone());
            }
        }
        self.rpc_call("transact", json!(params)).await
    }

    /// Create OVS bridge
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        // Generate UUIDs for bridge and port
        let bridge_uuid = format!("bridge-{}", bridge_name);
        let port_uuid = format!("port-{}", bridge_name);
        let iface_uuid = format!("iface-{}", bridge_name);

        let operations = json!([
            {
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "name": bridge_name,
                    "ports": ["set", [["named-uuid", port_uuid]]]
                },
                "uuid-name": bridge_uuid
            },
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": bridge_name,
                    "interfaces": ["set", [["named-uuid", iface_uuid]]]
                },
                "uuid-name": port_uuid
            },
            {
                "op": "insert",
                "table": "Interface",
                "row": {
                    "name": bridge_name,
                    "type": "internal"
                },
                "uuid-name": iface_uuid
            },
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [
                    ["bridges", "insert", ["set", [["named-uuid", bridge_uuid]]]]
                ]
            }
        ]);

        self.transact(operations).await?;
        Ok(())
    }

    /// Add port to bridge
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        // First, find the bridge UUID
        let bridge_uuid = self.find_bridge_uuid(bridge_name).await?;

        let port_uuid = format!("port-{}", port_name);
        let iface_uuid = format!("iface-{}", port_name);

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
                "row": {
                    "name": port_name
                },
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
}

impl Default for OvsdbClient {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/ovsdb_rpc_call.rs">
async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to OVSDB socket")?;

        let request = json!({
            "method": method,
            "params": params,
            "id": 0
        });

        let request_str = simd_json::to_string(&request)?;
        debug!("OVSDB request: {}", request_str);

        stream.write_all(request_str.as_bytes()).await?;
        stream.write_all(b"\n").await?;

        let mut response_buf = Vec::new();
        tokio::time::timeout(self.timeout, tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response_buf))
            .await
            .context("OVSDB response timeout")??;

        let mut response_str = String::from_utf8(response_buf)?;
        debug!("OVSDB response: {}", response_str.trim());

        let response: Value = unsafe { simd_json::from_str(&mut response_str)? };

        if let Some(error) = response.get("error") {
            if !error.is_null() {
                return Err(anyhow::anyhow!("OVSDB error: {}", error));
            }
        }

        Ok(response["result"].clone())
    }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/ovsdb.rs">
//! OVSDB JSON-RPC client for Open vSwitch integration
//!
//! Direct JSON-RPC client for /var/run/openvswitch/db.sock

use anyhow::{Context, Result};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, info};

/// OVSDB JSON-RPC client
pub struct OvsdbClient {
    socket_path: String,
    timeout: Duration,
}

impl OvsdbClient {
    /// Create a new OVSDB client with default socket path
    pub fn new() -> Self {
        Self {
            socket_path: "/var/run/openvswitch/db.sock".to_string(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Create with a custom socket path
    pub fn with_socket(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Set timeout for RPC calls
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Send a JSON-RPC request and get response
    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to OVSDB socket")?;

        let request = json!({
            "method": method,
            "params": params,
            "id": 0
        });

        let request_str = simd_json::to_string(&request)?;
        debug!("OVSDB request: {}", request_str);

        stream.write_all(request_str.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        // Signal request completion. OVSDB may not newline-terminate responses, so
        // line-oriented reads can block until timeout.
        stream.shutdown().await?;

        let mut response_bytes = Vec::new();
        tokio::time::timeout(self.timeout, stream.read_to_end(&mut response_bytes))
            .await
            .context("OVSDB response timeout")??;

        if response_bytes.is_empty() {
            return Err(anyhow::anyhow!("OVSDB returned empty response"));
        }

        let response_text =
            String::from_utf8(response_bytes).context("OVSDB response was not valid UTF-8")?;
        debug!("OVSDB response: {}", response_text.trim());
        let response: Value = Self::parse_json_response(&response_text)?;

        if let Some(error) = response.get("error") {
            if !error.is_null() {
                return Err(anyhow::anyhow!("OVSDB error: {}", error));
            }
        }

        Ok(response["result"].clone())
    }

    fn parse_json_response(response_text: &str) -> Result<Value> {
        let trimmed = response_text.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("OVSDB response contained only whitespace"));
        }

        // First try parsing the full payload.
        let mut payload = trimmed.to_string();
        if let Ok(value) = unsafe { simd_json::from_str::<Value>(payload.as_mut_str()) } {
            return Ok(value);
        }

        // Some servers can emit multiple lines; fall back to the last valid JSON line.
        for line in trimmed.lines().rev() {
            let candidate = line.trim();
            if candidate.is_empty() {
                continue;
            }

            let mut owned = candidate.to_string();
            if let Ok(value) = unsafe { simd_json::from_str::<Value>(owned.as_mut_str()) } {
                return Ok(value);
            }
        }

        Err(anyhow::anyhow!(
            "Failed to parse OVSDB JSON response payload"
        ))
    }

    /// List all databases
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        let result = self.rpc_call("list_dbs", json!([])).await?;
        Ok(simd_json::serde::from_owned_value(result)?)
    }

    /// Get schema for a database
    pub async fn get_schema(&self, db: &str) -> Result<Value> {
        self.rpc_call("get_schema", json!([db])).await
    }

    /// Execute a transaction
    pub async fn transact(&self, db: &str, operations: Value) -> Result<Value> {
        let mut params = vec![json!(db)];
        if let Some(ops_array) = operations.as_array() {
            for op in ops_array {
                params.push(op.clone());
            }
        }
        let result = self.rpc_call("transact", json!(params)).await?;

        // OVSDB can return per-operation errors inside the result array.
        if let Some(results) = result.as_array() {
            for (idx, op_result) in results.iter().enumerate() {
                if let Some(error) = op_result.get("error").and_then(|e| e.as_str()) {
                    let details = op_result
                        .get("details")
                        .and_then(|d| d.as_str())
                        .unwrap_or("no details");
                    return Err(anyhow::anyhow!(
                        "OVSDB operation {} failed: {} ({})",
                        idx,
                        error,
                        details
                    ));
                }
            }
        }

        Ok(result)
    }

    /// Create a bridge
    pub async fn create_bridge(&self, name: &str) -> Result<()> {
        let safe_name = Self::sanitize_ref(name);
        let bridge_uuid = format!("bridge_{}", safe_name);
        let port_uuid = format!("port_{}", safe_name);
        let iface_uuid = format!("iface_{}", safe_name);

        let operations = json!([
            {
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "name": name,
                    "ports": ["set", [["named-uuid", port_uuid]]]
                },
                "uuid-name": bridge_uuid
            },
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": name,
                    "interfaces": ["set", [["named-uuid", iface_uuid]]]
                },
                "uuid-name": port_uuid
            },
            {
                "op": "insert",
                "table": "Interface",
                "row": {
                    "name": name,
                    "type": "internal"
                },
                "uuid-name": iface_uuid
            },
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [
                    ["bridges", "insert", ["set", [["named-uuid", bridge_uuid]]]]
                ]
            }
        ]);

        self.transact("Open_vSwitch", operations).await?;
        info!("Created OVS bridge: {}", name);
        Ok(())
    }

    /// Delete a bridge
    pub async fn delete_bridge(&self, name: &str) -> Result<()> {
        let bridge_uuid = self.find_bridge_uuid(name).await?;

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

        self.transact("Open_vSwitch", operations).await?;
        info!("Deleted OVS bridge: {}", name);
        Ok(())
    }

    /// Add a port to a bridge
    pub async fn add_port(&self, bridge: &str, port: &str) -> Result<()> {
        let bridge_uuid = self.find_bridge_uuid(bridge).await?;
        let existing_ports = self.list_ports(bridge).await.unwrap_or_default();
        if existing_ports.iter().any(|p| p == port) {
            info!("Port {} already attached to bridge {}", port, bridge);
            return Ok(());
        }

        let existing_port_uuid = self.find_named_row_uuid("Port", port).await.ok();
        let existing_iface_uuid = self.find_named_row_uuid("Interface", port).await.ok();
        let safe_port = Self::sanitize_ref(port);
        let port_ref = format!("port_{}", safe_port);
        let iface_ref = format!("iface_{}", safe_port);

        let operations = if let Some(port_uuid) = existing_port_uuid {
            // Port exists but is not attached to this bridge yet.
            json!([
                {
                    "op": "mutate",
                    "table": "Bridge",
                    "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
                    "mutations": [
                        ["ports", "insert", ["set", [["uuid", &port_uuid]]]]
                    ]
                }
            ])
        } else if let Some(iface_uuid) = existing_iface_uuid {
            // Interface exists; create only Port row and attach it.
            json!([
                {
                    "op": "insert",
                    "table": "Port",
                    "row": {
                        "name": port,
                        "interfaces": ["set", [["uuid", &iface_uuid]]]
                    },
                    "uuid-name": port_ref
                },
                {
                    "op": "mutate",
                    "table": "Bridge",
                    "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
                    "mutations": [
                        ["ports", "insert", ["set", [["named-uuid", &port_ref]]]]
                    ]
                }
            ])
        } else {
            // Fresh system port.
            json!([
                {
                    "op": "insert",
                    "table": "Port",
                    "row": {
                        "name": port,
                        "interfaces": ["set", [["named-uuid", &iface_ref]]]
                    },
                    "uuid-name": port_ref
                },
                {
                    "op": "insert",
                    "table": "Interface",
                    "row": {
                        "name": port,
                        "type": "system"
                    },
                    "uuid-name": iface_ref
                },
                {
                    "op": "mutate",
                    "table": "Bridge",
                    "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
                    "mutations": [
                        ["ports", "insert", ["set", [["named-uuid", &port_ref]]]]
                    ]
                }
            ])
        };

        self.transact("Open_vSwitch", operations).await?;
        info!("Added port {} to bridge {}", port, bridge);
        Ok(())
    }

    /// List all bridges
    pub async fn list_bridges(&self) -> Result<Vec<String>> {
        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [],
            "columns": ["name"]
        }]);

        let result = self.transact("Open_vSwitch", operations).await?;

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

    /// List ports on a bridge
    pub async fn list_ports(&self, bridge: &str) -> Result<Vec<String>> {
        let bridge_uuid = self.find_bridge_uuid(bridge).await?;

        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
            "columns": ["ports"]
        }]);

        let result = self.transact("Open_vSwitch", operations).await?;

        let mut port_uuids = Vec::new();
        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                port_uuids = Self::extract_uuid_set(&first_row["ports"]);
            }
        }

        // Get port names
        let mut port_names = Vec::new();
        for port_uuid in port_uuids {
            let ops = json!([{
                "op": "select",
                "table": "Port",
                "where": [["_uuid", "==", ["uuid", &port_uuid]]],
                "columns": ["name"]
            }]);

            let result = self.transact("Open_vSwitch", ops).await?;
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

    /// Check if a bridge exists
    pub async fn bridge_exists(&self, name: &str) -> Result<bool> {
        match self.find_bridge_uuid(name).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get bridge info
    pub async fn get_bridge_info(&self, name: &str) -> Result<Value> {
        let bridge_uuid = self.find_bridge_uuid(name).await?;

        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
            "columns": []
        }]);

        let result = self.transact("Open_vSwitch", operations).await?;
        Ok(result[0]["rows"][0].clone())
    }

    /// Dump entire database
    pub async fn dump_db(&self, db: &str) -> Result<Value> {
        let schema = self.get_schema(db).await?;
        let tables = schema
            .get("tables")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("Invalid schema: missing tables"))?;

        let table_names: Vec<String> = tables.keys().cloned().collect();
        let mut out = simd_json::value::owned::Object::new();

        for name in table_names {
            // Select each table independently so one failure doesn't abort the whole dump.
            let result = self
                .rpc_call(
                    "transact",
                    json!([db, {"op": "select", "table": name, "where": []}]),
                )
                .await;
            let rows = match result {
                Ok(r) => r
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|r| r.get("rows"))
                    .cloned()
                    .unwrap_or_else(|| json!([])),
                Err(e) => {
                    tracing::warn!("dump_db: skipping table {}: {}", name, e);
                    json!([])
                }
            };
            out.insert(name, rows);
        }

        Ok(Value::Object(Box::new(out)))
    }

    /// Monitor a database for changes
    pub async fn monitor_db(&self, db: &str) -> Result<tokio::sync::mpsc::Receiver<Value>> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to OVSDB socket for monitoring")?;

        let schema = self.get_schema(db).await?;
        let tables = schema
            .get("tables")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("Invalid schema: missing tables"))?;

        let mut monitor_requests = simd_json::value::owned::Object::new();
        for (name, _) in tables {
            monitor_requests.insert(
                name.clone(),
                json!({
                    "columns": [], // All columns
                    "select": {
                        "initial": true,
                        "insert": true,
                        "delete": true,
                        "modify": true
                    }
                }),
            );
        }

        let request = json!({
            "method": "monitor",
            "params": [db, null, Value::Object(Box::new(monitor_requests))],
            "id": "monitor"
        });

        let request_str = simd_json::to_string(&request)?;
        stream.write_all(request_str.as_bytes()).await?;
        stream.write_all(b"\n").await?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line).await {
                if n == 0 {
                    break;
                }

                let mut line_clone = line.clone();
                if let Ok(update) = unsafe { simd_json::from_str::<Value>(line_clone.as_mut_str()) }
                {
                    if let Some(method) = update.get("method").and_then(|m| m.as_str()) {
                        if method == "update" && tx.send(update).await.is_err() {
                            break;
                        }
                    }
                }
                line.clear();
            }
        });

        Ok(rx)
    }

    /// Find bridge UUID by name
    async fn find_bridge_uuid(&self, name: &str) -> Result<String> {
        let operations = json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", name]],
            "columns": ["_uuid"]
        }]);

        let result = self.transact("Open_vSwitch", operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(uuid_array) = first_row["_uuid"].as_array() {
                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                        return Ok(uuid_array[1].as_str().unwrap().to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Bridge '{}' not found", name))
    }

    fn sanitize_ref(input: &str) -> String {
        input
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect()
    }

    fn extract_uuid_set(value: &Value) -> Vec<String> {
        // RFC7047 allows set columns to be encoded either as ["set", [...]]
        // or directly as a single atom (e.g. ["uuid", "..."]).
        if let Some(as_set) = value.as_array() {
            if as_set.len() == 2 && as_set[0] == "set" {
                if let Some(items) = as_set[1].as_array() {
                    return items
                        .iter()
                        .filter_map(Self::extract_uuid_atom)
                        .collect::<Vec<_>>();
                }
            }
        }
        Self::extract_uuid_atom(value).into_iter().collect()
    }

    fn extract_uuid_atom(value: &Value) -> Option<String> {
        let arr = value.as_array()?;
        if arr.len() == 2 && (arr[0] == "uuid" || arr[0] == "named-uuid") {
            return arr[1].as_str().map(|s| s.to_string());
        }
        None
    }

    async fn find_named_row_uuid(&self, table: &str, name: &str) -> Result<String> {
        let operations = json!([{
            "op": "select",
            "table": table,
            "where": [["name", "==", name]],
            "columns": ["_uuid"]
        }]);

        let result = self.transact("Open_vSwitch", operations).await?;
        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(uuid) = Self::extract_uuid_atom(&first_row["_uuid"]) {
                    return Ok(uuid);
                }
            }
        }
        Err(anyhow::anyhow!("{} '{}' not found", table, name))
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
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::net::UnixListener;

    #[test]
    fn parse_json_response_accepts_plain_json() {
        let parsed =
            OvsdbClient::parse_json_response(r#"{"result":["Open_vSwitch"],"error":null,"id":0}"#)
                .expect("parse response");
        assert_eq!(parsed["id"], 0);
        assert_eq!(parsed["result"][0], "Open_vSwitch");
    }

    #[test]
    fn parse_json_response_falls_back_to_last_valid_line() {
        let parsed = OvsdbClient::parse_json_response(
            "noise line\n{\"result\":[\"Open_vSwitch\"],\"error\":null,\"id\":0}\n",
        )
        .expect("parse response");
        assert_eq!(parsed["result"][0], "Open_vSwitch");
    }

    #[test]
    fn extract_uuid_set_supports_singleton_atom() {
        let value = json!(["uuid", "abc"]);
        let uuids = OvsdbClient::extract_uuid_set(&value);
        assert_eq!(uuids, vec!["abc".to_string()]);
    }

    #[test]
    fn extract_uuid_set_supports_set_encoding() {
        let value = json!(["set", [["uuid", "a"], ["uuid", "b"]]]);
        let uuids = OvsdbClient::extract_uuid_set(&value);
        assert_eq!(uuids, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn rpc_call_handles_response_without_trailing_newline() {
        let socket_path = unique_test_socket_path();
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).expect("bind unix listener");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request_buf = [0_u8; 1024];
            let _ = socket.read(&mut request_buf).await.expect("read request");

            let response = r#"{"result":["Open_vSwitch","_Server"],"error":null,"id":0}"#;
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
            socket.shutdown().await.expect("shutdown socket");
        });

        let client = OvsdbClient::with_socket(socket_path.to_string_lossy().to_string())
            .with_timeout(Duration::from_secs(2));
        let dbs = client.list_dbs().await.expect("list dbs");
        assert_eq!(dbs, vec!["Open_vSwitch".to_string(), "_Server".to_string()]);

        server.await.expect("server task");
        let _ = std::fs::remove_file(&socket_path);
    }

    fn unique_test_socket_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("op-jsonrpc-ovsdb-{}.sock", uuid::Uuid::new_v4()));
        path
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/protocol.rs">
//! JSON-RPC 2.0 protocol types

use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: Value,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
            id: Value::from(0),
        }
    }

    /// Create with a specific ID
    pub fn with_id(method: impl Into<String>, params: Value, id: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
            id,
        }
    }
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }

    /// Create an error response with data
    pub fn error_with_data(id: Value, code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
            id,
        }
    }
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Standard JSON-RPC error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Custom error codes (application-specific)
    pub const DATABASE_ERROR: i32 = -32000;
    pub const CONNECTION_ERROR: i32 = -32001;
    pub const NOT_FOUND: i32 = -32002;
    pub const PERMISSION_DENIED: i32 = -32003;
}

/// Parse a JSON-RPC request from a JSON value
#[allow(clippy::result_large_err)]
pub fn parse_request(value: Value) -> Result<JsonRpcRequest, JsonRpcResponse> {
    simd_json::serde::from_owned_value(value.clone()).map_err(|e| {
        JsonRpcResponse::error(
            value.get("id").cloned().unwrap_or(Value::null()),
            error_codes::INVALID_REQUEST,
            format!("Invalid request: {}", e),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = JsonRpcRequest::new("test", simd_json::json!(["arg1", "arg2"]));
        let json = simd_json::to_string(&req).unwrap();
        assert!(json.contains("\"method\":\"test\""));
    }

    #[test]
    fn test_response_serialization() {
        let resp = JsonRpcResponse::success(Value::from(1), simd_json::json!({"ok": true}));
        let json = simd_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/src/server.rs">
//! JSON-RPC server implementation
//!
//! Provides a unified JSON-RPC server that can handle multiple backends:
//! - NonNet database
//! - OVSDB proxy
//! - Custom handlers

use anyhow::{Context, Result};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::nonnet::NonNetDb;
use crate::ovsdb::OvsdbClient;
use crate::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse};

/// Handler function type
pub type HandlerFn = Box<dyn Fn(JsonRpcRequest) -> JsonRpcResponse + Send + Sync>;

/// JSON-RPC server configuration
#[derive(Clone)]
pub struct JsonRpcServerConfig {
    /// Unix socket path (optional)
    pub unix_socket: Option<String>,
    /// TCP address (optional)
    pub tcp_addr: Option<String>,
    /// Enable OVSDB proxy
    pub ovsdb_enabled: bool,
    /// Enable NonNet database
    pub nonnet_enabled: bool,
}

impl Default for JsonRpcServerConfig {
    fn default() -> Self {
        Self {
            unix_socket: Some("/var/run/op-dbus/jsonrpc.sock".to_string()),
            tcp_addr: None,
            ovsdb_enabled: true,
            nonnet_enabled: true,
        }
    }
}

/// JSON-RPC server
pub struct JsonRpcServer {
    config: JsonRpcServerConfig,
    nonnet: Option<Arc<NonNetDb>>,
    handlers: Arc<RwLock<HashMap<String, HandlerFn>>>,
}

impl JsonRpcServer {
    /// Create a new JSON-RPC server
    pub fn new(config: JsonRpcServerConfig) -> Self {
        let nonnet = if config.nonnet_enabled {
            Some(Arc::new(NonNetDb::new()))
        } else {
            None
        };

        Self {
            config,
            nonnet,
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(JsonRpcServerConfig::default())
    }

    /// Get reference to NonNet database
    pub fn nonnet(&self) -> Option<Arc<NonNetDb>> {
        self.nonnet.clone()
    }

    /// Register a custom handler
    pub async fn register_handler(&self, method: &str, handler: HandlerFn) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(method.to_string(), handler);
    }

    /// Run the server
    pub async fn run(self: Arc<Self>) -> Result<()> {
        let mut handles = Vec::new();

        // Start Unix socket server
        if let Some(ref socket_path) = self.config.unix_socket {
            let server = Arc::clone(&self);
            let path = socket_path.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = server.run_unix(&path).await {
                    error!("Unix socket server error: {}", e);
                }
            }));
        }

        // Start TCP server
        if let Some(ref addr) = self.config.tcp_addr {
            let server = Arc::clone(&self);
            let addr = addr.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = server.run_tcp(&addr).await {
                    error!("TCP server error: {}", e);
                }
            }));
        }

        // Wait for all servers
        for handle in handles {
            handle.await?;
        }

        Ok(())
    }

    /// Run Unix socket server
    async fn run_unix(&self, socket_path: &str) -> Result<()> {
        let path = Path::new(socket_path);

        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await.ok();
        }

        if path.exists() {
            tokio::fs::remove_file(path).await.ok();
        }

        let listener = UnixListener::bind(path).context("Failed to bind Unix socket")?;

        info!("JSON-RPC server listening on unix:{}", socket_path);

        loop {
            let (stream, _) = listener.accept().await?;
            let server = self.clone_for_connection();

            tokio::spawn(async move {
                if let Err(e) = server.handle_unix_connection(stream).await {
                    debug!("Connection error: {}", e);
                }
            });
        }
    }

    /// Run TCP server
    async fn run_tcp(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr)
            .await
            .context("Failed to bind TCP socket")?;

        info!("JSON-RPC server listening on tcp:{}", addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let server = self.clone_for_connection();

            tokio::spawn(async move {
                if let Err(e) = server.handle_tcp_connection(stream).await {
                    debug!("Connection error: {}", e);
                }
            });
        }
    }

    /// Clone server state for a new connection
    fn clone_for_connection(&self) -> JsonRpcServerConnection {
        JsonRpcServerConnection {
            config: self.config.clone(),
            nonnet: self.nonnet.clone(),
            handlers: Arc::clone(&self.handlers),
        }
    }
}

/// Server state for a single connection
struct JsonRpcServerConnection {
    config: JsonRpcServerConfig,
    nonnet: Option<Arc<NonNetDb>>,
    handlers: Arc<RwLock<HashMap<String, HandlerFn>>>,
}

impl JsonRpcServerConnection {
    /// Handle Unix socket connection
    async fn handle_unix_connection(&self, stream: UnixStream) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            let response = self.process_line(&mut line).await;
            let response_str = simd_json::to_string(&response)?;
            writer.write_all(response_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            line.clear();
        }

        Ok(())
    }

    /// Handle TCP connection
    async fn handle_tcp_connection(&self, stream: TcpStream) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            let response = self.process_line(&mut line).await;
            let response_str = simd_json::to_string(&response)?;
            writer.write_all(response_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            line.clear();
        }

        Ok(())
    }

    /// Process a JSON-RPC request line
    async fn process_line(&self, line: &mut String) -> JsonRpcResponse {
        match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
            Ok(value) => {
                match simd_json::serde::from_owned_value::<JsonRpcRequest>(value.clone()) {
                    Ok(request) => self.handle_request(request).await,
                    Err(e) => JsonRpcResponse::error(
                        value.get("id").cloned().unwrap_or(Value::null()),
                        error_codes::INVALID_REQUEST,
                        format!("Invalid request: {}", e),
                    ),
                }
            }
            Err(e) => JsonRpcResponse::error(
                Value::null(),
                error_codes::PARSE_ERROR,
                format!("Parse error: {}", e),
            ),
        }
    }

    /// Handle a JSON-RPC request
    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let method = &request.method;

        // Check custom handlers first
        {
            let handlers = self.handlers.read().await;
            if let Some(handler) = handlers.get(method) {
                return handler(request);
            }
        }

        // Built-in methods
        match method.as_str() {
            // NonNet database methods
            "list_dbs" | "get_schema" | "transact" if self.config.nonnet_enabled => {
                if let Some(ref nonnet) = self.nonnet {
                    return nonnet.handle_request(request).await;
                }
            }

            // OVSDB proxy methods
            "ovsdb.list_dbs" | "ovsdb.get_schema" | "ovsdb.transact"
                if self.config.ovsdb_enabled =>
            {
                return self.handle_ovsdb_request(request).await;
            }

            // Server info
            "server.info" => {
                return JsonRpcResponse::success(
                    request.id,
                    json!({
                        "name": "op-dbus-v2 JSON-RPC Server",
                        "version": env!("CARGO_PKG_VERSION"),
                        "ovsdb_enabled": self.config.ovsdb_enabled,
                        "nonnet_enabled": self.config.nonnet_enabled,
                    }),
                );
            }

            // Echo for testing
            "echo" => {
                return JsonRpcResponse::success(request.id, request.params);
            }

            _ => {}
        }

        JsonRpcResponse::error(
            request.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Unknown method: {}", method),
        )
    }

    /// Handle OVSDB proxy request
    async fn handle_ovsdb_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let client = OvsdbClient::new();

        let result = match request.method.as_str() {
            "ovsdb.list_dbs" => match client.list_dbs().await {
                Ok(dbs) => json!(dbs),
                Err(e) => {
                    return JsonRpcResponse::error(
                        request.id,
                        error_codes::DATABASE_ERROR,
                        e.to_string(),
                    )
                }
            },
            "ovsdb.get_schema" => {
                let db = request
                    .params
                    .as_array()
                    .and_then(|a| a.get(0))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Open_vSwitch");
                match client.get_schema(db).await {
                    Ok(schema) => schema,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            request.id,
                            error_codes::DATABASE_ERROR,
                            e.to_string(),
                        )
                    }
                }
            }
            "ovsdb.transact" => {
                let params = request.params.as_array();
                if let Some(params) = params {
                    if params.len() < 2 {
                        return JsonRpcResponse::error(
                            request.id,
                            error_codes::INVALID_PARAMS,
                            "Missing database or operations",
                        );
                    }
                    let db = params[0].as_str().unwrap_or("Open_vSwitch");
                    let ops = json!(params[1..].to_vec());
                    match client.transact(db, ops).await {
                        Ok(result) => result,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                request.id,
                                error_codes::DATABASE_ERROR,
                                e.to_string(),
                            )
                        }
                    }
                } else {
                    return JsonRpcResponse::error(
                        request.id,
                        error_codes::INVALID_PARAMS,
                        "Invalid params",
                    );
                }
            }
            _ => {
                return JsonRpcResponse::error(
                    request.id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("Unknown method: {}", request.method),
                );
            }
        };

        JsonRpcResponse::success(request.id, result)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/Cargo.toml">
[package]
name = "op-jsonrpc"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "JSON-RPC server with OVSDB and NonNet database support for op-dbus-v2"

[dependencies]
op-core = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/compare-op-jsonrpc.md">
# compare-op-jsonrpc

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 8 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 0 |
| Spec-listed source files | 0 |
| Spec-listed but missing | 0 |
| Extra implementation files | 8 |

## Current Implementation Overview

- JSON-RPC server with OVSDB and NonNet database support for op-dbus-v2
- Internal crate integrations: op-core.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `root` | ✅ Present | root source group | src/lib.rs, src/nonnet.rs, src/nonnet_staging.rs, src/ovsdb.rs, src/ovsdb_jsonrpc.rs, src/ovsdb_rpc_call.rs, src/protocol.rs, src/server.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| Architecture | ❌ Missing | no clear source match for SPEC.md | SPEC.md |
| Key Components | ❌ Missing | no clear source match for SPEC.md | SPEC.md |
| Module Structure | ✅ Implemented | src/nonnet.rs | SPEC.md |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - not listed in SPEC dependency block

### External Runtime Dependencies
- `tokio` - not listed in SPEC dependency block
- `serde` - not listed in SPEC dependency block
- `simd-json` - not listed in SPEC dependency block
- `anyhow` - not listed in SPEC dependency block
- `thiserror` - not listed in SPEC dependency block
- `tracing` - not listed in SPEC dependency block
- `uuid` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 8 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: nonnet, ovsdb, protocol, server.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-jsonrpc/SPEC.md">
# op-jsonrpc - Specification

## Overview
**Crate**: `op-jsonrpc`  
**Location**: `crates/op-jsonrpc`  
**Description**: JSON-RPC server with OVSDB and NonNet database support for op-dbus-v2

## Purpose

The `op-jsonrpc` crate provides a comprehensive JSON-RPC 2.0 server implementation with specialized support for Open vSwitch Database (OVSDB) integration and NonNet database management. It serves as the RPC communication layer for the operation-dbus system, enabling:

- **JSON-RPC 2.0 Protocol**: Standards-compliant RPC server over Unix sockets
- **OVSDB Integration**: Client for Open vSwitch database operations
- **NonNet Database**: State management for non-network plugins
- **Async Operations**: Non-blocking I/O with tokio runtime

This crate is essential for:
- Plugin communication via JSON-RPC
- Network configuration through OVSDB
- State persistence for non-network components
- Inter-process communication within operation-dbus

## Architecture

### Protocol Layer
Implements JSON-RPC 2.0 specification:
- Request/response message types
- Error handling with standard error codes
- Batch request support
- Notification messages (no response expected)

### Server Layer
Unix socket-based JSON-RPC server:
- Async request handling
- Method routing and dispatch
- Connection management
- Error recovery

### Database Integrations

#### OVSDB Client
Connects to Open vSwitch database for network operations:
- Schema introspection
- Transactional operations
- Monitor/update subscriptions
- Bridge and port management

#### NonNet Database
Custom database for non-network plugin state:
- Key-value storage
- Plugin configuration persistence
- State synchronization
- Query and update operations

## Key Components

### JsonRpcRequest
Represents a JSON-RPC 2.0 request.

```rust
pub struct JsonRpcRequest {
    pub jsonrpc: String,    // Protocol version ("2.0")
    pub method: String,     // Method name to invoke
    pub params: Value,      // Method parameters (JSON)
    pub id: Value,          // Request identifier
}
```

**Constructors**:
```rust
// Create with auto-generated ID
JsonRpcRequest::new("method_name", params)

// Create with specific ID
JsonRpcRequest::with_id("method_name", params, id)
```

### JsonRpcResponse
Represents a JSON-RPC 2.0 response.

```rust
pub struct JsonRpcResponse {
    pub jsonrpc: String,              // Protocol version ("2.0")
    pub result: Option<Value>,        // Success result
    pub error: Option<JsonRpcError>,  // Error details
    pub id: Value,                    // Request ID
}
```

**Constructors**:
```rust
// Success response
JsonRpcResponse::success(id, result)

// Error response
JsonRpcResponse::error(id, code, message)

// Error with additional data
JsonRpcResponse::error_with_data(id, code, message, data)
```

### JsonRpcError
Standard JSON-RPC error structure.

```rust
pub struct JsonRpcError {
    pub code: i32,           // Error code
    pub message: String,     // Error message
    pub data: Option<Value>, // Additional error data
}
```

**Standard Error Codes**:
- `-32700`: Parse error
- `-32600`: Invalid request
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error
- `-32000 to -32099`: Server-defined errors

### JsonRpcServer
Async JSON-RPC server over Unix sockets.

```rust
pub struct JsonRpcServer {
    // Server configuration and state
}
```

**Key Methods**:
- `new(socket_path)`: Create server bound to Unix socket
- `register_method(name, handler)`: Register RPC method handler
- `run()`: Start server event loop
- `shutdown()`: Graceful shutdown

### OvsdbClient
Client for Open vSwitch database operations.

```rust
pub struct OvsdbClient {
    // OVSDB connection and state
}
```

**Key Operations**:
- `connect(socket)`: Connect to OVSDB server
- `list_dbs()`: List available databases
- `get_schema(db)`: Retrieve database schema
- `transact(operations)`: Execute transactional operations
- `monitor(db, tables)`: Subscribe to table updates

### NonNetDb
Database for non-network plugin state.

```rust
pub struct NonNetDb {
    // Database state and storage
}
```

**Key Operations**:
- `new(path)`: Create/open database at path
- `get(key)`: Retrieve value by key
- `set(key, value)`: Store key-value pair
- `delete(key)`: Remove key
- `list()`: List all keys
- `query(filter)`: Query with filter criteria

## Module Structure

### Core Modules
- **protocol**: JSON-RPC 2.0 message types and builders
- **server**: Unix socket server implementation
- **ovsdb**: OVSDB client and operations
- **nonnet**: NonNet database implementation

### Supporting Modules
- **ovsdb_jsonrpc**: OVSDB-specific JSON-RPC extensions
- **nonnet_staging**: Staging area for NonNet operations

## Dependencies

### Core Dependencies
- **op-core**: Core types and utilities
- **tokio**: Async runtime and I/O
- **serde**: Serialization framework
- **simd-json**: High-performance JSON parsing
- **uuid**: Request ID generation

### Error Handling
- **anyhow**: Flexible error handling
- **thiserror**: Custom error types

### Logging
- **tracing**: Structured logging and diagnostics

## Usage

### Starting a JSON-RPC Server

```rust
use op_jsonrpc::{JsonRpcServer, JsonRpcRequest, JsonRpcResponse};
use simd_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create server
    let mut server = JsonRpcServer::new("/tmp/op-jsonrpc.sock").await?;
    
    // Register method handler
    server.register_method("echo", |req: JsonRpcRequest| async move {
        JsonRpcResponse::success(req.id, req.params)
    });
    
    // Run server
    server.run().await?;
    
    Ok(())
}
```

### OVSDB Integration

```rust
use op_jsonrpc::OvsdbClient;

// Connect to OVSDB
let mut client = OvsdbClient::connect("/var/run/openvswitch/db.sock").await?;

// List databases
let dbs = client.list_dbs().await?;
println!("Available databases: {:?}", dbs);

// Get schema
let schema = client.get_schema("Open_vSwitch").await?;

// Execute transaction
let result = client.transact("Open_vSwitch", vec![
    // Transaction operations
]).await?;
```

### NonNet Database Operations

```rust
use op_jsonrpc::NonNetDb;
use simd_json::json;

// Open database
let db = NonNetDb::new("/var/lib/op-dbus/nonnet.db").await?;

// Store plugin state
db.set("plugin.my-service.config", json!({
    "enabled": true,
    "port": 8080
})).await?;

// Retrieve state
let config = db.get("plugin.my-service.config").await?;

// Query with filter
let all_configs = db.query("plugin.*.config").await?;
```

### Method Registration

```rust
// Register multiple methods
server.register_method("add", |req| async move {
    let params = req.params.as_array().unwrap();
    let a = params[0].as_i64().unwrap();
    let b = params[1].as_i64().unwrap();
    JsonRpcResponse::success(req.id, json!(a + b))
});

server.register_method("get_status", |req| async move {
    JsonRpcResponse::success(req.id, json!({
        "status": "running",
        "uptime": 12345
    }))
});
```

## Protocol Compliance

### JSON-RPC 2.0 Specification
- ✅ Request/response format
- ✅ Error handling
- ✅ Batch requests
- ✅ Notifications
- ✅ Standard error codes

### OVSDB Protocol
- ✅ RFC 7047 compliance
- ✅ Transactional operations
- ✅ Monitor protocol
- ✅ Schema introspection

## Integration Points

### Operation-DBUS Architecture
```
D-Bus Services
     ↓
JSON-RPC Server (op-jsonrpc)
     ↓
├── OVSDB Client → Open vSwitch
└── NonNet DB → Plugin State
```

### Plugin Communication
Plugins communicate with core services via JSON-RPC:
1. Plugin connects to Unix socket
2. Sends JSON-RPC requests
3. Receives responses
4. Handles notifications

### Network Configuration
OVSDB integration enables:
- Bridge creation and management
- Port configuration
- Flow table operations
- Network topology queries

## Performance Considerations

### JSON Parsing
- **simd-json**: SIMD-accelerated parsing for high throughput
- **Zero-copy**: Minimize allocations where possible

### Connection Handling
- **Async I/O**: Non-blocking operations via tokio
- **Connection Pooling**: Reuse connections to OVSDB
- **Backpressure**: Handle slow clients gracefully

### Database Operations
- **Batching**: Group NonNet operations for efficiency
- **Caching**: Cache frequently accessed state
- **Indexing**: Optimize query performance

## Error Handling

### Protocol Errors
- Parse errors (malformed JSON)
- Invalid request structure
- Method not found
- Invalid parameters

### Database Errors
- OVSDB connection failures
- Transaction conflicts
- NonNet storage errors
- Schema validation failures

### Recovery Strategies
- Automatic reconnection to OVSDB
- Transaction retry with backoff
- Graceful degradation on errors
- Detailed error logging

## Testing

### Unit Tests
- Protocol message serialization
- Error response generation
- Method routing logic

### Integration Tests
- End-to-end RPC communication
- OVSDB transaction handling
- NonNet database operations

### Mock Support
- Mock OVSDB server for testing
- In-memory NonNet database
- Simulated network conditions

## Security Considerations

### Unix Socket Permissions
- Restrict socket file permissions
- Validate client credentials
- Rate limiting per connection

### Input Validation
- Validate all RPC parameters
- Sanitize database queries
- Prevent injection attacks

### OVSDB Security
- Secure connection to OVSDB
- Validate transaction operations
- Audit database modifications

## Future Enhancements

- **WebSocket Transport**: Support WebSocket in addition to Unix sockets
- **TLS Support**: Encrypted RPC communication
- **Authentication**: Client authentication and authorization
- **Batch Optimization**: Parallel batch request processing
- **Metrics**: Prometheus metrics for RPC operations
- **Tracing**: Distributed tracing integration
- **Schema Validation**: Automatic parameter validation from schemas
- **Code Generation**: Generate client stubs from method definitions

## Related Crates

- **op-core**: Core types and utilities
- **op-network**: Network configuration using OVSDB
- **op-plugins**: Plugin system using JSON-RPC
- **op-services**: Service management via RPC

---
*JSON-RPC 2.0 server with OVSDB and NonNet database support*
</file>

</files>
