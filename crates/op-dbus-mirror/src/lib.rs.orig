//! op-dbus-mirror: 1:1 D-Bus projection of internal databases
//!
//! This crate provides a mechanism to mirror the internal OVSDB and NonNet
//! database structures into a D-Bus object hierarchy.

use anyhow::Result;
use op_core::types::BusType;
use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::ovsdb::OvsdbClient;
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::sync::Arc;
use zbus::{connection::Builder, Connection};
use dashmap::DashMap;
use sqlx::{sqlite::SqlitePool, Row};

pub mod tree;
pub mod object;
pub mod dbus_interface;

/// D-Bus Mirror Service
///
/// Responsible for maintaining a 1:1 D-Bus projection of internal databases.
pub struct DbusMirror {
    ovsdb: Arc<OvsdbClient>,
    nonnet: Arc<NonNetDb>,
    connection: Connection,
    /// Map of registered D-Bus object paths
    projected_objects: DashMap<String, ()>,
    /// Enterprise state database pool
    db_pool: Option<SqlitePool>,
}

impl DbusMirror {
    /// Create a new D-Bus mirror
    pub async fn new(
        bus_type: BusType,
        ovsdb: Arc<OvsdbClient>,
        nonnet: Arc<NonNetDb>,
    ) -> Result<Self> {
        let connection = match bus_type {
            BusType::System => Builder::system()?.name("org.opdbus.v1")?.build().await?,
            BusType::Session => Builder::session()?.name("org.opdbus.v1")?.build().await?,
        };

        // Initialize Enterprise DB pool if it exists
        let db_path = "/var/lib/op-dbus/state.db";
        let db_pool = if std::path::Path::new(db_path).exists() {
            Some(SqlitePool::connect(&format!("sqlite://{}", db_path)).await?)
        } else {
            None
        };

        Ok(Self {
            ovsdb,
            nonnet,
            connection,
            projected_objects: DashMap::new(),
            db_pool,
        })
    }

    /// Reconcile the D-Bus tree with the current database state
    ///
    /// This function performs a full mirror sync:
    /// 1. Dumps OVSDB and NonNet
    /// 2. Maps tables/rows to D-Bus paths
    /// 3. Registers/unregisters objects to match the DB
    pub async fn reconcile(&self) -> Result<()> {
        tracing::info!("Starting 1:1 D-Bus mirror reconciliation");

        // 1. Mirror OVSDB
        self.mirror_ovsdb().await?;

        // 2. Mirror NonNet
        self.mirror_nonnet().await?;

        // 3. Mirror Enterprise Namespace (org.opdbus.*)
        self.mirror_enterprise().await?;

        tracing::info!("D-Bus mirror reconciliation complete");
        Ok(())
    }

    /// Mirror Enterprise Namespace into their respective paths
    async fn mirror_enterprise(&self) -> Result<()> {
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(()),
        };

        // Get all live objects
        let rows = sqlx::query("SELECT object_path, state FROM live_objects")
            .fetch_all(pool)
            .await?;

        for row in rows {
            let path: String = row.get("object_path");
            let mut state_str: String = row.get("state");
            
            let state_val: Value = unsafe { 
                simd_json::from_str(state_str.as_mut_str())? 
            };
            
            self.register_projected_object(&path, state_val).await?;
        }

        // Also ensure we request names for all pre-populated services
        let services = sqlx::query("SELECT service_name FROM namespace_services WHERE enabled = 1")
            .fetch_all(pool)
            .await?;

        for s_row in services {
            let service_name: String = s_row.get("service_name");
            // Request the name on the bus so we own the namespace
            if let Err(e) = self.connection.request_name(service_name.clone()).await {
                tracing::debug!("Could not request name {}: {}", service_name, e);
            }
        }

