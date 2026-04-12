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

/// NonNet database state
pub struct NonNetDb {
    state: Arc<RwLock<NonNetState>>,
    update_tx: broadcast::Sender<NonNetUpdate>,
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

fn normalize_object_types(def: &Value) -> Option<Value> {
    let object_types = def.get("object_types")?.as_object()?;
    let mut normalized = simd_json::value::owned::Object::new();

    for (name, object_type) in object_types.iter() {
        let Some(object_schema) = object_type.as_object() else {
            continue;
        };
        let mut object_schema = object_schema.clone();
        object_schema
            .entry("schema_derived".to_string())
            .or_insert_with(|| Value::from(true));
        normalized.insert(name.clone(), Value::Object(Box::new(object_schema)));
    }

    Some(Value::Object(Box::new(normalized)))
}

fn first_schema_derived_base_path(object_types: &Value) -> Option<String> {
    object_types
        .as_object()?
        .iter()
        .find_map(|(_, object_schema)| {
            let schema_derived = object_schema
                .get("schema_derived")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !schema_derived {
                return None;
            }
            object_schema
                .get("base_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
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
        Self {
            state: Arc::new(RwLock::new(NonNetState::default())),
            update_tx,
        }
    }

    /// Subscribe to database updates
    pub fn subscribe(&self) -> broadcast::Receiver<NonNetUpdate> {
        self.update_tx.subscribe()
    }

    /// Set the tables/schema from plugin state.
    ///
    /// `schema_defs` is an optional map of plugin name → raw schema JSON
    /// (the `base_object` column from the `plugins` table). When present,
    /// `object_types.*.base_path` is propagated into the NonNet schema so
    /// that the D-Bus mirror can project to schema-derived paths instead of
    /// generic `/dynamic/` paths.
    pub async fn load_from_plugins(
        &self,
        plugins: &HashMap<String, Value>,
        schema_defs: Option<&HashMap<String, Value>>,
    ) {
        let mut state = self.state.write().await;

        // Build schema and tables from plugin state
        let mut schema_tables = simd_json::value::owned::Object::new();
        let mut tables = HashMap::new();

        for (db_name, value) in plugins {
            // Skip network plugin
            if db_name == "net" {
                continue;
            }

            // Infer columns from the value structure
            let columns = infer_columns(value);

            let object_types = schema_defs
                .and_then(|defs| defs.get(db_name))
                .and_then(normalize_object_types);
            let base_path = object_types
                .as_ref()
                .and_then(first_schema_derived_base_path);

            let mut table_schema = json!({"columns": columns});
            if let Some(bp) = &base_path {
                if let Some(obj) = table_schema.as_object_mut() {
                    obj.insert("base_path".to_string(), Value::from(bp.clone()));
                }
            }
            if let Some(ot) = object_types {
                if let Some(obj) = table_schema.as_object_mut() {
                    obj.insert("object_types".to_string(), ot);
                }
            }
            schema_tables.insert(db_name.clone(), table_schema);

            // Convert value to rows - 1:1 mirror of plugin state
            let rows = value_to_rows(value);
            tables.insert(db_name.clone(), rows.clone());

            // Broadcast initial load as update
            let _ = self.update_tx.send(NonNetUpdate {
                db_name: NONNET_DB_NAME.to_string(),
                table: db_name.clone(),
                rows,
            });
        }

        state.schema = json!({
            "name": NONNET_DB_NAME,
            "tables": Value::Object(Box::new(schema_tables))
        });
        state.tables = tables;

        debug!("NonNet DB loaded {} tables", state.tables.len());
    }

    /// Seed the NonNet schema directly from plugin schema definitions.
    ///
    /// Each entry in `schema_defs` is a plugin name → raw schema JSON (with
    /// `object_types` containing `base_path`). This creates empty tables with
    /// schema metadata so the D-Bus mirror can project paths immediately,
    /// without calling `query_current_state()` or touching OVSDB.
    pub async fn load_from_schema_defs(&self, schema_defs: &HashMap<String, Value>) {
        let mut state = self.state.write().await;

        let mut schema_tables = simd_json::value::owned::Object::new();

        for (plugin_name, def) in schema_defs {
            let mut table_schema = simd_json::value::owned::Object::new();

            if let Some(ot) = normalize_object_types(def) {
                if let Some(first_bp) = first_schema_derived_base_path(&ot) {
                    table_schema.insert("base_path".to_string(), Value::from(first_bp));
                }
                table_schema.insert("object_types".to_string(), ot);
            }

            // Infer columns from object_type properties
            if let Some(props) = def.get("common_properties").and_then(|v| v.as_object()) {
                let cols: Vec<Value> = props.iter().map(|(k, _)| Value::from(k.clone())).collect();
                table_schema.insert("columns".to_string(), Value::Array(cols));
            }

            schema_tables.insert(plugin_name.clone(), Value::Object(Box::new(table_schema)));

            // Create empty table entry so list_dbs/get_schema work
            state
                .tables
                .entry(plugin_name.clone())
                .or_insert_with(Vec::new);
        }

        state.schema = json!({
            "name": NONNET_DB_NAME,
            "tables": Value::Object(Box::new(schema_tables))
        });

        info!(
            "NonNet schema seeded from {} plugin definitions",
            schema_defs.len()
        );
    }

    /// Update a specific table
    pub async fn update_table(&self, name: &str, rows: Vec<Value>) {
        let mut state = self.state.write().await;
        state.tables.insert(name.to_string(), rows.clone());

        // Keep inferred columns in sync without losing schema-derived
        // projection metadata that was seeded from plugin definitions.
        let mut schema_tables = state
            .schema
            .get("tables")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        for (table_name, table_rows) in state.tables.iter() {
            let columns = infer_columns(&Value::Array(table_rows.clone()));
            let mut table_schema = schema_tables
                .remove(table_name)
                .and_then(|value| match value {
                    Value::Object(object) => Some(*object),
                    _ => None,
                })
                .unwrap_or_default();
            table_schema.insert("columns".to_string(), columns);
            schema_tables.insert(table_name.clone(), Value::Object(Box::new(table_schema)));
        }
        state.schema = json!({
            "name": NONNET_DB_NAME,
            "tables": Value::Object(Box::new(schema_tables))
        });

        let _ = self.update_tx.send(NonNetUpdate {
            db_name: NONNET_DB_NAME.to_string(),
            table: name.to_string(),
            rows,
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

                            // Apply limit and offset if provided
                            let offset =
                                op.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                            let limit =
                                op.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

                            let processed_rows = if offset > 0 || limit.is_some() {
                                let mut start = offset;
                                if start > rows.len() {
                                    start = rows.len();
                                }
                                let mut end = match limit {
                                    Some(l) => start + l,
                                    None => rows.len(),
                                };
                                if end > rows.len() {
                                    end = rows.len();
                                }
                                rows[start..end].to_vec()
                            } else {
                                rows
                            };

                            results.push(json!({"rows": processed_rows}));
                        }
                        "count" => {
                            let table = op.get("table").and_then(|v| v.as_str()).unwrap_or("");
                            let count = state.tables.get(table).map(|r| r.len()).unwrap_or(0);
                            results.push(json!({"count": count}));
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

        db.load_from_plugins(&plugins, None).await;

        let request = JsonRpcRequest::new("list_dbs", json!([]));
        let response = db.handle_request(request).await;

        assert!(response.result.is_some());
    }
}
