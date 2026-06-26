//! Schema-Driven D-Bus Passthrough
//!
//! Replaces the hand-rolled `DbusPassthrough` service with a schema-aware
//! version that automatically resolves D-Bus destinations from plugin IDs.
//!
//! ## Key Differences from the Original DbusPassthrough
//!
//! | Feature                | Old (Hand-Rolled)           | New (Schema-Driven)              |
//! |------------------------|-----------------------------|----------------------------------|
//! | Destination resolution | Caller must specify         | Auto-resolved from schema        |
//! | Path resolution        | Caller must specify         | Auto-derived from plugin_id      |
//! | Interface resolution   | Caller must specify         | Auto-derived from canonical.rs   |
//! | Method validation      | None                        | Validated against schema fields  |
//! | Identity injection     | None                        | Peer credentials → session_id    |
//! | Shared socket support  | No                          | Yes (container.sock)             |
//!
//! ## Usage
//!
//! gRPC clients call `SchemaCall` with just `plugin_id` and `method_name`:
//! ```protobuf
//! message SchemaCallRequest {
//!   string plugin_id = 1;    // e.g. "qdrant", "unix_socket"
//!   string method_name = 2;  // e.g. "createunixsocket"
//!   string json_args = 3;    // JSON-encoded arguments
//! }
//! ```
//!
//! The bridge resolves the rest from the live schema catalog.

use std::sync::Arc;

use crate::schema_router::{SchemaRouter, SchemaRouterError};
use tonic::{Request, Response, Status};
use tracing::{debug, info};

/// Schema-driven passthrough service implementation.
///
/// This wraps the `SchemaRouter` and exposes it as gRPC methods that
/// can be called by the existing `DbusPassthrough` proto service, but
/// with automatic schema resolution.
pub struct SchemaPassthrough {
    router: Arc<SchemaRouter>,
}

impl SchemaPassthrough {
    pub fn new(router: Arc<SchemaRouter>) -> Self {
        Self { router }
    }

    /// Schema-driven method call: resolves destination from plugin_id.
    ///
    /// This is the replacement for `DbusPassthrough::Call` when the caller
    /// knows the plugin_id but not the D-Bus destination/path/interface.
    pub async fn schema_call(
        &self,
        plugin_id: &str,
        method_name: &str,
        json_args: &str,
    ) -> Result<String, Status> {
        debug!(
            plugin_id,
            method_name, "SchemaPassthrough: routing call via schema"
        );

        self.router
            .call_method(plugin_id, method_name, json_args)
            .await
            .map_err(Status::from)
    }

    /// Schema-driven property get: resolves destination from plugin_id.
    pub async fn schema_get_property(
        &self,
        plugin_id: &str,
        property_name: &str,
    ) -> Result<String, Status> {
        debug!(
            plugin_id,
            property_name, "SchemaPassthrough: getting property via schema"
        );

        self.router
            .get_property(plugin_id, property_name)
            .await
            .map_err(Status::from)
    }

    /// Schema-driven property set: resolves destination from plugin_id.
    pub async fn schema_set_property(
        &self,
        plugin_id: &str,
        property_name: &str,
        json_value: &str,
    ) -> Result<(), Status> {
        debug!(
            plugin_id,
            property_name, "SchemaPassthrough: setting property via schema"
        );

        self.router
            .set_property(plugin_id, property_name, json_value)
            .await
            .map_err(Status::from)
    }

    /// List all plugins that are routable through the schema.
    pub async fn list_routable_plugins(&self) -> Vec<String> {
        self.router.list_plugin_ids().await
    }

    /// Reload the schema router (e.g. after a schema file update).
    pub async fn reload_schema(&self) {
        self.router.reload().await;
        info!("SchemaPassthrough: schema routes reloaded");
    }
}

/// Integration point: enhances the existing `DbusPassthrough` implementation
/// to support schema-driven routing alongside explicit routing.
///
/// When a `DbusCallRequest` arrives with an empty `destination` field but a
/// non-empty `path` that matches `/org/opdbus/v1/plugins/<plugin_id>`, the
/// schema passthrough kicks in and auto-resolves the destination and interface.
pub fn resolve_from_schema_or_explicit(
    destination: &str,
    path: &str,
    interface: &str,
) -> ResolvedRoute {
    // If destination is already specified, use explicit routing (backward compat).
    if !destination.is_empty() {
        return ResolvedRoute::Explicit {
            destination: destination.to_string(),
            path: path.to_string(),
            interface: interface.to_string(),
        };
    }

    // Try to extract plugin_id from the path.
    let plugin_base = op_plugins::canonical::PLUGIN_BASE_PATH;
    if path.starts_with(plugin_base) {
        let rest = &path[plugin_base.len()..];
        let rest = rest.trim_start_matches('/');
        let plugin_id = rest.split('/').next().unwrap_or("");
        if !plugin_id.is_empty() {
            return ResolvedRoute::SchemaResolved {
                plugin_id: plugin_id.to_string(),
                sub_path: rest[plugin_id.len()..].to_string(),
            };
        }
    }

    // Fallback: treat as explicit with whatever was provided.
    ResolvedRoute::Explicit {
        destination: destination.to_string(),
        path: path.to_string(),
        interface: interface.to_string(),
    }
}

/// The result of route resolution.
#[derive(Debug, Clone)]
pub enum ResolvedRoute {
    /// Caller provided explicit D-Bus coordinates.
    Explicit {
        destination: String,
        path: String,
        interface: String,
    },
    /// Route was resolved from the schema catalog via plugin_id.
    SchemaResolved {
        plugin_id: String,
        sub_path: String,
    },
}
