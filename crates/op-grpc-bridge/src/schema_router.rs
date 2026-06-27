//! Schema-Driven Dynamic gRPC Router
//!
//! Reads the live schema catalog from `/dev/shm/live-schema.json` (combined
//! monolith) and/or per-plugin schema files from the `schemas/plugin/` directory.
//! Dynamically routes incoming gRPC calls to their corresponding D-Bus objects
//! at `/org/opdbus/v1/plugins/<plugin>`.
//!
//! Every plugin defined in the schema catalog is automatically exposed as a
//! gRPC-callable service through the bridge. Unknown methods are rejected by
//! schema validation — no hand-rolling required.
//!
//! ## Schema Access Paths (both supported)
//!
//! 1. **Per-plugin files:** `schemas/plugin/<name>.json` — individual schema objects.
//! 2. **Combined monolith:** `/dev/shm/live-schema.json` — derived catalog of all plugins.
//!
//! The manifest hash is read, never re-computed. Consumers trust the manifest.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::RwLock;
use tracing::{debug, info};
use zbus::{Connection, Proxy};

use op_plugins::canonical::{self, BASE_SERVICE_NAME};

/// The combined monolith catalog in shared memory.
const LIVE_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

/// Per-plugin schema directory (relative to project root, resolved at runtime).
const PER_PLUGIN_SCHEMA_DIR: &str = "/etc/opdbus/schemas/plugin";

/// A schema-derived route entry for a plugin.
#[derive(Debug, Clone)]
pub struct PluginRoute {
    /// Plugin identifier (e.g. "unix_socket", "wireguard").
    pub plugin_id: String,
    /// Canonical D-Bus object path: `/org/opdbus/v1/plugins/<plugin_id>`.
    pub dbus_path: String,
    /// Canonical D-Bus destination (bus name): `org.opdbus.v1`.
    pub dbus_destination: String,
    /// Canonical D-Bus interface: `org.opdbus.v1.Plugin.<PluginName>`.
    pub dbus_interface: String,
    /// Methods declared in the schema (name → argument JSON schema).
    pub methods: HashMap<String, JsonValue>,
    /// Properties declared in the schema (name → field schema).
    pub properties: HashMap<String, JsonValue>,
}

/// The dynamic schema router that maps plugin_id → D-Bus route.
pub struct SchemaRouter {
    /// Plugin routes keyed by plugin_id.
    routes: Arc<RwLock<HashMap<String, PluginRoute>>>,
    /// D-Bus system connection (shared with SchemaEngine).
    dbus_connection: Arc<tokio::sync::OnceCell<Connection>>,
}

impl SchemaRouter {
    /// Create a new SchemaRouter and load routes from both schema sources.
    ///
    /// Loads synchronously at startup — the schema in /dev/shm is authoritative
    /// present-state and is always available. No polling or watchers.
    pub fn new(dbus_connection: Arc<tokio::sync::OnceCell<Connection>>) -> Self {
        let routes = Self::load_all_routes();
        info!(
            count = routes.len(),
            "SchemaRouter initialized from schema catalog"
        );
        Self {
            routes: Arc::new(RwLock::new(routes)),
            dbus_connection,
        }
    }

    /// Register D-Bus objects for all plugins in the catalog.
    ///
    /// This makes the bridge the authoritative creator of the D-Bus objects
    /// defined in the schema.
    pub async fn register_objects(&self) -> anyhow::Result<()> {
        let conn = self
            .dbus_connection
            .get()
            .ok_or_else(|| anyhow::anyhow!("D-Bus connection not initialized"))?;

        let routes = self.routes.read().await;
        for (plugin_id, route) in routes.iter() {
            let interface = SchemaBackedInterface::new(plugin_id.clone(), route.clone());
            let path = route.dbus_path.clone();

            debug!(plugin_id, path, "Registering authoritative D-Bus object");
            if let Err(e) = conn.object_server().at(path.as_str(), interface).await {
                debug!(plugin_id, path, error = %e, "Failed to register D-Bus object (likely already registered)");
            }
        }
        Ok(())
    }

    /// Reload routes and re-register objects.
    pub async fn reload(&self) -> anyhow::Result<()> {
        let routes = Self::load_all_routes();
        let count = routes.len();
        {
            let mut w = self.routes.write().await;
            *w = routes;
        }
        info!(count, "SchemaRouter reloaded plugin routes");
        self.register_objects().await
    }

