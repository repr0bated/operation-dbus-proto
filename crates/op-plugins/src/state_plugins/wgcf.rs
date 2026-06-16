use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WgcfConfig {
    pub enabled: bool,
    pub fwmark: u32,
    pub wireguard_port: u16,
    pub config_path: String,
}

impl Default for WgcfConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fwmark: 0x51820,
            wireguard_port: 51820,
            config_path: "/etc/wireguard/wgcf.conf".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WgcfState {
    pub software: String,
    pub version: String,
    pub dependencies: Vec<String>,
    pub oscal_source: Option<String>,
    pub config: WgcfConfig,
    pub tools: Value,
}

pub struct WgcfPlugin {
    config: WgcfConfig,
}

impl WgcfPlugin {
    pub fn new(config: WgcfConfig) -> Self {
        Self { config }
    }
    pub(crate) fn current_state() -> WgcfState {
        WgcfState {
            software: "wgcf".to_string(),
            version: "1.0.0".to_string(),
            dependencies: vec!["net".to_string()],
            oscal_source: Some("/opdbus/v1/plugins/oscal_subid_registry".to_string()),
            config: WgcfConfig::default(),
            tools: json!([
                {
                    "name": "wgcf.generate",
                    "description": "Generate a WGCF profile",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "wgcf.register",
                    "description": "Register a new Cloudflare WARP account",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            ]),
        }
    }
}

#[async_trait]
impl StatePlugin for WgcfPlugin {
    fn name(&self) -> &'static str {
        "wgcf"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(wgcf_schema())
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
            "Checkpoints not supported by wgcf schema plugin"
        ))
    }

    async fn rollback(&self, _checkpoint: &op_state::Checkpoint) -> Result<()> {
        Err(anyhow::anyhow!(
            "Rollbacks not supported by wgcf schema plugin"
        ))
    }
}

pub(crate) fn wgcf_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::wgcf::WgcfPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "wgcf",
        "net",
        "1.0.0",
        "WireGuard Cloudflare (WGCF) state and execution schema",
        &state,
    )
}
