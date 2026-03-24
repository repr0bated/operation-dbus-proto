//! Dinit state plugin - manages services via dinitctl and dinit-dbus conventions.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DinitConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<HashMap<String, DinitServiceConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DinitServiceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,
}

pub struct DinitStatePlugin;

impl DinitStatePlugin {
    pub fn new() -> Self {
        Self
    }

    async fn list_services(&self) -> Result<Vec<String>> {
        let output = Command::new("dinitctl")
            .args(["list"])
            .output()
            .await
            .context("failed to execute dinitctl list")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .filter_map(|line| line.split_whitespace().next().map(|s| s.to_string()))
            .collect())
    }

    async fn query_service(&self, service: &str) -> Result<DinitServiceConfig> {
        let output = Command::new("dinitctl")
            .args(["status", service])
            .output()
            .await
            .context("failed to execute dinitctl status")?;

        let state = if output.status.success() {
            let status_text = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if status_text.contains("started") || status_text.contains("running") {
                "active".to_string()
            } else {
                "inactive".to_string()
            }
        } else {
            "unknown".to_string()
        };

        Ok(DinitServiceConfig {
            state: Some(state),
            enabled: None,
            properties: None,
        })
    }

    async fn start_service(&self, service: &str) -> Result<()> {
        let output = Command::new("dinitctl")
            .args(["start", service])
            .output()
            .await
            .context("failed to execute dinitctl start")?;
        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!("failed to start service {}", service)
        }
    }

    async fn stop_service(&self, service: &str) -> Result<()> {
        let output = Command::new("dinitctl")
            .args(["stop", service])
            .output()
            .await
            .context("failed to execute dinitctl stop")?;
        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!("failed to stop service {}", service)
        }
    }

    async fn apply_service_config(&self, service: &str, config: &DinitServiceConfig) -> Result<()> {
        if let Some(state) = config.state.as_deref() {
            if state == "active" {
                self.start_service(service).await?;
            } else if state == "inactive" {
                self.stop_service(service).await?;
            }
        }
        Ok(())
    }
}

impl Default for DinitStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for DinitStatePlugin {
    fn name(&self) -> &str {
        "dinit"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/run/dinitctl").exists()
    }

    fn unavailable_reason(&self) -> String {
        "dinitctl socket not found at /run/dinitctl".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut services = HashMap::new();
        for service in self.list_services().await? {
            if let Ok(cfg) = self.query_service(&service).await {
                services.insert(service, cfg);
            }
        }
        let config = DinitConfig {
            services: Some(services),
        };
        Ok(simd_json::serde::to_owned_value(config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: DinitConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: DinitConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        if let Some(desired_services) = &desired_config.services {
            for (name, desired_svc) in desired_services {
                let current_svc = current_config.services.as_ref().and_then(|s| s.get(name));
                if current_svc != Some(desired_svc) {
                    actions.push(StateAction::Modify {
                        resource: name.clone(),
                        changes: simd_json::serde::to_owned_value(desired_svc)?,
                    });
                }
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
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let service_cfg: DinitServiceConfig =
                    simd_json::serde::from_owned_value(changes.clone())?;
                match self.apply_service_config(resource, &service_cfg).await {
                    Ok(_) => changes_applied.push(format!("Applied dinit config for {}", resource)),
                    Err(e) => errors.push(format!("Failed to apply {}: {}", resource, e)),
                }
            }
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
        let current_config: DinitConfig = simd_json::serde::from_owned_value(current)?;
        let desired_config: DinitConfig = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(current_config == desired_config)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("dinit-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_config: DinitConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        if let Some(services) = old_config.services {
            for (name, cfg) in services {
                self.apply_service_config(&name, &cfg).await?;
            }
        }
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
