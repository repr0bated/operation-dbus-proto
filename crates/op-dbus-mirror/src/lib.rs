//! op-dbus-mirror: 1:1 D-Bus publication of internal databases
//!
//! This crate publishes the internal OVSDB and NonNet database structures as a
//! D-Bus object hierarchy without introducing a second source of truth.

use anyhow::Result;
use dashmap::DashMap;
use managed_objects::{
    build_interface_map, ManagedObjectRegistry, ObjectManagerInterface, OBJECT_MANAGER_PATH,
    PROJECTED_IFACE,
};
use op_core::types::BusType;
use op_grpc_bridge::{OperationGrpcServer, SchemaEngine};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_state::manager::StateManager;
use procfs::Current as _;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::sync::Arc;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};
use zbus::{connection::Builder, Connection};

pub mod dbus_interface;
pub mod event;
pub mod event_dispatcher;
pub mod event_sources;
pub mod heartbeat;
pub mod jsonrpc_interface;
pub mod managed_objects;
pub mod object;
pub mod plugin_interface;
pub mod session;
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
    /// Registry backing the org.freedesktop.DBus.ObjectManager at OBJECT_MANAGER_PATH.
    /// Tracks every plugin object so GetManagedObjects can enumerate them all.
    plugin_registry: ManagedObjectRegistry,
    /// Optional handle to the gRPC server so the ComponentRegistry can be
    /// mirrored into the D-Bus tree under /org/opdbus/v1/registry/.
    grpc_server: Option<Arc<OperationGrpcServer>>,
    /// StateManager for enumerating all registered plugins (active or not).
    state_manager: Option<Arc<StateManager>>,
    /// Current data and sequence numbers per object path
    current_data: DashMap<String, (Value, u64)>,
    /// Per-session state keyed by peer name
    pub sessions: DashMap<String, session::MirrorSession>,
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
            BusType::System => Builder::system()?.name("org.opdbus.v1")?.build().await?,
            BusType::Session => Builder::session()?.name("org.opdbus.v1")?.build().await?,
        };

        Ok(Self {
            ovsdb,
            nonnet,
            schema_engine,
            connection,
            published_objects: DashMap::new(),
            plugin_registry: Arc::new(DashMap::new()),
            grpc_server: None,
            state_manager: None,
            current_data: DashMap::new(),
            sessions: DashMap::new(),
        })
    }

    /// Attach a gRPC server so the ComponentRegistry is mirrored into D-Bus.
    pub fn with_grpc_server(mut self, grpc_server: Arc<OperationGrpcServer>) -> Self {
        self.grpc_server = Some(grpc_server);
        self
    }

    /// Attach the StateManager so all registered plugins are always visible in
    /// the managed objects tree (active or not).
    pub fn with_state_manager(mut self, state_manager: Arc<StateManager>) -> Self {
        self.state_manager = Some(state_manager);
        self
    }

    /// Start the mirror service.
    ///
    /// Performs an initial full-tree publication and then enters an event-driven
    /// loop to publish deltas from all data sources.
    pub async fn start(self: Arc<Self>) -> Result<()> {
        tracing::info!("Starting D-Bus mirror publication service...");

        // Initial full sync
        if let Err(e) = self.refresh_full_tree().await {
            tracing::error!("Initial D-Bus mirror sync failed: {}", e);
        }

        // Register ObjectManager at the root to manage EVERYTHING.
        let om = ObjectManagerInterface::new(self.plugin_registry.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1", om)
            .await?;

        // Register PluginsV1 at the plugins path.
        let plugin_iface = plugin_interface::PluginInterface::new();
        let plugin_snap = plugin_iface.snapshot_handle();
        self.connection
            .object_server()
            .at("/opdbus/v1/plugins", plugin_iface)
            .await?;

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

        // Create event dispatcher
        let dispatcher = crate::event_dispatcher::EventDispatcher::new(
            self.clone(),
            self.ovsdb.clone(),
            self.nonnet.clone(),
            self.state_manager.clone(),
            self.grpc_server.clone(),
        );

        // Spawn all event sources
        if let Err(e) = dispatcher.spawn_event_sources().await {
            tracing::error!("Failed to spawn event sources: {}", e);
        }

        // Spawn heartbeat task
        if let Err(e) =
            crate::heartbeat::spawn_heartbeat_task(self.clone(), dispatcher.broadcast_tx.clone())
                .await
        {
            tracing::error!("Failed to spawn heartbeat task: {}", e);
        }

        // Run event loop
        if let Err(e) = dispatcher.run_event_loop().await {
            tracing::error!("Event loop failed: {}", e);
        }

        // Populate fixed objects immediately on startup.
        self.refresh_plugin_snapshot(&plugin_snap).await;

        // Watch ComponentRegistry for live register/deregister events
        if let Some(grpc) = &self.grpc_server {
            let (_, mut rx) = grpc.registry_watch().await;
            let mirror = self.clone();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => mirror.apply_registry_event(event).await,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "ComponentRegistry watcher lagged by {} events, resyncing",
                                n
                            );
                            if let Err(e) = mirror.refresh_full_tree().await {
                                tracing::error!("Registry resync failed: {}", e);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

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
        tracing::info!("DEBUG: Starting full tree refresh");

        // 1. Scan OVSDB (Authoritative for Network)
        tracing::info!("DEBUG: Publishing OVSDB snapshot");
        if let Err(e) = self.publish_ovsdb_snapshot(&mut active_paths).await {
            tracing::warn!("OVSDB snapshot failed: {}", e);
        }

        // 2. Scan Procfs (Host state)
        tracing::info!("DEBUG: Publishing host snapshot");
        if let Err(e) = self.publish_host_snapshot(&mut active_paths).await {
            tracing::warn!("Procfs snapshot failed: {}", e);
        }

        // 3. Scan NonNet (Authoritative for Plugins not in Enterprise DB)
        tracing::info!("DEBUG: Publishing NonNet snapshot");
        if let Err(e) = self.publish_nonnet_snapshot(&mut active_paths).await {
            tracing::warn!("NonNet snapshot failed: {}", e);
        }

        // 4. gRPC ComponentRegistry snapshot (host/plugin now use fixed objects)
        if let Err(e) = self.publish_registry_snapshot(&mut active_paths).await {
            tracing::warn!("ComponentRegistry snapshot failed: {}", e);
        }

        // 6. Scan freedesktop system services
        if let Err(e) = self.publish_system_services(&mut active_paths).await {
            tracing::warn!("System services snapshot failed: {}", e);
        }

        // 5. Network interfaces and WireGuard peers
        if let Err(e) = self.publish_network_snapshot(&mut active_paths).await {
            tracing::warn!("Network snapshot failed: {}", e);
        }

        // 7. Running processes
        if let Err(e) = self.publish_process_snapshot(&mut active_paths).await {
            tracing::warn!("Process snapshot failed: {}", e);
        }

        // 8. Keep plugin objects published even when a plugin is currently inactive.
        if let Err(e) = self.publish_plugin_snapshot(&mut active_paths).await {
            tracing::warn!("Plugin snapshot failed: {}", e);
        }

        // 9. Remove any D-Bus objects that no longer exist in any authority
        self.remove_stale_publications(&active_paths).await?;

        Ok(())
    }

    async fn publish_host_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let sections = vec![
            "cpuinfo",
            "meminfo",
            "loadavg",
            "uptime",
            "stat",
            "vmstat",
            "diskstats",
            "mounts",
            "version",
        ];

        for section in sections {
            let path = format!("/org/opdbus/v1/host/{}", section);
            let data = match section {
                "meminfo" => self.gather_meminfo().await?,
                "cpuinfo" => self.gather_cpuinfo().await?,
                "loadavg" => self.gather_loadavg().await?,
                _ => serde_json::json!({ "status": "available" }),
            };
            self.publish_object(&path, data).await?;
            active_paths.insert(path);
        }

        Ok(())
    }

    async fn gather_meminfo(&self) -> Result<Value> {
        match procfs::Meminfo::current() {
            Ok(meminfo) => Ok(serde_json::to_value(meminfo).unwrap_or_default()),
            Err(e) => {
                tracing::warn!("Failed to read /proc/meminfo: {}", e);
                Ok(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    async fn gather_cpuinfo(&self) -> Result<Value> {
        match procfs::CpuInfo::current() {
            Ok(cpuinfo) => Ok(serde_json::to_value(cpuinfo).unwrap_or_default()),
            Err(e) => {
                tracing::warn!("Failed to read /proc/cpuinfo: {}", e);
                Ok(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    async fn gather_loadavg(&self) -> Result<Value> {
        match procfs::LoadAverage::current() {
            Ok(loadavg) => Ok(serde_json::to_value(loadavg).unwrap_or_default()),
            Err(e) => {
                tracing::warn!("Failed to read /proc/loadavg: {}", e);
                Ok(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    async fn publish_ovsdb_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        tracing::debug!("Scanning OVSDB for projection...");
        self.publish_object(
            "/org/opdbus/v1/ovsdb",
            serde_json::json!({
                "kind": "database",
                "database": "Open_vSwitch",
                "source": "ovsdb",
            }),
        )
        .await?;
        active_paths.insert("/org/opdbus/v1/ovsdb".to_string());
        self.publish_object(
            "/org/opdbus/v1/ovs",
            serde_json::json!({
                "kind": "database_alias",
                "database": "Open_vSwitch",
                "source": "ovsdb",
                "canonical_path": "/org/opdbus/v1/ovsdb",
            }),
        )
        .await?;
        active_paths.insert("/org/opdbus/v1/ovs".to_string());

        let dump_serde = self.ovsdb.dump_db("Open_vSwitch").await?;
        tracing::info!("DEBUG: OVSDB dump retrieved");
        // Convert serde_json::Value → serde_json::Value for compatibility
        let dump: Value = {
            let s = serde_json::to_string(&dump_serde)
                .map_err(|e| anyhow::anyhow!("dump_db serialize error: {}", e))?;
            serde_json::from_str(&s).map_err(|e| anyhow::anyhow!("dump_db parse error: {}", e))?
        };

        if let Value::Object(tables) = dump {
            tracing::info!("DEBUG: OVSDB dump contains {} tables", tables.len());
            let table_list: Vec<_> = tables
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            for (table_name, table_data) in table_list {
                let rows = table_data
                    .get("rows")
                    .and_then(|r| r.as_array())
                    .cloned()
                    .unwrap_or_default();
                let table_payload = serde_json::json!({
                    "kind": "table",
                    "database": "Open_vSwitch",
                    "table": table_name,
                    "row_count": rows.len(),
                });
                let table_path = format!("/org/opdbus/v1/ovsdb/{}", table_name);
                self.publish_object(&table_path, table_payload.clone())
                    .await?;
                active_paths.insert(table_path);

                let ovs_table_path = format!("/org/opdbus/v1/ovs/{}", table_name);
                self.publish_object(&ovs_table_path, table_payload).await?;
                active_paths.insert(ovs_table_path);

                if let Some(rows) = table_data.get("rows").and_then(|r| r.as_array()) {
                    tracing::info!("DEBUG: OVSDB table {} has {} rows", table_name, rows.len());
                    let rows_vec: Vec<_> = rows.to_vec();
                    for (idx, row_data) in rows_vec.into_iter().enumerate() {
                        let row_id = Self::extract_uuid(&row_data);
                        let id = if row_id == "unknown" {
                            idx.to_string()
                        } else {
                            row_id
                        };
                        let path = format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, id);
                        self.publish_object(&path, row_data).await?;
                        active_paths.insert(path.clone());

                        let ovs_path = format!("/org/opdbus/v1/ovs/{}/{}", table_name, id);
                        self.publish_object(
                            &ovs_path,
                            serde_json::json!({
                                "canonical_path": path,
                                "database": "Open_vSwitch",
                                "table": table_name,
                            }),
                        )
                        .await?;
                        active_paths.insert(ovs_path);
                    }
                }
            }
        }

        Ok(())
    }

    async fn publish_nonnet_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        tracing::info!("DEBUG: NonNet snapshot started");
        self.publish_object(
            "/org/opdbus/v1/nonnet",
            serde_json::json!({
                "kind": "database_root",
                "source": "nonnet",
            }),
        )
        .await?;
        active_paths.insert("/org/opdbus/v1/nonnet".to_string());

        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", simd_json::json!([]));
        let response = self.nonnet.handle_request(request).await;

        let dbs: Vec<Value> = response
            .result
            .and_then(|v| serde_json::to_value(&v).ok())
            .and_then(|v| v.as_array().map(|a| a.to_vec()))
            .unwrap_or_default();

        tracing::info!("DEBUG: NonNet has {} databases", dbs.len());

        for db_name_val in dbs {
            if let Some(db_name) = db_name_val.as_str() {
                tracing::info!("DEBUG: Scanning NonNet DB: {}", db_name);
                let db_path = format!(
                    "/org/opdbus/v1/nonnet/{}",
                    Self::sanitize_dbus_path_segment(db_name)
                );
                self.publish_object(
                    &db_path,
                    serde_json::json!({
                        "kind": "database",
                        "database": db_name,
                        "source": "nonnet",
                    }),
                )
                .await?;
                active_paths.insert(db_path.clone());

                let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new(
                    "get_schema",
                    simd_json::json!([db_name]),
                );
                let schema_resp = self.nonnet.handle_request(schema_req).await;

                let schema_serde: Option<Value> = schema_resp
                    .result
                    .and_then(|v| serde_json::to_value(&v).ok());
                if let Some(tables) =
                    schema_serde.and_then(|s| s.get("tables").and_then(|v| v.as_object().cloned()))
                {
                    tracing::info!("DEBUG: NonNet DB {} has {} tables", db_name, tables.len());
                    let table_names: Vec<String> = tables.keys().map(|k| k.to_string()).collect();
                    for table_name in table_names {
                        let table_path = format!(
                            "{}/{}",
                            db_path,
                            Self::sanitize_dbus_path_segment(&table_name)
                        );
                        let dump_req = op_jsonrpc::protocol::JsonRpcRequest::new(
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
                        let dump_resp = self.nonnet.handle_request(dump_req).await;

                        let rows = dump_resp.result.and_then(|v| serde_json::to_value(&v).ok());
                        let rows = rows
                            .as_ref()
                            .and_then(|r| r.as_array())
                            .and_then(|results| results.first())
                            .and_then(|result| result.get("rows"))
                            .and_then(|v| v.as_array().map(|a| a.to_vec()))
                            .unwrap_or_default();

                        self.publish_object(
                            &table_path,
                            serde_json::json!({
                                "kind": "table",
                                "database": db_name,
                                "table": table_name,
                                "row_count": rows.len(),
                            }),
                        )
                        .await?;
                        active_paths.insert(table_path.clone());

                        tracing::info!(
                            "DEBUG: NonNet DB {} table {} has {} rows",
                            db_name,
                            table_name,
                            rows.len()
                        );
                        for (idx, row) in rows.into_iter().enumerate() {
                            let id = Self::extract_uuid(&row);
                            let id = if id == "unknown" { idx.to_string() } else { id };
                            let path =
                                format!("{}/{}", table_path, Self::sanitize_dbus_path_segment(&id));
                            self.publish_object(&path, row).await?;
                            active_paths.insert(path);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn publish_network_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        use std::process::Command;

        // Read network interfaces from /proc/net/dev
        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                let name = parts[0].trim_end_matches(':');
                let path = format!(
                    "/org/opdbus/v1/network/interface/{}",
                    Self::sanitize_path_segment(name)
                );
                let data = serde_json::json!({ "name": name, "source": "/proc/net/dev" });
                self.publish_object(&path, data).await?;
                active_paths.insert(path);
            }
        }

        // Read WireGuard peers from `wg show all dump`
        if let Ok(out) = Command::new("wg").args(["show", "all", "dump"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut current_iface = String::new();
            for line in text.lines() {
                let cols: Vec<&str> = line.split('\t').collect();
                if cols.len() == 5 {
                    // Interface line: iface pubkey privkey listen_port fwmark
                    current_iface = cols[0].to_string();
                    let path = format!(
                        "/org/opdbus/v1/network/wireguard/{}",
                        Self::sanitize_path_segment(&current_iface)
                    );
                    let data = serde_json::json!({
                        "interface": current_iface,
                        "public_key": cols[1],
                        "listen_port": cols[3],
                        "fwmark": cols[4],
                    });
                    self.publish_object(&path, data).await?;
                    active_paths.insert(path);
                } else if cols.len() >= 8 && !current_iface.is_empty() {
                    // Peer line: iface pubkey preshared endpoint allowed_ips latest_handshake rx tx keepalive
                    let peer_key = cols[1];
                    let safe_key = peer_key.replace(['/', '+'], "_").replace('=', "");
                    let path = format!(
                        "/org/opdbus/v1/network/wireguard/{}/peer/{}",
                        Self::sanitize_path_segment(&current_iface),
                        safe_key
                    );
                    let data = serde_json::json!({
                        "interface": current_iface,
                        "public_key": peer_key,
                        "endpoint": cols[3],
                        "allowed_ips": cols[4],
                        "latest_handshake": cols[5].parse::<u64>().unwrap_or(0),
                        "transfer_rx": cols[6].parse::<u64>().unwrap_or(0),
                        "transfer_tx": cols[7].parse::<u64>().unwrap_or(0),
                    });
                    self.publish_object(&path, data).await?;
                    active_paths.insert(path);
                }
            }
        }

        Ok(())
    }

    async fn publish_process_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        use procfs::process::all_processes;

        if let Ok(procs) = all_processes() {
            for proc in procs.flatten() {
                let pid = proc.pid();
                let stat = match proc.stat() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let path = format!("/org/opdbus/v1/process/_{}", pid);
                let data = serde_json::json!({
                    "pid": pid,
                    "name": stat.comm,
                    "state": stat.state().map(|s| format!("{:?}", s)).unwrap_or_default(),
                    "ppid": stat.ppid,
                    "threads": stat.num_threads,
                });
                self.publish_object(&path, data).await?;
                active_paths.insert(path);
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
            if name_str.starts_with(':')
                || name_str.starts_with("org.opdbus")
                || Self::SKIP_SERVICES.contains(&name_str)
            {
                continue;
            }

            // Walk the full object tree for this service recursively
            let mut queue: Vec<String> = vec!["/".to_string()];
            while let Some(obj_path) = queue.pop() {
                let proxy = match zbus::fdo::IntrospectableProxy::builder(&system_conn)
                    .destination(name_str)
                    .and_then(|b| b.path(obj_path.as_str()))
                {
                    Ok(b) => match b.build().await {
                        Ok(p) => p,
                        Err(_) => continue,
                    },
                    Err(_) => continue,
                };

                let xml = match proxy.introspect().await {
                    Ok(x) => x,
                    Err(_) => continue,
                };

                let node = match zbus_xml::Node::try_from(xml.as_str()) {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                // Enqueue child paths for recursive walk
                for child in node.nodes() {
                    if let Some(child_name) = child.name() {
                        let child_path = if obj_path == "/" {
                            format!("/{}", child_name)
                        } else {
                            format!("{}/{}", obj_path.trim_end_matches('/'), child_name)
                        };
                        queue.push(child_path);
                    }
                }

                // Only publish nodes that have meaningful interfaces
                let mut interfaces = Vec::new();
                let mut methods = Vec::new();
                let mut properties = Vec::new();
                let mut signals = Vec::new();

                for iface in node.interfaces() {
                    let iface_name: String = iface.name().to_string();
                    if iface_name == "org.freedesktop.DBus.Introspectable"
                        || iface_name == "org.freedesktop.DBus.Peer"
                        || iface_name == "org.freedesktop.DBus.Properties"
                    {
                        continue;
                    }
                    interfaces.push(Value::from(iface_name));
                    for m in iface.methods() {
                        methods.push(Value::from(m.name().to_string()));
                    }
                    for p in iface.properties() {
                        properties.push(Value::from(p.name().to_string()));
                    }
                    for s in iface.signals() {
                        signals.push(Value::from(s.name().to_string()));
                    }
                }

                if interfaces.is_empty() {
                    continue;
                }

                // Map the real object path into our namespace
                let safe_obj = obj_path
                    .trim_start_matches('/')
                    .replace(['/', '-'], "_");
                let safe_svc = name_str.replace('.', "/").replace('-', "_");
                let mirror_path = if safe_obj.is_empty() {
                    format!("/org/opdbus/v1/system/{}", safe_svc)
                } else {
                    format!("/org/opdbus/v1/system/{}/{}", safe_svc, safe_obj)
                };

                let data = serde_json::json!({
                    "service": name_str,
                    "path": obj_path,
                    "interfaces": interfaces,
                    "methods": methods,
                    "properties": properties,
                    "signals": signals,
                });

                self.publish_object(&mirror_path, data).await?;
                active_paths.insert(mirror_path);
            }
        }

        Ok(())
    }

    async fn publish_registry_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let grpc = match &self.grpc_server {
            Some(g) => g,
            None => return Ok(()),
        };
        let (components, _) = grpc.registry_watch().await;
        for info in components {
            let path = Self::registry_dbus_path(&info.component_id);
            let data = Self::component_info_to_value(&info);
            self.publish_object(&path, data).await?;
            active_paths.insert(path);
        }
        Ok(())
    }

    /// Handle a single live ComponentRegistry event from the broadcast channel.
    async fn apply_registry_event(&self, event: op_grpc_bridge::proto::registry::RegistryEvent) {
        use op_grpc_bridge::proto::registry::RegistryEventType;
        let event_type = RegistryEventType::try_from(event.event_type)
            .unwrap_or(RegistryEventType::RegistryEventRegistered);

        match event_type {
            RegistryEventType::RegistryEventRegistered
            | RegistryEventType::RegistryEventUpdated => {
                if let Some(info) = event.component {
                    let path = Self::registry_dbus_path(&info.component_id);
                    let data = Self::component_info_to_value(&info);
                    if let Err(e) = self.publish_object(&path, data).await {
                        tracing::warn!("registry publish failed for {}: {}", path, e);
                    }
                }
            }
            RegistryEventType::RegistryEventDeregistered => {
                if let Some(info) = event.component {
                    let path = Self::registry_dbus_path(&info.component_id);
                    let op = match ObjectPath::try_from(path.as_str()) {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let _ = self
                        .connection
                        .object_server()
                        .remove::<object::MirrorObject, _>(op)
                        .await;
                    self.published_objects.remove(&path);
                }
            }
            _ => {}
        }
    }

    fn registry_dbus_path(component_id: &str) -> String {
        let safe = component_id.replace(['.', '-', ':'], "_");
        format!("/org/opdbus/v1/registry/{}", safe)
    }

    fn plugin_dbus_path(plugin_id: &str) -> String {
        format!(
            "/opdbus/v1/plugins/{}",
            Self::sanitize_dbus_path_segment(plugin_id)
        )
    }

    fn is_permanent_plugin_path(path: &str) -> bool {
        if !path.starts_with("/opdbus/v1/plugins/") {
            return false;
        }

        let remainder = &path["/opdbus/v1/plugins/".len()..];
        !remainder.is_empty() && !remainder.contains('/')
    }

    fn sanitize_dbus_path_segment(segment: &str) -> String {
        let mut out = String::with_capacity(segment.len());
        for ch in segment.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                out.push(ch);
            } else {
                out.push('_');
            }
        }

        if out.is_empty() {
            "_".to_string()
        } else {
            out
        }
    }

    fn component_info_to_value(info: &op_grpc_bridge::proto::registry::ComponentInfo) -> Value {
        use serde_json::Map;
        let mut map = Map::new();
        map.insert(
            "component_id".into(),
            Value::from(info.component_id.clone()),
        );
        map.insert(
            "component_type".into(),
            Value::from(info.component_type.clone()),
        );
        map.insert("name".into(), Value::from(info.name.clone()));
        map.insert("description".into(), Value::from(info.description.clone()));
        map.insert("endpoint".into(), Value::from(info.endpoint.clone()));
        map.insert("version".into(), Value::from(info.version.clone()));
        map.insert("status".into(), Value::from(info.status as i64));
        map.insert("schema_json".into(), Value::from(info.schema_json.clone()));
        let caps: Vec<Value> = info
            .capabilities
            .iter()
            .map(|s| Value::from(s.clone()))
            .collect();
        map.insert("capabilities".into(), Value::Array(caps));
        let mut meta = Map::new();
        for (k, v) in &info.metadata {
            meta.insert(k.clone(), Value::from(v.clone()));
        }
        map.insert("metadata".into(), Value::Object(meta));
        Value::Object(map)
    }

    /// Push current plugin state into the fixed PluginInterface object.
    async fn refresh_plugin_snapshot(&self, handle: &plugin_interface::PluginSnapshot) {
        let sm = match &self.state_manager {
            Some(sm) => sm.clone(),
            None => return,
        };
        let live_state_raw = sm.query_current_state().await.unwrap_or_default();
        let live_state: std::collections::HashMap<String, Value> = live_state_raw
            .into_iter()
            .map(|(k, v)| (k, serde_json::to_value(&v).unwrap_or_default()))
            .collect();
        let mut map = std::collections::HashMap::new();
        for name in sm.list_plugins() {
            let json = match live_state.get(&name) {
                Some(state) => {
                    let mut obj = match state {
                        Value::Object(o) => o.clone(),
                        other => {
                            let mut m = Map::new();
                            m.insert("state".into(), other.clone());
                            m
                        }
                    };
                    obj.insert("active".into(), Value::from(true));
                    serde_json::to_string(&Value::Object(obj)).unwrap_or_default()
                }
                None => format!("{{\"active\":false,\"name\":{:?}}}", name),
            };
            map.insert(name, json);
        }
        *handle.write().await = map;
    }

    /// Publish every registered plugin as a stable D-Bus object.
    ///
    /// If a plugin cannot currently provide state, it still appears in the tree
    /// with `active: false`.
    async fn publish_plugin_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let sm = match &self.state_manager {
            Some(sm) => sm.clone(),
            None => return Ok(()),
        };

        let live_state_raw = sm.query_current_state().await.unwrap_or_default();
        let live_state: std::collections::HashMap<String, Value> = live_state_raw
            .into_iter()
            .map(|(k, v)| (k, serde_json::to_value(&v).unwrap_or_default()))
            .collect();

        for plugin_name in sm.list_plugins() {
            let path = Self::plugin_dbus_path(&plugin_name);
            let data = match live_state.get(&plugin_name) {
                Some(state) => {
                    let mut obj = match state {
                        Value::Object(o) => o.clone(),
                        other => {
                            let mut m = Map::new();
                            m.insert("state".into(), other.clone());
                            m
                        }
                    };
                    obj.insert("active".into(), Value::from(true));
                    obj.insert("name".into(), Value::from(plugin_name.clone()));
                    Value::Object(obj)
                }
                None => serde_json::json!({
                    "active": false,
                    "name": plugin_name.clone(),
                }),
            };

            self.publish_object(&path, data.clone()).await?;
            active_paths.insert(path.clone());

            if live_state.contains_key(&plugin_name) {
                self.publish_plugin_children(&path, &data, active_paths)
                    .await?;
            }
        }

        Ok(())
    }

    async fn publish_plugin_children(
        &self,
        root_path: &str,
        data: &Value,
        active_paths: &mut HashSet<String>,
    ) -> Result<()> {
        let mut projected = Vec::new();
        self.collect_plugin_children(root_path, data, &mut projected);

        for (path, value) in projected {
            self.publish_object(&path, value.clone()).await?;
            active_paths.insert(path.clone());
        }

        Ok(())
    }

    fn collect_plugin_children(
        &self,
        root_path: &str,
        data: &Value,
        out: &mut Vec<(String, Value)>,
    ) {
        match data {
            Value::Object(map) => {
                for (key, value) in map.iter() {
                    let child_path = format!(
                        "{}/{}",
                        root_path,
                        Self::sanitize_dbus_path_segment(key.as_str())
                    );
                    out.push((child_path.clone(), Self::child_value_payload(value)));
                    self.collect_plugin_children(&child_path, value, out);
                }
            }
            Value::Array(items) => {
                for (idx, value) in items.iter().enumerate() {
                    let child_path = format!("{}/{}", root_path, idx);
                    out.push((child_path.clone(), Self::child_value_payload(value)));
                    self.collect_plugin_children(&child_path, value, out);
                }
            }
            _ => {}
        }
    }

    fn child_value_payload(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(map.clone()),
            Value::Array(items) => {
                let mut payload = serde_json::Map::new();
                payload.insert("items".into(), Value::Array(items.clone()));
                Value::Object(payload)
            }
            scalar => {
                let mut payload = serde_json::Map::new();
                payload.insert("value".into(), scalar.clone());
                Value::Object(payload)
            }
        }
    }

    async fn publish_object(&self, path: &str, data: Value) -> Result<()> {
        // Get current data and sequence from the store
        let mut entry = self
            .current_data
            .entry(path.to_string())
            .or_insert_with(|| (serde_json::json!({}), 0u64));
        let (current_data, sequence) = &mut *entry;

        // Check if data has changed
        let changed = *current_data != data;

        if changed {
            // Increment sequence number
            *sequence += 1;

            // Update stored data
            *current_data = data.clone();

            if self.published_objects.contains_key(path) {
                // Object already registered — update data in-place and signal if changed.
                if let Ok(iface_ref) = self
                    .connection
                    .object_server()
                    .interface::<_, object::MirrorObject>(path)
                    .await
                {
                    let _ = iface_ref.get_mut().await.update_data(data.clone());
                    // Emit PropertiesChanged with only changed fields
                    let emitter = iface_ref.signal_emitter();
                    let _ = iface_ref.get().await.data_updated(emitter).await;
                    // Ensure the ObjectManager knows the properties changed as well
                    self.register_in_object_manager(path, &data).await;
                }
            } else {
                let obj = object::MirrorObject::new(data.clone());
                self.connection.object_server().at(path, obj).await?;
                self.published_objects.insert(path.to_string(), ());

                self.register_in_object_manager(path, &data).await;
            }
        }

        Ok(())
    }

    /// Load plugin state into the mirror (Seeding).
    ///
    /// Each plugin gets both a `MirrorObject` at `/opdbus/v1/plugins/{id}`
    /// and an entry in the `ObjectManagerInterface` registry so that
    /// `GetManagedObjects` immediately returns all loaded plugins.
    pub async fn load_plugin_state(&self, plugins: &std::collections::HashMap<String, Value>) {
        for (plugin_id, state) in plugins {
            let path = Self::plugin_dbus_path(plugin_id);
            if let Err(e) = self.publish_object(&path, state.clone()).await {
                tracing::error!("Failed to seed mirror for {}: {}", plugin_id, e);
                continue;
            }
            let mut active_paths = HashSet::new();
            if let Err(e) = self
                .publish_plugin_children(&path, state, &mut active_paths)
                .await
            {
                tracing::warn!(
                    "Failed to seed child plugin objects for {}: {}",
                    plugin_id,
                    e
                );
            }
        }
    }

    /// Insert a plugin object into the ObjectManager registry and emit
    /// `InterfacesAdded` if the ObjectManager interface is already up.
    async fn register_in_object_manager(&self, path: &str, data: &Value) {
        let op = match OwnedObjectPath::try_from(path.to_string()) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("register_in_object_manager: invalid path {path}: {e}");
                return;
            }
        };

        let json_str = serde_json::to_string(data).unwrap_or_default();
        let existed = self
            .plugin_registry
            .insert(op.clone(), build_interface_map(&json_str))
            .is_some();

        // Best-effort: emit InterfacesAdded only when the object first appears.
        if existed {
            return;
        }

        match self
            .connection
            .object_server()
            .interface::<_, ObjectManagerInterface>(OBJECT_MANAGER_PATH)
            .await
        {
            Ok(iface_ref) => {
                let emitter = iface_ref.signal_emitter();
                if let Err(e) = ObjectManagerInterface::interfaces_added(
                    emitter,
                    op,
                    build_interface_map(&json_str),
                )
                .await
                {
                    tracing::warn!("InterfacesAdded signal failed for {path}: {e}");
                }
            }
            Err(_) => {
                // ObjectManager not yet registered — registry is already updated,
                // so GetManagedObjects will return this entry once it comes up.
            }
        }
    }

    /// Remove a plugin object from the ObjectManager registry and emit
    /// `InterfacesRemoved`.
    async fn deregister_from_object_manager(&self, path: &str) {
        let op = match OwnedObjectPath::try_from(path.to_string()) {
            Ok(p) => p,
            Err(_) => return,
        };

        if self.plugin_registry.remove(&op).is_none() {
            return; // was not a managed plugin object
        }

        if let Ok(iface_ref) = self
            .connection
            .object_server()
            .interface::<_, ObjectManagerInterface>(OBJECT_MANAGER_PATH)
            .await
        {
            let emitter = iface_ref.signal_emitter();
            let interfaces = vec![PROJECTED_IFACE.to_string()];
            if let Err(e) =
                ObjectManagerInterface::interfaces_removed(emitter, op, interfaces).await
            {
                tracing::warn!("InterfacesRemoved signal failed for {path}: {e}");
            }
        }
    }

    async fn remove_stale_publications(&self, active_paths: &HashSet<String>) -> Result<()> {
        let mut to_remove = Vec::new();
        for entry in self.published_objects.iter() {
            let key = entry.key();
            // Plugin objects are permanent — they stay in the tree with active:false
            // rather than being removed when inactive.
            if Self::is_permanent_plugin_path(key) {
                continue;
            }
            if !active_paths.contains(key) {
                to_remove.push(key.clone());
            }
        }

        for path in to_remove {
            let op = ObjectPath::try_from(path.as_str())?;
            self.connection
                .object_server()
                .remove::<object::MirrorObject, _>(op)
                .await?;
            self.published_objects.remove(&path);

            // If this was a plugin-managed object, remove it from the registry
            // and emit InterfacesRemoved.
            if path.starts_with("/opdbus/v1/plugins/") {
                self.deregister_from_object_manager(&path).await;
            }
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

    fn extract_uuid(row: &Value) -> String {
        if let Some(uuid_val) = row.get("_uuid") {
            if let Some(uuid_arr) = uuid_val.as_array() {
                if uuid_arr.len() == 2 && uuid_arr[0].as_str() == Some("uuid") {
                    if let Some(uuid_str) = uuid_arr[1].as_str() {
                        return Self::sanitize_path_segment(uuid_str);
                    }
                }
            }
        }
        if let Some(uuid) = row.get("uuid").and_then(|v| v.as_str()) {
            return Self::sanitize_path_segment(uuid);
        }
        if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
            return Self::sanitize_path_segment(id);
        }
        if let Some(s) = row.get("name").and_then(|v| v.as_str()) {
            return Self::sanitize_path_segment(s);
        }
        "unknown".to_string()
    }

    fn sanitize_path_segment(raw: &str) -> String {
        raw.chars()
            .map(|ch| match ch {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => ch,
                '-' | '.' | ':' | ' ' | '/' => '_',
                _ => '_',
            })
            .collect()
    }
}

fn entry_path_to_dbus(path: &str) -> String {
    path.to_string()
}

pub mod prelude {
    pub use super::DbusMirror;
}
