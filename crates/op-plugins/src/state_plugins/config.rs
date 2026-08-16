use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
#[cfg(test)]
use op_state_store::{FieldSchema, FieldType};
use op_state_store::{PluginSchema, SideEffect};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::json;

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Schema-only view of the `config` plugin. The runtime store uses a typed
/// `HashMap<String, Value>`; the published schema preserves the original opaque
/// `configs` value so the contract stays unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.config.schema@v1"))]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct ConfigSchemaState {
    #[schemars(
        description = "Configuration map",
        example = example_configs(),
        extend("default" = serde_json::json!({}), "x-oscal-subid" = "mut.software.plugin.config.configs@v1")
    )]
    pub configs: JsonValue,
    /// Uncapped fields discovered from the Nickel configuration sources.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.config.inspector-fields@v1"))]
    pub inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
}

fn example_configs() -> JsonValue {
    serde_json::json!({
        "identity_sled": {
            "cozo_path": "/var/lib/op-dbus/identity-cozo"
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
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "config",
        "1.0.0",
        "Global key/value config store",
        &root,
    );

    // Output structs for methods that return data
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListKeysOutput {
        pub keys: Vec<String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetValueOutput {
        pub value: Option<String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ExportConfigOutput {
        pub config: serde_json::Value,
    }

    // Add methods
    schema.methods.insert(
        "list_keys".to_string(),
        method_decl_from_schemars_with_output::<(), ListKeysOutput>(
            "list_keys",
            SideEffect::Read,
            true,
            "config.read",
            "obs.service.config.key.list@v1",
        ),
    );
    schema.methods.insert(
        "get_value".to_string(),
        method_decl_from_schemars_with_output::<(), GetValueOutput>(
            "get_value",
            SideEffect::Read,
            true,
            "config.read",
            "obs.service.config.value.get@v1",
        ),
    );
    schema.methods.insert(
        "set_value".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "set_value",
            SideEffect::Mutation,
            false,
            "config.invoke",
            "mut.service.config.value.set@v1",
        ),
    );
    schema.methods.insert(
        "delete_key".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "delete_key",
            SideEffect::Mutation,
            true,
            "config.invoke",
            "mut.service.config.key.delete@v1",
        ),
    );
    schema.methods.insert(
        "export_config".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "export_config",
            SideEffect::Read,
            true,
            "config.read",
            "obs.service.config.export@v1",
        ),
    );

    schema
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
    fn schema_is_schemars_seeded_and_typed() {
        let schema = config_plugin_schema();
        assert_eq!(schema.name, "config");
        assert_eq!(schema.version, "1.0.0");
        assert_eq!(schema.description, "Global key/value config store");
        assert!(schema.fields.contains_key("configs"));
        assert!(schema.methods.contains_key("list_keys"));
        assert!(schema.methods.contains_key("get_value"));
        assert!(schema.methods.contains_key("set_value"));
        assert!(schema.methods.contains_key("delete_key"));
        assert!(schema.methods.contains_key("export_config"));
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

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.config.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `enum.rs.Command.Convert`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.convert@v1"))]
        pub convert: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Doc`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.doc@v1"))]
        pub doc: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Eval`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.eval@v1"))]
        pub eval: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Export`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.export@v1"))]
        pub export: Option<u64>,

        /// Discovered from Repomix path `enum.rs.Command.Format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.format@v1"))]
        pub format: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.GenCompletions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.gencompletions@v1"))]
        pub gencompletions: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Package`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.package@v1"))]
        pub package: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.PprintAst`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.pprintast@v1"))]
        pub pprintast: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Query`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.query@v1"))]
        pub query: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Repl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.repl@v1"))]
        pub repl: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Test`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.test@v1"))]
        pub test: Option<String>,

        /// Discovered from Repomix path `enum.rs.Command.Typecheck`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.typecheck@v1"))]
        pub typecheck: Option<String>,

        /// Discovered from Repomix path `struct.rs.GlobalOptions.color`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.color@v1"))]
        pub color: Option<String>,

        /// Discovered from Repomix path `struct.rs.GlobalOptions.error_format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.error-format@v1"))]
        pub error_format: Option<String>,

        /// Discovered from Repomix path `struct.rs.GlobalOptions.metrics`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.metrics@v1"))]
        pub metrics: Option<String>,

        /// Discovered from Repomix path `struct.rs.Options.command`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.command@v1"))]
        pub command: Option<String>,

        /// Discovered from Repomix path `struct.rs.Options.global`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.config.global@v1"))]
        pub global: Option<String>,
    }

    /// Metadata needed when promoting a generated typed method into `schema.methods`.
    pub struct MethodCandidate {
        pub name: &'static str,
        pub side_effect: &'static str,
        pub idempotent: bool,
        pub required_capability: &'static str,
        pub subid: &'static str,
        pub repomix_path: &'static str,
        pub command: &'static [&'static str],
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[];

    /// Promote every generated method into the sealed plugin schema.
    pub(super) fn register_methods(schema: &mut op_state_store::PluginSchema) {
        use super::super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    }
}

// Promotion checklist (Fable contract):
// 1. Move owned fields into the plugin State struct with concrete Rust types.
// 2. Replace method placeholders with dedicated typed Input/Output fields.
// 3. Register with method_decl_from_schemars_with_output and correct SideEffect.
// 4. Register every subid, implement dispatch, and add schema/subid tests.
// 5. Re-run op-plugin-lint; only then replace the original plugin file.
