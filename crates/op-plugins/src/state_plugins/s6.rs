//! S6 state plugin — manages services via s6-rc on Artix/Chimera Linux.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Path to the s6-rc live directory
const S6_RC_LIVE: &str = "/run/s6-rc";

/// Run s6-rc with the standard live-dir prefix plus additional args.
async fn s6rc(args: &[&str]) -> Result<std::process::Output> {
    tokio::process::Command::new("s6-rc")
        .arg("-l")
        .arg(S6_RC_LIVE)
        .args(args)
        .output()
        .await
        .context("failed to run s6-rc")
}

/// Per-service configuration in the desired state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct S6ServiceConfig {
    /// Desired state: "active" or "inactive"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// Top-level desired state for the s6 plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct S6Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<HashMap<String, S6ServiceConfig>>,
}

/// S6 state plugin — controls services through the `s6-rc` CLI.
pub struct S6StatePlugin;

impl S6StatePlugin {
    pub fn new() -> Self {
        Self
    }

    /// Return the names of all currently-up services.
    async fn list_running(&self) -> Result<Vec<String>> {
        // -a = show all supervised services (running only)
        let out = s6rc(&["-a", "list"]).await?;
        if !out.status.success() {
            return Ok(Vec::new());
        }
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Bring a service up; tolerate "already started" replies.
    async fn start_service(&self, name: &str) -> Result<()> {
        let out = s6rc(&["start", name]).await?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        // s6-rc exits non-zero if the service is already up
        if stderr.contains("already") {
            return Ok(());
        }
        anyhow::bail!("s6-rc start {} failed: {}", name, stderr);
    }

    /// Bring a service down; tolerate "already stopped" replies.
    async fn stop_service(&self, name: &str) -> Result<()> {
        let out = s6rc(&["stop", name]).await?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("already") {
            return Ok(());
        }
        anyhow::bail!("s6-rc stop {} failed: {}", name, stderr);
    }
}

impl Default for S6StatePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for S6StatePlugin {
    fn name(&self) -> &str {
        "s6"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::s6_plugin_schema())
    }

    /// The plugin is only available when the s6-rc live directory exists.
    fn is_available(&self) -> bool {
        std::path::Path::new(S6_RC_LIVE).exists()
    }

    fn unavailable_reason(&self) -> String {
        format!("s6-rc live directory not found at {S6_RC_LIVE}")
    }

    async fn query_current_state(&self) -> Result<Value> {
        let running = self.list_running().await?;
        let mut services = HashMap::new();
        for name in &running {
            services.insert(
                name.clone(),
                S6ServiceConfig {
                    state: Some("active".to_string()),
                },
            );
        }
        Ok(simd_json::serde::to_owned_value(S6Config {
            services: Some(services),
        })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: S6Config = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: S6Config = simd_json::serde::from_owned_value(desired.clone())?;
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
                let svc: S6ServiceConfig = simd_json::serde::from_owned_value(changes.clone())?;
                let result = match svc.state.as_deref() {
                    Some("active") => self.start_service(resource).await,
                    Some("inactive") => self.stop_service(resource).await,
                    _ => Ok(()),
                };
                match result {
                    Ok(()) => changes_applied.push(format!("Applied s6 config for {resource}")),
                    Err(e) => errors.push(format!("Failed to apply {resource}: {e}")),
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
        let current_config: S6Config = simd_json::serde::from_owned_value(current)?;
        let desired_config: S6Config = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(current_config == desired_config)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("s6-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: S6Config = simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        if let Some(services) = old.services {
            for (name, cfg) in services {
                match cfg.state.as_deref() {
                    Some("active") => {
                        let _ = self.start_service(&name).await;
                    }
                    Some("inactive") => {
                        let _ = self.stop_service(&name).await;
                    }
                    _ => {}
                }
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
