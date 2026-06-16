use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use op_state_store::{PluginSchema};
use super::plugin_schema_defs::{schema_from_state};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrayConfig {
    pub enabled: bool,
    pub socket_port: String,
    pub config_path: String,
}

impl Default for XrayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            socket_port: "gbr_xray".to_string(),
            config_path: "/etc/xray/config.json".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrayState {
    pub software: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub oscal_source: Option<String>,
    pub config: XrayConfig,
    /// Whether an xray process is currently running (host-native).
    pub running: bool,
    pub tools: Value,
}

/// Is an xray process currently running on the host? (`pgrep -x xray`)
fn xray_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "xray"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub struct XrayPlugin {
    config: XrayConfig,
}

impl XrayPlugin {
    pub fn new(config: XrayConfig) -> Self {
        Self { config }
    }
    pub(crate) fn current_state() -> XrayState {
        XrayState {
            software: "xray-core".to_string(),
            version: "1.0.0".to_string(),
            dependencies: vec!["incus".to_string()],
            oscal_source: Some("/org/opdbus/v1/plugins/oscal_subid_registry".to_string()),
            config: XrayConfig::default(),
            running: false,
            tools: json!([
                {
                    "name": "xray.run",
                    "description": "Run the Xray daemon",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "config": {
                                "type": "string",
                                "description": "The path to the config file"
                            }
                        },
                        "required": ["config"]
                    }
                }
            ]),
        }
    }
}

#[async_trait]
impl StatePlugin for XrayPlugin {
    fn name(&self) -> &'static str {
        "xray"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(xray_schema())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut state = Self::current_state();
        state.config = self.config.clone();
        state.running = xray_running();

        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: op_state::DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: "".to_string(),
                desired_hash: "".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        // xray control point (the ONLY projectable tree: /org/opdbus/v1/plugins/xray).
        // Host-native lifecycle — no out-of-tree `opdbus.v1.Xray` daemon.
        let mut changes = Vec::new();
        let mut errors = Vec::new();
        if self.config.enabled {
            // Reload (SIGHUP) so a running xray re-reads its config; if none is
            // running, the s6 supervisor (gbr-xray) is responsible for starting it.
            match std::process::Command::new("pkill")
                .args(["-HUP", "-x", "xray"])
                .status()
            {
                Ok(s) if s.success() => changes.push("xray reloaded (SIGHUP)".to_string()),
                Ok(_) => changes.push("xray not running; supervisor starts it".to_string()),
                Err(e) => errors.push(format!("xray reload failed: {e}")),
            }
        } else {
            match std::process::Command::new("pkill").args(["-x", "xray"]).status() {
                Ok(_) => changes.push("xray stopped".to_string()),
                Err(e) => errors.push(format!("xray stop failed: {e}")),
            }
        }
        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        Err(anyhow::anyhow!(
            "Checkpoints not supported by xray schema plugin"
        ))
    }

    async fn rollback(&self, _checkpoint: &op_state::Checkpoint) -> Result<()> {
        Err(anyhow::anyhow!(
            "Rollbacks not supported by xray schema plugin"
        ))
    }
}

pub(crate) fn xray_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::xray::XrayPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "xray",
        "net",
        "1.0.0",
        "Xray proxy state and execution schema",
        &state,
    )
}
