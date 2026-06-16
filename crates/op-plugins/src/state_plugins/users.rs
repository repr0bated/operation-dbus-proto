use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;

use simd_json::OwnedValue as Value;

use super::plugin_schema_defs::{any_field, simple_schema};
use op_state_store::PluginSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersState {
    pub users: Vec<UserConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub username: String,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub groups: Vec<String>,
    pub shell: Option<String>,
    pub present: bool,
}

pub struct UsersPlugin;

impl Default for UsersPlugin {
    fn default() -> Self {
        Self
    }
}

impl UsersPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for UsersPlugin {
    fn name(&self) -> &str {
        "users"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(users_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let content = tokio::fs::read_to_string("/etc/passwd")
            .await
            .unwrap_or_default();
        let mut users = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 7 {
                users.push(UserConfig {
                    username: parts[0].to_string(),
                    uid: parts[2].parse().ok(),
                    gid: parts[3].parse().ok(),
                    groups: vec![],
                    shell: Some(parts[6].to_string()),
                    present: true,
                });
            }
        }

        Ok(simd_json::serde::to_owned_value(UsersState { users })?)
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

pub(crate) fn users_schema() -> PluginSchema {
    simple_schema(
        "users",
        "User account declarations",
        &[],
        vec![("users", any_field(true, "Users list", Some(json!([]))))],
    )
}
