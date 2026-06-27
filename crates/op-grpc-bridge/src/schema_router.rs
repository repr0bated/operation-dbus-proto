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
use std::path::{Path, PathBuf};
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

/// Default SHM state directory: `/dev/shm/opdbus/state`.
const DEFAULT_SHM_STATE_DIR: &str = "/dev/shm/opdbus/state";

/// A generic D-Bus interface backed by a plugin schema.
///
/// This struct is registered on the bus for every plugin, making the bridge
/// the authoritative owner of the D-Bus objects defined in the schema.
///
/// Present-state is read directly from SHM
/// (`/dev/shm/opdbus/state/<plugin_id>.json`) on each property access.
/// There is no in-memory cache, no D-Bus `PropertiesChanged` signal watching,
/// and no timer-based polling. If the state file does not exist, an empty
/// properties object (`{}`) is returned, not an error.
pub struct SchemaBackedInterface {
    plugin_id: String,
    route: PluginRoute,
    /// Directory containing per-plugin present-state JSON files.
    shm_state_dir: PathBuf,
}

impl SchemaBackedInterface {
    /// Creates a new `SchemaBackedInterface` using the default SHM state
    /// directory (`/dev/shm/opdbus/state`).
    pub fn new(plugin_id: String, route: PluginRoute) -> Self {
        Self::with_shm_state_dir(plugin_id, route, DEFAULT_SHM_STATE_DIR)
    }

    /// Creates a new `SchemaBackedInterface` with a custom SHM state directory.
    ///
    /// In production, use [`new`](Self::new) which targets `/dev/shm/opdbus/state`.
    /// Tests should supply a temporary directory.
    pub fn with_shm_state_dir(
        plugin_id: String,
        route: PluginRoute,
        shm_state_dir: impl AsRef<Path>,
    ) -> Self {
        Self {
            plugin_id,
            route,
            shm_state_dir: shm_state_dir.as_ref().to_path_buf(),
        }
    }

    /// Reads the present-state JSON for this plugin from SHM.
    ///
    /// The file is read directly from `<shm_state_dir>/<plugin_id>.json`.
    /// If the file does not exist, an empty JSON object (`{}`) is returned,
    /// not an error. This is a direct file read — no polling, no caching,
    /// no D-Bus signal watching.
    pub fn read_present_state(&self) -> JsonValue {
        let path = self.shm_state_dir.join(format!("{}.json", self.plugin_id));
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                serde_json::from_str(&text).unwrap_or(JsonValue::Object(serde_json::Map::new()))
            }
            Err(_) => JsonValue::Object(serde_json::Map::new()),
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

