//! D-Bus Watcher - Monitors D-Bus for property changes and signals
//!
//! Watches specified D-Bus paths and interfaces, forwarding changes
//! to the sync engine for propagation to gRPC subscribers.

use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use zbus::{Connection, MatchRule, MessageStream};

use crate::sync_engine::{ChangeSource, ChangeType, StateChange, SyncEngine};

/// Configuration for D-Bus watching
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// D-Bus paths to watch
    pub paths: Vec<String>,
    /// Interfaces to watch (empty = all)
    pub interfaces: Vec<String>,
    /// Whether to use system bus (true) or session bus (false)
    pub use_system_bus: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            paths: vec![
                "/org/operation".to_string(),
                "/org/opdbus/v1".to_string(),
                "/org/opdbus/v1/ovsdb".to_string(),
                "/org/opdbus/v1/nonnet".to_string(),
            ],
            interfaces: vec!["org.opdbus.ProjectedObjectV1".to_string()],
            use_system_bus: true,
        }
    }
}

/// D-Bus watcher that monitors for changes
pub struct DbusWatcher {
    config: WatchConfig,
    sync_engine: Arc<SyncEngine>,
    connection: Option<Connection>,
    /// Mapping of D-Bus path to plugin ID
    path_to_plugin: Arc<RwLock<HashMap<String, String>>>,
}

impl DbusWatcher {
    /// Create a new D-Bus watcher
    pub fn new(config: WatchConfig, sync_engine: Arc<SyncEngine>) -> Self {
        Self {
            config,
            sync_engine,
            connection: None,
            path_to_plugin: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a path-to-plugin mapping
    pub async fn register_path(&self, path: String, plugin_id: String) {
        let mut mapping = self.path_to_plugin.write().await;
        mapping.insert(path, plugin_id);
    }

    /// Connect to D-Bus
    pub async fn connect(&mut self) -> Result<(), WatcherError> {
        let connection = if self.config.use_system_bus {
            Connection::system().await
        } else {
            Connection::session().await
        };

        self.connection = Some(connection.map_err(|e| WatcherError::Connection(e.to_string()))?);
        info!("Connected to D-Bus (system={})", self.config.use_system_bus);
        Ok(())
    }

    /// Start watching for changes
    pub async fn start(&self) -> Result<(), WatcherError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| WatcherError::NotConnected)?;

        // Set up PropertiesChanged signal matching
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.freedesktop.DBus.Properties")
            .map_err(|e| WatcherError::MatchRule(e.to_string()))?
            .member("PropertiesChanged")
            .map_err(|e| WatcherError::MatchRule(e.to_string()))?
            .build();

        // Subscribe to the match rule
        let proxy = zbus::fdo::DBusProxy::new(connection)
            .await
            .map_err(|e| WatcherError::Proxy(e.to_string()))?;

        proxy
            .add_match_rule(rule)
            .await
            .map_err(|e| WatcherError::MatchRule(e.to_string()))?;

        info!("Started watching D-Bus for PropertiesChanged signals");

        // Also watch for custom signals from our interfaces
        for interface in &self.config.interfaces {
            let rule = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .interface(interface.as_str())
                .map_err(|e| WatcherError::MatchRule(e.to_string()))?
                .build();

            proxy
                .add_match_rule(rule)
                .await
                .map_err(|e| WatcherError::MatchRule(e.to_string()))?;

            debug!("Added match rule for interface: {}", interface);
        }

        Ok(())
    }

