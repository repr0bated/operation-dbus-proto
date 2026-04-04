//! dinit D-Bus proxy

use tracing::info;
use zbus::{proxy, Connection};

#[proxy(
    interface = "org.dinit.Manager",
    default_service = "org.dinit",
    default_path = "/org/dinit/Manager"
)]
trait DinitManager {
    fn start_service(&self, name: &str) -> zbus::Result<()>;
    fn stop_service(&self, name: &str) -> zbus::Result<()>;
    fn get_service_status(&self, name: &str) -> zbus::Result<String>;
    fn list_services(&self) -> zbus::Result<Vec<String>>;
}

pub struct DinitProxy {
    proxy: DinitManagerProxy<'static>,
}

impl DinitProxy {
    pub async fn new() -> anyhow::Result<Self> {
        let conn = Connection::system().await?;
        let proxy = DinitManagerProxy::new(&conn).await?;

        // Test connection
        proxy.list_services().await?;
        info!("Connected to dinit-dbus");

        Ok(Self { proxy })
    }

    pub async fn start_service(&self, name: &str) -> anyhow::Result<u32> {
        self.proxy.start_service(name).await?;
        // dinit doesn't return PID directly, would need to query
        Ok(0)
    }

    pub async fn stop_service(&self, name: &str) -> anyhow::Result<()> {
        self.proxy.stop_service(name).await?;
        Ok(())
    }

    pub async fn get_status(&self, name: &str) -> anyhow::Result<String> {
        Ok(self.proxy.get_service_status(name).await?)
    }

    pub async fn list(&self) -> anyhow::Result<Vec<String>> {
        Ok(self.proxy.list_services().await?)
    }
}
