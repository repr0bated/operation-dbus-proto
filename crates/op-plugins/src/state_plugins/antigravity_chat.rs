use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use op_state_store::{PluginSchema};
use super::plugin_schema_defs::{schema_from_state};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityChatState {
    pub status: String,
    pub bridge: Value,
    pub auth: Value,
    pub models: Value,
    pub config: Value,
}
pub struct AntigravityChatPlugin;
impl Default for AntigravityChatPlugin {
    fn default() -> Self {
        Self
    }
}
impl AntigravityChatPlugin {
    pub fn new() -> Self {
        Self
    }
    pub(crate) fn current_state() -> AntigravityChatState {
        AntigravityChatState {
            status: "declared".to_string(),
            bridge: json!({"url": "http://127.0.0.1:3333", "status": "offline", "version": null, "last_seen": null}),
            auth: json!({"method": "oauth", "provider": "google", "token_file": "~/.config/antigravity/token.json", "token_valid": false, "scopes": ["https://www.googleapis.com/auth/cloud-platform"]}),
            models: json!([{"id": "gemini-2.0-flash", "name": "Gemini 2.0 Flash", "available": false}, {"id": "gemini-1.5-pro", "name": "Gemini 1.5 Pro", "available": false}, {"id": "gemini-1.5-flash", "name": "Gemini 1.5 Flash", "available": false}]),
            config: json!({"headless": true, "display_service": "antigravity-display", "vnc_port": 5900, "extract_script": "antigravity-extract-token.sh", "code_assist": true}),
        }
    }
}
#[async_trait]
impl StatePlugin for AntigravityChatPlugin {
    fn name(&self) -> &str {
        "antigravity_chat"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(antigravity_chat_schema())
    }
    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(Self::current_state())?)
    }
    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
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
            state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?,
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

pub(crate) fn antigravity_chat_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(
        super::antigravity_chat::AntigravityChatPlugin::current_state(),
    )
    .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "antigravity_chat",
        "llm",
        "1.0.0",
        "Antigravity Chat — OAuth bridge, Gemini models, headless IDE",
        &state,
    )
}
