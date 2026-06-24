//! Mutation Engine - The Authoritative Source for State and Schema DNA
//!
//! The Mutation Engine is the central coordinator that:
//! - Authoritatively routes all mutations (gRPC and D-Bus)
//! - Ensures all state changes are strictly recorded in the Event Chain (Audit Log)
//! - Broadcasts authoritative state changes to gRPC subscribers
//! - Directly manages authoritative RCP stores (OVSDB, NonNet, SQLite)

use async_trait::async_trait;
use serde_json;
use simd_json::prelude::{ValueAsContainer, ValueAsMutContainer, ValueAsScalar, ValueObjectAccess};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, OnceCell, RwLock, Semaphore};
use zbus::zvariant::OwnedValue as ZOwnedValue;
use zbus::{Connection, Proxy};

use base64::Engine;
use op_identity::{read_sled, write_sled_full};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::rovs_proxy::OvsdbDbusClient;
use op_state_store::{Decision, EventChain, OperationType};

/// A state change projected from the authoritative system bus
#[derive(Debug, Clone)]
pub struct StateChange {
    pub change_id: String,
    pub event_id: u64,
    pub plugin_id: String,
    pub object_path: String,
    pub change_type: ChangeType,
    pub member_name: Option<String>,
    pub old_value: Option<simd_json::OwnedValue>,
    pub new_value: simd_json::OwnedValue,
    pub tags_touched: Vec<String>,
    pub event_hash: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub actor_id: String,
    pub source: ChangeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    PropertySet,
    PropertyDelete,
    MethodCall,
    Signal,
    ObjectAdded,
    ObjectRemoved,
    SchemaMigration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeSource {
    DBus,
    Grpc,
    Internal,
}

pub struct MutationEngine {
    /// Authoritative Event Chain
    pub event_chain: Arc<RwLock<EventChain>>,
    /// Real-time change projection channel
    change_tx: broadcast::Sender<StateChange>,
    /// State cache for instant gRPC retrieval
    state_cache: Arc<RwLock<HashMap<String, simd_json::OwnedValue>>>,
    /// System D-Bus connection authority
    pub dbus_connection: Arc<OnceCell<Connection>>,
    /// Session bus connection for projection tree introspection
    session_bus: Arc<OnceCell<Connection>>,
    /// Resource limiter for D-Bus operations
    #[allow(dead_code)]
    dbus_call_limiter: Arc<Semaphore>,

    /// Authoritative RCP stores
    pub ovsdb: Arc<OvsdbDbusClient>,
    pub nonnet: Arc<NonNetDb>,
    /// In-process plugin handles for MethodCall dispatch (e.g. createunixsocket).
    pub unix_socket: Arc<op_plugins::state_plugins::UnixSocketPlugin>,
}

impl std::fmt::Debug for MutationEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MutationEngine").finish()
    }
}

#[async_trait]
impl op_core::state_publisher::StatePublisher for MutationEngine {
    async fn publish_change(
        &self,
        plugin_id: String,
        path: String,
        change_type: op_core::state_publisher::ChangeType,
        property: Option<String>,
        old_value: Option<simd_json::OwnedValue>,
        new_value: simd_json::OwnedValue,
        tags: Vec<String>,
        source: String,
    ) -> anyhow::Result<()> {
        let internal_type = match change_type {
            op_core::state_publisher::ChangeType::PropertySet => ChangeType::PropertySet,
            op_core::state_publisher::ChangeType::Signal => ChangeType::Signal,
            op_core::state_publisher::ChangeType::Deleted => ChangeType::ObjectRemoved,
        };

        self.process_authoritative_change(
            plugin_id,
            path,
            internal_type,
            property,
            old_value,
            new_value,
            tags,
            source,
            ChangeSource::Internal,
        )
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!(e))
    }
}

