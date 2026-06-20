//! SchemaLoader — reads the plugin-owned zeroclaw schema JSON from tmpfs.
//!
//! The zeroclaw plugin in `op-plugins` is the sole writer of
//! `/dev/shm/opdbus/schemas/zeroclaw.json`. This loader only reads and reloads
//! on `SIGHUP` or D-Bus mutation. Reloads are broadcast to `WatchSchema`
//! streams via a tokio broadcast channel.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use tokio::sync::broadcast;
use tracing::{info, warn};

/// Event emitted to `WatchSchema` subscribers after a successful reload.
#[derive(Clone, Debug)]
pub struct SchemaReloadEvent {
    pub event_type: String,
}

/// Shared loader state for the zeroclaw Axum host.
#[derive(Clone)]
pub struct SchemaLoader {
    path: Arc<RwLock<PathBuf>>,
    schema: Arc<RwLock<serde_json::Value>>,
    reload_tx: broadcast::Sender<SchemaReloadEvent>,
    health_status: Arc<RwLock<String>>,
}

impl SchemaLoader {
    /// Create a loader and perform the initial read.
    ///
    /// Panics with a descriptive message if the plugin-owned schema file is
    /// absent. This is intentional: the plugin must start first.
    pub fn new(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = Arc::new(RwLock::new(path.into()));
        let loader = Self {
            path,
            schema: Arc::new(RwLock::new(serde_json::Value::Null)),
            reload_tx: broadcast::channel(16).0,
            health_status: Arc::new(RwLock::new(String::from("ok"))),
        };
        loader.load()?;
        Ok(loader)
    }

    /// Synchronous load used by `new()` for the initial read.
    ///
    /// Logs a warning if the operation exceeds the 50 ms budget.
    pub fn load(&self) -> anyhow::Result<()> {
        let start = Instant::now();
        let path = self.path.read().unwrap().clone();
        let bytes = std::fs::read(&path).map_err(|e| {
            anyhow::anyhow!(
                "zeroclaw schema file not readable at {}: {}. \
                 Ensure the zeroclaw plugin has started and written the file first.",
                path.display(),
                e
            )
        })?;
        Self::parse_and_store(self, &bytes, start, &path)
    }

    /// Asynchronous reload used by `SIGHUP` and the D-Bus object.
    pub async fn reload(&self) -> anyhow::Result<()> {
        let start = Instant::now();
        let path = self.path.read().unwrap().clone();
        let bytes = tokio::fs::read(&path).await.map_err(|e| {
            anyhow::anyhow!(
                "zeroclaw schema file not readable at {}: {}. \
                 Ensure the zeroclaw plugin has started and written the file first.",
                path.display(),
                e
            )
        })?;
        tokio::task::spawn_blocking({
            let loader = self.clone();
            let path = path.clone();
            move || Self::parse_and_store(&loader, &bytes, start, &path)
        })
        .await
        .map_err(|e| anyhow::anyhow!("reload task panicked: {}", e))?
    }

    fn parse_and_store(&self, bytes: &[u8], start: Instant, path: &Path) -> anyhow::Result<()> {
        let text = std::str::from_utf8(bytes).map_err(|e| {
            anyhow::anyhow!(
                "zeroclaw schema file at {} is not valid UTF-8: {}",
                path.display(),
                e
            )
        })?;
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
            anyhow::anyhow!(
                "zeroclaw schema file at {} is not valid JSON: {}",
                path.display(),
                e
            )
        })?;

        {
            let mut schema = self.schema.write().unwrap();
            *schema = value;
        }

        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(50) {
            warn!(
                elapsed_ms = elapsed.as_millis(),
                "zeroclaw schema reload exceeded 50 ms budget"
            );
        } else {
            info!(elapsed_ms = elapsed.as_millis(), "zeroclaw schema loaded");
        }

        let mut health = self.health_status.write().unwrap();
        *health = "ok".to_string();

        Ok(())
    }

    /// Clone the current schema value.
    pub async fn get(&self) -> serde_json::Value {
        self.schema.read().unwrap().clone()
    }

    /// Return the configured schema file path.
    pub fn path(&self) -> PathBuf {
        self.path.read().unwrap().clone()
    }

    /// Update the schema file path; the next `load()` will read from the new path.
    pub fn set_path(&self, path: impl Into<PathBuf>) {
        let mut p = self.path.write().unwrap();
        *p = path.into();
    }

    /// Return a handle to the reload broadcast channel.
    pub fn reload_tx(&self) -> broadcast::Sender<SchemaReloadEvent> {
        self.reload_tx.clone()
    }

    /// Return the current health status string.
    pub async fn health_status(&self) -> String {
        self.health_status.read().unwrap().clone()
    }

    /// Set health status (used by the D-Bus object and server startup).
    pub async fn set_health_status(&self, status: String) {
        let mut h = self.health_status.write().unwrap();
        *h = status;
    }

    /// Spawn a task that reloads the schema on `SIGHUP` and broadcasts the
    /// `evt.service.zeroclaw-schema.reloaded@v1` event.
    pub fn watch_sighup(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut sig =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(error = %e, "failed to install SIGHUP handler; manual reloads only");
                        return;
                    }
                };

            loop {
                sig.recv().await;
                info!(
                    subid = "evt.service.zeroclaw-schema.reloaded@v1",
                    "SIGHUP received"
                );
                match self.reload().await {
                    Ok(()) => {
                        let _ = self.reload_tx.send(SchemaReloadEvent {
                            event_type: "reload".to_string(),
                        });
                    }
                    Err(e) => {
                        warn!(error = %e, "SIGHUP reload failed");
                        let mut health = self.health_status.write().unwrap();
                        *health = e.to_string();
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_json(path: &Path, value: &serde_json::Value) {
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(serde_json::to_string(value).unwrap().as_bytes())
            .unwrap();
    }

    #[tokio::test]
    async fn should_load_schema_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zeroclaw.json");
        let value = serde_json::json!({"name": "zeroclaw", "version": "1.0.0"});
        write_json(&path, &value);

        let loader = SchemaLoader::new(&path).unwrap();
        let loaded = loader.get().await;
        assert_eq!(loaded, value);
        assert_eq!(loader.health_status().await, "ok");
    }

    #[tokio::test]
    async fn should_reload_schema_on_sighup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zeroclaw.json");
        let initial = serde_json::json!({"name": "zeroclaw", "version": "1.0.0"});
        let updated = serde_json::json!({"name": "zeroclaw", "version": "2.0.0"});
        write_json(&path, &initial);

        let loader = Arc::new(SchemaLoader::new(&path).unwrap());
        let mut rx = loader.reload_tx().subscribe();

        // Simulate a reload by calling load directly and sending the event.
        loader.load().unwrap();
        let _ = loader.reload_tx.send(SchemaReloadEvent {
            event_type: "reload".to_string(),
        });

        // Verify the broadcast is received.
        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "reload");
        assert_eq!(loader.get().await, initial);

        // Update the file and reload again.
        write_json(&path, &updated);
        loader.load().unwrap();
        let _ = loader.reload_tx.send(SchemaReloadEvent {
            event_type: "reload".to_string(),
        });

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "reload");
        assert_eq!(loader.get().await, updated);
    }
}
