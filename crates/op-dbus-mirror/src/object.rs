//! Mirror Object D-Bus Interface

use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use zbus::interface;

/// A generic D-Bus object representing a database row
pub struct MirrorObject {
    data: Value,
}

impl MirrorObject {
    pub fn new(data: Value) -> Self {
        Self { data }
    }

    pub fn update_data(&mut self, new_data: Value) -> bool {
        if self.data == new_data {
            return false;
        }
        tracing::debug!("Updating MirrorObject data");
        self.data = new_data;
        true
    }
}

#[interface(name = "org.opdbus.ProjectedObjectV1")]
impl MirrorObject {
    /// Get the full JSON representation of the row
    #[zbus(property)]
    async fn json_data(&self) -> String {
        simd_json::to_string(&self.data).unwrap_or_default()
    }

    /// Get a specific property value by key
    async fn get_property(&self, key: String) -> String {
        self.data
            .get(&key)
            .map(|v| simd_json::to_string(v).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Signal emitted when json_data changes
    #[zbus(signal)]
    pub async fn data_updated(&self, ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()>;
}

/// A D-Bus object representing a projected table or object type.
///
/// Published at the base_path of a schema-derived object type.
pub struct TableObject {
    plugin_id: String,
    object_type: String,
    rcp_db: String,
    rcp_table: String,
}

impl TableObject {
    pub fn new(plugin_id: String, object_type: String, rcp_db: String, rcp_table: String) -> Self {
        Self {
            plugin_id,
            object_type,
            rcp_db,
            rcp_table,
        }
    }
}

#[interface(name = "org.opdbus.ProjectedTableV1")]
impl TableObject {
    #[zbus(property)]
    async fn plugin_id(&self) -> String {
        self.plugin_id.clone()
    }

    #[zbus(property)]
    async fn object_type(&self) -> String {
        self.object_type.clone()
    }

    #[zbus(property)]
    async fn rcp_db(&self) -> String {
        self.rcp_db.clone()
    }

    #[zbus(property)]
    async fn rcp_table(&self) -> String {
        self.rcp_table.clone()
    }
}

/// Lazy-loaded table summary for large branches.
///
/// Instead of projecting every row as an individual D-Bus object (which is
/// untenable for 100k+ rows), publish this summary at the table path and
/// let callers enumerate on demand via `list_ids` / `get_row`.
pub struct TableSummaryObject {
    db_name: String,
    table_name: String,
    row_count: u64,
    nonnet: Arc<NonNetDb>,
}

impl TableSummaryObject {
    pub fn new(db_name: String, table_name: String, row_count: u64, nonnet: Arc<NonNetDb>) -> Self {
        Self {
            db_name,
            table_name,
            row_count,
            nonnet,
        }
    }
}

#[interface(name = "org.opdbus.LazyTableV1")]
impl TableSummaryObject {
    /// Total number of rows in this table
    #[zbus(property)]
    async fn count(&self) -> u64 {
        self.row_count
    }

    /// Table name
    #[zbus(property)]
    async fn table(&self) -> String {
        self.table_name.clone()
    }

    /// Database name
    #[zbus(property)]
    async fn database(&self) -> String {
        self.db_name.clone()
    }

    /// Enumerate row IDs with offset/limit pagination
    async fn list_ids(&self, offset: u64, limit: u64) -> zbus::fdo::Result<Vec<String>> {
        let select_req = op_jsonrpc::protocol::JsonRpcRequest::new(
            "transact",
            simd_json::json!([
                self.db_name.clone(),
                {"op": "select", "table": self.table_name.clone(), "offset": offset, "limit": limit}
            ]),
        );
        let resp = self.nonnet.handle_request(select_req).await;

        let ids = resp
            .result
            .and_then(|r: Value| r.as_array().and_then(|a| a.first().cloned()))
            .and_then(|r: Value| r.get("rows").cloned())
            .and_then(|v: Value| v.as_array().map(|a| a.to_vec()))
            .unwrap_or_default()
            .iter()
            .map(|row| extract_id(row))
            .collect();

        Ok(ids)
    }

    /// Fetch the full JSON of a single row by ID
    async fn get_row(&self, id: String) -> zbus::fdo::Result<String> {
        let select_req = op_jsonrpc::protocol::JsonRpcRequest::new(
            "transact",
            simd_json::json!([
                self.db_name.clone(),
                {"op": "select", "table": self.table_name.clone(), "where": [["_uuid", "==", id.clone()]]}
            ]),
        );
        let resp = self.nonnet.handle_request(select_req).await;

        let row = resp
            .result
            .and_then(|r: Value| r.as_array().and_then(|a| a.first().cloned()))
            .and_then(|r: Value| r.get("rows").cloned())
            .and_then(|v: Value| v.as_array().and_then(|a| a.first().cloned()));

        match row {
            Some(data) => Ok(simd_json::to_string(&data).unwrap_or_default()),
            None => Err(zbus::fdo::Error::Failed(format!("Row '{}' not found", id))),
        }
    }
}

fn extract_id(row: &Value) -> String {
    extract_id_with_field(row, None)
}

fn extract_id_with_field(row: &Value, id_field: Option<&str>) -> String {
    if let Some(field) = id_field {
        if let Some(s) = row.get(field).and_then(|v| v.as_str()) {
            return s.to_string();
        }
        if let Some(v) = row.get(field).and_then(|v| v.as_u64()) {
            return v.to_string();
        }
        if let Some(v) = row.get(field).and_then(|v| v.as_i64()) {
            return v.to_string();
        }
    }
    if let Some(s) = row.get("uuid").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(s) = row.get("_uuid").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(s) = row.get("id").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(s) = row.get("name").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    "unknown".to_string()
}

/// Lazy-loaded summary for large OVSDB-backed tables.
pub struct OvsdbTableSummaryObject {
    table_name: String,
    id_field: Option<String>,
    row_count: u64,
    ovsdb: Arc<OvsdbClient>,
}

impl OvsdbTableSummaryObject {
    pub fn new(
        table_name: String,
        id_field: Option<String>,
        row_count: u64,
        ovsdb: Arc<OvsdbClient>,
    ) -> Self {
        Self {
            table_name,
            id_field,
            row_count,
            ovsdb,
        }
    }
}

#[interface(name = "org.opdbus.LazyTableV1")]
impl OvsdbTableSummaryObject {
    #[zbus(property)]
    async fn count(&self) -> u64 {
        self.row_count
    }

    #[zbus(property)]
    async fn table(&self) -> String {
        self.table_name.clone()
    }

    #[zbus(property)]
    async fn database(&self) -> String {
        "Open_vSwitch".to_string()
    }

    async fn list_ids(&self, offset: u64, limit: u64) -> zbus::fdo::Result<Vec<String>> {
        let rows = self
            .ovsdb
            .select_table(&self.table_name)
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        let start = usize::min(offset as usize, rows.len());
        let end = usize::min(start.saturating_add(limit as usize), rows.len());

        Ok(rows[start..end]
            .iter()
            .map(|row| extract_id_with_field(row, self.id_field.as_deref()))
            .collect())
    }

    async fn get_row(&self, id: String) -> zbus::fdo::Result<String> {
        let rows = self
            .ovsdb
            .select_table(&self.table_name)
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;

        rows.into_iter()
            .find(|row| extract_id_with_field(row, self.id_field.as_deref()) == id)
            .map(|row| simd_json::to_string(&row).unwrap_or_default())
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("Row '{}' not found", id)))
    }
}
