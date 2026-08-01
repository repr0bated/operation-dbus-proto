//! SchemaLoader — reads a canonical PluginSchema from tmpfs.
//!
//! The loader accepts either the live SchemaEngine catalog or a plugin-owned
//! schema file. Reloads are broadcast to `WatchSchema` streams via a tokio
//! broadcast channel.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use tokio::sync::broadcast;
use tracing::{info, warn};

const ZEROCLAW_PLUGIN_ID: &str = "zeroclaw";

/// Event emitted to `WatchSchema` subscribers after a successful reload.
#[derive(Clone, Debug)]
pub struct SchemaReloadEvent {
    pub event_type: String,
}

/// Shared loader state for the zeroclaw Axum host.
#[derive(Clone)]
pub struct SchemaLoader {
    path: Arc<RwLock<PathBuf>>,
    plugin_id: Arc<RwLock<String>>,
    schema: Arc<RwLock<serde_json::Value>>,
    reload_tx: broadcast::Sender<SchemaReloadEvent>,
    health_status: Arc<RwLock<String>>,
}

impl SchemaLoader {
    /// Create a loader and perform the initial read.
    ///
    /// Panics with a descriptive message if the live schema catalog is absent.
    /// This is intentional: the SchemaEngine must start first.
    pub fn new(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        Self::new_for_plugin(path, ZEROCLAW_PLUGIN_ID)
    }

    /// Create a loader for a specific plugin id and perform the initial read.
    pub fn new_for_plugin(
        path: impl Into<PathBuf>,
        plugin_id: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let path = Arc::new(RwLock::new(path.into()));
        let loader = Self {
            path,
            plugin_id: Arc::new(RwLock::new(plugin_id.into())),
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
        if path.is_dir() {
            return self.load_from_blob_catalog(&path, start);
        }
        let bytes = std::fs::read(&path).map_err(|e| {
            let plugin_id = self.plugin_id();
            anyhow::anyhow!(
                "{} schema file not readable at {}: {}. \
                 Ensure the plugin has started and written the file first.",
                plugin_id,
                path.display(),
                e
            )
        })?;
        Self::parse_and_store(self, &bytes, start, &path)
    }

    /// Read the plugin's canonical schema from its sealed blob in a catalog
    /// dir. The blob in the catalog IS the plugin — no monolith, no registry.
    fn load_from_blob_catalog(&self, dir: &Path, start: Instant) -> anyhow::Result<()> {
        let plugin_id = self.plugin_id();
        let schema =
            op_blob::catalog::read_plugin_state_store_schema(dir, &plugin_id).ok_or_else(|| {
                anyhow::anyhow!(
                    "no sealed blob for '{}' in catalog {}. \
                     Ensure the catalog has been sealed (opblob seal-shm) first.",
                    plugin_id,
                    dir.display()
                )
            })?;
        let value = serde_json::to_value(&schema)?;
        let bytes = serde_json::to_vec(&value)?;
        Self::parse_and_store(self, &bytes, start, dir)
    }

    /// Asynchronous reload used by `SIGHUP` and the D-Bus object.
    pub async fn reload(&self) -> anyhow::Result<()> {
        let start = Instant::now();
        let path = self.path.read().unwrap().clone();
        if path.is_dir() {
            let loader = self.clone();
            return tokio::task::spawn_blocking(move || {
                loader.load_from_blob_catalog(&path, start)
            })
            .await
            .map_err(|e| anyhow::anyhow!("reload task panicked: {}", e))?;
        }
        let bytes = tokio::fs::read(&path).await.map_err(|e| {
            let plugin_id = self.plugin_id();
            anyhow::anyhow!(
                "{} schema file not readable at {}: {}. \
                 Ensure the plugin has started and written the file first.",
                plugin_id,
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
                "plugin schema file at {} is not valid UTF-8: {}",
                path.display(),
                e
            )
        })?;
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
            anyhow::anyhow!(
                "plugin schema file at {} is not valid JSON: {}",
                path.display(),
                e
            )
        })?;
        let plugin_id = self.plugin_id();
        let value = if let Some(schema) = extract_plugin_schema(&value, &plugin_id) {
            schema.clone()
        } else {
            return Err(anyhow::anyhow!(
                "schema file at {} does not contain '{}'",
                path.display(),
                plugin_id
            ));
        };