    /// List all currently routable plugin IDs.
    pub async fn list_plugin_ids(&self) -> Vec<String> {
        let r = self.routes.read().await;
        r.keys().cloned().collect()
    }

    /// Get the route for a specific plugin.
    pub async fn get_route(&self, plugin_id: &str) -> Option<PluginRoute> {
        let r = self.routes.read().await;
        r.get(plugin_id).cloned()
    }

    /// Call a method on a plugin via its schema-derived D-Bus route.
    pub async fn call_method(
        &self,
        plugin_id: &str,
        method_name: &str,
        json_args: &str,
    ) -> Result<String, SchemaRouterError> {
        let route = self
            .get_route(plugin_id)
            .await
            .ok_or_else(|| SchemaRouterError::PluginNotFound(plugin_id.to_string()))?;

        if !route.methods.contains_key(method_name) {
            return Err(SchemaRouterError::MethodNotFound {
                plugin_id: plugin_id.to_string(),
                method: method_name.to_string(),
                available: route.methods.keys().cloned().collect(),
            });
        }

        let conn = self
            .dbus_connection
            .get()
            .ok_or(SchemaRouterError::DbusUnavailable)?;

        let proxy = Proxy::new(
            conn,
            route.dbus_destination.as_str(),
            route.dbus_path.as_str(),
            route.dbus_interface.as_str(),
        )
        .await
        .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?;

        let result: String = proxy
            .call(method_name, &(json_args.to_string(),))
            .await
            .map_err(|e| SchemaRouterError::DbusCallFailed {
                method: method_name.to_string(),
                error: e.to_string(),
            })?;

        Ok(result)
    }

    /// Get a property from a plugin via its schema-derived D-Bus route.
    pub async fn get_property(
        &self,
        plugin_id: &str,
        property_name: &str,
    ) -> Result<String, SchemaRouterError> {
        let route = self
            .get_route(plugin_id)
            .await
            .ok_or_else(|| SchemaRouterError::PluginNotFound(plugin_id.to_string()))?;

        if !route.properties.contains_key(property_name) {
            return Err(SchemaRouterError::PropertyNotFound {
                plugin_id: plugin_id.to_string(),
                property: property_name.to_string(),
                available: route.properties.keys().cloned().collect(),
            });
        }

        let conn = self
            .dbus_connection
            .get()
            .ok_or(SchemaRouterError::DbusUnavailable)?;

        let props = zbus::fdo::PropertiesProxy::builder(conn)
            .destination(route.dbus_destination.as_str())
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?
            .path(route.dbus_path.as_str())
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?
            .build()
            .await
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(route.dbus_interface.as_str())
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?;

        let val: zbus::zvariant::OwnedValue =
            props.get(iface, property_name).await.map_err(|e| {
                SchemaRouterError::DbusCallFailed {
                    method: format!("Get({})", property_name),
                    error: e.to_string(),
                }
            })?;

        serde_json::to_string(&val)
            .map_err(|e| SchemaRouterError::SerializationFailed(e.to_string()))
    }

    /// Set a property on a plugin via its schema-derived D-Bus route.
    pub async fn set_property(
        &self,
        plugin_id: &str,
        property_name: &str,
        json_value: &str,
    ) -> Result<(), SchemaRouterError> {
        let route = self
            .get_route(plugin_id)
            .await
            .ok_or_else(|| SchemaRouterError::PluginNotFound(plugin_id.to_string()))?;

        if !route.properties.contains_key(property_name) {
            return Err(SchemaRouterError::PropertyNotFound {
                plugin_id: plugin_id.to_string(),
                property: property_name.to_string(),
                available: route.properties.keys().cloned().collect(),
            });
        }

        let conn = self
            .dbus_connection
            .get()
            .ok_or(SchemaRouterError::DbusUnavailable)?;

        let props = zbus::fdo::PropertiesProxy::builder(conn)
            .destination(route.dbus_destination.as_str())
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?
            .path(route.dbus_path.as_str())
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?
            .build()
            .await
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?;

        let iface = zbus::names::InterfaceName::try_from(route.dbus_interface.as_str())
            .map_err(|e| SchemaRouterError::ProxyBuildFailed(e.to_string()))?;

        let value: serde_json::Value = serde_json::from_str(json_value)
            .map_err(|e| SchemaRouterError::SerializationFailed(e.to_string()))?;

        let zval = json_to_zvariant_value(&value)?;

        props.set(iface, property_name, zval).await.map_err(|e| {
            SchemaRouterError::DbusCallFailed {
                method: format!("Set({})", property_name),
                error: e.to_string(),
            }
        })?;

        Ok(())
    }

