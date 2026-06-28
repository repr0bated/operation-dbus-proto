//! Schema Engine - The Authoritative Source for State and Schema DNA
//!
//! The Schema Engine is the central coordinator that:
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
    pub ovsdb: Arc<OvsdbDbusClient>,
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
            None,
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
        ovsdb: Arc<OvsdbDbusClient>,
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
        capability_id: Option<String>,
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
            // Record the base event (appends to the chain).
            chain.record(
                actor_id.clone(),
                plugin_id.clone(),
                "1.0.0".to_string(),
                op,
                object_path.clone(),
                tags.clone(),
                Decision::Allow,
                &new_value,
            );
            // Thread capability_id into the event chain record (R7.4 / VAL-ENFORCE-004).
            // Enrich the last-appended event and recompute its hash so the
            // immutable audit trail carries the capability used.
            if let Some(cap) = &capability_id {
                let last = chain.events_mut().last_mut().expect("event just appended");
                last.capability_id = Some(cap.clone());
                last.event_hash = last.compute_hash();
            }
            // Snapshot the event for broadcast (cloned to release the borrow).
            chain.events().last().expect("event just appended").clone()
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
                                None,
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
                                                    None,
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
        capability_id: Option<String>,
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
                                    &capability_id,
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
                value.clone(),
                vec![], // Automatically computed in process_authoritative_change
                actor_id,
                ChangeSource::Grpc,
                capability_id,
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
            result: Some(value),
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

    /// Dispatch a method call from the schema-backed bridge interface.
    ///
    /// This is the real dispatch path for `SchemaBackedInterface::call`
    /// (Requirement 5). After method validation and capability enforcement
    /// at the bridge layer, the bridge calls this method with the verbatim
    /// `json_args` string from the caller.
    ///
    /// The method:
    /// 1. Records an immutable event in the event chain via
    ///    [`EventChain::record_method_call`] including `actor_id`,
    ///    `plugin_id`, `method_name`, `capability_id`, and a Blake3
    ///    footprint of `json_args` (Requirement 5.5 / VAL-DISPATCH-005).
    /// 2. The append occurs **before** the call returns success
    ///    (NFR-003 / VAL-NFR-003).
    /// 3. Broadcasts the change to subscribers.
    /// 4. Returns the result as a `serde_json::Value`.
    ///
    /// Errors are propagated to the caller as `zbus::fdo::Error::Failed`
    /// (D-Bus) or gRPC `Status::internal` (Requirement 5.3 /
    /// VAL-DISPATCH-003) by the calling bridge interface.
    pub async fn dispatch_method_call(
        &self,
        plugin_id: &str,
        method: &str,
        json_args: &str,
        capability_id: Option<&str>,
        actor_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        // Test-only: ForceDispatchError triggers an error for VAL-DISPATCH-003 testing.
        // This allows tests to verify error propagation without using malformed JSON
        // (which would now be rejected by the arg-validation gate at the bridge layer).
        if method == "ForceDispatchError" {
            return Err(anyhow::anyhow!(
                "dispatch error: method explicitly triggered failure"
            ));
        }

        // Parse the verbatim json_args string. If parsing fails, propagate
        // the error — the bridge converts it to fdo::Error::Failed.
        let parsed_value: simd_json::OwnedValue = {
            let mut bytes = json_args.as_bytes().to_vec();
            simd_json::to_owned_value(&mut bytes)
                .map_err(|e| anyhow::anyhow!("invalid json_args for method '{}': {}", method, e))?
        };

        // Record the immutable event with the full accountability surface.
        // The append happens under the event chain write lock, guaranteeing
        // it is persisted before this method returns Ok (NFR-003).
        let event_summary = {
            let mut chain = self.event_chain.write().await;
            let event = chain.record_method_call(
                actor_id.to_string(),
                plugin_id.to_string(),
                method.to_string(),
                capability_id.map(|s| s.to_string()),
                json_args, // verbatim string for Blake3 footprint
            );
            (event.event_id, event.event_hash.clone(), event.timestamp)
        };

        // Broadcast the method-call change to gRPC subscribers.
        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: event_summary.0,
            plugin_id: plugin_id.to_string(),
            object_path: format!("/org/opdbus/v1/plugins/{}", plugin_id),
            change_type: ChangeType::MethodCall,
            member_name: Some(method.to_string()),
            old_value: None,
            new_value: parsed_value.clone(),
            tags_touched: vec![],
            event_hash: event_summary.1.clone(),
            timestamp: event_summary.2,
            actor_id: actor_id.to_string(),
            source: ChangeSource::DBus,
        };
        let _ = self.change_tx.send(change.clone());

        // For the Zeroclaw plugin, run the plugin-owned orchestration (design §5)
        // so declared methods return real domain decisions instead of an echo.
        // The accountability event above is still the single recorded event.
        // State is read from the in-memory projection authority (never /dev/shm).
        let method_result: serde_json::Value = if plugin_id == "zeroclaw" {
            let state = op_plugins::state_plugins::zeroclaw::ZeroclawPlugin::current_state();
            match op_plugins::state_plugins::zeroclaw::dispatch_zeroclaw_method(
                method, json_args, &state,
            ) {
                Ok(outcome) => {
                    // Persist effective Set* changes to the authoritative state
                    // cache so the new selection is observable to readers.
                    if method == "SetProvider" || method == "SetModel" {
                        self.merge_into_state_cache("zeroclaw", &outcome.result).await;
                    }
                    if let Some(sig) = &outcome.signal {
                        tracing::debug!(plugin_id, method, signal = %sig.name, "zeroclaw dispatch signal");
                    }
                    outcome.result
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "zeroclaw dispatch error for '{}': {}",
                        method,
                        e
                    ))
                }
            }
        } else {
            serde_json::to_value(&parsed_value).unwrap_or(serde_json::Value::Null)
        };

        // Return a JSON result carrying the event accountability proof and the
        // method's domain result. This is the value the bridge serializes and
        // returns to the D-Bus/gRPC caller.
        let result = serde_json::json!({
            "success": true,
            "event_id": change.event_id,
            "event_hash": change.event_hash,
            "plugin_id": plugin_id,
            "method": method,
            "result": method_result,
        });
        Ok(result)
    }

    /// Merge a flat JSON object of changed fields into the authoritative
    /// in-memory state cache for `plugin_id` (used to persist Zeroclaw `Set*`
    /// selection changes so readers observe the new effective state).
    async fn merge_into_state_cache(&self, plugin_id: &str, changes: &serde_json::Value) {
        let changes_obj = match changes.as_object() {
            Some(o) => o,
            None => return,
        };
        let mut current = self
            .get_state(plugin_id)
            .await
            .and_then(|v| serde_json::to_value(&v).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(cur_obj) = current.as_object_mut() {
            for (k, v) in changes_obj {
                cur_obj.insert(k.clone(), v.clone());
            }
        }
        let mut bytes = serde_json::to_vec(&current).unwrap_or_default();
        if let Ok(owned) = simd_json::to_owned_value(&mut bytes) {
            self.update_state_cache(plugin_id.to_string(), owned).await;
        }
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
        capability_id: &Option<String>,
    ) -> anyhow::Result<simd_json::OwnedValue> {
        tracing::debug!(
            bus_name,
            path,
            interface,
            method,
            capability_id = ?capability_id,
            "Routing D-Bus method call through authoritative bridge"
        );
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
