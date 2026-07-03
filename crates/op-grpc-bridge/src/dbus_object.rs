//! D-Bus object for the zeroclaw Axum host.
//!
//! Exposes `/org/opdbus/v1/services/zeroclaw_axum_host` with the runtime
//! configuration properties required by the spec. Writes to `BindAddr` or
//! `SchemaPath` trigger a reload signal; the server task picks it up and
//! reconfigures without restarting the process.

use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::schema_loader::SchemaLoader;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8090";

/// D-Bus-facing configuration object for the zeroclaw Axum host.
#[derive(Clone)]
pub struct ZeroclawAxumHostObject {
    bind_addr: Arc<RwLock<String>>,
    schema_path: Arc<RwLock<String>>,
    reload_interval_secs: Arc<RwLock<u32>>,
    loader: Arc<SchemaLoader>,
    rebind_tx: mpsc::Sender<String>,
}

impl ZeroclawAxumHostObject {
    pub fn new(loader: Arc<SchemaLoader>, rebind_tx: mpsc::Sender<String>) -> anyhow::Result<Self> {
        let initial_path = loader.path().to_string_lossy().to_string();
        Ok(Self {
            bind_addr: Arc::new(RwLock::new(DEFAULT_BIND_ADDR.to_string())),
            schema_path: Arc::new(RwLock::new(initial_path)),
            reload_interval_secs: Arc::new(RwLock::new(0)),
            loader,
            rebind_tx,
        })
    }

    /// Read the current TCP bind address.
    async fn bind_addr_value(&self) -> String {
        self.bind_addr.read().await.clone()
    }

    /// Read the current plugin-owned schema file path.
    async fn schema_path_value(&self) -> String {
        self.schema_path.read().await.clone()
    }

    /// Read the current periodic reload interval in seconds.
    async fn reload_interval_value(&self) -> u32 {
        *self.reload_interval_secs.read().await
    }
}

#[zbus::interface(name = "org.opdbus.v1.ZeroclawAxumHost")]
impl ZeroclawAxumHostObject {
    #[zbus(property)]
    async fn bind_addr(&self) -> String {
        self.bind_addr_value().await
    }

    #[zbus(property)]
    async fn set_bind_addr(&self, value: String) {
        let mut addr = self.bind_addr.write().await;
        *addr = value.clone();
        drop(addr);
        info!(subid = "mut.service.zeroclaw-axum-host.reload@v1", new_bind = %value, "BindAddr changed; signaling rebind");
        if let Err(e) = self.rebind_tx.send(value).await {
            warn!(error = %e, "failed to send rebind request");
        }
    }

    #[zbus(property)]
    async fn schema_path(&self) -> String {
        self.schema_path_value().await
    }

    #[zbus(property)]
    async fn set_schema_path(&self, value: String) {
        let mut path = self.schema_path.write().await;
        *path = value.clone();
        drop(path);
        self.loader.set_path(value.clone());
        info!(subid = "mut.service.zeroclaw-axum-host.reload@v1", new_path = %value, "SchemaPath changed; reloading");
        if let Err(e) = self.loader.reload().await {
            warn!(error = %e, "schema path reload failed");
            self.loader.set_health_status(e.to_string()).await;
        }
    }

    #[zbus(property)]
    async fn reload_interval_secs(&self) -> u32 {
        self.reload_interval_value().await
    }

    #[zbus(property)]
    async fn set_reload_interval_secs(&self, value: u32) {
        let mut interval = self.reload_interval_secs.write().await;
        *interval = value;
    }

    #[zbus(property)]
    async fn health_status(&self) -> String {
        self.loader.health_status().await
    }
}

/// Register the zeroclaw Axum host object on the system bus.
pub async fn register_zeroclaw_host_object(
    object: ZeroclawAxumHostObject,
) -> anyhow::Result<zbus::Connection> {
    let connection = zbus::Connection::system().await?;
    connection
        .object_server()
        .at("/org/opdbus/v1/services/zeroclaw_axum_host", object)
        .await?;
    info!("registered /org/opdbus/v1/services/zeroclaw_axum_host");
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_expose_dbus_properties() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live-schema.json");
        std::fs::write(&path, r#"{"zeroclaw":[{"name":"zeroclaw"}]}"#).unwrap();

        let loader = Arc::new(SchemaLoader::new(&path).unwrap());
        let (rebind_tx, _rebind_rx) = mpsc::channel(1);
        let obj = ZeroclawAxumHostObject::new(loader, rebind_tx).unwrap();

        assert_eq!(obj.bind_addr_value().await, "0.0.0.0:8090");
        assert_eq!(obj.schema_path_value().await, path.to_string_lossy());
        assert_eq!(obj.reload_interval_value().await, 0);
        assert_eq!(obj.health_status().await, "ok");
    }
}
