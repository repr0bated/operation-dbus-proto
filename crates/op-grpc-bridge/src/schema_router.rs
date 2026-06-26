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

    /// Reload routes from both schema sources.
    ///
    /// Called reactively when a schema mutation arrives (not on a timer).
    pub async fn reload(&self) {
        let routes = Self::load_all_routes();
        let count = routes.len();
        let mut w = self.routes.write().await;
        *w = routes;
        info!(count, "SchemaRouter reloaded plugin routes");
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
    ///
    /// Validates the method exists in the schema before making the D-Bus call.
    /// Unknown methods are rejected with `MethodNotFound`.
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

        // Schema validation: reject methods not declared in the schema.
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
    ///
    /// Validates the property exists in the schema before reading.
    pub async fn get_property(
        &self,
        plugin_id: &str,
        property_name: &str,
    ) -> Result<String, SchemaRouterError> {
        let route = self
            .get_route(plugin_id)
            .await
            .ok_or_else(|| SchemaRouterError::PluginNotFound(plugin_id.to_string()))?;

        // Schema validation: reject properties not declared in the schema.
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

        let val: zbus::zvariant::OwnedValue = props
            .get(iface, property_name)
            .await
            .map_err(|e| SchemaRouterError::DbusCallFailed {
                method: format!("Get({})", property_name),
                error: e.to_string(),
            })?;

        serde_json::to_string(&val)
            .map_err(|e| SchemaRouterError::SerializationFailed(e.to_string()))
    }

    /// Set a property on a plugin via its schema-derived D-Bus route.
    ///
    /// Validates the property exists in the schema before writing.
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

        // Schema validation: reject properties not declared in the schema.
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

        props
            .set(iface, property_name, zval.into())
            .await
            .map_err(|e| SchemaRouterError::DbusCallFailed {
                method: format!("Set({})", property_name),
                error: e.to_string(),
            })?;

        Ok(())
    }

    // ── Private helpers ─────────────────────────────────────────────────────

    /// Load all plugin routes from both schema access paths.
    ///
    /// Priority: combined monolith first (authoritative), then per-plugin files
    /// for any plugins not already in the monolith.
    fn load_all_routes() -> HashMap<String, PluginRoute> {
        let mut routes = HashMap::new();

        // Path 1: Combined monolith from /dev/shm/live-schema.json
        if let Some(monolith_routes) = Self::load_from_monolith() {
            for (id, route) in monolith_routes {
                routes.insert(id, route);
            }
        }

        // Path 2: Per-plugin schema files (fills gaps not covered by monolith)
        if let Some(per_plugin_routes) = Self::load_from_per_plugin_dir() {
            for (id, route) in per_plugin_routes {
                routes.entry(id).or_insert(route);
            }
        }

        routes
    }

    /// Load routes from the combined monolith at /dev/shm/live-schema.json.
    fn load_from_monolith() -> Option<HashMap<String, PluginRoute>> {
        let bytes = std::fs::read(LIVE_SCHEMA_PATH).ok()?;
        let root: JsonValue = serde_json::from_slice(&bytes).ok()?;
        let catalog = root.as_object()?;

        let mut routes = HashMap::new();
        for (plugin_id, schema_value) in catalog {
            // The monolith may store schemas as arrays (versioned) or objects.
            let schema = match schema_value {
                JsonValue::Array(arr) => arr.first()?,
                JsonValue::Object(_) => schema_value,
                _ => continue,
            };
            let route = Self::build_route(plugin_id, schema);
            debug!(plugin_id, dbus_path = %route.dbus_path, "route from monolith");
            routes.insert(plugin_id.clone(), route);
        }

        info!(
            count = routes.len(),
            "SchemaRouter: loaded from {}",
            LIVE_SCHEMA_PATH
        );
        Some(routes)
    }

    /// Load routes from per-plugin schema files in the schema directory.
    fn load_from_per_plugin_dir() -> Option<HashMap<String, PluginRoute>> {
        let dir = Path::new(PER_PLUGIN_SCHEMA_DIR);
        if !dir.is_dir() {
            debug!(
                path = %dir.display(),
                "Per-plugin schema directory not found, skipping"
            );
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
                    debug!(plugin_id, path = %path.display(), "route from per-plugin file");
                    routes.insert(plugin_id, route);
                }
            }
        }

        if !routes.is_empty() {
            info!(
                count = routes.len(),
                "SchemaRouter: loaded from {}",
                PER_PLUGIN_SCHEMA_DIR
            );
        }

        Some(routes)
    }

    /// Build a PluginRoute from a schema entry.
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

    /// Extract method definitions from a schema.
    ///
    /// Looks for:
    /// - A top-level "methods" object (key → definition)
    /// - Fields with `field_type: "method"` or `"Method"`
    /// - Properties with `"capabilities"` containing `can_write: true` (implicit mutators)
    fn extract_methods(schema: &JsonValue) -> HashMap<String, JsonValue> {
        let mut methods = HashMap::new();

        // Explicit "methods" key
        if let Some(method_map) = schema.get("methods").and_then(|m| m.as_object()) {
            for (name, def) in method_map {
                methods.insert(name.clone(), def.clone());
            }
        }

        // Fields with type "method"
        if let Some(fields) = schema.get("fields").and_then(|f| f.as_array()) {
            for field in fields {
                let field_type = field.get("field_type").and_then(|t| t.as_str());
                if field_type == Some("method") || field_type == Some("Method") {
                    if let Some(name) = field.get("name").and_then(|n| n.as_str()) {
                        methods.insert(name.to_string(), field.clone());
                    }
                }
            }
        }

        methods
    }

    /// Extract property definitions from a schema.
    ///
    /// Everything in "properties" or "fields" that is not a method is a property.
    fn extract_properties(schema: &JsonValue) -> HashMap<String, JsonValue> {
        let mut properties = HashMap::new();

        // Top-level "properties" object (JSON Schema style)
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (name, def) in props {
                // Skip metadata fields that aren't D-Bus properties
                if name == "name" || name == "version" || name == "plugin_type" {
                    continue;
                }
                properties.insert(name.clone(), def.clone());
            }
        }

        // "fields" array (PluginSchema style)
        if let Some(fields) = schema.get("fields").and_then(|f| f.as_array()) {
            for field in fields {
                let field_type = field.get("field_type").and_then(|t| t.as_str());
                if field_type != Some("method") && field_type != Some("Method") {
                    if let Some(name) = field.get("name").and_then(|n| n.as_str()) {
                        properties.insert(name.to_string(), field.clone());
                    }
                }
            }
        }

        properties
    }
}

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum SchemaRouterError {
    #[error("plugin not found in schema catalog: {0}")]
    PluginNotFound(String),

    #[error("method '{method}' not found on plugin '{plugin_id}' (available: {available:?})")]
    MethodNotFound {
        plugin_id: String,
        method: String,
        available: Vec<String>,
    },

    #[error("property '{property}' not found on plugin '{plugin_id}' (available: {available:?})")]
    PropertyNotFound {
        plugin_id: String,
        property: String,
        available: Vec<String>,
    },

    #[error("D-Bus system bus unavailable")]
    DbusUnavailable,

    #[error("failed to build D-Bus proxy: {0}")]
    ProxyBuildFailed(String),

    #[error("D-Bus call '{method}' failed: {error}")]
    DbusCallFailed { method: String, error: String },

    #[error("serialization failed: {0}")]
    SerializationFailed(String),
}

