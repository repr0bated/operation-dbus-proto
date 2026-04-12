//! State manager for coordinating plugins and schemas

use crate::plugin::{ApplyResult, StateDiff, StatePlugin};
use anyhow::{anyhow, Result};
use op_state_store::StateStore;
use tokio::sync::RwLock;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;

/// State manager for coordinating plugins and their lifecycle
pub struct StateManager {
    plugins: Arc<RwLock<HashMap<String, Arc<dyn StatePlugin>>>>,
    #[allow(dead_code)]
    store: Option<Arc<dyn StateStore>>,
}

impl StateManager {
    /// Create a new state manager
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            store: None,
        }
    }

    /// Create a new state manager with a persistent store
    pub fn with_store(store: Arc<dyn StateStore>) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            store: Some(store),
        }
    }

    /// Register a plugin
    pub async fn register_plugin(&self, name: String, plugin: Arc<dyn StatePlugin>) {
        let mut plugins = self.plugins.write().await;
        plugins.insert(name, plugin);
    }

    /// Get a plugin by name
    pub async fn get_plugin(&self, name: &str) -> Option<Arc<dyn StatePlugin>> {
        let plugins = self.plugins.read().await;
        plugins.get(name).cloned()
    }

    /// List all registered plugins
    pub async fn list_plugins(&self) -> Vec<String> {
        let plugins = self.plugins.read().await;
        plugins.keys().cloned().collect()
    }

    /// Query the current state of all plugins (best-effort).
    ///
    /// Individual plugin failures are logged and skipped so that one
    /// slow/unavailable backend (e.g. OVSDB) does not prevent the rest
    /// of the system from seeding.
    pub async fn query_current_state(&self) -> Result<HashMap<String, Value>> {
        let plugins = self.plugins.read().await;
        let mut states = HashMap::new();

        for (name, plugin) in plugins.iter() {
            match plugin.query_current_state().await {
                Ok(state) => {
                    states.insert(name.clone(), state);
                }
                Err(e) => {
                    tracing::warn!(plugin = %name, "Skipping plugin state query: {}", e);
                }
            }
        }

        Ok(states)
    }

    /// Get the current state of a single plugin
    pub async fn query_plugin_state(&self, plugin_name: &str) -> Result<Value> {
        let plugin = self
            .get_plugin(plugin_name)
            .await
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_name))?;
        plugin.query_current_state().await
    }

    /// Validate a desired plugin state.
    ///
    /// Runtime Definition Policy:
    /// 1. The live plugin code is the primary authority for its own schema.
    /// 2. This manager does not use an external catalog for live validation.
    pub async fn validate_plugin_state(&self, plugin_name: &str, desired: &Value) -> Result<()> {
        let plugin = self
            .get_plugin(plugin_name)
            .await
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_name))?;

        // Validate using the live plugin's internal schema
        if let Some(schema) = plugin.schema() {
            let validation = schema.validate(desired);
            if validation.valid {
                return Ok(());
            }
            return Err(anyhow!(
                "State rejected by live plugin schema '{}': {}",
                plugin_name,
                validation.errors.join("; ")
            ));
        }

        // If no schema is provided by the plugin, it's considered schema-less at runtime
        Ok(())
    }

    /// Apply a state change to a plugin
    pub async fn apply_diff(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let plugin = self
            .get_plugin(&diff.plugin)
            .await
            .ok_or_else(|| anyhow!("Plugin '{}' not found", diff.plugin))?;
        plugin.apply_state(diff).await
    }

    /// Compatibility helper for older D-Bus code.
    /// Calculates diff between current and desired state and applies it.
    pub async fn apply_plugin_state(&self, plugin_name: &str, desired: Value) -> Result<ApplyResult> {
        let plugin = self
            .get_plugin(plugin_name)
            .await
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_name))?;
        
        let current = plugin.query_current_state().await?;
        let diff = plugin.calculate_diff(&current, &desired).await?;
        plugin.apply_state(&diff).await
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}
