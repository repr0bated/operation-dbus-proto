//! Schema Engine - The Authoritative Source for State and Schema DNA
//!
//! The Schema Engine is the central coordinator that:
//! - Authoritatively routes all mutations (gRPC and D-Bus)
//! - Ensures all state changes are strictly recorded in the Event Chain (Audit Log)
//! - Broadcasts authoritative state changes to gRPC subscribers
//! - Directly manages authoritative RCP stores (OVSDB, NonNet, SQLite)

use async_trait::async_trait;
use simd_json::prelude::{ValueAsMutContainer, ValueObjectAccess, ValueAsScalar, ValueAsContainer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, OnceCell, RwLock, Semaphore};
use zbus::zvariant::OwnedValue as ZOwnedValue;
use zbus::{Connection, Proxy};

use op_state_store::{Decision, EventChain, OperationType};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;

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

pub struct SchemaEngine {
    /// Authoritative Event Chain
    pub event_chain: Arc<RwLock<EventChain>>,
    /// Real-time change projection channel
    change_tx: broadcast::Sender<StateChange>,
    /// State cache for instant gRPC retrieval
    state_cache: Arc<RwLock<HashMap<String, simd_json::OwnedValue>>>,
    /// System D-Bus connection authority
    pub dbus_connection: Arc<OnceCell<Connection>>,
    /// Resource limiter for D-Bus operations
    #[allow(dead_code)]
    dbus_call_limiter: Arc<Semaphore>,
    
    /// Authoritative RCP stores
    pub ovsdb: Arc<OvsdbClient>,
    pub nonnet: Arc<NonNetDb>,
}

impl std::fmt::Debug for SchemaEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemaEngine").finish()
    }
}

#[async_trait]
impl op_core::state_publisher::StatePublisher for SchemaEngine {
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

impl SchemaEngine {
    /// Create a new authoritative Schema Engine
    pub fn new(
        event_chain: Arc<RwLock<EventChain>>,
        ovsdb: Arc<OvsdbClient>,
        nonnet: Arc<NonNetDb>,
    ) -> Self {
        let (change_tx, _) = broadcast::channel(1024);
        Self {
            event_chain,
            change_tx,
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            dbus_connection: Arc::new(OnceCell::new()),
            dbus_call_limiter: Arc::new(Semaphore::new(32)),
            ovsdb,
            nonnet,
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

    /// Start the Schema Engine background tasks.
    /// Subscribes to authoritative RCP stores and broadcasts changes.
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let me = self.clone();
        
        // 1. Subscribe to NonNet updates
        let mut nonnet_rx = self.nonnet.subscribe();
        let nonnet_self = me.clone();
        tokio::spawn(async move {
            while let Ok(update) = nonnet_rx.recv().await {
                let _ = nonnet_self.process_authoritative_change(
                    update.table.clone(),
                    format!("/org/opdbus/v1/nonnet/{}/{}", update.db_name, update.table),
                    ChangeType::PropertySet,
                    None,
                    None,
                    simd_json::json!(update.rows),
                    vec!["nonnet".to_string()],
                    "nonnet-db".to_string(),
                    ChangeSource::Internal,
                ).await;
            }
        });

        // 2. Subscribe to OVSDB updates
        let ovsdb_self = me.clone();
        tokio::spawn(async move {
            if let Ok(mut rx) = ovsdb_self.ovsdb.monitor_db("Open_vSwitch").await {
                while let Some(update) = rx.recv().await {
                    if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
                        if params.len() >= 3 {
                            if let Some(tables) = params[2].as_object() {
                                for (table_name, table_update) in tables.iter() {
                                    let table_name_owned: String = table_name.to_string();
                                    let _ = ovsdb_self.process_authoritative_change(
                                        "net".to_string(),
                                        format!("/org/opdbus/v1/ovsdb/{}", table_name_owned),
                                        ChangeType::PropertySet,
                                        Some(table_name_owned),
                                        None,
                                        table_update.clone(),
                                        vec!["ovsdb".to_string(), "network".to_string()],
                                        "ovsdb-monitor".to_string(),
                                        ChangeSource::DBus,
                                    ).await;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Unified mutation entry point. Writes to authoritative RCP stores and
    /// triggers the event recording/broadcast pipeline.
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
                                    if let (Some(br), Some(port)) = (args[0].as_str(), args[1].as_str()) {
                                        self.ovsdb.add_port(br, port).await?;
                                    }
                                }
                            }
                        }
                        _ => {
                            // Fallback to generic D-Bus call if it's a known service
                            let _ = self.call_dbus_method(
                                &format!("org.opdbus.{}.v1", plugin_id),
                                &object_path,
                                "org.opdbus.OvsdbV1",
                                method,
                                vec![value.clone()],
                                &actor_id,
                                &_capability_id,
                            ).await?;
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
                            self.ovsdb.set_bridge_property(&br_name, prop, val_str).await?;
                        }
                    }
                }
            }
        } else {
            // NonNet / Generic Plugin Path
            if change_type == ChangeType::PropertySet {
                // Get old value for the footprint before update from cache
                old_value = self.get_state(&plugin_id).await
                    .and_then(|v| {
                        if let Some(prop) = &member_name {
                            v.get(prop).cloned()
                        } else {
                            Some(v)
                        }
                    });

                // For NonNet plugins, we update the NonNetDb which is authoritative for non-network state
                if let Some(rows) = value.as_array() {
                    let rows_vec: Vec<simd_json::OwnedValue> = rows.iter().cloned().collect();
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
                value.clone(),
                vec![], // Automatically computed in process_authoritative_change
                actor_id,
                ChangeSource::Grpc,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(MutationResult {
            success: true,
            event_id: change.event_id,
            event_hash: change.event_hash,
            result: Some(value),
            error: None,
        })
    }

    /// Backward-compatible wrapper for gRPC Mutations.
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
        self.mutate(plugin_id, object_path, change_type, member_name, value, actor_id, capability_id).await
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