    /// Run the watcher loop (spawns a task)
    pub fn spawn(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run_loop().await {
                error!("D-Bus watcher error: {}", e);
            }
        })
    }

    /// Main watcher loop
    async fn run_loop(&self) -> Result<(), WatcherError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| WatcherError::NotConnected)?;

        let mut stream = MessageStream::from(connection);

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(message) => {
                    if let Err(e) = self.handle_message(&message).await {
                        warn!("Error handling D-Bus message: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Error receiving D-Bus message: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Handle a D-Bus message
    async fn handle_message(&self, message: &zbus::Message) -> Result<(), WatcherError> {
        let header = message.header();

        // Get the path
        let path = header
            .path()
            .ok_or_else(|| WatcherError::InvalidMessage("No path in message".to_string()))?
            .to_string();

        // Check if this path is in our watch list
        let should_watch = self
            .config
            .paths
            .iter()
            .any(|p| path.starts_with(p) || p == "*");

        if !should_watch {
            return Ok(());
        }

        // Get the interface and member
        let interface = header
            .interface()
            .map(|i| i.to_string())
            .unwrap_or_default();
        let member = header.member().map(|m| m.to_string()).unwrap_or_default();

        // Determine plugin ID from path
        let plugin_id = {
            let mapping = self.path_to_plugin.read().await;
            mapping
                .get(&path)
                .cloned()
                .or_else(|| self.extract_plugin_from_path(&path))
                .unwrap_or_else(|| "unknown".to_string())
        };

        // Handle PropertiesChanged
        if interface == "org.freedesktop.DBus.Properties" && member == "PropertiesChanged" {
            self.handle_properties_changed(&path, &plugin_id, message)
                .await?;
        }
        // Handle other signals
        else if message.message_type() == zbus::message::Type::Signal {
            self.handle_signal(&path, &plugin_id, &interface, &member, message)
                .await?;
        }

        Ok(())
    }

    /// Handle PropertiesChanged signal
    async fn handle_properties_changed(
        &self,
        path: &str,
        plugin_id: &str,
        message: &zbus::Message,
    ) -> Result<(), WatcherError> {
        // Deserialize the PropertiesChanged body
        // Body: (interface_name, changed_properties, invalidated_properties)
        let body: (
            String,
            HashMap<String, zbus::zvariant::OwnedValue>,
            Vec<String>,
        ) = message
            .body()
            .deserialize()
            .map_err(|e| WatcherError::Deserialize(e.to_string()))?;

        let (interface_name, changed_props, _invalidated) = body;

        for (prop_name, value) in changed_props {
            // Convert zvariant to simd_json::OwnedValue
            let json_value = zvariant_to_json(&value);

            debug!(
                "Property changed: path={}, interface={}, property={}, value={:?}",
                path, interface_name, prop_name, json_value
            );

            // Forward to sync engine
            if let Err(e) = self
                .sync_engine
                .process_dbus_change(
                    plugin_id.to_string(),
                    path.to_string(),
                    ChangeType::PropertySet,
                    Some(prop_name.clone()),
                    None, // TODO: track old values
                    json_value,
                    vec![], // TODO: compute tags from schema
                    "dbus".to_string(),
                )
                .await
            {
                warn!("Failed to process D-Bus change: {}", e);
            }
        }

        Ok(())
    }

    /// Handle a D-Bus signal
    async fn handle_signal(
        &self,
        path: &str,
        plugin_id: &str,
        interface: &str,
        member: &str,
        message: &zbus::Message,
    ) -> Result<(), WatcherError> {
        // Get signal arguments as JSON
        let body_bytes = message.body().data().to_vec();
        let args = if body_bytes.is_empty() {
            simd_json::json!([])
        } else {
            // Try to deserialize as generic value
            simd_json::json!({
                "raw": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &body_bytes)
            })
        };

        debug!(
            "Signal received: path={}, interface={}, member={}",
            path, interface, member
        );

        // Forward to sync engine
        if let Err(e) = self
            .sync_engine
            .process_dbus_change(
                plugin_id.to_string(),
                path.to_string(),
                ChangeType::Signal,
                Some(format!("{}.{}", interface, member)),
                None,
                args,
                vec![],
                "dbus".to_string(),
            )
            .await
        {
            warn!("Failed to process D-Bus signal: {}", e);
        }

        Ok(())
    }

    /// Extract plugin ID from a D-Bus path
    fn extract_plugin_from_path(&self, path: &str) -> Option<String> {
        // Pattern: /org/operation/{plugin}/...
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 4 && parts[1] == "org" && parts[2] == "operation" {
            Some(parts[3].to_string())
        } else {
            None
        }
    }
}

/// Errors that can occur in the D-Bus watcher
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Not connected to D-Bus")]
    NotConnected,
    #[error("Match rule error: {0}")]
    MatchRule(String),
    #[error("Proxy error: {0}")]
    Proxy(String),
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
    #[error("Deserialize error: {0}")]
    Deserialize(String),
}

/// Convert zbus::zvariant::OwnedValue to simd_json::OwnedValue
fn zvariant_to_json(value: &zbus::zvariant::OwnedValue) -> simd_json::OwnedValue {
    use zbus::zvariant::Value;

    match value.downcast_ref::<Value>() {
        Ok(v) => match v {
            Value::Bool(b) => simd_json::json!(b),
            Value::U8(n) => simd_json::json!(n),
            Value::I16(n) => simd_json::json!(n),
            Value::U16(n) => simd_json::json!(n),
            Value::I32(n) => simd_json::json!(n),
            Value::U32(n) => simd_json::json!(n),
            Value::I64(n) => simd_json::json!(n),
            Value::U64(n) => simd_json::json!(n),
            Value::F64(n) => simd_json::json!(n),
            Value::Str(s) => simd_json::json!(s.as_str()),
            Value::Array(arr) => {
                let items: Vec<simd_json::OwnedValue> = arr
                    .iter()
                    .filter_map(|item| {
                        let owned = zbus::zvariant::OwnedValue::try_from(item.clone()).ok()?;
                        Some(zvariant_to_json(&owned))
                    })
                    .collect();
                simd_json::json!(items)
            }
            Value::Dict(dict) => {
                let mut map = simd_json::value::owned::Object::new();
                for (k, v) in dict.iter() {
                    if let Ok(key) = k.downcast_ref::<&str>() {
                        let owned = zbus::zvariant::OwnedValue::try_from(v.clone()).ok();
                        if let Some(owned) = owned {
                            map.insert(key.to_string(), zvariant_to_json(&owned));
                        }
                    }
                }
                simd_json::OwnedValue::Object(Box::new(map))
            }
            _ => simd_json::json!(format!("{:?}", v)),
        },
        Err(_) => simd_json::json!(format!("{:?}", value)),
    }
}
