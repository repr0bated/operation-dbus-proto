//! Fixed D-Bus object at /opdbus/v1/plugins
//!
//! Exposes all registered plugins (active or inactive) through methods.
//! Sits alongside the org.freedesktop.DBus.ObjectManager interface on the
//! same path so clients can use either GetManagedObjects or these helpers.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zbus::interface;

/// Snapshot of all plugins: name → JSON state (includes "active" bool).
pub type PluginSnapshot = Arc<RwLock<HashMap<String, String>>>;

pub struct PluginInterface {
    plugins: PluginSnapshot,
}

impl Default for PluginInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginInterface {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn snapshot_handle(&self) -> PluginSnapshot {
        self.plugins.clone()
    }
}

#[interface(name = "org.opdbus.PluginsV1")]
impl PluginInterface {
    /// Names of all registered plugins (active and inactive).
    async fn list(&self) -> Vec<String> {
        self.plugins.read().await.keys().cloned().collect()
    }

    /// Full state JSON for a single plugin. Returns "{}" if unknown.
    async fn get(&self, name: String) -> String {
        self.plugins
            .read()
            .await
            .get(&name)
            .cloned()
            .unwrap_or_else(|| "{}".to_string())
    }

    /// All plugins and their state as a map of name → JSON.
    async fn get_all(&self) -> HashMap<String, String> {
        self.plugins.read().await.clone()
    }
}