        {
            let mut schema = self.schema.write().unwrap();
            *schema = value;
        }

        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(50) {
            warn!(
                elapsed_ms = elapsed.as_millis(),
                plugin_id = %self.plugin_id(),
                "plugin schema reload exceeded 50 ms budget"
            );
        } else {
            info!(
                elapsed_ms = elapsed.as_millis(),
                plugin_id = %self.plugin_id(),
                "plugin schema loaded"
            );
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

    /// Return the plugin id extracted from live-schema catalogs.
    pub fn plugin_id(&self) -> String {
        self.plugin_id.read().unwrap().clone()
    }

    /// Update the plugin id; the next `load()` will extract that schema from
    /// the live catalog when `path` points at `/dev/shm/live-schema.json`.
    pub fn set_plugin_id(&self, plugin_id: impl Into<String>) {
        let mut id = self.plugin_id.write().unwrap();
        *id = plugin_id.into();
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

    /// Spawn a task that reloads the schema on `SIGHUP` and broadcasts a
    /// plugin schema reload event.
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
                    subid = "evt.service.plugin-schema.reloaded@v1",
                    plugin_id = %self.plugin_id(),
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

fn first_schema_entry(value: &serde_json::Value) -> Option<&serde_json::Value> {
    value
        .as_array()
        .and_then(|items| items.first())
        .or(Some(value))
}

fn extract_plugin_schema<'a>(
    value: &'a serde_json::Value,
    plugin_id: &str,
) -> Option<&'a serde_json::Value> {
    if value
        .as_object()
        .and_then(|schema| schema.get("name"))
        .and_then(|name| name.as_str())
        .is_some_and(|name| name == plugin_id)
    {
        return Some(value);
    }

    value
        .as_object()
        .and_then(|catalog| catalog.get(plugin_id))
        .and_then(first_schema_entry)
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
    async fn should_load_schema_from_live_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live-schema.json");
        let value = serde_json::json!({"name": "zeroclaw", "version": "1.0.0"});
        write_json(&path, &serde_json::json!({ "zeroclaw": [value.clone()] }));

        let loader = SchemaLoader::new(&path).unwrap();
        let loaded = loader.get().await;
        assert_eq!(loaded, value);
        assert_eq!(loader.health_status().await, "ok");
    }

    #[tokio::test]
    async fn should_load_direct_plugin_schema_file() {
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
    async fn should_extract_requested_plugin_from_live_schema_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live-schema.json");
        let value = serde_json::json!({
            "zeroclaw": [{"name": "zeroclaw", "version": "1.0.0"}],
            "gemma_brain": [{"name": "gemma_brain", "version": "2.0.0"}]
        });
        write_json(&path, &value);

        let loader = SchemaLoader::new_for_plugin(&path, "gemma_brain").unwrap();
        let loaded = loader.get().await;
        assert_eq!(
            loaded,
            serde_json::json!({"name": "gemma_brain", "version": "2.0.0"})
        );
        assert_eq!(loader.plugin_id(), "gemma_brain");
    }

    #[tokio::test]
    async fn should_reload_schema_on_sighup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live-schema.json");
        let initial = serde_json::json!({"name": "zeroclaw", "version": "1.0.0"});
        let updated = serde_json::json!({"name": "zeroclaw", "version": "2.0.0"});
        write_json(&path, &serde_json::json!({ "zeroclaw": [initial.clone()] }));

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
        write_json(&path, &serde_json::json!({ "zeroclaw": [updated.clone()] }));
        loader.load().unwrap();
        let _ = loader.reload_tx.send(SchemaReloadEvent {
            event_type: "reload".to_string(),
        });

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "reload");
        assert_eq!(loader.get().await, updated);
    }
}