impl MutationEngine {
    /// Create a new authoritative Mutation Engine
    pub fn new(
        event_chain: Arc<RwLock<EventChain>>,
        ovsdb: Arc<OvsdbDbusClient>,
        nonnet: Arc<NonNetDb>,
    ) -> Self {
        let (change_tx, _) = broadcast::channel(1024);
        Self {
            event_chain,
            change_tx,
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            dbus_connection: Arc::new(OnceCell::new()),
            session_bus: Arc::new(OnceCell::new()),
            dbus_call_limiter: Arc::new(Semaphore::new(32)),
            ovsdb,
            nonnet,
            unix_socket: Arc::new(op_plugins::state_plugins::UnixSocketPlugin::new()),
        }
    }

    /// Authoritative D-Bus connection getter
    pub async fn dbus_connection(&self) -> anyhow::Result<Connection> {
        self.dbus_connection
            .get_or_try_init(|| async { Connection::system().await })
            .await
            .cloned()
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Get or create the session bus connection (for projection tree introspection)
    async fn session_bus(&self) -> anyhow::Result<Connection> {
        self.session_bus
            .get_or_try_init(|| async {
                let addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
                    .unwrap_or_else(|_| op_core::config::SESSION_BUS_ADDRESS.to_string());
                zbus::connection::Builder::address(addr.as_str())
                    .map_err(|e| anyhow::anyhow!("Builder::address: {}", e))?
                    .build()
                    .await
                    .map_err(|e| anyhow::anyhow!("Session bus connect: {}", e))
            })
            .await
            .cloned()
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Crawl the D-Bus projection tree for a plugin and return its full state
    /// as JSON. Introspects `/org/opdbus/v1/plugins/<plugin_id>`, recursively
    /// descends child nodes, and reads all properties from all interfaces.
    async fn crawl_plugin_dbus_tree(
        &self,
        plugin_id: &str,
    ) -> Option<simd_json::OwnedValue> {
        let conn = match self.session_bus().await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(error = %e, "session bus unavailable for crawl");
                return None;
            }
        };

        let root_path = format!("/org/opdbus/v1/plugins/{}", plugin_id);
        let bus_name = "org.opdbus.v1.plugins";

        let mut result = simd_json::value::owned::Object::new();
        Self::crawl_path(&conn, bus_name, &root_path, &mut result).await;
        if result.is_empty() {
            None
        } else {
            Some(simd_json::OwnedValue::Object(Box::new(result)))
        }
    }

    /// Recursively introspect a D-Bus path, reading properties and descending
    /// into child nodes.
    async fn crawl_path(
        conn: &Connection,
        bus_name: &str,
        path: &str,
        out: &mut simd_json::value::owned::Object,
    ) {
        // Introspect to discover interfaces and child nodes
        let introspect_xml: String = match conn
            .call_method(
                Some(bus_name),
                path,
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
        {
            Ok(msg) => msg.body().deserialize::<String>().unwrap_or_default(),
            Err(e) => {
                tracing::debug!(path = %path, error = %e, "introspect failed");
                return;
            }
        };

        // Parse the introspection XML for interfaces and child nodes
        let interfaces = Self::parse_interfaces(&introspect_xml);
        let child_nodes = Self::parse_child_nodes(&introspect_xml);

        // Read properties from each non-standard interface
        for iface in &interfaces {
            // Skip standard D-Bus interfaces
            if iface.starts_with("org.freedesktop.DBus.") {
                continue;
            }

            let props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
                match conn
                    .call_method(
                        Some(bus_name),
                        path,
                        Some("org.freedesktop.DBus.Properties"),
                        "GetAll",
                        &iface,
                    )
                    .await
                {
                    Ok(msg) => msg
                        .body()
                        .deserialize::<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>()
                        .unwrap_or_default(),
                    Err(_) => std::collections::HashMap::new(),
                };

            let mut iface_obj = simd_json::value::owned::Object::new();
            for (prop_name, prop_val) in &props {
                // zvariant::OwnedValue serializes as {"signature":"s","value":"..."}
                // Extract just the value field for a clean JSON projection.
                let json_val: serde_json::Value =
                    serde_json::to_value(&prop_val).unwrap_or(serde_json::Value::Null);
                let extracted = json_val.get("value").unwrap_or(&json_val);
                let json_str = serde_json::to_string(extracted)
                    .unwrap_or_else(|_| "null".to_string());
                let mut bytes = json_str.into_bytes();
                if let Ok(v) = simd_json::to_owned_value(&mut bytes) {
                    iface_obj.insert(prop_name.clone(), v);
                }
            }
            if !iface_obj.is_empty() {
                out.insert(iface.clone(), simd_json::OwnedValue::Object(Box::new(iface_obj)));
            }
        }

        // Recurse into child nodes
        for child in &child_nodes {
            let child_path = format!("{}/{}", path.trim_end_matches('/'), child);
            let mut child_obj = simd_json::value::owned::Object::new();
            Box::pin(Self::crawl_path(conn, bus_name, &child_path, &mut child_obj))
                .await;
            if !child_obj.is_empty() {
                out.insert(child.clone(), simd_json::OwnedValue::Object(Box::new(child_obj)));
            }
        }
    }

    /// Parse interface names from D-Bus introspection XML
    fn parse_interfaces(xml: &str) -> Vec<String> {
        let mut interfaces = Vec::new();
        for line in xml.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("<interface name=\"") {
                if let Some(end) = rest.find("\">") {
                    interfaces.push(rest[..end].to_string());
                }
            }
        }
        interfaces
    }

