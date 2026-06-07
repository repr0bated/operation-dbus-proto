use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

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

        // Read gcloud config directly from file (AGENTS.md §4: no subprocess bypasses)
        let mut account = None;
        let mut project_id = None;

        if let Some(home) = dirs::home_dir() {
            let active_config_path = home.join(".config/gcloud/active_config");
            let active_config = tokio::fs::read_to_string(&active_config_path)
                .await
                .unwrap_or_default()
                .trim()
                .to_string();

            let config_name = if active_config.is_empty() {
                "config_default"
            } else {
                &format!("config_{}", active_config)
            };

            let config_path = home.join(".config/gcloud/configurations").join(config_name);

            if let Ok(content) = tokio::fs::read_to_string(&config_path).await {
                let mut current_section = String::new();
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('[') && trimmed.ends_with(']') {
                        current_section = trimmed[1..trimmed.len() - 1].to_string();
                    } else if let Some((key, value)) = trimmed.split_once('=') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"').to_string();
                        if current_section == "core" {
                            match key {
                                "account" => account = Some(value),
                                "project" => project_id = Some(value),
                                _ => {}
                            }
                        }
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

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::gcloud_adc_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = Self::check_auth_status().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
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
