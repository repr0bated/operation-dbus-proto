use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// OAuth bridge endpoint for Antigravity Chat.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity-chat.bridge.schema@v1"))]
pub struct Bridge {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.bridge-url@v1"))]
    pub url: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity-chat.bridge-status@v1"))]
    pub status: String,
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity-chat.bridge-version@v1"))]
    pub version: Option<String>,
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity-chat.bridge-last-seen@v1"))]
    pub last_seen: Option<String>,
}

/// Authentication credentials and token state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity-chat.auth.schema@v1"))]
pub struct Auth {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.auth-method@v1"))]
    pub method: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.auth-provider@v1"))]
    pub provider: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.auth-token-file@v1"))]
    pub token_file: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity-chat.auth-token-valid@v1"))]
    pub token_valid: bool,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity-chat.auth-scopes@v1"))]
    pub scopes: Vec<String>,
}

/// Declared Gemini model entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity-chat.model.schema@v1"))]
pub struct Model {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity-chat.model-id@v1"))]
    pub id: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity-chat.model-name@v1"))]
    pub name: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity-chat.model-available@v1"))]
    pub available: bool,
}

/// Headless IDE and runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.antigravity-chat.config.schema@v1"))]
pub struct ChatConfig {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.config-headless@v1"))]
    pub headless: bool,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.config-display-service@v1"))]
    pub display_service: String,
    #[serde(default)]
    #[schemars(
        range(min = 0, max = 65535),
        extend("x-oscal-subid" = "mut.software.antigravity-chat.config-vnc-port@v1")
    )]
    pub vnc_port: u16,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.config-extract-script@v1"))]
    pub extract_script: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.config-code-assist@v1"))]
    pub code_assist: bool,
}

/// Top-level Antigravity Chat state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.antigravity-chat.schema@v1"))]
#[schemars(extend("x-oscal-category" = "llm"))]
pub struct AntigravityChatState {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.antigravity-chat.status@v1"))]
    pub status: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.bridge@v1"))]
    pub bridge: Bridge,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.antigravity-chat.auth@v1"))]
    pub auth: Auth,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.antigravity-chat.models@v1"))]
    pub models: Vec<Model>,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.antigravity-chat.config@v1"))]
    pub config: ChatConfig,
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
            bridge: Bridge {
                url: "http://127.0.0.1:3333".to_string(),
                status: "offline".to_string(),
                version: None,
                last_seen: None,
            },
            auth: Auth {
                method: "oauth".to_string(),
                provider: "google".to_string(),
                token_file: "~/.config/antigravity/token.json".to_string(),
                token_valid: false,
                scopes: vec!["https://www.googleapis.com/auth/cloud-platform".to_string()],
            },
            models: vec![
                Model {
                    id: "gemini-2.0-flash".to_string(),
                    name: "Gemini 2.0 Flash".to_string(),
                    available: false,
                },
                Model {
                    id: "gemini-1.5-pro".to_string(),
                    name: "Gemini 1.5 Pro".to_string(),
                    available: false,
                },
                Model {
                    id: "gemini-1.5-flash".to_string(),
                    name: "Gemini 1.5 Flash".to_string(),
                    available: false,
                },
            ],
            config: ChatConfig {
                headless: true,
                display_service: "antigravity-display".to_string(),
                vnc_port: 5900,
                extract_script: "antigravity-extract-token.sh".to_string(),
                code_assist: true,
            },
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
        let mut schema = antigravity_chat_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
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
    let root = serde_json::to_value(schemars::schema_for!(AntigravityChatState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "antigravity_chat",
        "1.0.0",
        "Antigravity Chat — OAuth bridge, Gemini models, headless IDE",
        &root,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use serde_json::Value as JVal;

    #[test]
    fn schema_is_schemars_seeded_and_typed() {
        let schema = antigravity_chat_schema();
        assert_eq!(schema.name, "antigravity_chat");
        assert_eq!(schema.version, "1.0.0");
        assert!(schema.fields.contains_key("status"));
        assert!(schema.fields.contains_key("bridge"));
        assert!(schema.fields.contains_key("auth"));
        assert!(schema.fields.contains_key("models"));
        assert!(schema.fields.contains_key("config"));
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(AntigravityChatState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(
            !subids.is_empty(),
            "expected at least one x-oscal-subid in the derived schema"
        );
        for subid in subids {
            validate_subid(&subid).unwrap_or_else(|e| panic!("invalid subid {subid}: {e}"));
        }
    }

    fn collect_subids(node: &JVal, out: &mut Vec<String>) {
        if let Some(subid) = node.get("x-oscal-subid").and_then(JVal::as_str) {
            out.push(subid.to_string());
        }
        if let Some(props) = node.get("properties").and_then(JVal::as_object) {
            for v in props.values() {
                collect_subids(v, out);
            }
        }
        if let Some(defs) = node
            .get("$defs")
            .or_else(|| node.get("definitions"))
            .and_then(JVal::as_object)
        {
            for v in defs.values() {
                collect_subids(v, out);
            }
        }
        if let Some(items) = node.get("items") {
            collect_subids(items, out);
        }
        if let Some(alternatives) = node
            .get("anyOf")
            .or_else(|| node.get("oneOf"))
            .and_then(JVal::as_array)
        {
            for v in alternatives {
                collect_subids(v, out);
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("antigravity_chat", |_ctx| std::sync::Arc::new(AntigravityChatPlugin::new()))
}
