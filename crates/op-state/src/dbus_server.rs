//! D-Bus server for system bus integration

use crate::manager::StateManager;
use crate::plugin::StatePlugin;
use crate::DesiredState;
use anyhow::Result;
use op_core::{dbus::connect_and_claim_name, types::BusType};
use op_jsonrpc::ovsdb::OvsdbClient;
use op_state_store::SchemaRegistry;
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
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
                .and_then(|result| simd_json::to_string(&result).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))),
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
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
            .and_then(|result| simd_json::to_string(&result).map_err(|e| zbus::fdo::Error::Failed(e.to_string())))
    }
}

/// D-Bus interface for an individual plugin
pub struct PluginDbusHost {
    pub plugin: Arc<dyn StatePlugin>,
    /// Shared schema catalog used for reference ONLY.
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
        // 1. Try live plugin first
        if let Some(schema) = self.plugin.schema() {
            return Ok(simd_json::to_string(&schema).unwrap_or_default());
        }

        // 2. Fallback to catalog
        let plugin_name = self.plugin.name();
        let catalog = self.schema_registry.read();
        let schema = catalog
            .get_copies(plugin_name)
            .map(|copies| copies.json_schema.clone())
            .ok_or_else(|| {
                zbus::fdo::Error::Failed(format!(
                    "Schema '{}' not found in live plugin or catalog",
                    plugin_name
                ))
            })?;

        Ok(simd_json::to_string(&schema).unwrap_or_default())
    }
}

pub async fn register_on_connection(
    connection: &Connection,
    state_manager: Arc<StateManager>,
    _ovsdb: Arc<OvsdbClient>,
) -> Result<()> {
    let state_iface = StateManagerDBus { state_manager };
    connection
        .object_server()
        .at("/org/opdbus/state", state_iface)
        .await?;
    Ok(())
}

pub async fn start_system_bus(
    state_manager: Arc<StateManager>,
    ovsdb: Arc<OvsdbClient>,
) -> Result<()> {
    let connection = connect_and_claim_name(BusType::System, "org.opdbus").await?;
    serve_connection(connection, state_manager, ovsdb).await
}

pub async fn start_session_bus(
    state_manager: Arc<StateManager>,
    ovsdb: Arc<OvsdbClient>,
) -> Result<()> {
    let connection = connect_and_claim_name(BusType::Session, "org.opdbus").await?;
    serve_connection(connection, state_manager, ovsdb).await
}

async fn serve_connection(
    connection: Connection,
    state_manager: Arc<StateManager>,
    ovsdb: Arc<OvsdbClient>,
) -> Result<()> {
    register_on_connection(&connection, state_manager, ovsdb).await?;
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
