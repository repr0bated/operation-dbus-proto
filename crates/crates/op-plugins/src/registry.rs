//! Plugin Registry - Sole Authoritative Source for State
//!
//! Provides a durable, NUMA-aware, BTRFS-integrated registry for all plugins.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use op_state::StatePlugin;

/// Registry record for a plugin
pub struct PluginRecord {
    pub name: String,
    pub plugin: Arc<RwLock<dyn StatePlugin>>,
    pub storage_path: PathBuf,
    pub change_count: u64,
}

/// Sole authoritative plugin registry
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, PluginRecord>>>,
    base_path: PathBuf,
}

impl PluginRegistry {
    /// Create a new authoritative registry
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Register a plugin instance
    pub async fn register(&self, name: String, plugin: Arc<RwLock<dyn StatePlugin>>) -> Result<()> {
        let mut plugins = self.plugins.write().await;

        if plugins.contains_key(&name) {
            return Err(anyhow!("Plugin '{}' already registered", name));
        }

        // Ensure BTRFS subvolume for plugin storage
        let storage_path = self.create_plugin_subvolume(&name).await?;

        plugins.insert(
            name.clone(),
            PluginRecord {
                name,
                plugin,
                storage_path,
                change_count: 0,
            },
        );

        Ok(())
    }

    /// Get a plugin by name
    pub async fn get(&self, name: &str) -> Option<Arc<RwLock<dyn StatePlugin>>> {
        let plugins = self.plugins.read().await;
        plugins.get(name).map(|r| r.plugin.clone())
    }

    async fn create_plugin_subvolume(&self, name: &str) -> Result<PathBuf> {
        let path = self.base_path.join("plugins").join(name);

        if path.exists() {
            return Ok(path);
        }

        tokio::fs::create_dir_all(path.parent().unwrap()).await?;

        let output = Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(&path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("not a btrfs filesystem") {
                warn!(
                    "BTRFS subvolume creation failed: {}. Falling back to directory.",
                    stderr
                );
            }
            tokio::fs::create_dir_all(&path).await?;
        }

        Ok(path)
    }
}