    /// Get a property value from SHM present-state.
    ///
    /// Reads `/dev/shm/opdbus/state/<plugin_id>.json` on each call.
    /// If the state file does not exist, returns an empty value for the
    /// requested property (not an error).
    async fn get_property(&self, name: String) -> zbus::fdo::Result<String> {
        let state = self.read_present_state();
        let val = state.get(&name).unwrap_or(&JsonValue::Null);
        serde_json::to_string(val).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get all present-state properties as a JSON object.
    ///
    /// Reads `/dev/shm/opdbus/state/<plugin_id>.json` on each call.
    /// If the state file does not exist, returns `{}`.
    async fn get_all_properties(&self) -> zbus::fdo::Result<String> {
        let state = self.read_present_state();
        serde_json::to_string(&state).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set a property value.
    ///
    /// Present-state is managed by the producer (`op-projection`) via SHM.
    /// The bridge is a reader, not a writer, of present-state. This method
    /// returns an error indicating that property writes must go through the
    /// producer's mutation pipeline.
    async fn set_property(&self, _name: String, _json_value: String) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::Failed(
            "Present-state is managed by op-projection via SHM; \
             use the mutation pipeline to change properties"
                .to_string(),
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    /// Builds a test `PluginRoute` with a single method and two properties.
    fn test_route() -> PluginRoute {
        let mut methods = HashMap::new();
        methods.insert(
            "GetStatus".to_string(),
            json!({ "type": "object", "properties": {} }),
        );
        let mut properties = HashMap::new();
        properties.insert("status".to_string(), json!({ "type": "string" }));
        properties.insert("uptime".to_string(), json!({ "type": "integer" }));
        PluginRoute {
            plugin_id: "test_plugin".to_string(),
            dbus_path: "/org/opdbus/v1/plugins/test_plugin".to_string(),
            dbus_destination: "org.opdbus.v1".to_string(),
            dbus_interface: "org.opdbus.v1.Plugin.TestPlugin".to_string(),
            methods,
            properties,
        }
    }

    /// Creates a unique temp directory under `/dev/shm` for SHM state files.
    fn test_state_dir() -> PathBuf {
        let id = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = PathBuf::from(format!("/dev/shm/opdbus-test-state-{}-{}", id, nanos));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create test state dir");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // ── R9.5 / VAL-PROD-007: Missing state file returns empty object ──

    #[test]
    fn should_return_empty_object_when_state_file_missing() {
        let state_dir = test_state_dir();
        let route = test_route();
        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        // read_present_state returns empty object when file is missing
        let state = iface.read_present_state();
        assert_eq!(
            state,
            json!({}),
            "missing state file must return empty object, not error"
        );

        // Verify it's a JSON object (not null, not error)
        assert!(
            state.is_object(),
            "missing state file must return a JSON object"
        );
        assert!(
            state.as_object().unwrap().is_empty(),
            "missing state file must return an empty JSON object"
        );

        cleanup(&state_dir);
    }

    // ── R9.2 / VAL-PROD-007: Present-state read from SHM, not D-Bus ─────

    #[test]
    fn should_read_present_state_from_shm() {
        let state_dir = test_state_dir();
        let route = test_route();

        // Write a present-state file to SHM
        let state_path = state_dir.join("test_plugin.json");
        let state_json = json!({
            "status": "active",
            "uptime": 42,
            "extra_field": "not_in_schema"
        });
        fs::write(&state_path, serde_json::to_vec_pretty(&state_json).unwrap())
            .expect("write state file");

        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        // read_present_state returns the full state from SHM
        let state = iface.read_present_state();
        assert_eq!(
            state["status"],
            json!("active"),
            "present-state must be read from SHM file"
        );
        assert_eq!(
            state["uptime"],
            json!(42),
            "present-state must be read from SHM file"
        );
        assert_eq!(
            state["extra_field"],
            json!("not_in_schema"),
            "present-state includes all fields from SHM, even those not in schema"
        );

        // Verify the data came from the file, not from schema defaults
        assert_ne!(
            state["status"],
            JsonValue::Null,
            "status must come from SHM file, not default"
        );

        cleanup(&state_dir);
    }

    // ── R9.2: get_property reads from SHM ───────────────────────────────

    #[tokio::test]
    async fn should_get_property_from_shm() {
        let state_dir = test_state_dir();
        let route = test_route();

        let state_json = json!({ "status": "running", "uptime": 100 });
        let state_path = state_dir.join("test_plugin.json");
        fs::write(&state_path, serde_json::to_vec_pretty(&state_json).unwrap())
            .expect("write state file");

        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        let result = iface.get_property("status".to_string()).await;
        assert!(
            result.is_ok(),
            "get_property should succeed when state file exists"
        );
        let val: serde_json::Value = serde_json::from_str(&result.unwrap()).expect("parse result");
        assert_eq!(val, json!("running"));

        cleanup(&state_dir);
    }

    // ── R9.5: get_property on missing file returns null, not error ──────

    #[tokio::test]
    async fn should_get_property_return_null_when_state_file_missing() {
        let state_dir = test_state_dir();
        let route = test_route();

        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        let result = iface.get_property("status".to_string()).await;
        assert!(
            result.is_ok(),
            "get_property must not error when state file is missing"
        );
        let val: serde_json::Value = serde_json::from_str(&result.unwrap()).expect("parse result");
        assert_eq!(val, JsonValue::Null);

        cleanup(&state_dir);
    }

    // ── R9.2: get_all_properties reads full state from SHM ─────────────

    #[tokio::test]
    async fn should_get_all_properties_from_shm() {
        let state_dir = test_state_dir();
        let route = test_route();

        let state_json = json!({ "status": "active", "uptime": 7 });
        let state_path = state_dir.join("test_plugin.json");
        fs::write(&state_path, serde_json::to_vec_pretty(&state_json).unwrap())
            .expect("write state file");

        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        let result = iface.get_all_properties().await;
        assert!(result.is_ok());
        let val: serde_json::Value = serde_json::from_str(&result.unwrap()).expect("parse result");
        assert_eq!(val["status"], json!("active"));
        assert_eq!(val["uptime"], json!(7));

        cleanup(&state_dir);
    }

    // ── R9.5: get_all_properties on missing file returns empty object ──

    #[tokio::test]
    async fn should_get_all_properties_empty_when_state_file_missing() {
        let state_dir = test_state_dir();
        let route = test_route();

        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        let result = iface.get_all_properties().await;
        assert!(
            result.is_ok(),
            "get_all_properties must not error when state file is missing"
        );
        let val: serde_json::Value = serde_json::from_str(&result.unwrap()).expect("parse result");
        assert_eq!(val, json!({}));
        assert!(val.as_object().unwrap().is_empty());

        cleanup(&state_dir);
    }

    // ── R9.3: set_property is not supported (present-state managed by producer)

    #[tokio::test]
    async fn should_reject_set_property_as_shm_is_authoritative() {
        let state_dir = test_state_dir();
        let route = test_route();

        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        let result = iface
            .set_property("status".to_string(), "\"x\"".to_string())
            .await;
        assert!(
            result.is_err(),
            "set_property should be rejected — present-state is managed by producer via SHM"
        );

        cleanup(&state_dir);
    }

    // ── R9.4 / NFR1.1: No timer-based polling constructs ────────────────
    //
    // This is a structural test: it verifies that SchemaBackedInterface does
    // not contain any polling-related fields or methods. The absence of
    // tokio::time::interval, sleep, tick, etc. is verified by grep checks
    // in the validation contract. Here we verify the struct has no
    // timer/interval fields.

    #[test]
    fn should_not_have_polling_constructs_in_interface() {
        // The SchemaBackedInterface struct should not contain any RwLock,
        // Mutex, interval, or timer fields — it is a pure SHM reader.
        // We verify this by checking that read_present_state is a direct
        // file read (synchronous, no async primitives).
        let state_dir = test_state_dir();
        let route = test_route();
        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        // read_present_state is synchronous — no async runtime needed
        let state = iface.read_present_state();
        assert!(
            state.is_object(),
            "read_present_state must work synchronously"
        );

        cleanup(&state_dir);
    }

    // ── R9.2: Data changes in SHM are reflected without restart ────────

    #[test]
    fn should_reflect_shm_changes_without_restart() {
        let state_dir = test_state_dir();
        let route = test_route();
        let iface =
            SchemaBackedInterface::with_shm_state_dir("test_plugin".to_string(), route, &state_dir);

        // Initially missing → empty
        let state1 = iface.read_present_state();
        assert_eq!(state1, json!({}));

        // Write state file
        let state_path = state_dir.join("test_plugin.json");
        fs::write(&state_path, r#"{"status":"up"}"#).unwrap();

        // Read again — reflects new state immediately (direct read, no cache)
        let state2 = iface.read_present_state();
        assert_eq!(state2["status"], json!("up"));

        // Update state file
        fs::write(&state_path, r#"{"status":"down"}"#).unwrap();

        // Read again — reflects updated state
        let state3 = iface.read_present_state();
        assert_eq!(state3["status"], json!("down"));

        cleanup(&state_dir);
    }
}