    // ── Private helpers ─────────────────────────────────────────────────────

    fn load_all_routes() -> HashMap<String, PluginRoute> {
        let mut routes = HashMap::new();
        if let Some(monolith_routes) = Self::load_from_monolith() {
            for (id, route) in monolith_routes {
                routes.insert(id, route);
            }
        }
        if let Some(per_plugin_routes) = Self::load_from_per_plugin_dir() {
            for (id, route) in per_plugin_routes {
                routes.entry(id).or_insert(route);
            }
        }
        routes
    }

    fn load_from_monolith() -> Option<HashMap<String, PluginRoute>> {
        let bytes = std::fs::read(LIVE_SCHEMA_PATH).ok()?;
        let root: JsonValue = serde_json::from_slice(&bytes).ok()?;
        let catalog = root.as_object()?;

        let mut routes = HashMap::new();
        for (plugin_id, schema_value) in catalog {
            let schema = match schema_value {
                JsonValue::Array(arr) => arr.first()?,
                JsonValue::Object(_) => schema_value,
                _ => continue,
            };
            let route = Self::build_route(plugin_id, schema);
            routes.insert(plugin_id.clone(), route);
        }
        Some(routes)
    }

    fn load_from_per_plugin_dir() -> Option<HashMap<String, PluginRoute>> {
        let dir = Path::new(PER_PLUGIN_SCHEMA_DIR);
        if !dir.is_dir() {
            return None;
        }
        let mut routes = HashMap::new();
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let plugin_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if plugin_id.is_empty() {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(schema) = serde_json::from_slice::<JsonValue>(&bytes) {
                    let route = Self::build_route(&plugin_id, &schema);
                    routes.insert(plugin_id, route);
                }
            }
        }
        Some(routes)
    }

    fn build_route(plugin_id: &str, schema: &JsonValue) -> PluginRoute {
        let dbus_path = canonical::plugin_path(plugin_id);
        let dbus_destination = BASE_SERVICE_NAME.to_string();
        let dbus_interface = canonical::plugin_interface(plugin_id);
        let methods = Self::extract_methods(schema);
        let properties = Self::extract_properties(schema);
        PluginRoute {
            plugin_id: plugin_id.to_string(),
            dbus_path,
            dbus_destination,
            dbus_interface,
            methods,
            properties,
        }
    }

    fn extract_methods(schema: &JsonValue) -> HashMap<String, JsonValue> {
        let mut methods = HashMap::new();
        if let Some(method_map) = schema.get("methods").and_then(|m| m.as_object()) {
            for (name, def) in method_map {
                methods.insert(name.clone(), def.clone());
            }
        }
        methods
    }

    fn extract_properties(schema: &JsonValue) -> HashMap<String, JsonValue> {
        let mut properties = HashMap::new();
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (name, def) in props {
                properties.insert(name.clone(), def.clone());
            }
        }
        properties
    }
}

/// A generic D-Bus interface backed by a plugin schema.
///
/// This struct is registered on the bus for every plugin, making the bridge
/// the authoritative owner of the D-Bus objects defined in the schema.
pub struct SchemaBackedInterface {
    plugin_id: String,
    route: PluginRoute,
    /// Local state for properties (initialized from schema defaults).
    state: Arc<RwLock<HashMap<String, JsonValue>>>,
}

impl SchemaBackedInterface {
    pub fn new(plugin_id: String, route: PluginRoute) -> Self {
        let mut initial_state = HashMap::new();
        for (name, def) in &route.properties {
            let default = def.get("default").cloned().unwrap_or(JsonValue::Null);
            initial_state.insert(name.clone(), default);
        }
        Self {
            plugin_id,
            route,
            state: Arc::new(RwLock::new(initial_state)),
        }
    }
}

