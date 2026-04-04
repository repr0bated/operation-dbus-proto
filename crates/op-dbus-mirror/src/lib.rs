//! op-dbus-mirror: 1:1 D-Bus publication of internal databases
//!
//! This crate publishes the internal OVSDB and NonNet database structures as a
//! D-Bus object hierarchy without introducing a second source of truth.

use anyhow::Result;
use dashmap::DashMap;
use op_core::types::BusType;
use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::ovsdb::OvsdbClient;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use sqlx::{sqlite::SqlitePool, Row};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use zbus::{connection::Builder, Connection};

pub mod dbus_interface;
pub mod jsonrpc_interface;
pub mod object;
pub mod tree;

/// D-Bus publication service.
///
/// Responsible for maintaining a 1:1 D-Bus object view of authoritative
/// internal databases.
pub struct DbusMirror {
    ovsdb: Arc<OvsdbClient>,
    nonnet: Arc<NonNetDb>,
    connection: Connection,
    /// Published D-Bus object paths managed by this service.
    published_objects: DashMap<String, ()>,
    /// Enterprise state database pool
    db_pool: Option<SqlitePool>,
    /// Monotonic counter for generating unique fallback IDs when rows lack a UUID.
    fallback_id: AtomicU64,
}

impl DbusMirror {
    /// Create a new D-Bus publication service.
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
            published_objects: DashMap::new(),
            db_pool,
            fallback_id: AtomicU64::new(0),
        })
    }

    /// Publish the current authoritative state into the D-Bus object tree.
    pub async fn publish_snapshot(&self) -> Result<()> {
        tracing::info!("Publishing 1:1 D-Bus snapshot from authoritative stores");

        let mut active_paths = HashSet::new();

        self.publish_ovsdb_snapshot(&mut active_paths).await?;
        self.publish_nonnet_snapshot(&mut active_paths).await?;
        self.publish_enterprise_snapshot(&mut active_paths).await?;
        self.remove_stale_publications(&active_paths).await?;

        tracing::info!("D-Bus snapshot publication complete");
        Ok(())
    }

    /// Compatibility shim for existing callers still using the old name.
    pub async fn reconcile(&self) -> Result<()> {
        self.publish_snapshot().await
    }

    /// Publish enterprise namespace objects into their respective paths.
    async fn publish_enterprise_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
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

            let state_val: Value = unsafe { simd_json::from_str(state_str.as_mut_str())? };

            self.publish_object(&path, state_val).await?;
            active_paths.insert(path);
        }

        // Ensure we request names for all pre-populated services.
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

    /// Publish OVSDB rows into `/org/opdbus/v1/ovsdb/`.
    async fn publish_ovsdb_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let db_name = "Open_vSwitch";
        let dump = self.ovsdb.dump_db(db_name).await?;

        if let Value::Object(tables) = dump {
            for (table_name, rows) in tables.iter() {
                if let Some(row_arr) = rows.as_array() {
                    for row in row_arr {
                        let uuid = self.extract_uuid(row);
                        let path = format!(
                            "/org/opdbus/v1/ovsdb/{}/{}",
                            table_name,
                            uuid.replace('-', "_")
                        );

                        if let Err(e) = self.publish_object(&path, row.clone()).await {
                            tracing::warn!("Failed to publish OVSDB object {}: {}", path, e);
                            continue;
                        }
                        active_paths.insert(path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Publish NonNet rows into `/org/opdbus/v1/nonnet/`.
    async fn publish_nonnet_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", Value::Array(vec![]));
        let response = self.nonnet.handle_request(request).await;

        if let Some(dbs) = response
            .result
            .and_then(|v: Value| v.as_array().map(|a| a.to_vec()))
        {
            for db_val in dbs {
                if let Some(db_name) = db_val.as_str() {
                    // Get schema to find tables
                    let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new(
                        "get_schema",
                        Value::Array(vec![Value::from(db_name)]),
                    );
                    let schema_resp = self.nonnet.handle_request(schema_req).await;

                    if let Some(schema) = schema_resp.result {
                        if let Some(tables) =
                            schema.get("tables").and_then(|v: &Value| v.as_object())
                        {
                            for (table_name, _) in tables.iter() {
                                let select_req = op_jsonrpc::protocol::JsonRpcRequest::new(
                                    "transact",
                                    simd_json::json!([
                                        db_name,
                                        {
                                            "op": "select",
                                            "table": table_name,
                                            "where": []
                                        }
                                    ]),
                                );
                                let select_resp = self.nonnet.handle_request(select_req).await;

                                if let Some(results) = select_resp
                                    .result
                                    .and_then(|v: Value| v.as_array().map(|a| a.to_vec()))
                                {
                                    if let Some(rows) = results
                                        .get(0)
                                        .and_then(|r: &Value| r.get("rows"))
                                        .and_then(|v: &Value| v.as_array())
                                    {
                                        for row in rows {
                                            let uuid = self.extract_uuid(row);
                                            let path = format!(
                                                "/org/opdbus/v1/nonnet/{}/{}/{}",
                                                db_name,
                                                table_name,
                                                uuid.replace('-', "_")
                                            );
                                            self.publish_object(&path, row.clone()).await?;
                                            active_paths.insert(path);
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

    async fn publish_object(&self, path: &str, data: Value) -> Result<()> {
        let path_owned = path.to_string();

        // Ensure path is valid D-Bus path.
        let dbus_path = zbus::zvariant::ObjectPath::try_from(path_owned.clone())
            .map_err(|e| anyhow::anyhow!("Invalid D-Bus path {}: {}", path_owned, e))?;

        if self.published_objects.contains_key(&path_owned) {
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
                }
                Err(e) => {
                    tracing::warn!("Failed to get interface for {}: {}", path_owned, e);
                }
            }
            return Ok(());
        }

        tracing::info!("Publishing new object: {}", path_owned);
        let obj = object::MirrorObject::new(data);
        self.connection.object_server().at(dbus_path, obj).await?;
        self.published_objects.insert(path_owned, ());

        Ok(())
    }

    async fn publish_nonnet_table(
        &self,
        db_name: &str,
        table: &str,
        rows: Vec<Value>,
    ) -> Result<()> {
        let prefix = format!("/org/opdbus/v1/nonnet/{}/{}/", db_name, table);
        let mut active_paths = HashSet::new();

        for row in rows {
            let uuid = self.extract_uuid(&row);
            let path = format!("{prefix}{}", uuid.replace('-', "_"));
            self.publish_object(&path, row).await?;
            active_paths.insert(path);
        }

        self.remove_stale_publications_with_prefix(&prefix, &active_paths)
            .await?;

        Ok(())
    }

    async fn unpublish_object(&self, path: &str) -> Result<()> {
        if !self.published_objects.contains_key(path) {
            return Ok(());
        }

        match self
            .connection
            .object_server()
            .remove::<object::MirrorObject, _>(path)
            .await
        {
            Ok(_) => {
                self.published_objects.remove(path);
                tracing::info!("Unpublished object: {}", path);
            }
            Err(e) => {
                tracing::warn!("Failed to unpublish {}: {}", path, e);
            }
        }

        Ok(())
    }

    async fn remove_stale_publications(&self, active_paths: &HashSet<String>) -> Result<()> {
        let stale_paths: Vec<String> = self
            .published_objects
            .iter()
            .filter(|entry| !active_paths.contains(entry.key()))
            .map(|entry| entry.key().clone())
            .collect();

        for path in stale_paths {
            self.unpublish_object(&path).await?;
        }

        Ok(())
    }

    async fn remove_stale_publications_with_prefix(
        &self,
        prefix: &str,
        active_paths: &HashSet<String>,
    ) -> Result<()> {
        let stale_paths: Vec<String> = self
            .published_objects
            .iter()
            .filter(|entry| entry.key().starts_with(prefix) && !active_paths.contains(entry.key()))
            .map(|entry| entry.key().clone())
            .collect();

        for path in stale_paths {
            self.unpublish_object(&path).await?;
        }

        Ok(())
    }

    fn extract_uuid(&self, row: &Value) -> String {
        // OVSDB rows usually have a _uuid field which is ["uuid", "actual-uuid-string"]
        if let Some(uuid_val) = row.get("_uuid") {
            if let Some(arr) = uuid_val.as_array() {
                if arr.len() == 2 && arr[0] == "uuid" {
                    if let Some(s) = arr[1].as_str() {
                        return s.to_string();
                    }
                }
            }
            if let Some(s) = uuid_val.as_str() {
                return s.to_string();
            }
        }

        // Fallback to 'name' if _uuid is missing
        if let Some(s) = row.get("name").and_then(|v: &Value| v.as_str()) {
            return s.to_string();
        }

        // Last resort: unique monotonic ID so rows don't collide
        format!("anon_{}", self.fallback_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn published_count(&self) -> usize {
        self.published_objects.len()
    }

    /// Backward-compatible alias while old callers are updated.
    pub fn projected_count(&self) -> usize {
        self.published_count()
    }

    /// Get list of all published object paths.
    pub fn list_published_paths(&self) -> Vec<String> {
        self.published_objects
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Backward-compatible alias while old callers are updated.
    pub fn list_projected_paths(&self) -> Vec<String> {
        self.list_published_paths()
    }

    /// Load plugin state into the plugin-backed NonNet database.
    pub async fn load_plugin_state(&self, plugins: &std::collections::HashMap<String, Value>) {
        self.nonnet.load_from_plugins(plugins).await;
    }

    /// Start the publication service.
    pub async fn start(self: Arc<Self>) -> Result<()> {
        // Register control interfaces first so method endpoints stay available
        // even when backend publication is degraded.
        let interface = dbus_interface::DbusMirrorInterface::new(self.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1", interface)
            .await?;

        // Register OVSDB JSON-RPC interface at /org/opdbus/v1/ovsdb
        let ovsdb_interface = jsonrpc_interface::OvsdbInterface::new(self.ovsdb.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1/ovsdb", ovsdb_interface)
            .await?;

        // Register NonNet JSON-RPC interface at /org/opdbus/v1/nonnet
        let nonnet_interface = jsonrpc_interface::NonNetInterface::new(self.nonnet.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1/nonnet", nonnet_interface)
            .await?;

        // Publish an initial snapshot after interface registration.
        if let Err(e) = self.publish_snapshot().await {
            tracing::error!("Initial D-Bus snapshot publication failed: {}", e);
        }

        // 1. Publish plugin-backed NonNet updates directly from the source events.
        let mut nonnet_rx = self.nonnet.subscribe();
        let mirror_clone = self.clone();
        tokio::spawn(async move {
            while let Ok(update) = nonnet_rx.recv().await {
                if let Err(e) = mirror_clone
                    .publish_nonnet_table(&update.db_name, &update.table, update.rows)
                    .await
                {
                    tracing::error!(
                        "Failed to publish NonNet table {} from plugin state: {}",
                        update.table,
                        e
                    );
                }
            }
        });

        // 2. Publish OVSDB monitor events directly from the source events.
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
                                                let path = format!(
                                                    "/org/opdbus/v1/ovsdb/{}/{}",
                                                    table_name,
                                                    uuid.replace('-', "_")
                                                );
                                                if let Err(e) = mirror_ovs_clone
                                                    .publish_object(&path, new_row.clone())
                                                    .await
                                                {
                                                    tracing::error!(
                                                        "Failed to publish OVSDB object {}: {}",
                                                        path,
                                                        e
                                                    );
                                                }
                                            } else {
                                                let path = format!(
                                                    "/org/opdbus/v1/ovsdb/{}/{}",
                                                    table_name,
                                                    uuid.replace('-', "_")
                                                );
                                                if let Err(e) =
                                                    mirror_ovs_clone.unpublish_object(&path).await
                                                {
                                                    tracing::error!(
                                                        "Failed to unpublish OVSDB object {}: {}",
                                                        path,
                                                        e
                                                    );
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

        // 3. Optional repair publication for cases where an upstream source does
        // not emit fine-grained events. Disabled by default.
        let repair_seconds = std::env::var("OP_DBUS_PUBLICATION_REPAIR_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0);

        if let Some(repair_seconds) = repair_seconds {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(repair_seconds));
            loop {
                interval.tick().await;
                if let Err(e) = self.publish_snapshot().await {
                    tracing::error!("D-Bus snapshot repair publication failed: {}", e);
                }
            }
        }

        std::future::pending::<()>().await;
        Ok(())
    }
}

pub mod prelude {
    pub use super::DbusMirror;
}
