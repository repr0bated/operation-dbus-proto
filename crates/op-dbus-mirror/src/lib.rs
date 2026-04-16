//! op-dbus-mirror: 1:1 D-Bus publication of internal databases
//!
//! This crate publishes the internal OVSDB and NonNet database structures as a
//! D-Bus object hierarchy without introducing a second source of truth. It is a
//! pure 1:1 projection of the authoritative RCP stores.

use anyhow::Result;
use dashmap::DashMap;
use op_core::{dbus::connect_and_claim_name, types::BusType};
use op_grpc_bridge::SchemaEngine;
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

pub mod dbus_interface;
pub mod jsonrpc_interface;
pub mod object;
pub mod tree;

const LAZY_LOAD_THRESHOLD: usize = 1000;
const NONNET_CHUNK_SIZE: usize = 500;
const OVSDB_TABLE_TIMEOUT_SECS: u64 = 5;

#[derive(Clone, Debug)]
struct ProjectionSpec {
    plugin_id: String,
    object_type: String,
    base_path: String,
    rcp_db: String,
    rcp_table: String,
    id_field: Option<String>,
}

impl ProjectionSpec {
    fn object_path(&self, id: &str) -> String {
        format!(
            "{}/{}",
            sanitize_dbus_path(&self.base_path).trim_end_matches('/'),
            sanitize_path_segment(id)
        )
    }

    fn dynamic_path(&self) -> String {
        format!(
            "/org/opdbus/v1/dynamic/{}/{}",
            sanitize_path_segment(&self.plugin_id),
            sanitize_path_segment(&self.object_type)
        )
    }
}

/// D-Bus publication service.
///
/// Maintains a 1:1 D-Bus object view of the two authoritative RCP stores:
///   - OVSDB (network state)
///   - NonNet (non-network plugin state)
pub struct DbusMirror {
    ovsdb: Option<Arc<OvsdbClient>>,
    nonnet: Arc<NonNetDb>,
    schema_engine: Option<Arc<SchemaEngine>>,
    connection: Connection,
    /// Published D-Bus object paths managed by this service.
    published_objects: DashMap<String, ()>,
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
        let connection = connect_and_claim_name(bus_type, "org.opdbus.v1").await?;

        // Attempt to claim standard Freedesktop networking name for compatibility
        if let Err(e) =
            op_core::dbus::request_additional_name(&connection, "org.freedesktop.network1").await
        {
            tracing::warn!("Failed to claim org.freedesktop.network1: {}", e);
        } else {
            tracing::info!("Successfully claimed org.freedesktop.network1 for compatibility");
        }