#[zbus::interface(name = "org.opdbus.v1.PluginV1")]
impl SchemaBackedInterface {
    /// Generic method call dispatcher.
    ///
    /// All schema-defined methods are funneled through this call, allowing
    /// dynamic dispatch without compile-time traits for every plugin.
    async fn call(&self, method: String, _json_args: String) -> zbus::fdo::Result<String> {
        if !self.route.methods.contains_key(&method) {
            return Err(zbus::fdo::Error::UnknownMethod(method));
        }
        // In the bridge, a D-Bus call to a SchemaBackedInterface usually means
        // a container is calling a host service. The bridge handles this by
        // either performing the action or proxying it.
        info!(plugin_id = %self.plugin_id, method, "D-Bus method call received");

        // This is where the authoritative bridge logic lives.
        // For now, we return a successful response acknowledging the call.
        // In a full implementation, this would trigger the actual plugin action.
        Ok(format!(
            r#"{{"success": true, "plugin": "{}", "method": "{}"}}"#,
            self.plugin_id, method
        ))
    }

    /// Get a property value.
    async fn get_property(&self, name: String) -> zbus::fdo::Result<String> {
        let state = self.state.read().await;
        let val = state
            .get(&name)
            .ok_or_else(|| zbus::fdo::Error::UnknownProperty(name))?;
        serde_json::to_string(val).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set a property value.
    async fn set_property(&self, name: String, json_value: String) -> zbus::fdo::Result<()> {
        if !self.route.properties.contains_key(&name) {
            return Err(zbus::fdo::Error::UnknownProperty(name));
        }
        let val: JsonValue = serde_json::from_str(&json_value)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        let mut state = self.state.write().await;
        state.insert(name, val);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaRouterError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    #[error("Method {method} not found for plugin {plugin_id}. Available: {available:?}")]
    MethodNotFound {
        plugin_id: String,
        method: String,
        available: Vec<String>,
    },
    #[error("Property {property} not found for plugin {plugin_id}. Available: {available:?}")]
    PropertyNotFound {
        plugin_id: String,
        property: String,
        available: Vec<String>,
    },
    #[error("D-Bus unavailable")]
    DbusUnavailable,
    #[error("D-Bus proxy build failed: {0}")]
    ProxyBuildFailed(String),
    #[error("D-Bus call failed: {method}: {error}")]
    DbusCallFailed { method: String, error: String },
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),
}

impl From<SchemaRouterError> for tonic::Status {
    fn from(err: SchemaRouterError) -> Self {
        match err {
            SchemaRouterError::PluginNotFound(id) => {
                tonic::Status::not_found(format!("Plugin not found: {}", id))
            }
            SchemaRouterError::MethodNotFound {
                plugin_id,
                method,
                available,
            } => tonic::Status::not_found(format!(
                "Method {} not found for plugin {}. Available: {:?}",
                method, plugin_id, available
            )),
            SchemaRouterError::PropertyNotFound {
                plugin_id,
                property,
                available,
            } => tonic::Status::not_found(format!(
                "Property {} not found for plugin {}. Available: {:?}",
                property, plugin_id, available
            )),
            SchemaRouterError::DbusUnavailable => {
                tonic::Status::unavailable("D-Bus connection unavailable")
            }
            SchemaRouterError::ProxyBuildFailed(msg) => {
                tonic::Status::internal(format!("D-Bus proxy build failed: {}", msg))
            }
            SchemaRouterError::DbusCallFailed { method, error } => {
                tonic::Status::internal(format!("D-Bus call failed: {}: {}", method, error))
            }
            SchemaRouterError::SerializationFailed(msg) => {
                tonic::Status::internal(format!("Serialization failed: {}", msg))
            }
        }
    }
}

fn json_to_zvariant_value(
    value: &serde_json::Value,
) -> Result<zbus::zvariant::Value<'static>, SchemaRouterError> {
    use zbus::zvariant::Value as ZValue;
    match value {
        serde_json::Value::Null => Ok(ZValue::from("")),
        serde_json::Value::Bool(b) => Ok(ZValue::from(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(ZValue::from(i))
            } else if let Some(f) = n.as_f64() {
                Ok(ZValue::from(f))
            } else {
                Ok(ZValue::from(n.to_string()))
            }
        }
        serde_json::Value::String(s) => Ok(ZValue::from(s.clone())),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect();
            Ok(ZValue::from(items))
        }
        serde_json::Value::Object(_) => {
            let s = serde_json::to_string(value)
                .map_err(|e| SchemaRouterError::SerializationFailed(e.to_string()))?;
            Ok(ZValue::from(s))
        }
    }
}
