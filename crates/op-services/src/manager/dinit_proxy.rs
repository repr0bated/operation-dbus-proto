//! dinit D-Bus proxy

use std::collections::HashMap;

use anyhow::Context;
use tracing::info;
use zbus::{proxy, Connection, Error as ZbusError};

type DinitFlags = HashMap<String, bool>;
type DinitStatusRecord = (String, String, String, String, DinitFlags, u32, i32, i32);
type DinitServiceRecord = (
    String,
    String,
    String,
    String,
    String,
    DinitFlags,
    u32,
    i32,
    i32,
);

#[proxy(
    interface = "org.chimera.dinit.Manager",
    default_service = "org.chimera.dinit",
    default_path = "/org/chimera/dinit"
)]
trait DinitManager {
    #[zbus(name = "StartService")]
    fn start_service(&self, name: &str, wait_for_load: bool) -> zbus::Result<()>;

    #[zbus(name = "StopService")]
    fn stop_service(
        &self,
        name: &str,
        pin_stopped: bool,
        gentle: bool,
        restart: bool,
    ) -> zbus::Result<()>;

    #[zbus(name = "GetServiceStatus")]
    fn get_service_status(&self, name: &str) -> zbus::Result<DinitStatusRecord>;

    #[zbus(name = "ListServices")]
    fn list_services(&self) -> zbus::Result<Vec<DinitServiceRecord>>;
}

pub struct DinitProxy {
    conn: Connection,
}

impl DinitProxy {
    pub async fn new() -> anyhow::Result<Self> {
        let conn = Connection::system().await?;
        let proxy = DinitManagerProxy::new(&conn)
            .await
            .context("failed to create dinit D-Bus proxy")?;

        // Test connection
        proxy.list_services().await?;
        info!("Connected to dinit-dbus");

        Ok(Self { conn })
    }

    async fn proxy(&self) -> anyhow::Result<DinitManagerProxy<'_>> {
        DinitManagerProxy::new(&self.conn)
            .await
            .context("failed to create dinit D-Bus proxy")
    }

    fn is_service_already(error: &ZbusError) -> bool {
        matches!(
            error,
            ZbusError::MethodError(name, _, _)
                if name.as_str() == "org.chimera.dinit.Error.ServiceAlready"
        )
    }

    pub async fn start_service(&self, name: &str) -> anyhow::Result<u32> {
        let proxy = self.proxy().await?;
        match proxy.start_service(name, false).await {
            Ok(()) => {}
            Err(error) if Self::is_service_already(&error) => {}
            Err(error) => return Err(error.into()),
        }

        let status = proxy.get_service_status(name).await?;
        let has_pid = status.4.get("has_pid").copied().unwrap_or(false);
        Ok(if has_pid { status.5 } else { 0 })
    }

    pub async fn stop_service(&self, name: &str) -> anyhow::Result<()> {
        let proxy = self.proxy().await?;
        match proxy.stop_service(name, false, false, false).await {
            Ok(()) => {}
            Err(error) if Self::is_service_already(&error) => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    pub async fn get_status(&self, name: &str) -> anyhow::Result<String> {
        let proxy = self.proxy().await?;
        let status = proxy.get_service_status(name).await?;
        Ok(status.0)
    }

    pub async fn list(&self) -> anyhow::Result<Vec<String>> {
        let proxy = self.proxy().await?;
        let services = proxy.list_services().await?;
        Ok(services.into_iter().map(|service| service.0).collect())
    }
}