        Ok(Self {
            ovsdb: Some(ovsdb),
            nonnet,
            schema_engine,
            connection,
            published_objects: DashMap::new(),
            fallback_id: AtomicU64::new(0),
        })
    }

    /// Start the mirror service.
    pub async fn start(self: Arc<Self>) -> Result<()> {
        tracing::info!("Starting D-Bus mirror publication service...");

        // Start background refresh task using event-driven updates.
        // OVSDB monitor is optional — don't block startup if unavailable.
        let mut ovsdb_rx = if let Some(ref ovsdb) = self.ovsdb {
            match ovsdb.monitor_db("Open_vSwitch").await {
                Ok(rx) => Some(rx),
                Err(e) => {
                    tracing::warn!(
                        "OVSDB monitor unavailable, skipping network projection: {}",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };
        let mut nonnet_rx = self.nonnet.subscribe();
        let mirror_ref = Arc::clone(&self);

        // Initial full publication from all RCP stores in background
        let init_mirror = Arc::clone(&self);
        tokio::spawn(async move {
            if let Err(e) = init_mirror.refresh_full_tree().await {
                tracing::error!("Initial D-Bus mirror sync failed: {}", e);
            }
        });

        // Register mirror-management interface
        let interface = dbus_interface::DbusMirrorInterface::new(Arc::clone(&self));
        self.connection
            .object_server()
            .at("/org/opdbus/v1", interface)
            .await?;

        // Register OVSDB JSON-RPC interface at /org/opdbus/v1/ovsdb (only if OVSDB available)
        if let Some(ref ovsdb) = self.ovsdb {
            let ovsdb_interface =
                jsonrpc_interface::OvsdbInterface::new(ovsdb.clone(), self.schema_engine.clone());
            self.connection
                .object_server()
                .at("/org/opdbus/v1/ovsdb", ovsdb_interface)
                .await?;
        }

        // Register NonNet JSON-RPC interface at /org/opdbus/v1/nonnet
        let nonnet_interface = jsonrpc_interface::NonNetInterface::new(
            self.nonnet.clone(),
            self.schema_engine.clone(),
        );
        self.connection
            .object_server()
            .at("/org/opdbus/v1/nonnet", nonnet_interface)
            .await?;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = Arc::clone(&mirror_ref).refresh_full_tree().await {
                            tracing::error!("Periodic D-Bus mirror refresh failed: {}", e);
                        }
                    }
                    result = nonnet_rx.recv() => {
                        match result {
                            Ok(_update) => {
                                tracing::debug!("Received NonNet event, triggering mirror refresh");
                                if let Err(e) = Arc::clone(&mirror_ref).refresh_full_tree().await {
                                    tracing::error!("Event-driven D-Bus mirror refresh failed: {}", e);
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!("NonNet event receiver lagged by {} messages", n);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                tracing::error!("NonNet event receiver closed, sleeping before next poll");
                                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            }
                        }
                    }
                    result = async {
                        if let Some(rx) = ovsdb_rx.as_mut() {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        match result {
                            Some(_update) => {
                                tracing::debug!("Received OVSDB event, triggering mirror refresh");
                                if let Err(e) = Arc::clone(&mirror_ref).refresh_full_tree().await {
                                    tracing::error!("Event-driven D-Bus mirror refresh failed: {}", e);
                                }
                            }
                            None => {
                                tracing::error!("OVSDB event receiver closed, disabling OVSDB event-driven updates");
                                ovsdb_rx = None;
                            }
                        }
                    }
                }
            }
        });

        std::future::pending::<()>().await;
        Ok(())
    }

    /// Compatibility method for dbus_interface
    pub async fn publish_snapshot(&self) -> Result<()> {
        let mut active_paths = HashSet::new();
        self.publish_nonnet_snapshot_sequential(&mut active_paths)
            .await?;
        self.publish_ovsdb_snapshot(&mut active_paths).await?;
        self.remove_stale_publications(&active_paths).await?;
        Ok(())
    }

    pub fn published_count(&self) -> u64 {
        self.published_objects.len() as u64
    }

    pub fn projected_count(&self) -> u64 {
        self.published_objects.len() as u64
    }

    /// Full scan of all authoritative RCP databases.
    /// Ensures the D-Bus tree exactly matches database state.
    pub async fn refresh_full_tree(self: Arc<Self>) -> Result<()> {
        let mut active_paths = HashSet::new();

        tracing::info!("Refreshing D-Bus projection hierarchy...");

        // 1. Launch NonNet Enumeration Pool in the background immediately
        // This handles complex schemas concurrently while we work on OVSDB.
        let mirror_for_pool = Arc::clone(&self);
        let mut nonnet_join_set: tokio::task::JoinSet<Vec<String>> = tokio::task::JoinSet::new();
        
        // We capture paths discovered by the pool here
        let pool_handle = tokio::spawn(async move {
            let mut pool_paths = HashSet::new();
            if let Err(e) = mirror_for_pool.publish_nonnet_snapshot_pool(&mut pool_paths).await {
                tracing::warn!("NonNet background enumeration failed: {}", e);
            }
            pool_paths
        });

        // 2. Project Network State (OVSDB)
        if self.ovsdb.is_some() {
            tracing::debug!("Refreshing OVSDB projection...");
            if let Err(e) = self.publish_ovsdb_snapshot(&mut active_paths).await {
                tracing::warn!("OVSDB snapshot failed (non-fatal): {}", e);
            }
        }

        // 3. Wait for NonNet background enumeration to complete
        tracing::debug!("Finalizing NonNet background projection...");
        match pool_handle.await {
            Ok(pool_paths) => {
                active_paths.extend(pool_paths);
            }
            Err(e) => tracing::error!("NonNet pool task panicked: {}", e),
        }

        // 4. Remove stale objects that no longer exist in any authoritative store
        tracing::info!("Culling stale D-Bus objects...");
        self.remove_stale_publications(&active_paths).await?;
        tracing::info!("D-Bus projection refresh complete (Total paths: {})", active_paths.len());

        Ok(())
    }

    /// Project OVSDB rows selected by schema-derived RCP metadata.
    async fn publish_ovsdb_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let ovsdb = match &self.ovsdb {
            Some(o) => o,
            None => return Ok(()),
        };

        let specs = self.ovsdb_projection_specs().await?;
        if specs.is_empty() {
            tracing::debug!("No schema-derived OVSDB projection specs found; skipping OVSDB");
            return Ok(());
        }

        // Always publish the base path for each spec to ensure tree visibility
        for spec in &specs {
            let path = spec.base_path.clone();
            let table_obj = object::TableObject::new(
                spec.plugin_id.clone(),
                spec.object_type.clone(),
                spec.rcp_db.clone(),
                spec.rcp_table.clone(),
            );
            if let Ok(op) = ObjectPath::try_from(path.as_str()) {
                if let Err(error) = self.connection.object_server().at(op, table_obj).await {
                    tracing::warn!("Failed to publish OVSDB table object at {}: {}", path, error);
                } else {
                    self.published_objects.insert(path.clone(), ());
                    active_paths.insert(path);
                }
            }
        }

        let native_schema = match tokio::time::timeout(
            Duration::from_secs(OVSDB_TABLE_TIMEOUT_SECS),
            ovsdb.get_schema(),
        )
        .await
        {
            Ok(Ok(schema)) => schema,
            Ok(Err(error)) => {
                tracing::warn!(
                    "Skipping OVSDB projection; native schema unavailable: {}",
                    error
                );
                return Ok(());
            }
            Err(_) => {
                tracing::warn!(
                    "Skipping OVSDB projection; native schema timed out after {}s",
                    OVSDB_TABLE_TIMEOUT_SECS
                );
                return Ok(());
            }
        };
        let native_tables: HashSet<String> = native_schema
            .get("tables")
            .and_then(|v| v.as_object())
            .map(|tables| tables.keys().cloned().collect())
            .unwrap_or_default();

        let mut by_table: HashMap<String, ProjectionSpec> = HashMap::new();
        for spec in specs {
            if !native_tables.contains(&spec.rcp_table) {
                tracing::warn!(
                    plugin = %spec.plugin_id,
                    object_type = %spec.object_type,
                    table = %spec.rcp_table,
                    "Skipping OVSDB projection spec absent from native schema"
                );
                continue;
            }
            by_table.insert(spec.rcp_table.clone(), spec);
        }

        let mut join_set = tokio::task::JoinSet::new();
        for (table_name, spec) in by_table {
            let ovsdb = Arc::clone(ovsdb);
            join_set.spawn(async move {
                let result =
                    tokio::time::timeout(Duration::from_secs(OVSDB_TABLE_TIMEOUT_SECS), async {
                        ovsdb.select_table(&table_name).await
                    })
                    .await;

                let rows = match result {
                    Ok(Ok(rows)) => Ok(rows),
                    Ok(Err(error)) => Err(error.to_string()),
                    Err(_) => Err(format!("timed out after {}s", OVSDB_TABLE_TIMEOUT_SECS)),
                };

                (table_name, spec, rows)
            });
        }

        while let Some(result) = join_set.join_next().await {
            let Ok((table_name, spec, rows_result)) = result else {
                continue;
            };
            let rows = match rows_result {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::warn!(table = %table_name, "Skipping OVSDB table projection: {}", error);
                    continue;
                }
            };

            if rows.len() > LAZY_LOAD_THRESHOLD {
                let table_path = spec.dynamic_path();
                let summary = object::OvsdbTableSummaryObject::new(
                    table_name.clone(),
                    spec.id_field.clone(),
                    rows.len() as u64,
                    Arc::clone(ovsdb),
                );
                if let Err(error) = self
                    .connection
                    .object_server()
                    .at(&*table_path, summary)
                    .await
                {
                    tracing::warn!(
                        "Failed to publish OVSDB lazy table {}: {}",
                        table_path,
                        error
                    );
                } else {
                    self.published_objects.insert(table_path.clone(), ());
                    active_paths.insert(table_path);
                }
                continue;
            }

            for row in rows {
                let id = self.extract_id_for_spec(&row, &spec);
                let path = spec.object_path(&id);
                self.publish_object(&path, row.clone()).await?;
                active_paths.insert(path.clone());

                if table_name == "Interface" || table_name == "Port" {
                    let compat_path = format!(
                        "/org/freedesktop/network1/link/{}",
                        sanitize_path_segment(&id)
                    );
                    self.publish_object(&compat_path, row.clone()).await?;
                    active_paths.insert(compat_path);
                }
            }
        }

        Ok(())
    }

    async fn ovsdb_projection_specs(&self) -> Result<Vec<ProjectionSpec>> {
        let mut specs = Vec::new();
        for (plugin_id, table_schema) in self.nonnet_schema_tables().await? {
            specs.extend(
                schema_projection_specs(&plugin_id, &table_schema)
                    .into_iter()
                    .filter(|spec| spec.rcp_db == "ovsdb"),
            );
        }
        Ok(specs)
    }

    async fn nonnet_schema_tables(&self) -> Result<HashMap<String, Value>> {
        let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new(
            "get_schema",
            Value::Array(vec![Value::from("OpNonNet")]),
        );
        let schema_resp = self.nonnet.handle_request(schema_req).await;

        Ok(schema_resp
            .result
            .and_then(|schema| schema.get("tables").and_then(|v| v.as_object().cloned()))
            .map(|tables| {
                tables
                    .iter()
                    .map(|(name, schema)| (name.clone(), schema.clone()))
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn select_nonnet_rows(
        &self,
        db_name: &str,
        table_name: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<Value> {
        let select_req = op_jsonrpc::protocol::JsonRpcRequest::new(
            "transact",
            Value::Array(vec![
                Value::from(db_name),
                simd_json::json!({
                    "op": "select",
                    "table": table_name,
                    "offset": offset,
                    "limit": limit
                }),
            ]),
        );
        let resp = self.nonnet.handle_request(select_req).await;

        resp.result
            .and_then(|r: Value| r.as_array().and_then(|a| a.first().cloned()))
            .and_then(|r: Value| r.get("rows").cloned())
            .and_then(|v: Value| v.as_array().map(|a| a.to_vec()))
            .unwrap_or_default()
    }

    async fn count_nonnet_rows(&self, db_name: &str, table_name: &str) -> usize {
        let count_req = op_jsonrpc::protocol::JsonRpcRequest::new(
            "transact",
            Value::Array(vec![
                Value::from(db_name),
                simd_json::json!({"op": "count", "table": table_name}),
            ]),
        );
        let count_resp = self.nonnet.handle_request(count_req).await;
        count_resp
            .result
            .and_then(|r| {
                r.as_array()
                    .and_then(|a| a.first())
                    .and_then(|o| o.get("count"))
                    .and_then(|v| v.as_u64())
            })
            .unwrap_or(0) as usize
    }

    fn extract_id_for_spec(&self, row: &Value, spec: &ProjectionSpec) -> String {
        if let Some(field) = &spec.id_field {
            if let Some(id) = extract_field_id(row, field) {
                return id;
            }
        }
        self.extract_uuid(row)
    }

    /// Sequential NonNet projection for small tables or compatibility.
    async fn publish_nonnet_snapshot_sequential(
        &self,
        active_paths: &mut HashSet<String>,
    ) -> Result<()> {
        for (table_name, table_schema) in self.nonnet_schema_tables().await? {
            let specs: Vec<ProjectionSpec> = schema_projection_specs(&table_name, &table_schema)
                .into_iter()
                .filter(|spec| spec.rcp_db != "ovsdb")
                .collect();
            if specs.is_empty() {
                continue;
            }

            // Always publish the base path for each spec to ensure tree visibility
            for spec in &specs {
                let path = spec.base_path.clone();
                let table_obj = object::TableObject::new(
                    spec.plugin_id.clone(),
                    spec.object_type.clone(),
                    spec.rcp_db.clone(),
                    spec.rcp_table.clone(),
                );
                if let Ok(op) = ObjectPath::try_from(path.as_str()) {
                    if let Err(error) = self.connection.object_server().at(op, table_obj).await {
                        tracing::warn!("Failed to publish table object at {}: {}", path, error);
                    } else {
                        self.published_objects.insert(path.clone(), ());
                        active_paths.insert(path);
                    }
                }
            }

            let count = self.count_nonnet_rows("OpNonNet", &table_name).await;
            for spec in specs {
                if count > LAZY_LOAD_THRESHOLD {
                    let table_path = spec.dynamic_path();
                    let summary = object::TableSummaryObject::new(
                        "OpNonNet".to_string(),
                        table_name.clone(),
                        count as u64,
                        self.nonnet.clone(),
                    );
                    if let Err(error) = self
                        .connection
                        .object_server()
                        .at(&*table_path, summary)
                        .await
                    {
                        tracing::warn!("Failed to publish lazy table {}: {}", table_path, error);
                    } else {
                        self.published_objects.insert(table_path.clone(), ());
                        active_paths.insert(table_path);
                    }
                    continue;
                }

                for row in self
                    .select_nonnet_rows("OpNonNet", &table_name, 0, count)
                    .await
                    .into_iter()
                    .filter(|row| row_matches_projection(row, &spec))
                {
                    let id = self.extract_id_for_spec(&row, &spec);
                    let path = spec.object_path(&id);
                    self.publish_object(&path, row).await?;
                    active_paths.insert(path);
                }
            }
        }
        Ok(())
    }

    /// Schema-derived NonNet projection using a concurrent task pool.
    ///
    /// The NonNet schema carries plugin-derived projection metadata.
    /// Only `schema_derived=true` object types are published. Oversized tables
    /// get a single `LazyTableV1` summary under `/org/opdbus/v1/dynamic/`.
    async fn publish_nonnet_snapshot_pool(
        self: Arc<Self>,
        active_paths: &mut HashSet<String>,
    ) -> Result<()> {
        let mut join_set: tokio::task::JoinSet<Vec<String>> = tokio::task::JoinSet::new();

        for (table_name, table_schema) in self.nonnet_schema_tables().await? {
            let specs: Vec<ProjectionSpec> = schema_projection_specs(&table_name, &table_schema)
                .into_iter()
                .filter(|spec| spec.rcp_db != "ovsdb")
                .collect();
            if specs.is_empty() {
                continue;
            }

            // Always publish the base path for each spec to ensure tree visibility
            for spec in &specs {
                let path = spec.base_path.clone();
                let table_obj = object::TableObject::new(
                    spec.plugin_id.clone(),
                    spec.object_type.clone(),
                    spec.rcp_db.clone(),
                    spec.rcp_table.clone(),
                );
                if let Ok(op) = ObjectPath::try_from(path.as_str()) {
                    if let Err(error) = self.connection.object_server().at(op, table_obj).await {
                        tracing::warn!("Failed to publish table object at {}: {}", path, error);
                    } else {
                        self.published_objects.insert(path.clone(), ());
                        active_paths.insert(path);
                    }
                }
            }

            let count = self.count_nonnet_rows("OpNonNet", &table_name).await;
            for spec in specs {
                if count > LAZY_LOAD_THRESHOLD {
                    let table_path = spec.dynamic_path();
                    tracing::info!(
                        "Lazy-loading schema-derived table {}.{} ({} rows)",
                        table_name,
                        spec.object_type,
                        count
                    );
                    let summary = object::TableSummaryObject::new(
                        "OpNonNet".to_string(),
                        table_name.clone(),
                        count as u64,
                        self.nonnet.clone(),
                    );
                    if let Err(error) = self
                        .connection
                        .object_server()
                        .at(&*table_path, summary)
                        .await
                    {
                        tracing::warn!("Failed to publish lazy table {}: {}", table_path, error);
                    } else {
                        self.published_objects.insert(table_path.clone(), ());
                        active_paths.insert(table_path);
                    }
                    continue;
                }

                tracing::info!(
                    "Projecting schema-derived table {}.{} ({} objects) -> {}",
                    table_name,
                    spec.object_type,
                    count,
                    spec.base_path
                );

                for offset in (0..count).step_by(NONNET_CHUNK_SIZE) {
                    let mirror = Arc::clone(&self);
                    let table_name_inner = table_name.clone();
                    let spec_inner = spec.clone();

                    join_set.spawn(async move {
                        let mut paths = Vec::new();
                        for row in mirror
                            .select_nonnet_rows(
                                "OpNonNet",
                                &table_name_inner,
                                offset,
                                NONNET_CHUNK_SIZE,
                            )
                            .await
                            .into_iter()
                            .filter(|row| row_matches_projection(row, &spec_inner))
                        {
                            let id = mirror.extract_id_for_spec(&row, &spec_inner);
                            let path = spec_inner.object_path(&id);
                            if mirror.publish_object(&path, row).await.is_ok() {
                                paths.push(path);
                            }
                        }
                        paths
                    });
                }
            }
        }

        // Wait for all chunks to finish and collect active paths
        while let Some(res) = join_set.join_next().await {
            if let Ok(paths) = res {
                for path in paths {
                    active_paths.insert(path);
                }
            }
        }

        Ok(())
    }

    async fn publish_object(&self, path: &str, data: Value) -> Result<()> {
        tracing::debug!("Publishing object: {}", path);
        if self.published_objects.contains_key(path) {
            let op = ObjectPath::try_from(path)?;
            if let Ok(iface_ref) = self
                .connection
                .object_server()
                .interface::<_, object::MirrorObject>(op)
                .await
            {
                let mut guard = iface_ref.get_mut().await;
                if guard.update_data(data) {
                    guard.data_updated(iface_ref.signal_context()).await.ok();
                }
            }
            return Ok(());
        }

        let obj = object::MirrorObject::new(data);
        self.connection.object_server().at(path, obj).await?;
        self.published_objects.insert(path.to_string(), ());

        Ok(())
    }

    async fn remove_stale_publications(&self, active_paths: &HashSet<String>) -> Result<()> {
        let mut to_remove = Vec::new();
        for entry in self.published_objects.iter() {
            if !active_paths.contains(entry.key()) {
                to_remove.push(entry.key().clone());
            }
        }

        for path in to_remove {
            if let Ok(op) = ObjectPath::try_from(path.as_str()) {
                let _ = self
                    .connection
                    .object_server()
                    .remove::<object::MirrorObject, _>(op)
                    .await;
            }
            if let Ok(op) = ObjectPath::try_from(path.as_str()) {
                let _ = self
                    .connection
                    .object_server()
                    .remove::<object::TableSummaryObject, _>(op)
                    .await;
            }
            if let Ok(op) = ObjectPath::try_from(path.as_str()) {
                let _ = self
                    .connection
                    .object_server()
                    .remove::<object::OvsdbTableSummaryObject, _>(op)
                    .await;
            }
            self.published_objects.remove(&path);
        }

        Ok(())
    }

    pub fn list_published_paths(&self) -> Vec<String> {
        self.published_objects
            .iter()
            .map(|e| e.key().clone())
            .collect()
    }

    fn extract_uuid(&self, row: &Value) -> String {
        for field in ["uuid", "_uuid", "id", "name"] {
            if let Some(id) = extract_field_id(row, field) {
                return id;
            }
        }
        format!("anon_{}", self.fallback_id.fetch_add(1, Ordering::Relaxed))
    }
}

fn schema_projection_specs(plugin_id: &str, table_schema: &Value) -> Vec<ProjectionSpec> {
    let Some(object_types) = table_schema.get("object_types").and_then(|v| v.as_object()) else {
        return Vec::new();
    };

    object_types
        .iter()
        .filter_map(|(object_type, object_schema)| {
            let schema_derived = object_schema
                .get("schema_derived")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !schema_derived {
                return None;
            }

            let base_path = object_schema.get("base_path").and_then(|v| v.as_str())?;
            let rcp_db = object_schema
                .get("rcp_db")
                .and_then(|v| v.as_str())
                .unwrap_or("nonnet");
            let rcp_table = object_schema
                .get("rcp_table")
                .and_then(|v| v.as_str())
                .unwrap_or(plugin_id);
            let id_field = object_schema
                .get("id_field")
                .and_then(|v| v.as_str())
                .map(|field| field.to_string());

            Some(ProjectionSpec {
                plugin_id: plugin_id.to_string(),
                object_type: object_type.to_string(),
                base_path: unify_dbus_path(base_path),
                rcp_db: rcp_db.to_string(),
                rcp_table: rcp_table.to_string(),
                id_field,
            })
        })
        .collect()
}

fn row_matches_projection(row: &Value, spec: &ProjectionSpec) -> bool {
    if spec.rcp_db == "ovsdb" {
        return true;
    }

    for field in ["object_type", "type", "kind"] {
        if let Some(row_type) = row.get(field).and_then(|value| value.as_str()) {
            return row_type == spec.object_type;
        }
    }

    true
}

fn extract_field_id(row: &Value, field: &str) -> Option<String> {
    let value = row.get(field)?;
    if let Some(id) = value.as_str() {
        return Some(id.to_string());
    }
    if let Some(id) = value.as_u64() {
        return Some(id.to_string());
    }
    if let Some(id) = value.as_i64() {
        return Some(id.to_string());
    }
    if let Some(values) = value.as_array() {
        if values.len() == 2 && values.first().and_then(|v| v.as_str()) == Some("uuid") {
            return values
                .get(1)
                .and_then(|v| v.as_str())
                .map(|id| id.to_string());
        }
    }
    None
}

fn sanitize_dbus_path(path: &str) -> String {
    let segments: Vec<String> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(sanitize_path_segment)
        .collect();
    format!("/{}", segments.join("/"))
}

/// Convert any path to unified /org/opdbus/v1/ hierarchy
/// Input: /org/opdbus/hardware/cpu -> Output: /org/opdbus/v1/hardware/cpu
/// Input: /org/opdbus/incus/containers -> Output: /org/opdbus/v1/incus/containers
fn unify_dbus_path(path: &str) -> String {
    // First sanitize
    let sanitized = sanitize_dbus_path(path);
    
    // If already starts with /org/opdbus/v1, return as-is
    if sanitized.starts_with("/org/opdbus/v1") {
        return sanitized;
    }
    
    // If starts with /org/opdbus (without v1), insert v1
    if sanitized.starts_with("/org/opdbus") {
        return sanitized.replace("/org/opdbus", "/org/opdbus/v1");
    }
    
    // Otherwise, prepend /org/opdbus/v1
    format!("/org/opdbus/v1{}", sanitized)
}

fn sanitize_path_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub mod prelude {
    pub use super::DbusMirror;
}

#[cfg(test)]
mod tests {
    use super::sanitize_path_segment;

    #[test]
    fn sanitizes_dbus_path_segments() {
        assert_eq!(sanitize_path_segment("bridge-01"), "bridge_01");
        assert_eq!(sanitize_path_segment("6f0c8b4e-9d9f"), "6f0c8b4e_9d9f");
    }
}
