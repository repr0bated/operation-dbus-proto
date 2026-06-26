//! Schema-Driven Dynamic gRPC Router
//!
//! Reads the live schema catalog from `/dev/shm/live-schema.json` and
//! dynamically routes incoming gRPC calls to their corresponding D-Bus
//! objects at `/org/opdbus/v1/plugins/<plugin>`.
//!
//! This replaces the need for hand-rolled per-plugin gRPC service
//! implementations. Every plugin defined in the schema catalog is
//! automatically exposed as a gRPC-callable service through the bridge.
//!
//! ## Architecture
//!
//! ```text
//! gRPC Client ──► SchemaRouter ──► D-Bus Object
//!                     │                  │
//!                     ├─ reads schema    │
//!                     │  from SHM        │
//!                     │                  │
//!                     └─ validates args  │
//!                        against schema  │
//! ```
//!
//! The router uses the existing `DbusPassthrough` transport but automatically
//! fills in the destination, path, and interface from the schema — so callers
//! only need to specify `plugin_id` and `method_name`.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::{Connection, Proxy};

use op_plugins::canonical::{BASE_SERVICE_NAME, PLUGIN_BASE_PATH};

/// The single active-schema catalog in shared memory.
const LIVE_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

/// A schema-derived route entry for a plugin.
#[derive(Debug, Clone)]
pub struct PluginRoute {
    /// Plugin identifier (e.g. "unix_socket", "qdrant", "wireguard").
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
    /// Create a new SchemaRouter and load routes from the live schema catalog.
    pub fn new(dbus_connection: Arc<tokio::sync::OnceCell<Connection>>) -> Self {
        let router = Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
            dbus_connection,
        };
        // Perform initial load synchronously (best-effort at startup).
        let routes = Self::load_routes_from_shm();
        let rt = router.routes.clone();
        tokio::spawn(async move {
            let mut w = rt.write().await;
            *w = routes;
        });
        router
    }

    /// Reload routes from the live schema catalog.
    pub async fn reload(&self) {
        let routes = Self::load_routes_from_shm();
        let count = routes.len();
        let mut w = self.routes.write().await;
        *w = routes;
        info!(count, "SchemaRouter reloaded plugin routes from SHM");
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
    /// This is the core "schema-driven passthrough" — the caller only needs
    /// to know the plugin_id and method_name; the router resolves the D-Bus
    /// destination, path, and interface automatically.
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

        // Validate that the method exists in the schema.
        if !route.methods.is_empty() && !route.methods.contains_key(method_name) {
            return Err(SchemaRouterError::MethodNotFound {
                plugin_id: plugin_id.to_string(),
                method: method_name.to_string(),
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

        // Convert JSON to zvariant Value for D-Bus transport.
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

    /// Load all plugin routes from the live schema catalog in shared memory.
    fn load_routes_from_shm() -> HashMap<String, PluginRoute> {
        let Ok(bytes) = std::fs::read(LIVE_SCHEMA_PATH) else {
            warn!("SchemaRouter: cannot read {}", LIVE_SCHEMA_PATH);
            return HashMap::new();
        };
        let Ok(root) = serde_json::from_slice::<JsonValue>(&bytes) else {
            warn!("SchemaRouter: invalid JSON in {}", LIVE_SCHEMA_PATH);
            return HashMap::new();
        };
        let Some(catalog) = root.as_object() else {
            warn!("SchemaRouter: root is not an object in {}", LIVE_SCHEMA_PATH);
            return HashMap::new();
        };

        let mut routes = HashMap::new();

        for (plugin_id, entries) in catalog {
            let schema = match entries.as_array().and_then(|items| items.first()) {
                Some(s) => s,
                None => continue,
            };

            let route = Self::build_route(plugin_id, schema);
            debug!(plugin_id, dbus_path = %route.dbus_path, "SchemaRouter: registered route");
            routes.insert(plugin_id.clone(), route);
        }

        info!(
            count = routes.len(),
            "SchemaRouter: loaded routes from {}",
            LIVE_SCHEMA_PATH
        );
        routes
    }

    /// Build a PluginRoute from a schema entry.
    fn build_route(plugin_id: &str, schema: &JsonValue) -> PluginRoute {
        let dbus_path = format!("{}/{}", PLUGIN_BASE_PATH, plugin_id);
        let dbus_destination = BASE_SERVICE_NAME.to_string();
        let dbus_interface = op_plugins::canonical::plugin_interface(plugin_id);

        // Extract methods from schema fields that have "method" type or from
        // a dedicated "methods" key.
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
    fn extract_methods(schema: &JsonValue) -> HashMap<String, JsonValue> {
        let mut methods = HashMap::new();

        // Look for a "methods" key in the schema.
        if let Some(method_map) = schema.get("methods").and_then(|m| m.as_object()) {
            for (name, def) in method_map {
                methods.insert(name.clone(), def.clone());
            }
        }

        // Also look in "fields" for entries with type "method".
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
    fn extract_properties(schema: &JsonValue) -> HashMap<String, JsonValue> {
        let mut properties = HashMap::new();

        if let Some(fields) = schema.get("fields").and_then(|f| f.as_array()) {
            for field in fields {
                let field_type = field.get("field_type").and_then(|t| t.as_str());
                // Everything that isn't a method is a property.
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

    #[error("method '{method}' not found on plugin '{plugin_id}'")]
    MethodNotFound { plugin_id: String, method: String },

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
            SchemaRouterError::DbusUnavailable => tonic::Status::unavailable(e.to_string()),
            SchemaRouterError::ProxyBuildFailed(_) => tonic::Status::internal(e.to_string()),
            SchemaRouterError::DbusCallFailed { .. } => tonic::Status::internal(e.to_string()),
            SchemaRouterError::SerializationFailed(_) => tonic::Status::internal(e.to_string()),
        }
    }
}

// ── JSON → zvariant conversion helper ───────────────────────────────────────

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
                Ok(ZValue::from(0i64))
            }
        }
        serde_json::Value::String(s) => Ok(ZValue::from(s.clone())),
        _ => {
            // For complex types, serialize to JSON string and pass as string.
            let json_str = serde_json::to_string(value)
                .map_err(|e| SchemaRouterError::SerializationFailed(e.to_string()))?;
            Ok(ZValue::from(json_str))
        }
    }
}