        Ok(())
    }

    /// Mirror OVSDB into /org/opdbus/v1/ovsdb/
    async fn mirror_ovsdb(&self) -> Result<()> {
        let db_name = "Open_vSwitch";
        let dump = self.ovsdb.dump_db(db_name).await?;
        
        if let Value::Object(tables) = dump {
            for (table_name, rows) in tables.iter() {
                if let Some(row_arr) = rows.as_array() {
                    for row in row_arr {
                        let uuid = self.extract_uuid(row).unwrap_or_else(|| "unknown".to_string());
                        let path = format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, uuid.replace('-', "_"));
                        
                        self.register_projected_object(&path, row.clone()).await?;
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Mirror NonNet into /org/opdbus/v1/nonnet/
    async fn mirror_nonnet(&self) -> Result<()> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", Value::Array(vec![]));
        let response = self.nonnet.handle_request(request).await;
        
        if let Some(dbs) = response.result.and_then(|v: Value| v.as_array().map(|a| a.to_vec())) {
            for db_val in dbs {
                if let Some(db_name) = db_val.as_str() {
                    // Get schema to find tables
                    let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new("get_schema", Value::Array(vec![Value::from(db_name)]));
                    let schema_resp = self.nonnet.handle_request(schema_req).await;
                    
                    if let Some(schema) = schema_resp.result {
                        if let Some(tables) = schema.get("tables").and_then(|v: &Value| v.as_object()) {
                            for (table_name, _) in tables.iter() {
                                let select_req = op_jsonrpc::protocol::JsonRpcRequest::new("transact", simd_json::json!([
                                    db_name,
                                    {
                                        "op": "select",
                                        "table": table_name,
                                        "where": []
                                    }
                                ]));
                                let select_resp = self.nonnet.handle_request(select_req).await;
                                
                                if let Some(results) = select_resp.result.and_then(|v: Value| v.as_array().map(|a| a.to_vec())) {
                                    if let Some(rows) = results.get(0).and_then(|r: &Value| r.get("rows")).and_then(|v: &Value| v.as_array()) {
                                        for row in rows {
                                            let uuid = self.extract_uuid(row).unwrap_or_else(|| "unknown".to_string());
                                            let path = format!("/org/opdbus/v1/nonnet/{}/{}/{}", db_name, table_name, uuid.replace('-', "_"));
                                            self.register_projected_object(&path, row.clone()).await?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn register_projected_object(&self, path: &str, data: Value) -> Result<()> {
        let path_owned = path.to_string();
        
        // Ensure path is valid D-Bus path
        let dbus_path = zbus::zvariant::ObjectPath::try_from(path_owned.clone())
            .map_err(|e| anyhow::anyhow!("Invalid D-Bus path {}: {}", path_owned, e))?;

        if self.projected_objects.contains_key(&path_owned) {
            // Signal property change on existing object
            let server = self.connection.object_server();
            match server.interface::<_, object::MirrorObject>(dbus_path).await {
                Ok(iface_ref) => {
                    let mut obj = iface_ref.get_mut().await;
                    if obj.update_data(data) {
                        tracing::debug!("Emitting property change signal for {}", path_owned);
                        // Emit the signal
                        let ctxt = iface_ref.signal_context();
                        iface_ref.get().await.data_updated(ctxt).await?;
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to get interface for {}: {}", path_owned, e);
                }
            }
            return Ok(())
        }

        tracing::info!("Registering new projected object: {}", path_owned);
        let obj = object::MirrorObject::new(data);
        self.connection.object_server().at(dbus_path, obj).await?;
        self.projected_objects.insert(path_owned, ());
        
        Ok(())
    }

    fn extract_uuid(&self, row: &Value) -> Option<String> {
        // OVSDB rows usually have a _uuid field which is ["uuid", "actual-uuid-string"]
        if let Some(uuid_val) = row.get("_uuid") {
            if let Some(arr) = uuid_val.as_array() {
                if arr.len() == 2 && arr[0] == "uuid" {
                    return arr[1].as_str().map(|s: &str| s.to_string());
                }
            }
            if let Some(s) = uuid_val.as_str() {
                return Some(s.to_string());
            }
        }
        
        // Fallback to 'name' or other identity fields if _uuid is missing
        row.get("name").and_then(|v: &Value| v.as_str()).map(|s: &str| s.to_string())
    }

    pub fn projected_count(&self) -> usize {
        self.projected_objects.len()
    }

    /// Start the mirror reconciliation loop
    pub async fn start(self: Arc<Self>) -> Result<()> {
        // Register the main mirror interface
        let interface = dbus_interface::DbusMirrorInterface::new(self.clone());
        self.connection.object_server().at("/org/opdbus/v1", interface).await?;

        // 1. Start NonNet update listener
        let mut nonnet_rx = self.nonnet.subscribe();
        let mirror_clone = self.clone();
        tokio::spawn(async move {
            while let Ok(update) = nonnet_rx.recv().await {
                for row in update.rows {
                    let uuid = mirror_clone.extract_uuid(&row).unwrap_or_else(|| "unknown".to_string());
                    let path = format!("/org/opdbus/v1/nonnet/OpNonNet/{}/{}", update.table, uuid.replace('-', "_"));
                    if let Err(e) = mirror_clone.register_projected_object(&path, row).await {
                        tracing::error!("Failed to register NonNet object {}: {}", path, e);
                    }
                }
            }
        });

        // 2. Start OVSDB monitor listener
        let ovsdb_clone = self.ovsdb.clone();
        let mirror_ovs_clone = self.clone();
        tokio::spawn(async move {
            if let Ok(mut rx) = ovsdb_clone.monitor_db("Open_vSwitch").await {
                while let Some(update) = rx.recv().await {
                    // Update format: ["update", null, {table: {uuid: {new: row}}}]
                    if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
                        if params.len() >= 3 {
                            if let Some(tables) = params[2].as_object() {
                                for (table_name, table_update) in tables.iter() {
                                    if let Some(uuids) = table_update.as_object() {
                                        for (uuid, row_update) in uuids.iter() {
                                            if let Some(new_row) = row_update.get("new") {
                                                let path = format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, uuid.replace('-', "_"));
                                                if let Err(e) = mirror_ovs_clone.register_projected_object(&path, new_row.clone()).await {
                                                    tracing::error!("Failed to register OVSDB object {}: {}", path, e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        // 3. Start periodic full reconciliation
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = self.reconcile().await {
                tracing::error!("D-Bus mirror reconciliation failed: {}", e);
            }
        }
    }
}

pub mod prelude {
    pub use super::DbusMirror;
}
