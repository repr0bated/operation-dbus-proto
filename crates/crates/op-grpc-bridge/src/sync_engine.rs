//! Sync Engine - Coordinates bidirectional state synchronization
//!
//! The sync engine is the central coordinator that:
//! - Routes D-Bus changes to gRPC subscribers
//! - Routes gRPC mutations to D-Bus
//! - Ensures all changes go through the event chain
//! - Maintains subscriber state

use anyhow;
use simd_json::prelude::{ValueAsContainer, ValueAsScalar};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
use tokio::sync::{broadcast, OnceCell, RwLock, Semaphore};
use tracing::{debug, info, warn};
use zbus::zvariant::{Array as ZArray, OwnedValue as ZOwnedValue, Str as ZStr, Value as ZValue};
use zbus::{Connection, Proxy};

use op_state_store::{ChainEvent, Decision, EventChain, OperationType};

/// A state change that can be synced bidirectionally
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

/// Subscription filter for state changes
#[derive(Debug, Clone, Default)]
pub struct SubscriptionFilter {
    pub plugin_ids: Vec<String>,
    pub path_patterns: Vec<String>,
    pub tags: Vec<String>,
}

impl SubscriptionFilter {
    pub fn matches(&self, change: &StateChange) -> bool {
        // Empty filter = match all
        if self.plugin_ids.is_empty() && self.path_patterns.is_empty() && self.tags.is_empty() {
            return true;
        }

        // Check plugin ID
        if !self.plugin_ids.is_empty() && !self.plugin_ids.contains(&change.plugin_id) {
            return false;
        }

        // Check path patterns (simple glob matching)
        if !self.path_patterns.is_empty() {
            let path_matches = self.path_patterns.iter().any(|pattern| {
                if pattern.contains('*') {
                    // Simple glob: * matches any segment
                    let pattern_parts: Vec<&str> = pattern.split('*').collect();
                    if pattern_parts.len() == 2 {
                        change.object_path.starts_with(pattern_parts[0])
                            && change.object_path.ends_with(pattern_parts[1])
                    } else {
                        change.object_path == *pattern
                    }
                } else {
                    change.object_path == *pattern
                }
            });
            if !path_matches {
                return false;
            }
        }

        // Check tags
        if !self.tags.is_empty() {
            let tag_matches = self
                .tags
                .iter()
                .any(|tag| change.tags_touched.contains(tag));
            if !tag_matches {
                return false;
            }
        }

        true
    }
}

/// The sync engine coordinates all state synchronization
pub struct SyncEngine {
    /// Event chain for audit trail
    event_chain: Arc<RwLock<EventChain>>,
    /// Broadcast channel for state changes
    change_tx: broadcast::Sender<StateChange>,
    /// Active subscriptions by subscriber ID
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionFilter>>>,
    /// Plugin state cache
    state_cache: Arc<RwLock<HashMap<String, simd_json::OwnedValue>>>,
    /// Shared D-Bus system connection (single socket per process)
    dbus_connection: Arc<OnceCell<Connection>>,
    /// Bounded in-flight D-Bus operations over the shared connection
    dbus_call_limiter: Arc<Semaphore>,
    /// Enforce D-Bus as the canonical write path for mutations.
    enforce_dbus_write_path: bool,
    /// Use org.opdbus.StateManager.ApplyContractMutation as canonical mutation ingress.
    prefer_state_manager_write_path: bool,
    /// Allow fallback to plugin-specific D-Bus members if canonical ingress is unavailable.
    allow_legacy_write_fallback: bool,
}

