use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::PathBuf;

const DEFAULT_CONFIG_STORE_PATH: &str = "/etc/op-dbus/config-store.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigStoreState {
    #[serde(default)]
    pub configs: HashMap<String, Value>,
}

pub struct ConfigPlugin {
    store_path: PathBuf,
}

impl Default for ConfigPlugin {
    fn default() -> Self {
        Self::new(DEFAULT_CONFIG_STORE_PATH)
    }
}

impl ConfigPlugin {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
        }
    }

    async fn load_store(&self) -> Result<ConfigStoreState> {
        match tokio::fs::read_to_string(&self.store_path).await {
            Ok(mut content) => {
                let parsed: ConfigStoreState =
                    unsafe { simd_json::from_str(&mut content) }.context("invalid config store")?;
                Ok(parsed)
            }
            Err(_) => Ok(ConfigStoreState {
                configs: HashMap::new(),
            }),
        }
    }

    async fn save_store(&self, state: &ConfigStoreState) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("failed to create config store directory")?;
        }

        let content = simd_json::to_string_pretty(state).context("serialize config store")?;
        tokio::fs::write(&self.store_path, content)
            .await
            .context("write config store")?;
        Ok(())
    }
}

#[async_trait]
impl StatePlugin for ConfigPlugin {
    fn name(&self) -> &str {
        "config"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = self.load_store().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: ConfigStoreState = simd_json::serde::from_owned_value(current.clone())?;
        let desired_state: ConfigStoreState = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        for (key, desired_value) in &desired_state.configs {
            match current_state.configs.get(key) {
                Some(current_value) if current_value == desired_value => {}
                Some(_) => actions.push(StateAction::Modify {
                    resource: key.clone(),
                    changes: desired_value.clone(),
                }),
                None => actions.push(StateAction::Create {
                    resource: key.clone(),
                    config: desired_value.clone(),
                }),
            }
        }

        for key in current_state.configs.keys() {
            if !desired_state.configs.contains_key(key) {
                actions.push(StateAction::Delete {
                    resource: key.clone(),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut state = self.load_store().await?;
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config } => {
                    state.configs.insert(resource.clone(), config.clone());
                    changes_applied.push(format!("created config key {}", resource));
                }
                StateAction::Modify { resource, changes } => {
                    state.configs.insert(resource.clone(), changes.clone());
                    changes_applied.push(format!("updated config key {}", resource));
                }
                StateAction::Delete { resource } => {
                    state.configs.remove(resource);
                    changes_applied.push(format!("deleted config key {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        if let Err(e) = self.save_store(&state).await {
            errors.push(e.to_string());
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let current_state: ConfigStoreState = simd_json::serde::from_owned_value(current)?;
        let desired_state: ConfigStoreState = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(current_state == desired_state)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("config-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_state: ConfigStoreState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        self.save_store(&old_state).await
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
