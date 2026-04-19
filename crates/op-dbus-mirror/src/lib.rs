//! op-dbus-mirror: 1:1 D-Bus publication of internal databases
//!
//! This crate publishes the internal OVSDB and NonNet database structures as a
//! D-Bus object hierarchy without introducing a second source of truth.

use anyhow::Result;
use dashmap::DashMap;
use op_core::types::BusType;
use op_grpc_bridge::SchemaEngine;
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use sqlx::{sqlite::SqlitePool, Row};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use zbus::zvariant::ObjectPath;
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
    schema_engine: Option<Arc<SchemaEngine>>,
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
        schema_engine: Option<Arc<SchemaEngine>>,
    ) -> Result<Self> {
        let connection = match bus_type {
            BusType::System => {
                Builder::system()?
                    .name("org.opdbus.v1")?
                    .name("org.opdbus")?
                    .build()
                    .await?
            }
            BusType::Session => {
                Builder::session()?
                    .name("org.opdbus.v1")?
                    .name("org.opdbus")?
                    .build()
                    .await?
            }
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
            schema_engine,
            connection,
            published_objects: DashMap::new(),
            db_pool,
            fallback_id: AtomicU64::new(0),
        })
    }

    /// Start the mirror service.
    ///
    /// Performs an initial full-tree publication and then enters a loop
    /// to periodically refresh and repair the mirror.
    pub async fn start(self: Arc<Self>) -> Result<()> {
        tracing::info!("Starting D-Bus mirror publication service...");

        // Initial full sync
        if let Err(e) = self.refresh_full_tree().await {
            tracing::error!("Initial D-Bus mirror sync failed: {}", e);
        }

        // Register mirror-management interface
        let interface = dbus_interface::DbusMirrorInterface::new(self.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1", interface)
            .await?;

        // Register OVSDB JSON-RPC interface at /org/opdbus/v1/ovsdb
        let ovsdb_interface =
            jsonrpc_interface::OvsdbInterface::new(self.ovsdb.clone(), self.schema_engine.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1/ovsdb", ovsdb_interface)
            .await?;

        // Register NonNet JSON-RPC interface at /org/opdbus/v1/nonnet
        let nonnet_interface = jsonrpc_interface::NonNetInterface::new(
            self.nonnet.clone(),
            self.schema_engine.clone(),
        );
        self.connection
            .object_server()
            .at("/org/opdbus/v1/nonnet", nonnet_interface)
            .await?;

        // Start background refresh task
        let mirror = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = mirror.refresh_full_tree().await {
                    tracing::error!("D-Bus mirror snapshot repair publication failed: {}", e);
                }
            }
        });

        std::future::pending::<()>().await;
        Ok(())
    }

    /// Compatibility method for dbus_interface
    pub async fn publish_snapshot(&self) -> Result<()> {
        self.refresh_full_tree().await
    }

    pub fn published_count(&self) -> u64 {
        self.published_objects.len() as u64
    }

    pub fn projected_count(&self) -> u64 {
        self.published_objects.len() as u64 // Currently 1:1
    }

    /// Perform a full scan of all authoritative databases and ensure
    /// the D-Bus tree exactly matches the database state.
    pub async fn refresh_full_tree(&self) -> Result<()> {
        let mut active_paths = HashSet::new();

        // 1. Scan Enterprise SQLite (Primary Authority for NonNet rows)
        if self.db_pool.is_some() {
            if let Err(e) = self.publish_enterprise_snapshot(&mut active_paths).await {
                tracing::warn!("Enterprise DB snapshot failed: {}", e);
            }
        }

        // 2. Scan OVSDB (Authoritative for Network)
        if let Err(e) = self.publish_ovsdb_snapshot(&mut active_paths).await {
            tracing::warn!("OVSDB snapshot failed: {}", e);
        }

        // 3. Scan NonNet (Authoritative for Plugins not in Enterprise DB)
        if let Err(e) = self.publish_nonnet_snapshot(&mut active_paths).await {
            tracing::warn!("NonNet snapshot failed: {}", e);
        }

        // 4. Scan freedesktop system services
        if let Err(e) = self.publish_system_services(&mut active_paths).await {
            tracing::warn!("System services snapshot failed: {}", e);
        }

        // 5. Remove any D-Bus objects that no longer exist in any authority
        self.remove_stale_publications(&active_paths).await?;

        Ok(())
    }

    async fn publish_enterprise_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let pool = self.db_pool.as_ref().unwrap();
        let rows = sqlx::query("SELECT id, plugin_id, object_path, state_json FROM state_entries")
            .fetch_all(pool)
            .await?;

        for row in rows {
            let plugin_id: String = row.try_get("plugin_id")?;
            let object_path: String = row.try_get("object_path")?;
            let state_str: String = row.try_get("state_json")?;

            let mut state_str = state_str;
            let state_val: Value = unsafe { simd_json::from_str(state_str.as_mut_str())? };

            let full_path = format!("/org/opdbus/v1/plugins/{}{}", plugin_id, object_path);
            self.publish_object(&full_path, state_val).await?;
            active_paths.insert(full_path);
        }

        Ok(())
    }

    async fn publish_ovsdb_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let dump = self.ovsdb.dump_db("Open_vSwitch").await?;

        if let Value::Object(tables) = dump {
            for (table_name, table_data) in tables.iter() {
                if let Some(rows) = table_data.get("rows").and_then(|v| v.as_object()) {
                    for (uuid, row_data) in rows.iter() {
                        let path = format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, uuid);
                        self.publish_object(&path, row_data.clone()).await?;
                        active_paths.insert(path);
                    }
                }
            }
        }

        Ok(())
    }

    async fn publish_nonnet_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", Value::Array(vec![]));
        let response = self.nonnet.handle_request(request).await;

        let dbs = response
            .result
            .and_then(|v: Value| v.as_array().map(|a| a.to_vec()))
            .unwrap_or_default();

        for db_name_val in dbs {
            if let Some(db_name) = db_name_val.as_str() {
                let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new(
                    "get_schema",
                    Value::Array(vec![Value::from(db_name)]),
                );
                let schema_resp = self.nonnet.handle_request(schema_req).await;

                if let Some(tables) = schema_resp
                    .result
                    .and_then(|schema| schema.get("tables").and_then(|v| v.as_object().cloned()))
                {
                    for (table_name, _) in tables.iter() {
                        let dump_req = op_jsonrpc::protocol::JsonRpcRequest::new(
                            "dump",
                            Value::Array(vec![Value::from(db_name)]),
                        );
                        let dump_resp = self.nonnet.handle_request(dump_req).await;

                        if let Some(rows) = dump_resp
                            .result
                            .and_then(|r: Value| r.get(table_name).cloned())
                            .and_then(|r: Value| r.get("rows").cloned())
                            .and_then(|v: Value| v.as_array().map(|a| a.to_vec()))
                        {
                            for row in rows {
                                let id = self.extract_uuid(&row);
                                let path = format!(
                                    "/org/opdbus/v1/nonnet/{}/{}/{}",
                                    db_name, table_name, id
                                );
                                self.publish_object(&path, row.clone()).await?;
                                active_paths.insert(path);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Services that are too large or ephemeral to project, or that op-dbus replaces.
    const SKIP_SERVICES: &'static [&'static str] = &[
        "org.freedesktop.systemd1",       // thousands of unit objects
        "org.freedesktop.NetworkManager", // replaced by op-dbus, huge ephemeral tree
        "fi.w1.wpa_supplicant1",          // ephemeral BSS/scan results
        "org.freedesktop.DBus",           // meta bus service
    ];

    async fn publish_system_services(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let system_conn = zbus::Connection::system().await?;
        let proxy = zbus::fdo::DBusProxy::new(&system_conn).await?;
        let names = proxy.list_names().await?;

        for name in names {
            let name_str = name.as_str();
            // Skip unique connections, our own service, and known-huge services
            if name_str.starts_with(':')
                || name_str.starts_with("org.opdbus")
                || Self::SKIP_SERVICES.iter().any(|s| *s == name_str)
            {
                continue;
            }

            // Sanitize for D-Bus object path: replace dots and hyphens with underscores
            let safe_name = name_str.replace('.', "/").replace('-', "_");

            let introspect_proxy = zbus::fdo::IntrospectableProxy::builder(&system_conn)
                .destination(name_str)?
                .path("/")?
                .build()
                .await?;

            let mut interfaces = Vec::new();
            let mut methods = Vec::new();
            let mut properties = Vec::new();
            let mut signals = Vec::new();

            if let Ok(xml) = introspect_proxy.introspect().await {
                if let Ok(node) = zbus_xml::Node::try_from(xml.as_str()) {
                    for iface in node.interfaces() {
                        let iface_name: String = iface.name().to_string();
                        // Skip standard D-Bus plumbing interfaces
                        if iface_name == "org.freedesktop.DBus.Introspectable"
                            || iface_name == "org.freedesktop.DBus.Peer"
                            || iface_name == "org.freedesktop.DBus.Properties"
                        {
                            continue;
                        }
                        interfaces.push(Value::from(iface_name));
                        for m in iface.methods() {
                            let n: String = m.name().to_string();
                            methods.push(Value::from(n));
                        }
                        for p in iface.properties() {
                            let n: String = p.name().to_string();
                            properties.push(Value::from(n));
                        }
                        for s in iface.signals() {
                            let n: String = s.name().to_string();
                            signals.push(Value::from(n));
                        }
                    }
                }
            }

            let service_data = simd_json::json!({
                "service": name_str,
                "interfaces": interfaces,
                "methods": methods,
                "properties": properties,
                "signals": signals,
            });

            let path = format!("/org/opdbus/v1/system/{}", safe_name);
            self.publish_object(&path, service_data).await?;
            active_paths.insert(path);
        }

        Ok(())
    }

    async fn publish_object(&self, path: &str, data: Value) -> Result<()> {
        if self.published_objects.contains_key(path) {
            // Signal property update if needed
            // TODO: Track versions to avoid redundant signals
            return Ok(());
        }

        let obj = object::MirrorObject::new(data);
        self.connection.object_server().at(path, obj).await?;
        self.published_objects.insert(path.to_string(), ());

        Ok(())
    }

    /// Load plugin state into the mirror (Seeding).
    pub async fn load_plugin_state(&self, plugins: &std::collections::HashMap<String, Value>) {
        let mut active_paths = HashSet::new();
        for (plugin_id, state) in plugins {
            let path = format!("/org/opdbus/v1/plugins/{}", plugin_id);
            if let Err(e) = self.publish_object(&path, state.clone()).await {
                tracing::error!("Failed to seed mirror for {}: {}", plugin_id, e);
            }
            active_paths.insert(path);
        }
    }

    async fn remove_stale_publications(&self, active_paths: &HashSet<String>) -> Result<()> {
        let mut to_remove = Vec::new();
        for entry in self.published_objects.iter() {
            if !active_paths.contains(entry.key()) {
                to_remove.push(entry.key().clone());
            }
        }

        for path in to_remove {
            let op = ObjectPath::try_from(path.as_str())?;
            self.connection
                .object_server()
                .remove::<object::MirrorObject, _>(op)
                .await?;
            self.published_objects.remove(&path);
        }

        Ok(())
    }

    pub fn list_published_paths(&self) -> Vec<String> {
        self.published_objects
            .iter()
            .map(|e| entry_path_to_dbus(e.key()))
            .collect()
    }

    /// Expose the underlying D-Bus connection so callers can register
    /// additional interfaces on the same bus name.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    fn extract_uuid(&self, row: &Value) -> String {
        if let Some(uuid) = row.get("uuid").and_then(|v| v.as_str()) {
            return uuid.to_string();
        }
        if let Some(uuid) = row.get("_uuid").and_then(|v| v.as_str()) {
            return uuid.to_string();
        }
        if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
            return id.to_string();
        }
        if let Some(s) = row.get("name").and_then(|v| v.as_str()) {
            return s.to_string();
        }

        // Deterministic fallback ID
        format!("anon_{}", self.fallback_id.fetch_add(1, Ordering::Relaxed))
    }
}

fn entry_path_to_dbus(path: &str) -> String {
    path.to_string()
}

pub mod prelude {
    pub use super::DbusMirror;
}