impl From<SchemaRouterError> for tonic::Status {
    fn from(e: SchemaRouterError) -> Self {
        match &e {
            SchemaRouterError::PluginNotFound(_) => tonic::Status::not_found(e.to_string()),
            SchemaRouterError::MethodNotFound { .. } => {
                tonic::Status::unimplemented(e.to_string())
            }
            SchemaRouterError::PropertyNotFound { .. } => {
                tonic::Status::not_found(e.to_string())
            }
            SchemaRouterError::DbusUnavailable => tonic::Status::unavailable(e.to_string()),
            SchemaRouterError::ProxyBuildFailed(_) => tonic::Status::internal(e.to_string()),
            SchemaRouterError::DbusCallFailed { .. } => tonic::Status::internal(e.to_string()),
            SchemaRouterError::SerializationFailed(_) => tonic::Status::internal(e.to_string()),
        }
    }
}

// ── JSON → zvariant conversion ─────────────────────────────────────────────

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
            // Convert to array of strings (most common D-Bus array type for JSON).
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
            // Serialize complex objects as JSON string for D-Bus transport.
            let s = serde_json::to_string(value)
                .map_err(|e| SchemaRouterError::SerializationFailed(e.to_string()))?;
            Ok(ZValue::from(s))
        }
    }
}
