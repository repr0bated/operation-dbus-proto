//! D-Bus interface for org.opdbus.services

use std::sync::Arc;
use tracing::info;
use zbus::{interface, Connection, SignalContext};

use crate::manager::ServiceManager;
use crate::schema::ServiceName;

pub struct DbusInterface {
    manager: Arc<ServiceManager>,
}

impl DbusInterface {
    pub fn new(manager: Arc<ServiceManager>) -> Self {
        Self { manager }
    }
}

#[interface(name = "org.opdbus.services.v1.Manager")]
impl DbusInterface {
    async fn start(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .start(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn stop(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .stop(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn restart(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .restart(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn get_status(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .get_status(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn list_services(&self) -> zbus::fdo::Result<Vec<String>> {
        let services = self
            .manager
            .list()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(services.into_iter().map(|s| s.name.to_string()).collect())
    }

    #[zbus(signal)]
    async fn service_state_changed(
        ctx: &SignalContext<'_>,
        name: &str,
        old_state: &str,
        new_state: &str,
    ) -> zbus::Result<()>;
}

pub async fn run_dbus_server(manager: Arc<ServiceManager>) -> anyhow::Result<()> {
    let conn = Connection::system().await?;

    let iface = DbusInterface::new(manager);
    conn.object_server()
        .at("/org/opdbus/services", iface)
        .await?;
    conn.request_name("org.opdbus.services").await?;

    info!("D-Bus interface started on org.opdbus.services");

    // Keep running
    std::future::pending::<()>().await;
    Ok(())
}
