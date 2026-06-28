//! D-Bus server for system bus integration
//!
//! NOTE: op-state has no s6 service. The bridge (op-grpc-bridge) owns
//! org.opdbus.v1 and all /org/opdbus/v1/plugins/* paths. This module
//! provides register_on_connection for library use only.

use crate::manager::StateManager;
use crate::plugin::StatePlugin;
use crate::DesiredState;
use anyhow::Result;
use op_state_store::{SchemaCatalog, SchemaRegistry};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use zbus::Connection;

/// D-Bus interface for the state manager
pub struct StateManagerDBus {
    state_manager: Arc<StateManager>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct ProjectedObject {
    origin_service: String,
    origin_path: String,
}

#[derive(Default)]
#[allow(dead_code)]
struct PublicationRegistry {
    published_paths: std::collections::HashSet<String>,
    paths_by_service: HashMap<String, std::collections::HashSet<String>>,
}

#[allow(dead_code)]
impl PublicationRegistry {
    fn insert(&mut self, service: &str, path: String) -> bool {
        if !self.published_paths.insert(path.clone()) {
            return false;
        }
        self.paths_by_service
            .entry(service.to_string())
            .or_default()
            .insert(path);
        true
    }
    fn remove_path(&mut self, service: &str, path: &str) {
        self.published_paths.remove(path);
        if let Some(paths) = self.paths_by_service.get_mut(service) {
            paths.remove(path);
            if paths.is_empty() {
                self.paths_by_service.remove(service);
            }
        }
    }
    fn remove_service(&mut self, service: &str) -> Vec<String> {
        let paths = self.paths_by_service.remove(service).unwrap_or_default();
        for path in &paths {
            self.published_paths.remove(path);
        }
        paths.into_iter().collect()
    }
    fn total_paths(&self) -> usize {
        self.published_paths.len()
    }
}

#[zbus::interface(name = "org.opdbus.ProjectedObjectV1")]
#[allow(dead_code)]
impl ProjectedObject {
    #[zbus(property)]
    async fn origin_service(&self) -> String {
        self.origin_service.clone()
    }
    #[zbus(property)]
    async fn origin_path(&self) -> String {
        self.origin_path.clone()
    }
}

#[zbus::interface(name = "org.opdbus.StateManager")]
impl StateManagerDBus {
    async fn apply_openflow_state(&self, state_json: String) -> zbus::fdo::Result<String> {
        let mut state_json_mut = state_json;
        match unsafe { simd_json::from_str::<DesiredState>(&mut state_json_mut) } {
            Ok(desired_state) => self
                .state_manager
                .apply_plugin_state("openflow", desired_state.state)
                .await
                .and_then(|result| simd_json::to_string(&result).map_err(Into::into))
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string())),
            Err(e) => Err(zbus::fdo::Error::InvalidArgs(format!(
                "Invalid JSON: {}",
                e
            ))),
        }
    }

    async fn query_state(&self) -> zbus::fdo::Result<String> {
        match self.state_manager.query_current_state().await {
            Ok(state) => match simd_json::to_string(&QueryStateResponse { plugins: state }) {
                Ok(json) => Ok(json),
                Err(e) => Err(zbus::fdo::Error::Failed(format!(
                    "Serialization failed: {}",
                    e
                ))),
            },
            Err(e) => Err(zbus::fdo::Error::Failed(format!("Query failed: {}", e))),
        }
    }

    async fn apply_contract_mutation(&self, request_json: String) -> zbus::fdo::Result<String> {
        let mut request_json_mut = request_json;
        let request: ContractMutationRequest =
            unsafe { simd_json::from_str(&mut request_json_mut) }.map_err(|e| {
                zbus::fdo::Error::InvalidArgs(format!("Invalid contract mutation payload: {}", e))
            })?;

        self.state_manager
            .apply_plugin_state(&request.plugin_id, request.value)
            .await
            .and_then(|result| simd_json::to_string(&result).map_err(Into::into))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
}

/// D-Bus interface for an individual plugin
pub struct PluginDbusHost {
    pub plugin: Arc<dyn StatePlugin>,
    /// Compatibility name kept on the host shape for older call sites. This is
    /// the shared schema catalog used to resolve the canonical plugin document.
    pub schema_registry: Arc<RwLock<SchemaRegistry>>,
}

#[zbus::interface(name = "org.opdbus.PluginV1")]
impl PluginDbusHost {
    #[zbus(property)]
    async fn name(&self) -> String {
        self.plugin.name().to_string()
    }

    #[zbus(property)]
    async fn version(&self) -> String {
        self.plugin.version().to_string()
    }

    #[zbus(property)]
    async fn description(&self) -> String {
        self.plugin.metadata().description
    }

    async fn get_state(&self) -> zbus::fdo::Result<String> {
        let state = self
            .plugin
            .query_current_state()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(simd_json::to_string(&state).unwrap_or_default())
    }

    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        let plugin_name = self.plugin.name();
        let catalog = self.schema_registry.read();
        let schema = catalog
            .get_copies(plugin_name)
            .map(|copies| copies.json_schema.clone())
            .ok_or_else(|| {
                zbus::fdo::Error::Failed(format!(
                    "Schema '{}' not found in shared catalog",
                    plugin_name
                ))
            })?;

        Ok(simd_json::to_string(&schema).unwrap_or_default())
    }
}

/// Preferred architectural name for `PluginDbusHost` schema lookup state.
pub type SharedSchemaCatalog = Arc<RwLock<SchemaCatalog>>;

/// Compatibility alias for older call sites that still say `registry`.
pub type SharedSchemaRegistry = SharedSchemaCatalog;

/// Register the state manager interface on an existing connection.
/// Used by other components; does NOT claim org.opdbus.v1.
pub async fn register_on_connection(
    connection: &Connection,
    state_manager: Arc<StateManager>,
) -> Result<()> {
    let state_iface = StateManagerDBus { state_manager };
    connection
        .object_server()
        .at("/org/opdbus/v1/state", state_iface)
        .await?;
    Ok(())
}

/// NOTE: These functions are kept for API compatibility but are deprecated.
/// op-state has no s6 service; op-grpc-bridge owns org.opdbus.v1.

pub async fn start_system_bus(_state_manager: Arc<StateManager>) -> Result<()> {
    // Dead code: no s6 service. op-grpc-bridge owns org.opdbus.v1.
    std::future::pending::<()>().await;
    Ok(())
}

pub async fn start_session_bus(_state_manager: Arc<StateManager>) -> Result<()> {
    // Dead code: no s6 service. op-grpc-bridge owns org.opdbus.v1.
    std::future::pending::<()>().await;
    Ok(())
}

#[derive(Debug, Serialize)]
struct QueryStateResponse {
    plugins: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ContractMutationRequest {
    plugin_id: String,
    value: Value,
}