    /// Parse child node names from D-Bus introspection XML
    fn parse_child_nodes(xml: &str) -> Vec<String> {
        let mut nodes = Vec::new();
        for line in xml.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("<node name=\"") {
                if let Some(end) = rest.find("\"") {
                    nodes.push(rest[..end].to_string());
                }
            }
        }
        nodes
    }

    fn compute_tags(&self, plugin_id: &str, object_path: &str) -> Vec<String> {
        let mut tags = Vec::new();
        if plugin_id == "net" || object_path.contains("/ovsdb/") {
            tags.push("network".to_string());
            tags.push("ovsdb".to_string());
        } else if object_path.contains("/nonnet/") {
            tags.push("nonnet".to_string());
            tags.push("plugin".to_string());
        } else {
            tags.push("state".to_string());
            tags.push(plugin_id.to_string());
        }
        tags
    }

    /// Process a change that has already happened in an authoritative store.
    /// This records the change in the event chain and broadcasts it to gRPC.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_authoritative_change(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        old_value: Option<simd_json::OwnedValue>,
        new_value: simd_json::OwnedValue,
        mut tags: Vec<String>,
        actor_id: String,
        source: ChangeSource,
    ) -> Result<StateChange, String> {
        if tags.is_empty() {
            tags = self.compute_tags(&plugin_id, &object_path);
        }

        let event = {
            let mut chain = self.event_chain.write().await;
            let op = match change_type {
                ChangeType::PropertySet => OperationType::PropertySet,
                ChangeType::ObjectRemoved => OperationType::Custom("delete".to_string()),
                _ => OperationType::EmitSignal,
            };
            let event = chain.record(
                actor_id.clone(),
                plugin_id.clone(),
                "1.0.0".to_string(),
                op,
                object_path.clone(),
                tags.clone(),
                Decision::Allow,
                &new_value,
            );
            event.clone()
        };

        self.update_cached_plugin_state(
            &plugin_id,
            &object_path,
            change_type,
            member_name.as_deref(),
            &new_value,
        )
        .await;

        // Write the plugin's state to the shm projection layer.
        //
        // The projection IS the state — `/dev/shm/opdbus/projections/<plugin>.json`
        // is the single source of truth that the D-Bus tree derives from.
        // We write a composite object: `data` holds the mutation value, and
        // `_introspection` holds the D-Bus tree crawl result. The projection
        // server reads `data` for the `ProjectedObject.Data` property and
        // derives child paths from it. The `_introspection` field (underscore-
        // prefixed) is skipped by path derivation so it doesn't create child
        // objects (no recursive echo).
        if change_type != ChangeType::ObjectRemoved {
            let crawl_result = self.crawl_plugin_dbus_tree(&plugin_id).await;

            let projection_value = if let Some(crawl) = crawl_result {
                simd_json::json!({
                    "data": new_value,
                    "_introspection": crawl,
                })
            } else {
                new_value.clone()
            };

            match simd_json::to_string(&projection_value) {
                Ok(json) => {
                    if let Err(e) =
                        op_core::projection_shm::write_projection(&plugin_id, json.as_bytes())
                    {
                        tracing::warn!(plugin_id = %plugin_id, error = %e, "Failed to write shm projection");
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        plugin_id = %plugin_id,
                        error = %e,
                        "Failed to serialize plugin state for shm projection"
                    );
                }
            }
        }

        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: event.event_id,
            plugin_id,
            object_path,
            change_type,
            member_name,
            old_value,
            new_value,
            tags_touched: tags,
            event_hash: event.event_hash.clone(),
            timestamp: event.timestamp,
            actor_id,
            source,
        };

        let _ = self.change_tx.send(change.clone());
        Ok(change)
    }

    /// Start the Mutation Engine background tasks.
    /// Subscribes to authoritative RCP stores and broadcasts changes.
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let me = self.clone();

        // 1. Subscribe to NonNet updates
        let mut nonnet_rx = self.nonnet.subscribe();
        let nonnet_self = me.clone();
        tokio::spawn(async move {
            loop {
                match nonnet_rx.recv().await {
                    Ok(update) => {
                        let _ = nonnet_self
                            .process_authoritative_change(
                                update.table.clone(),
                                format!(
                                    "/org/opdbus/v1/nonnet/{}/{}",
                                    update.db_name, update.table
                                ),
                                ChangeType::PropertySet,
                                None,
                                None,
                                simd_json::json!(update.rows),
                                vec!["nonnet".to_string()],
                                "nonnet-db".to_string(),
                                ChangeSource::Internal,
                            )
                            .await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("NonNet subscription lagged by {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // 2. Subscribe to OVSDB updates
        let ovsdb_self = me.clone();
        tokio::spawn(async move {
            if let Ok(mut rx) = ovsdb_self.ovsdb.monitor_db("Open_vSwitch").await {
                loop {
                    match rx.recv().await {
                        Ok(update) => {
                            if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
                                if params.len() >= 3 {
                                    if let Some(tables) = params[2].as_object() {
                                        for (table_name, table_update) in tables.iter() {
                                            let table_name_owned: String = table_name.to_string();
                                            // monitor_db returns serde_json::Value; convert to
                                            // simd_json::OwnedValue required by process_authoritative_change.
                                            let simd_val: simd_json::OwnedValue = {
                                                match serde_json::to_string(table_update)
                                                    .ok()
                                                    .and_then(|s| {
                                                        let mut b = s.into_bytes();
                                                        simd_json::to_owned_value(&mut b).ok()
                                                    }) {
                                                    Some(v) => v,
                                                    None => continue,
                                                }
                                            };
                                            let _ = ovsdb_self
                                                .process_authoritative_change(
                                                    "net".to_string(),
                                                    format!(
                                                        "/org/opdbus/v1/ovsdb/{}",
                                                        table_name_owned
                                                    ),
                                                    ChangeType::PropertySet,
                                                    Some(table_name_owned),
                                                    None,
                                                    simd_val,
                                                    vec![
                                                        "ovsdb".to_string(),
                                                        "network".to_string(),
                                                    ],
                                                    "ovsdb-monitor".to_string(),
                                                    ChangeSource::DBus,
                                                )
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("OVSDB subscription lagged by {} events", n);
                            continue;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        });

        Ok(())
    }

    /// Unified mutation entry point. Writes to authoritative RCP stores and
    /// triggers the event recording/broadcast pipeline.
    #[allow(clippy::too_many_arguments)]
    pub async fn mutate(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        _capability_id: Option<String>,
    ) -> anyhow::Result<MutationResult> {
        let mut old_value = None;
        let mut authoritative_value = value.clone();

        // Resolve the acting identity from the Sled (/dev/shm/plugin_schema.dat)
        // when the caller omitted it. The Sled carries the WireGuard footprint +
        // trace_id — that identity is authoritative for every mutation.
        let actor_id = if actor_id.is_empty() {
            sled_footprint().unwrap_or_else(|| "anonymous".to_string())
        } else {
            actor_id
        };

        // 1. Write to authoritative RCP store
        if plugin_id == "net" || object_path.contains("/ovsdb/") {
            // OVSDB Authoritative Path
            if change_type == ChangeType::MethodCall {
                if let Some(method) = &member_name {
                    match method.as_str() {
                        "create_bridge" => {
                            if let Some(name) = value.as_str() {
                                self.ovsdb.create_bridge(name).await?;
                            }
                        }
                        "add_port" => {
                            if let Some(args) = value.as_array() {
                                if args.len() >= 2 {
                                    if let (Some(br), Some(port)) =
                                        (args[0].as_str(), args[1].as_str())
                                    {
                                        self.ovsdb.add_port(br, port).await?;
                                    }
                                }
                            }
                        }
                        _ => {
                            // Fallback to generic D-Bus call if it's a known service
                            let _ = self
                                .call_dbus_method(
                                    &format!("org.opdbus.{}.v1", plugin_id),
                                    &object_path,
                                    "org.opdbus.OvsdbV1",
                                    method,
                                    vec![value.clone()],
                                    &actor_id,
                                    &_capability_id,
                                )
                                .await?;
                        }
                    }
                }
            } else if change_type == ChangeType::PropertySet {
                if let Some(prop) = &member_name {
                    // Extract bridge name from path if possible
                    // Path format: /org/opdbus/v1/ovsdb/Bridge/bridge_name
                    let parts: Vec<&str> = object_path.split('/').collect();
                    if parts.len() >= 6 && parts[4] == "Bridge" {
                        let br_name = parts[5].replace('_', "-");
                        if let Some(val_str) = value.as_str() {
                            self.ovsdb
                                .set_bridge_property(&br_name, prop, val_str)
                                .await?;
                        }
                    }
                }
            }
        } else if plugin_id == "unix_socket" && change_type == ChangeType::MethodCall {
            // unix_socket plugin dispatch: createunixsocket <name> <ports csv/vec>.
            // The method registers the name+ports routing tag against the
            // shared container.sock transport; the transport owner is not
            // replaced during registration.
            if let Some(method) = &member_name {
                if method == "createunixsocket" {
                    let (name, ports) = parse_socket_args(&value);
                    let result = self
                        .unix_socket
                        .create_unix_socket(name.clone(), ports.clone());
                    if !result.success {
                        anyhow::bail!(result.errors.join("; "));
                    }
                    authoritative_value = unix_socket_state_after_registration(&name, &ports);
                }
            }
        } else {
            // NonNet / Generic Plugin Path
            if change_type == ChangeType::PropertySet {
                // Get old value for the footprint before update from cache
                old_value = self.get_state(&plugin_id).await.and_then(|v| {
                    if let Some(prop) = &member_name {
                        v.get(prop).cloned()
                    } else {
                        Some(v)
                    }
                });

                // For NonNet plugins, we update the NonNetDb which is authoritative for non-network state
                if let Some(rows) = value.as_array() {
                    let rows_vec: Vec<simd_json::OwnedValue> = rows.to_vec();
                    self.nonnet.update_table(&plugin_id, rows_vec).await;
                }
            }
        }

        // 2. Record and broadcast change
        let change = self
            .process_authoritative_change(
                plugin_id,
                object_path,
                change_type,
                member_name,
                old_value,
                authoritative_value.clone(),
                vec![], // Automatically computed in process_authoritative_change
                actor_id,
                ChangeSource::Grpc,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Write the Identity Sled with the updated mutation index.
        {
            let (existing_pubkey_b64, existing_trace_hex) = if let Ok((ptr, _mmap)) = read_sled() {
                unsafe {
                    let sled = &*ptr;
                    (
                        base64::engine::general_purpose::STANDARD.encode(sled.wireguard_pubkey),
                        sled.trace_id_hex(),
                    )
                }
            } else {
                (String::new(), String::new())
            };
            if let Err(e) =
                write_sled_full(&existing_pubkey_b64, change.event_id, &existing_trace_hex)
            {
                tracing::warn!("sled write after mutation failed: {}", e);
            }
        }

        Ok(MutationResult {
            success: true,
            event_id: change.event_id,
            event_hash: change.event_hash,
            result: Some(authoritative_value),
            error: None,
        })
    }

    /// Backward-compatible wrapper for gRPC Mutations.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_grpc_mutation(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        capability_id: Option<String>,
    ) -> anyhow::Result<MutationResult> {
        self.mutate(
            plugin_id,
            object_path,
            change_type,
            member_name,
            value,
            actor_id,
            capability_id,
        )
        .await
    }

    /// Fetch current state for a specific plugin from authoritative cache
    pub async fn get_state(&self, plugin_id: &str) -> Option<simd_json::OwnedValue> {
        let cache = self.state_cache.read().await;
        cache.get(plugin_id).cloned()
    }

    /// Update the authoritative state cache
    pub async fn update_state_cache(&self, plugin_id: String, state: simd_json::OwnedValue) {
        let mut cache = self.state_cache.write().await;
        cache.insert(plugin_id, state);
    }

    async fn update_cached_plugin_state(
        &self,
        plugin_id: &str,
        object_path: &str,
        change_type: ChangeType,
        property: Option<&str>,
        new_value: &simd_json::OwnedValue,
    ) {
        if object_path.starts_with("schema/") {
            return;
        }

        let mut cache = self.state_cache.write().await;

        match change_type {
            ChangeType::ObjectRemoved => {
                cache.remove(plugin_id);
            }
            ChangeType::PropertySet => {
                if let Some(property) = property {
                    let entry = cache
                        .entry(plugin_id.to_string())
                        .or_insert_with(|| simd_json::json!({}));

                    if let Some(existing) = entry.as_object_mut() {
                        existing.insert(property.to_string(), new_value.clone());
                    } else {
                        let mut state = simd_json::value::owned::Object::new();
                        state.insert(property.to_string(), new_value.clone());
                        *entry = simd_json::OwnedValue::Object(Box::new(state));
                    }
                } else {
                    cache.insert(plugin_id.to_string(), new_value.clone());
                }
            }
            _ => {}
        }
    }

    /// Route a D-Bus method call through the authoritative bridge
    #[allow(clippy::too_many_arguments)]
    pub async fn call_dbus_method(
        &self,
        bus_name: &str,
        path: &str,
        interface: &str,
        method: &str,
        _args: Vec<simd_json::OwnedValue>,
        _actor_id: &str,
        _capability_id: &Option<String>,
    ) -> anyhow::Result<simd_json::OwnedValue> {
        let conn = self.dbus_connection().await?;
        let proxy = Proxy::new(&conn, bus_name, path, interface).await?;
        let result: ZOwnedValue = proxy.call(method, &()).await?;
        simd_json::serde::to_owned_value(&result).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn change_tx(&self) -> broadcast::Sender<StateChange> {
        self.change_tx.clone()
    }
}

/// Result of an authoritative mutation
#[derive(Debug, Clone)]
pub struct MutationResult {
    pub success: bool,
    pub event_id: u64,
    pub event_hash: String,
    pub result: Option<simd_json::OwnedValue>,
    pub error: Option<MutationError>,
}

#[derive(Debug, Clone)]
pub struct MutationError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    NotFound,
    PermissionDenied,
    ValidationFailed,
    ReadOnly,
    Internal,
}

/// Read the WireGuard footprint from the Sled (1:1 shared memory identity at
/// `/dev/shm/plugin_schema.dat`). Returns the hex footprint used as the default
/// `actor_id` when a caller omits identity.
fn sled_footprint() -> Option<String> {
    // SAFETY: read_sled returns a valid pointer to IdentitySled in shared memory
    // for the lifetime of the process; we copy the footprint out and drop the ptr.
    let (ptr, _mmap) = read_sled().ok()?;
    unsafe {
        let sled = &*ptr;
        if sled.hashed_footprint != [0u8; 32] {
            Some(hex::encode(sled.hashed_footprint))
        } else {
            None
        }
    }
}

/// Build the authoritative `unix_socket` projection state after a successful
/// `createunixsocket` mutation. The mutation argument is not state; the state is
/// the registered service list under the single shared container socket.
fn unix_socket_state_after_registration(name: &str, ports: &[u16]) -> simd_json::OwnedValue {
    let mut sockets = existing_unix_socket_sockets();
    sockets.retain(|socket| {
        socket
            .get("name")
            .and_then(|value| value.as_str())
            .map(|existing_name| existing_name != name)
            .unwrap_or(true)
    });

    sockets.push(simd_json::json!({
        "name": name,
        "path": op_plugins::state_plugins::unix_socket::SHARED_CONTAINER_SOCKET,
        "ports": ports,
        "protocol": "grpc",
        "label": "",
    }));

    simd_json::json!({ "sockets": sockets })
}

fn existing_unix_socket_sockets() -> Vec<simd_json::OwnedValue> {
    let Some(mut bytes) = op_core::projection_shm::read_projection_bytes("unix_socket") else {
        return Vec::new();
    };
    let Ok(state) = simd_json::to_owned_value(&mut bytes) else {
        return Vec::new();
    };

    let sockets = state
        .get("sockets")
        .and_then(|value| value.as_array())
        .or_else(|| {
            state
                .get("data")
                .and_then(|data| data.get("sockets"))
                .and_then(|value| value.as_array())
        });

    sockets
        .into_iter()
        .flatten()
        .filter(|socket| {
            let has_name = socket
                .get("name")
                .and_then(|value| value.as_str())
                .map(|name| !name.is_empty())
                .unwrap_or(false);
            let has_path = socket
                .get("path")
                .and_then(|value| value.as_str())
                .is_some();
            has_name && has_path
        })
        .cloned()
        .collect()
}

/// Parse createunixsocket arguments. Accepts either a JSON array
/// `[name, [ports...]]` or an object `{ "name": "...", "ports": [...] }`.
/// `ports` may be a JSON array of numbers or a CSV string ("6334" / "6334,8080").
fn parse_socket_args(value: &simd_json::OwnedValue) -> (String, Vec<u16>) {
    if let Some(obj) = value.as_object() {
        return parse_socket_arg_object(obj);
    }
    if let Some(arr) = value.as_array() {
        if arr.len() == 1 {
            if let Some(obj) = arr.first().and_then(|v| v.as_object()) {
                return parse_socket_arg_object(obj);
            }
        }
        let name = arr
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let ports = parse_ports_value(arr.get(1));
        return (name, ports);
    }
    (String::new(), Vec::new())
}

fn parse_socket_arg_object(
    obj: &simd_json::value::owned::Object,
) -> (String, Vec<u16>) {
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let ports = parse_ports_value(obj.get("ports"));
    (name, ports)
}

fn parse_ports_value(value: Option<&simd_json::OwnedValue>) -> Vec<u16> {
    let Some(v) = value else {
        return Vec::new();
    };
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(parse_port_number)
            .map(|n| n as u16)
            .collect();
    }
    if let Some(s) = v.as_str() {
        return s
            .split(',')
            .filter_map(|p| p.trim().parse::<u16>().ok())
            .collect();
    }
    if let Some(n) = parse_port_number(v) {
        return vec![n as u16];
    }
    Vec::new()
}

fn parse_port_number(value: &simd_json::OwnedValue) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| (n >= 0).then_some(n as u64)))
        .or_else(|| {
            value.as_f64().and_then(|n| {
                (n.is_finite() && n >= 0.0 && n <= u16::MAX as f64).then_some(n as u64)
            })
        })
}
