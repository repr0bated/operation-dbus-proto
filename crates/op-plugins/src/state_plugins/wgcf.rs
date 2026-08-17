use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

use super::plugin_scaffold_helpers::{
    method_decl_from_schemars, method_decl_from_schemars_with_output,
};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.wgcf.config.schema@v1"))]
pub struct WgcfConfig {
    /// Whether WGCF is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.wgcf.config.enabled@v1"))]
    pub enabled: bool,
    /// fwmark value for WireGuard packets.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.wgcf.config.fwmark@v1"))]
    pub fwmark: u32,
    /// WireGuard listen port.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.wgcf.config.wireguard-port@v1"))]
    pub wireguard_port: u16,
    /// Path to the generated WireGuard config file.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.wgcf.config.config-path@v1"))]
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

/// Runtime state of the WGCF plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.wgcf.schema@v1"))]
#[schemars(extend("x-oscal-category" = "network"))]
pub struct WgcfState {
    /// Software identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.wgcf.software@v1"))]
    pub software: String,
    /// Software version.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.wgcf.version@v1"))]
    pub version: String,
    /// Runtime dependencies.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.wgcf.dependencies@v1"))]
    pub dependencies: Vec<String>,
    /// OSCAL subid registry source path.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.plugin.wgcf.oscal-source@v1"))]
    pub oscal_source: Option<String>,
    /// WGCF configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.wgcf.config@v1"))]
    pub config: WgcfConfig,
    /// MCP tool definitions exposed by this plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.wgcf.tools@v1"))]
    pub tools: serde_json::Value,
}

impl Default for WgcfState {
    fn default() -> Self {
        Self {
            software: "wgcf".to_string(),
            version: "1.0.0".to_string(),
            dependencies: vec!["net".to_string()],
            oscal_source: Some("/opdbus/v1/plugins/oscal_subid_registry".to_string()),
            config: WgcfConfig::default(),
            tools: serde_json::json!([
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
            tools: serde_json::json!([
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
        let mut schema = wgcf_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RegisterInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RotateKeysInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetEndpointInput {
    pub endpoint: String,
}

/// Input struct for GenerateConfig method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GenerateConfigInput {
    /// Interface name
    pub interface: Option<String>,
    /// DNS servers
    #[serde(default)]
    pub dns: Vec<String>,
}

/// Input struct for ApplyConfig method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApplyConfigInput {
    /// Config content
    pub config: String,
    /// Apply immediately
    #[serde(default)]
    pub immediate: bool,
}

/// Input struct for Refresh method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RefreshInput {
    /// Refresh type: keys, ips, all
    #[serde(default = "default_refresh_type")]
    pub refresh_type: String,
}

fn default_refresh_type() -> String {
    "all".to_string()
}

/// Derived `wgcf` schema from the typed [`WgcfState`] struct via schemars.
pub(crate) fn wgcf_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(WgcfState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "wgcf",
        "1.0.0",
        "WireGuard Cloudflare (WGCF) state and execution schema",
        &root,
    );
    let state = simd_json::serde::to_owned_value(&WgcfState::default())
        .expect("WgcfState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);

    schema.methods.insert(
        "register".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            RegisterInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "register",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.wgcf.register@v1",
            "mut.software.wgcf.register@v1",
        ),
    );
    schema.methods.insert(
        "update".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            UpdateInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "update",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.wgcf.update@v1",
            "mut.software.wgcf.update@v1",
        ),
    );
    schema.methods.insert(
        "status".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            StatusInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "status",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.wgcf.status@v1",
            "obs.software.wgcf.status@v1",
        ),
    );
    schema.methods.insert(
        "rotate_keys".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            RotateKeysInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "rotate_keys",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.wgcf.keys.rotate@v1",
            "mut.software.wgcf.keys.rotate@v1",
        ),
    );
    schema.methods.insert(
        "set_endpoint".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetEndpointInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "set_endpoint",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.wgcf.endpoint.set@v1",
            "mut.software.wgcf.endpoint.set@v1",
        ),
    );

    // GenerateConfig method
    schema.methods.insert(
        "generate_config".to_string(),
        method_decl_from_schemars_with_output::<
            GenerateConfigInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GenerateConfig",
            op_state_store::SideEffect::Mutation,
            false,
            "wgcf.write",
            "mut.software.wgcf.config.generate@v1",
        ),
    );

    // ApplyConfig method
    schema.methods.insert(
        "apply_config".to_string(),
        method_decl_from_schemars_with_output::<
            ApplyConfigInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ApplyConfig",
            op_state_store::SideEffect::Mutation,
            false,
            "wgcf.write",
            "mut.software.wgcf.config.apply@v1",
        ),
    );

    // Refresh method
    schema.methods.insert(
        "refresh".to_string(),
        method_decl_from_schemars_with_output::<
            RefreshInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "Refresh",
            op_state_store::SideEffect::Mutation,
            false,
            "wgcf.write",
            "mut.software.wgcf.refresh@v1",
        ),
    );

    schema.capabilities.insert(
        "cap.software.wgcf.register@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.software.wgcf.register@v1".to_string(),
            description: "Grants: register.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.software.wgcf.update@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.software.wgcf.update@v1".to_string(),
            description: "Grants: update.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.software.wgcf.status@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.software.wgcf.status@v1".to_string(),
            description: "Grants: status.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.software.wgcf.keys.rotate@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.software.wgcf.keys.rotate@v1".to_string(),
            description: "Grants: rotate_keys.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.software.wgcf.endpoint.set@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.software.wgcf.endpoint.set@v1".to_string(),
            description: "Grants: set_endpoint.".to_string(),
        },
    );
    schema.capabilities.insert(
        "wgcf.write".to_string(),
        op_state_store::CapabilityDecl {
            id: "wgcf.write".to_string(),
            description: "Grants: generate_config, apply_config, refresh.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use crate::state_plugins::schemars_adapter::schema_diffs;
    use serde_json::Value as JVal;

    fn collect_subids(value: &JVal, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(JVal::String(subid)) = obj.get("x-oscal-subid") {
                out.push(subid.clone());
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

    #[test]
    fn all_subids_are_valid() {
        let root = serde_json::to_value(schemars::schema_for!(WgcfState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("wgcf", |_ctx| std::sync::Arc::new(WgcfPlugin::new(WgcfConfig::default())))
}
