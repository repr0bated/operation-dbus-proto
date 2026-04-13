//! Stateless D-Bus client for the authoritative op-dbus state tree.
//!
//! Reads and writes service definitions via `org.opdbus.StateManager`
//! instead of hoarding state in a local SQLite database.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use tracing::{debug, info};
use zbus::{Connection, Proxy};

use crate::schema::{ServiceDef, ServiceName};

const PLUGIN_ID: &str = "services";

/// Wrapper for the QueryState response.
#[derive(Debug, Deserialize)]
struct QueryStateResponse {
    plugins: HashMap<String, Value>,
}

/// The services plugin state stored in the D-Bus state tree.
#[derive(Debug, Default, Serialize, Deserialize)]
struct ServicesState {
    #[serde(default)]
    services: HashMap<String, ServiceDef>,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    /// Connect to the D-Bus state tree.
    pub async fn new() -> Result<Self> {
        let connection = if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
            Connection::session()
                .await
                .context("connect to session D-Bus for StateManager access")?
        } else {
            Connection::system()
                .await
                .context("connect to system D-Bus for StateManager access")?
        };

        info!("Store connected to D-Bus state tree");
        Ok(Self { connection })
    }

    async fn proxy(&self) -> Result<Proxy<'_>> {
        Proxy::new(
            &self.connection,
            "org.opdbus",
            "/org/opdbus/state",
            "org.opdbus.StateManager",
        )
        .await
        .context("create StateManager D-Bus proxy")
    }

    /// Fetch the full services plugin state from the state tree.
    async fn read_state(&self) -> Result<ServicesState> {
        let proxy = self.proxy().await?;
        let mut state_json: String = proxy
            .call("QueryState", &())
            .await
            .context("QueryState call failed")?;

        let response: QueryStateResponse = unsafe { simd_json::from_str(&mut state_json) }
            .context("parse QueryState response")?;

        match response.plugins.get(PLUGIN_ID) {
            Some(value) => {
                let state: ServicesState = simd_json::serde::from_owned_value(value.clone())
                    .context("parse services plugin state")?;
                debug!("read {} services from state tree", state.services.len());
                Ok(state)
            }
            None => {
                debug!("services plugin not yet present in state tree");
                Ok(ServicesState::default())
            }
        }
    }

    /// Write the full services plugin state back to the state tree.
    async fn write_state(&self, state: &ServicesState) -> Result<()> {
        let proxy = self.proxy().await?;
        let value = simd_json::serde::to_owned_value(state)
            .context("serialize services plugin state")?;
        let request = simd_json::json!({
            "plugin_id": PLUGIN_ID,
            "value": value,
        });
        let request_json =
            simd_json::to_string(&request).context("encode contract mutation")?;

        let _: String = proxy
            .call("ApplyContractMutation", &(request_json,))
            .await
            .context("ApplyContractMutation call failed")?;

        debug!("wrote services state to state tree");
        Ok(())
    }

    pub async fn get_service(&self, name: &ServiceName) -> Result<Option<ServiceDef>> {
        let state = self.read_state().await?;
        Ok(state.services.get(name.as_str()).cloned())
    }

    pub async fn save_service(&self, service: &ServiceDef) -> Result<()> {
        let mut state = self.read_state().await?;
        state
            .services
            .insert(service.name.to_string(), service.clone());
        self.write_state(&state).await
    }

    pub async fn delete_service(&self, name: &ServiceName) -> Result<()> {
        let mut state = self.read_state().await?;
        state.services.remove(name.as_str());
        self.write_state(&state).await
    }

    pub async fn list_services(&self) -> Result<Vec<ServiceDef>> {
        let state = self.read_state().await?;
        Ok(state.services.into_values().collect())
    }
}
