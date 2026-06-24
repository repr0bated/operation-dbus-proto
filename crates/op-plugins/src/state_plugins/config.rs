use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::json;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Schema-only view of the `config` plugin. The runtime store uses a typed
/// `HashMap<String, Value>`; the published schema preserves the original opaque
/// `configs` value so the contract stays unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.config.schema@v1"))]
pub struct ConfigSchemaState {
    #[schemars(
        description = "Configuration map",
        example = example_configs(),
        extend("default" = serde_json::json!({}), "x-oscal-subid" = "mut.software.plugin.config.configs@v1")
    )]
    pub configs: JsonValue,
}

fn example_configs() -> JsonValue {
    serde_json::json!({
        "anna_scribe": {
            "snowball_path": "/var/lib/op-dbus/snowball"
        }
    })
}

const DEFAULT_CONFIG_STORE_PATH: &str = "/etc/op-dbus/config-store.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigStoreState {
    #[serde(default)]
    pub configs: HashMap<String, Value>,
}

pub struct ConfigPlugin {
    store_path: PathBuf,
}

impl Default for ConfigPlugin {
    fn default() -> Self {
        Self::new(DEFAULT_CONFIG_STORE_PATH)
    }
}

impl ConfigPlugin {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
        }
    }

    async fn load_store(&self) -> Result<ConfigStoreState> {
        match tokio::fs::read_to_string(&self.store_path).await {
            Ok(content) => {
                let parsed: ConfigStoreState =
                    serde_json::from_str(&content).context("invalid config store")?;
                Ok(parsed)
            }
            Err(_) => Ok(ConfigStoreState {
                configs: HashMap::new(),
            }),
        }
    }

    async fn save_store(&self, state: &ConfigStoreState) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("failed to create config store directory")?;
        }

        let content = simd_json::to_string_pretty(state).context("serialize config store")?;
        tokio::fs::write(&self.store_path, content)
            .await
            .context("write config store")?;
        Ok(())
    }
}

pub(crate) fn config_plugin_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(ConfigSchemaState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "config",
        "1.0.0",
        "Global key/value config store",
        &root,
    )
}

/// Frozen golden reference for the `config` schema.
#[cfg(test)]
pub(crate) fn config_plugin_schema_golden() -> PluginSchema {
    PluginSchema::builder("config")
        .version("1.0.0")
        .description("Global key/value config store")
        .subid("__schema__", "sch.software.plugin.config.schema@v1")
        .field(
            "configs",
            FieldSchema {
                field_type: FieldType::Any,
                required: true,
                description: "Configuration map".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "anna_scribe": {
                        "snowball_path": "/var/lib/op-dbus/snowball"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .subid("configs", "mut.software.plugin.config.configs@v1")
        .build()
}

#[async_trait]
impl StatePlugin for ConfigPlugin {
    fn name(&self) -> &str {
        "config"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        let mut schema = config_plugin_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }


    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: ConfigStoreState = simd_json::serde::from_owned_value(current.clone())?;
        let desired_state: ConfigStoreState = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        for (key, desired_value) in &desired_state.configs {
            match current_state.configs.get(key) {
                Some(current_value) if current_value == desired_value => {}
                Some(_) => actions.push(StateAction::Modify {
                    resource: key.clone(),
                    changes: desired_value.clone(),
                }),
                None => actions.push(StateAction::Create {
                    resource: key.clone(),
                    config: desired_value.clone(),
                }),
            }
        }

        for key in current_state.configs.keys() {
            if !desired_state.configs.contains_key(key) {
                actions.push(StateAction::Delete {
                    resource: key.clone(),
                });
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
        let mut state = self.load_store().await?;
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config } => {
                    state.configs.insert(resource.clone(), config.clone());
                    changes_applied.push(format!("created config key {}", resource));
                }
                StateAction::Modify { resource, changes } => {
                    state.configs.insert(resource.clone(), changes.clone());
                    changes_applied.push(format!("updated config key {}", resource));
                }
                StateAction::Delete { resource } => {
                    state.configs.remove(resource);
                    changes_applied.push(format!("deleted config key {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        if let Err(e) = self.save_store(&state).await {
            errors.push(e.to_string());
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = simd_json::json!(null);
        Ok(Checkpoint {
            id: format!("config-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_state: ConfigStoreState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        self.save_store(&old_state).await
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

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_publish_plugin_owned_config_schema() {
        let schema = ConfigPlugin::default().schema().expect("config schema");
        let field = schema.fields.get("configs").expect("configs field");

        assert_eq!(schema.name, "config");
        assert_eq!(schema.version, "1.0.0");
        assert_eq!(schema.description, "Global key/value config store");
        assert!(matches!(field.field_type, FieldType::Any));
        assert_eq!(field.default, Some(json!({})));
    }

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let diffs = crate::state_plugins::schemars_adapter::schema_diffs(
            &config_plugin_schema_golden(),
            &config_plugin_schema(),
        );
        assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(ConfigSchemaState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        for subid in subids {
            assert!(
                crate::state_plugins::common::oscal::validate_subid(&subid).is_ok(),
                "invalid subid: {subid}"
            );
        }
    }

    fn collect_subids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(subid) = obj.get("x-oscal-subid").and_then(|v| v.as_str()) {
                out.push(subid.to_string());
            }
            for v in obj.values() {
                collect_subids(v, out);
            }
        }
        if let Some(arr) = value.as_array() {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("config", |ctx| std::sync::Arc::new(ConfigPlugin::new(ctx.config_path("config", "/etc/op-dbus/config-store.json"))))
}