impl SyncEngine {
    /// Create a new sync engine
    pub fn new(event_chain: Arc<RwLock<EventChain>>) -> Self {
        let (change_tx, _) = broadcast::channel(1024);
        let max_dbus_in_flight = std::env::var("OP_DBUS_DBUS_MAX_IN_FLIGHT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(32);
        let enforce_dbus_write_path = std::env::var("OP_DBUS_STRICT_WRITE_PATH")
            .ok()
            .map(|v| {
                let l = v.to_lowercase();
                !(l == "0" || l == "false" || l == "no")
            })
            .unwrap_or(!cfg!(test));
        let prefer_state_manager_write_path = std::env::var("OP_DBUS_CANONICAL_WRITE_PATH")
            .ok()
            .map(|v| {
                let l = v.to_lowercase();
                !(l == "0" || l == "false" || l == "no")
            })
            .unwrap_or(true);
        let allow_legacy_write_fallback = std::env::var("OP_DBUS_ALLOW_LEGACY_WRITE_FALLBACK")
            .ok()
            .map(|v| {
                let l = v.to_lowercase();
                !(l == "0" || l == "false" || l == "no")
            })
            .unwrap_or(false);

        Self {
            event_chain,
            change_tx,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            state_cache: Arc::new(RwLock::new(HashMap::new())),
            dbus_connection: Arc::new(OnceCell::new()),
            dbus_call_limiter: Arc::new(Semaphore::new(max_dbus_in_flight)),
            enforce_dbus_write_path,
            prefer_state_manager_write_path,
            allow_legacy_write_fallback,
        }
    }

    /// Subscribe to state changes with optional filter
    pub async fn subscribe(
        &self,
        subscriber_id: String,
        filter: SubscriptionFilter,
    ) -> broadcast::Receiver<StateChange> {
        let mut subs = self.subscriptions.write().await;
        subs.insert(subscriber_id.clone(), filter);
        debug!("Added subscription: {}", subscriber_id);
        self.change_tx.subscribe()
    }

    /// Unsubscribe from state changes
    pub async fn unsubscribe(&self, subscriber_id: &str) {
        let mut subs = self.subscriptions.write().await;
        subs.remove(subscriber_id);
        debug!("Removed subscription: {}", subscriber_id);
    }

    /// Process a change from D-Bus and propagate to gRPC subscribers
    pub async fn process_dbus_change(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        old_value: Option<simd_json::OwnedValue>,
        new_value: simd_json::OwnedValue,
        tags: Vec<String>,
        actor_id: String,
    ) -> Result<StateChange, SyncError> {
        // Record in event chain
        let event = {
            let mut chain = self.event_chain.write().await;
            let event = chain.record(
                actor_id.clone(),
                plugin_id.clone(),
                "1.0.0".to_string(), // TODO: get actual schema version
                change_type_to_operation(change_type),
                object_path.clone(),
                tags.clone(),
                Decision::Allow,
                &new_value,
            );
            event.clone()
        };

        // Create state change
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
            source: ChangeSource::DBus,
        };

        // Broadcast to subscribers
        if let Err(e) = self.change_tx.send(change.clone()) {
            warn!("No active subscribers for change: {}", e);
        }

        info!(
            "Processed D-Bus change: event_id={}, path={}",
            change.event_id, change.object_path
        );

        Ok(change)
    }

