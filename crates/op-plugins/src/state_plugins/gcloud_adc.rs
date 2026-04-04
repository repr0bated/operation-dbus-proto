use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcloudAdcState {
    pub account: Option<String>,
    pub project_id: Option<String>,
    pub authenticated: bool,
}

pub struct GcloudAdcPlugin;

impl Default for GcloudAdcPlugin {
    fn default() -> Self {
        Self
    }
}

impl GcloudAdcPlugin {
    pub fn new() -> Self {
        Self
    }

    async fn check_auth_status() -> Result<GcloudAdcState> {
        // Check for ADC existence
        let adc_path =
            dirs::home_dir().map(|p| p.join(".config/gcloud/application_default_credentials.json"));

        let authenticated = if let Some(path) = adc_path {
            path.exists()
        } else {
            false
        };

        // Try to get active account and project from gcloud config
        let output = Command::new("gcloud")
            .args(&["config", "list", "--format=json"])
            .output()
            .await;

        let mut account = None;
        let mut project_id = None;

        if let Ok(output) = output {
            if output.status.success() {
                if let Ok(json) = std::str::from_utf8(&output.stdout) {
                    if let Ok(val) = simd_json::to_owned_value(&mut json.as_bytes().to_vec()) {
                        account = val
                            .get("core")
                            .and_then(|c| c.get("account"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        project_id = val
                            .get("core")
                            .and_then(|c| c.get("project"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
        }

        Ok(GcloudAdcState {
            account,
            project_id,
            authenticated,
        })
    }
}

#[async_trait]
impl StatePlugin for GcloudAdcPlugin {
    fn name(&self) -> &str {
        "gcloud_adc"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = Self::check_auth_status().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, desired: &Value) -> Result<StateDiff> {
        // For now, no-op diff calculation
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
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
