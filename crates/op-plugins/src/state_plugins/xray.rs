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
    pub tools: Value,
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
            oscal_source: Some("/opdbus/v1/plugins/oscal_subid_registry".to_string()),
            config: XrayConfig::default(),
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