    /// Process a mutation request from gRPC
    pub async fn process_grpc_mutation(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        capability_id: Option<String>,
    ) -> Result<MutationResult, SyncError> {
        self.process_mutation_internal(
            ChangeSource::Grpc,
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

    /// Process a mutation request from JSON-RPC.
    /// Uses the same enforcement pipeline as gRPC mutations.
    pub async fn process_jsonrpc_mutation(
        &self,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        capability_id: Option<String>,
    ) -> Result<MutationResult, SyncError> {
        self.process_mutation_internal(
            ChangeSource::Internal,
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

    /// Get the broadcast sender for new changes
    pub fn change_sender(&self) -> broadcast::Sender<StateChange> {
        self.change_tx.clone()
    }

    /// Get current state for a plugin
    pub async fn get_state(&self, plugin_id: &str) -> Option<simd_json::OwnedValue> {
        let cache = self.state_cache.read().await;
        cache.get(plugin_id).cloned()
    }

    /// Update state cache
    pub async fn update_state_cache(&self, plugin_id: String, state: simd_json::OwnedValue) {
        let mut cache = self.state_cache.write().await;
        cache.insert(plugin_id, state);
    }

    /// Get the event chain (for queries)
    pub fn event_chain(&self) -> Arc<RwLock<EventChain>> {
        self.event_chain.clone()
    }

    async fn process_mutation_internal(
        &self,
        source: ChangeSource,
        plugin_id: String,
        object_path: String,
        change_type: ChangeType,
        member_name: Option<String>,
        value: simd_json::OwnedValue,
        actor_id: String,
        capability_id: Option<String>,
    ) -> Result<MutationResult, SyncError> {
        validate_contract_envelope(&plugin_id, &object_path, &value)?;
        if self.enforce_dbus_write_path {
            self.apply_mutation_via_dbus(
                &plugin_id,
                &object_path,
                change_type,
                member_name.as_deref(),
                &value,
                &actor_id,
                &capability_id,
            )
            .await?;
        }
        let tags = extract_tags_from_contract(&value);

        // Record in event chain (mandatory for all mutations)
        let event = {
            let mut chain = self.event_chain.write().await;
            let mut event = ChainEvent::new(
                chain.next_event_id(),
                chain.last_hash().to_string(),
                actor_id.clone(),
                plugin_id.clone(),
                "1.0.0".to_string(),
                change_type_to_operation(change_type),
                object_path.clone(),
                tags.clone(),
                Decision::Allow,
                &value,
            );

            if let Some(cap) = capability_id {
                event = event.with_capability(cap);
            }

            if event.event_hash.is_empty() {
                return Err(SyncError::EventChain(
                    "mutation denied: missing event hash footprint".to_string(),
                ));
            }

            chain.append(event.clone());
            event
        };

        // Create state change for broadcasting
        let change = StateChange {
            change_id: uuid::Uuid::new_v4().to_string(),
            event_id: event.event_id,
            plugin_id: plugin_id.clone(),
            object_path: object_path.clone(),
            change_type,
            member_name,
            old_value: None,
            new_value: value.clone(),
            tags_touched: tags,
            event_hash: event.event_hash.clone(),
            timestamp: event.timestamp,
            actor_id,
            source,
        };

        if let Err(e) = self.change_tx.send(change.clone()) {
            warn!("No active subscribers for change: {}", e);
        }

        info!(
            "Processed mutation: source={:?}, event_id={}, path={}",
            source, event.event_id, object_path
        );

        Ok(MutationResult {
            success: true,
            event_id: event.event_id,
            event_hash: event.event_hash,
            result: Some(value),
            error: None,
        })
    }

    /// Call a D-Bus method directly (shared-server path for gRPC).
    pub async fn call_dbus_method(
        &self,
        plugin_id: &str,
        object_path: &str,
        interface_name: &str,
        method_name: &str,
        args: Vec<simd_json::OwnedValue>,
        _actor_id: &str,
        _capability_id: &Option<String>,
    ) -> Result<simd_json::OwnedValue, SyncError> {
        let _permit = self
            .dbus_call_limiter
            .acquire()
            .await
            .map_err(|e| SyncError::DBus(format!("D-Bus limiter closed: {}", e)))?;
        let connection = self.dbus_connection().await?;

        let proxy = Proxy::new(&connection, plugin_id, object_path, interface_name)
            .await
            .map_err(|e| SyncError::DBus(format!("Proxy error: {}", e)))?;

        let zbus_args: Vec<ZOwnedValue> = args
            .iter()
            .map(simd_json_to_zvariant)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| SyncError::Validation(format!("Argument conversion error: {}", e)))?;

        let result: ZOwnedValue = proxy
            .call(method_name, &zbus_args)
            .await
            .map_err(|e| SyncError::DBus(format!("Method call error: {}", e)))?;

        simd_json::serde::to_owned_value(&result)
            .map_err(|e| SyncError::DBus(format!("Result serialization error: {}", e)))
    }

    pub async fn dbus_connection(&self) -> Result<Connection, SyncError> {
        self.dbus_connection
            .get_or_try_init(|| async {
                Connection::system()
                    .await
                    .map_err(|e| SyncError::DBus(format!("System bus error: {}", e)))
            })
            .await
            .cloned()
    }

    async fn apply_mutation_via_dbus(
        &self,
        plugin_id: &str,
        object_path: &str,
        change_type: ChangeType,
        member_name: Option<&str>,
        value: &simd_json::OwnedValue,
        actor_id: &str,
        capability_id: &Option<String>,
    ) -> Result<(), SyncError> {
        if self.prefer_state_manager_write_path {
            let canonical_result = self
                .apply_mutation_via_state_manager(
                    plugin_id,
                    object_path,
                    change_type,
                    member_name,
                    value,
                    actor_id,
                    capability_id,
                )
                .await;

            if canonical_result.is_ok() {
                return Ok(());
            }

            if !self.allow_legacy_write_fallback {
                return canonical_result;
            }
        }

        self.apply_mutation_via_plugin_surface(
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

    async fn apply_mutation_via_state_manager(
        &self,
        plugin_id: &str,
        object_path: &str,
        change_type: ChangeType,
        member_name: Option<&str>,
        value: &simd_json::OwnedValue,
        actor_id: &str,
        capability_id: &Option<String>,
    ) -> Result<(), SyncError> {
        let request = simd_json::json!({
            "plugin_id": plugin_id,
            "object_path": object_path,
            "change_type": format!("{:?}", change_type),
            "member_name": member_name,
            "value": value,
            "actor_id": actor_id,
            "capability_id": capability_id
        });
        let request_json = simd_json::to_string(&request).map_err(|e| {
            SyncError::Validation(format!("canonical mutation encode error: {}", e))
        })?;

        let _permit = self
            .dbus_call_limiter
            .acquire()
            .await
            .map_err(|e| SyncError::DBus(format!("D-Bus limiter closed: {}", e)))?;
        let connection = self.dbus_connection().await?;
        let proxy = Proxy::new(
            &connection,
            "org.opdbus.v1",
            "/org/opdbus/v1/state",
            "org.opdbus.StateManagerV1",
        )
        .await
        .map_err(|e| SyncError::DBus(format!("StateManager proxy error: {}", e)))?;

        let _: String = proxy
            .call("ApplyContractMutation", &(request_json,))
            .await
            .map_err(|e| SyncError::DBus(format!("ApplyContractMutation error: {}", e)))?;
        Ok(())
    }

    async fn apply_mutation_via_plugin_surface(
        &self,
        plugin_id: &str,
        object_path: &str,
        change_type: ChangeType,
        member_name: Option<&str>,
        value: &simd_json::OwnedValue,
        actor_id: &str,
        capability_id: &Option<String>,
    ) -> Result<(), SyncError> {
        let bus_name = format!("org.opdbus.{}.v1", plugin_id);
        let iface_name = bus_name.clone();

        match change_type {
            ChangeType::PropertySet => {
                let property = member_name.ok_or_else(|| {
                    SyncError::Validation(
                        "member_name is required for PropertySet D-Bus mutation".to_string(),
                    )
                })?;
                let prop_value = extract_property_value_for_member(value, property);
                let zbus_value = simd_json_to_zvariant(&prop_value).map_err(|e| {
                    SyncError::Validation(format!("Property conversion error: {}", e))
                })?;

                let _permit = self
                    .dbus_call_limiter
                    .acquire()
                    .await
                    .map_err(|e| SyncError::DBus(format!("D-Bus limiter closed: {}", e)))?;
                let connection = self.dbus_connection().await?;
                let props_proxy = Proxy::new(
                    &connection,
                    bus_name.as_str(),
                    object_path,
                    "org.freedesktop.DBus.Properties",
                )
                .await
                .map_err(|e| SyncError::DBus(format!("Properties proxy error: {}", e)))?;

                let _: () = props_proxy
                    .call("Set", &(iface_name.as_str(), property, zbus_value))
                    .await
                    .map_err(|e| SyncError::DBus(format!("Property set error: {}", e)))?;
                Ok(())
            }
            ChangeType::MethodCall => {
                let method = member_name.ok_or_else(|| {
                    SyncError::Validation(
                        "member_name is required for MethodCall D-Bus mutation".to_string(),
                    )
                })?;
                let args = value
                    .as_object()
                    .and_then(|obj| obj.get("args"))
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.to_vec())
                    .unwrap_or_default();

                let _ = self
                    .call_dbus_method(
                        &bus_name,
                        object_path,
                        &iface_name,
                        method,
                        args,
                        actor_id,
                        capability_id,
                    )
                    .await?;
                Ok(())
            }
            _ => Err(SyncError::Validation(format!(
                "change_type '{:?}' is not supported for strict D-Bus mutation path",
                change_type
            ))),
        }
    }
}

fn extract_property_value_for_member(
    value: &simd_json::OwnedValue,
    member_name: &str,
) -> simd_json::OwnedValue {
    value
        .as_object()
        .and_then(|obj| obj.get("tunable"))
        .and_then(|v| v.as_object())
        .and_then(|tun| tun.get(member_name))
        .cloned()
        .unwrap_or_else(|| value.clone())
}

fn simd_json_to_zvariant(value: &simd_json::OwnedValue) -> Result<ZOwnedValue, anyhow::Error> {
    if let Some(obj) = value.as_object() {
        if let (Some(sig_val), Some(inner)) = (obj.get("sig"), obj.get("value")) {
            if let Some(sig) = sig_val.as_str() {
                return zvariant_from_sig(sig, inner);
            }
        }
    }

    if let Some(s) = value.as_str() {
        return Ok(ZOwnedValue::from(ZStr::from(s)));
    }
    if let Some(b) = value.as_bool() {
        return Ok(ZOwnedValue::from(b));
    }
    if let Some(i) = value.as_i64() {
        return Ok(ZOwnedValue::from(i));
    }
    if let Some(u) = value.as_u64() {
        return Ok(ZOwnedValue::from(u));
    }
    if let Some(f) = value.as_f64() {
        return Ok(ZOwnedValue::from(f));
    }

    Err(anyhow::anyhow!(
        "Unsupported argument type; use tagged {{sig,value}} or primitives"
    ))
}

fn zvariant_from_sig(
    sig: &str,
    value: &simd_json::OwnedValue,
) -> Result<ZOwnedValue, anyhow::Error> {
    match sig {
        "s" => value
            .as_str()
            .map(|v| ZOwnedValue::from(ZStr::from(v)))
            .ok_or_else(|| anyhow::anyhow!("Expected string for sig 's'")),
        "b" => value
            .as_bool()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected bool for sig 'b'")),
        "i" => value
            .as_i64()
            .map(|v| ZOwnedValue::from(v as i32))
            .ok_or_else(|| anyhow::anyhow!("Expected i32 for sig 'i'")),
        "u" => value
            .as_u64()
            .map(|v| ZOwnedValue::from(v as u32))
            .ok_or_else(|| anyhow::anyhow!("Expected u32 for sig 'u'")),
        "x" => value
            .as_i64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected i64 for sig 'x'")),
        "t" => value
            .as_u64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected u64 for sig 't'")),
        "d" => value
            .as_f64()
            .map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected f64 for sig 'd'")),
        "ay" => {
            let arr = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected array for sig 'ay'"))?;
            let bytes: Result<Vec<u8>, anyhow::Error> = arr
                .iter()
                .map(|v| {
                    v.as_u64()
                        .map(|n| n as u8)
                        .ok_or_else(|| anyhow::anyhow!("Expected u8 in ay array"))
                })
                .collect();
            ZOwnedValue::try_from(ZValue::Array(ZArray::from(bytes?)))
                .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
        }
        _ if sig.starts_with('a') => {
            let inner = &sig[1..];
            let arr = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected array for sig '{}'", sig))?;
            match inner {
                "s" => {
                    let items: Result<Vec<String>, anyhow::Error> = arr
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(|s| s.to_string())
                                .ok_or_else(|| anyhow::anyhow!("Expected string in array"))
                        })
                        .collect();
                    ZOwnedValue::try_from(ZValue::Array(ZArray::from(items?)))
                        .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
                }
                "i" => {
                    let items: Result<Vec<i32>, anyhow::Error> = arr
                        .iter()
                        .map(|v| {
                            v.as_i64()
                                .map(|n| n as i32)
                                .ok_or_else(|| anyhow::anyhow!("Expected i32 in array"))
                        })
                        .collect();
                    ZOwnedValue::try_from(ZValue::Array(ZArray::from(items?)))
                        .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
                }
                "u" => {
                    let items: Result<Vec<u32>, anyhow::Error> = arr
                        .iter()
                        .map(|v| {
                            v.as_u64()
                                .map(|n| n as u32)
                                .ok_or_else(|| anyhow::anyhow!("Expected u32 in array"))
                        })
                        .collect();
                    ZOwnedValue::try_from(ZValue::Array(ZArray::from(items?)))
                        .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
                }
                "b" => {
                    let items: Result<Vec<bool>, anyhow::Error> = arr
                        .iter()
                        .map(|v| {
                            v.as_bool()
                                .ok_or_else(|| anyhow::anyhow!("Expected bool in array"))
                        })
                        .collect();
                    ZOwnedValue::try_from(ZValue::Array(ZArray::from(items?)))
                        .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
                }
                "d" => {
                    let items: Result<Vec<f64>, anyhow::Error> = arr
                        .iter()
                        .map(|v| {
                            v.as_f64()
                                .ok_or_else(|| anyhow::anyhow!("Expected f64 in array"))
                        })
                        .collect();
                    ZOwnedValue::try_from(ZValue::Array(ZArray::from(items?)))
                        .map_err(|e| anyhow::anyhow!("Array conversion error: {}", e))
                }
                _ => Err(anyhow::anyhow!("Unsupported array signature '{}'", sig)),
            }
        }
        _ => Err(anyhow::anyhow!("Unsupported signature '{}'", sig)),
    }
}

/// Result of a mutation operation
#[derive(Debug, Clone)]
pub struct MutationResult {
    pub success: bool,
    pub event_id: u64,
    pub event_hash: String,
    pub result: Option<simd_json::OwnedValue>,
    pub error: Option<MutationError>,
}

/// Error during mutation
#[derive(Debug, Clone)]
pub struct MutationError {
    pub code: ErrorCode,
    pub message: String,
    pub deny_reason: Option<op_state_store::DenyReason>,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    NotFound,
    PermissionDenied,
    ValidationFailed,
    ReadOnly,
    TagLocked,
    Internal,
}

/// Errors that can occur in sync engine
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Event chain error: {0}")]
    EventChain(String),
    #[error("D-Bus error: {0}")]
    DBus(String),
    #[error("gRPC error: {0}")]
    Grpc(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

fn change_type_to_operation(change_type: ChangeType) -> OperationType {
    match change_type {
        ChangeType::PropertySet => OperationType::PropertySet,
        ChangeType::PropertyDelete => OperationType::PropertySet,
        ChangeType::MethodCall => OperationType::MethodCall,
        ChangeType::Signal => OperationType::EmitSignal,
        ChangeType::ObjectAdded => OperationType::ApplyTunablePatch,
        ChangeType::ObjectRemoved => OperationType::ApplyTunablePatch,
        ChangeType::SchemaMigration => OperationType::Migrate,
    }
}

fn validate_contract_envelope(
    plugin_id: &str,
    object_path: &str,
    value: &simd_json::OwnedValue,
) -> Result<(), SyncError> {
    let obj = value
        .as_object()
        .ok_or_else(|| SyncError::Validation("state must be a JSON object".to_string()))?;

    for required in [
        "schema_version",
        "plugin",
        "object_type",
        "object_id",
        "stub",
        "immutable",
        "tunable",
        "observed",
        "meta",
        "semantic_index",
        "privacy_index",
    ] {
        if !obj.contains_key(required) {
            return Err(SyncError::Validation(format!(
                "missing required contract field '{}'",
                required
            )));
        }
    }

    let payload_plugin = obj
        .get("plugin")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SyncError::Validation("plugin must be a string".to_string()))?;

    if payload_plugin != plugin_id {
        return Err(SyncError::Validation(format!(
            "plugin mismatch: path plugin='{}' payload plugin='{}'",
            plugin_id, payload_plugin
        )));
    }

    let object_id = obj
        .get("object_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SyncError::Validation("object_id must be a string".to_string()))?;

    if object_id.is_empty() {
        return Err(SyncError::Validation(
            "object_id must not be empty".to_string(),
        ));
    }

    if !object_path.is_empty() && !object_path.ends_with(object_id) {
        return Err(SyncError::Validation(format!(
            "object_path '{}' does not align with object_id '{}'",
            object_path, object_id
        )));
    }

    Ok(())
}

fn extract_tags_from_contract(value: &simd_json::OwnedValue) -> Vec<String> {
    value
        .as_object()
        .and_then(|obj| obj.get("meta"))
        .and_then(|meta| meta.as_object())
        .and_then(|meta_obj| meta_obj.get("tags"))
        .and_then(|tags| tags.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::ChainConfig;

    #[tokio::test]
    async fn test_subscription_filter() {
        let filter = SubscriptionFilter {
            plugin_ids: vec!["lxc".to_string()],
            path_patterns: vec![],
            tags: vec![],
        };

        let change = StateChange {
            change_id: "1".to_string(),
            event_id: 1,
            plugin_id: "lxc".to_string(),
            object_path: "/org/operation/lxc/100".to_string(),
            change_type: ChangeType::PropertySet,
            member_name: Some("running".to_string()),
            old_value: None,
            new_value: simd_json::json!(true),
            tags_touched: vec!["container".to_string()],
            event_hash: "abc".to_string(),
            timestamp: chrono::Utc::now(),
            actor_id: "user1".to_string(),
            source: ChangeSource::DBus,
        };

        assert!(filter.matches(&change));

        let filter2 = SubscriptionFilter {
            plugin_ids: vec!["net".to_string()],
            path_patterns: vec![],
            tags: vec![],
        };

        assert!(!filter2.matches(&change));
    }

    #[tokio::test]
    async fn test_sync_engine_dbus_change() {
        let chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
        let engine = SyncEngine::new(chain);

        let change = engine
            .process_dbus_change(
                "lxc".to_string(),
                "/org/operation/lxc/100".to_string(),
                ChangeType::PropertySet,
                Some("running".to_string()),
                Some(simd_json::json!(false)),
                simd_json::json!(true),
                vec!["container".to_string()],
                "user1".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(change.event_id, 1);
        assert_eq!(change.plugin_id, "lxc");
        assert_eq!(change.source, ChangeSource::DBus);
    }

    #[tokio::test]
    async fn test_sync_engine_grpc_mutation() {
        let chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
        let engine = SyncEngine::new(chain);
        let payload = simd_json::json!({
            "schema_version": "1.0.0",
            "plugin": "lxc",
            "object_type": "container",
            "object_id": "container-1",
            "stub": {"system_id":"node-1","source":"grpc","source_ref":"client","discovered_at":"2026-01-01T00:00:00Z"},
            "immutable": {"created_at":"2026-01-01T00:00:00Z","created_by_plugin":"lxc","identity_keys":["object_id"],"provider":"proxmox"},
            "tunable": {"memory":1024},
            "observed": {"last_observed_at":"2026-01-01T00:00:00Z"},
            "meta": {"dependencies":[],"include_in_recovery":true,"recovery_priority":1,"sensitivity":"internal","tags":["container"],"enabled":true},
            "semantic_index": {"include_paths":["/tunable"],"exclude_paths":[],"chunking":{"strategy":"json-path-group","max_tokens":512},"redaction":{"enabled":true}},
            "privacy_index": {"redaction":{"rules":[],"default_action":"mask","secret_paths":[],"pii_paths":[],"hash_salt_ref":"vault://salt","reversible":false}}
        });

        let result = engine
            .process_grpc_mutation(
                "lxc".to_string(),
                "/org/operation/lxc/container-1".to_string(),
                ChangeType::PropertySet,
                Some("memory".to_string()),
                payload,
                "grpc-client".to_string(),
                Some("admin".to_string()),
            )
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.event_id, 1);
    }

    #[tokio::test]
    async fn test_mutation_rejects_missing_contract_fields() {
        let chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
        let engine = SyncEngine::new(chain);

        let result = engine
            .process_grpc_mutation(
                "lxc".to_string(),
                "/org/operation/lxc/container-1".to_string(),
                ChangeType::PropertySet,
                Some("memory".to_string()),
                simd_json::json!({"plugin":"lxc"}), // incomplete contract envelope
                "grpc-client".to_string(),
                None,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sync_engine_jsonrpc_mutation_uses_same_pipeline() {
        let chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
        let engine = SyncEngine::new(chain);
        let payload = simd_json::json!({
            "schema_version": "1.0.0",
            "plugin": "lxc",
            "object_type": "container",
            "object_id": "container-1",
            "stub": {"system_id":"node-1","source":"jsonrpc","source_ref":"opnonnet","discovered_at":"2026-01-01T00:00:00Z"},
            "immutable": {"created_at":"2026-01-01T00:00:00Z","created_by_plugin":"lxc","identity_keys":["object_id"],"provider":"proxmox"},
            "tunable": {"memory":1024},
            "observed": {"last_observed_at":"2026-01-01T00:00:00Z"},
            "meta": {"dependencies":[],"include_in_recovery":true,"recovery_priority":1,"sensitivity":"internal","tags":["container"],"enabled":true},
            "semantic_index": {"include_paths":["/tunable"],"exclude_paths":[],"chunking":{"strategy":"json-path-group","max_tokens":512},"redaction":{"enabled":true}},
            "privacy_index": {"redaction":{"rules":[],"default_action":"mask","secret_paths":[],"pii_paths":[],"hash_salt_ref":"vault://salt","reversible":false}}
        });

        let result = engine
            .process_jsonrpc_mutation(
                "lxc".to_string(),
                "/org/operation/lxc/container-1".to_string(),
                ChangeType::PropertySet,
                Some("memory".to_string()),
                payload,
                "jsonrpc-client".to_string(),
                None,
            )
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.event_id, 1);
    }
}
