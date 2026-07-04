//! `wg_opdbus` plugin — hypervisor WireGuard identity interface contract.
//!
//! This plugin does not define a second WireGuard model. It binds an op-dbus
//! identity role to an interface described by the existing `wireguard` plugin.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{PluginSchema, SideEffect};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};
use super::wireguard::WireGuardInterface;

const PLUGIN_NAME: &str = "wg_opdbus";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "network";
const PLUGIN_DESCRIPTION: &str = "Hypervisor WireGuard identity interface for op-dbus";
const PLUGIN_DISPLAY_NAME: &str = "A.N.N.A. Scribe WireGuard Identity";

#[derive(Debug, Clone, Default)]
pub struct WgOpdbusPlugin;

impl WgOpdbusPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.network.wg-opdbus.schema@v1"))]
pub struct WgOpdbusState {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.wg-opdbus.wireguard-plugin@v1"))]
    pub wireguard_plugin: Option<String>,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.wg-opdbus.interface-binding@v1"))]
    pub interface: Option<WireGuardInterface>,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.wg-opdbus.config@v1"))]
    pub config_path: Option<String>,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.wg-opdbus.identity-role@v1"))]
    pub identity_role: Option<String>,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.wg-opdbus.routes@v1"))]
    pub route_targets: Vec<String>,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.network.wg-opdbus.netmaker-separate@v1"))]
    pub netmaker_plugin: Option<String>,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.network.wg-opdbus.wgcf-present@v1"))]
    pub excluded_plugins: Vec<String>,
}

impl Default for WgOpdbusState {
    fn default() -> Self {
        Self {
            wireguard_plugin: None,
            interface: None,
            config_path: None,
            identity_role: None,
            route_targets: Vec::new(),
            netmaker_plugin: None,
            excluded_plugins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.network.wg-opdbus.interface.create.input@v1"))]
pub struct CreateInterfaceInput {
    pub wireguard_plugin: String,
    pub interface: WireGuardInterface,
    #[serde(default)]
    pub config_path: Option<String>,
    #[serde(default)]
    pub identity_role: Option<String>,
    #[serde(default)]
    pub route_targets: Vec<String>,
    #[serde(default)]
    pub netmaker_plugin: Option<String>,
    #[serde(default)]
    pub excluded_plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wg-opdbus.status.input@v1"))]
pub struct GetStatusInput {
    pub wireguard_plugin: String,
    pub interface_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.wg-opdbus.status.output@v1"))]
pub struct WgOpdbusStatusOutput {
    pub wireguard_plugin: Option<String>,
    pub interface: Option<WireGuardInterface>,
    pub config_path: Option<String>,
    pub identity_role: Option<String>,
    pub route_targets: Vec<String>,
    pub netmaker_plugin: Option<String>,
    pub excluded_plugins: Vec<String>,
}

impl From<WgOpdbusState> for WgOpdbusStatusOutput {
    fn from(state: WgOpdbusState) -> Self {
        Self {
            wireguard_plugin: state.wireguard_plugin,
            interface: state.interface,
            config_path: state.config_path,
            identity_role: state.identity_role,
            route_targets: state.route_targets,
            netmaker_plugin: state.netmaker_plugin,
            excluded_plugins: state.excluded_plugins,
        }
    }
}

fn default_state_value() -> Value {
    let bytes = serde_json::to_vec(&WgOpdbusState::default())
        .expect("wg_opdbus default state serializes");
    let mut bytes = bytes;
    simd_json::to_owned_value(&mut bytes).expect("wg_opdbus default state is valid JSON")
}

#[async_trait]
impl StatePlugin for WgOpdbusPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(wg_opdbus_schema())
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: PLUGIN_NAME.to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
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

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        Ok(desired.get("wireguard_plugin").and_then(|v| v.as_str()).is_some()
            && desired.get("interface").is_some())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("{}-{}", PLUGIN_NAME, uuid::Uuid::new_v4()),
            plugin: PLUGIN_NAME.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: default_state_value(),
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
            atomic_operations: true,
        }
    }
}

pub(crate) fn wg_opdbus_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(WgOpdbusState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());
    schema.dependencies.push("wireguard".to_string());

    schema.methods.insert(
        "create_interface".to_string(),
        method_decl_from_schemars_with_output::<CreateInterfaceInput, AckOutput>(
            "create_interface",
            SideEffect::Mutation,
            true,
            "wg_opdbus.invoke",
            "mut.network.wg-opdbus.interface.create@v1",
        ),
    );

    schema.methods.insert(
        "get_status".to_string(),
        method_decl_from_schemars_with_output::<GetStatusInput, WgOpdbusStatusOutput>(
            "get_status",
            SideEffect::Read,
            true,
            "wg_opdbus.read",
            "obs.network.wg-opdbus.status.get@v1",
        ),
    );

    schema.example = Some(default_state_value());
    schema
}

inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(WgOpdbusPlugin::new()))
}
