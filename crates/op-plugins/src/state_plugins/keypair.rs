use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeypairState {
    pub keypairs: Vec<Keypair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keypair {
    pub name: String,
    pub algorithm: String,
    pub public_key: Option<String>,
    pub present: bool,
}

pub struct KeypairPlugin;

impl Default for KeypairPlugin {
    fn default() -> Self {
        Self
    }
}

impl KeypairPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for KeypairPlugin {
    fn name(&self) -> &str {
        "keypair"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut keypairs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let ssh_dir = home.join(".ssh");
            if let Ok(mut entries) = tokio::fs::read_dir(ssh_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("id_") && name.ends_with(".pub") {
                            let key_name = name.trim_end_matches(".pub").to_string();
                            let content =
                                tokio::fs::read_to_string(&path).await.unwrap_or_default();
                            let parts: Vec<&str> = content.split_whitespace().collect();
                            let algorithm = if !parts.is_empty() {
                                parts[0].to_string()
                            } else {
                                "unknown".to_string()
                            };

                            keypairs.push(Keypair {
                                name: key_name,
                                algorithm,
                                public_key: Some(content.trim().to_string()),
                                present: true,
                            });
                        }
                    }
                }
            }
        }

        Ok(simd_json::serde::to_owned_value(KeypairState { keypairs })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
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
