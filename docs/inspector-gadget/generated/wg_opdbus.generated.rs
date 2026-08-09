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
#[schemars(extend("x-oscal-category" = "network"))]
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
    let bytes =
        serde_json::to_vec(&WgOpdbusState::default()).expect("wg_opdbus default state serializes");
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
        Ok(desired
            .get("wireguard_plugin")
            .and_then(|v| v.as_str())
            .is_some()
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

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.wg-opdbus.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `enum.rs.TunError.Disconnected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.disconnected@v1"))]
        pub disconnected: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.ConnectionExpired`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.connectionexpired@v1"))]
        pub connectionexpired: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.DestinationBufferTooSmall`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.destinationbuffertoosmall@v1"))]
        pub destinationbuffertoosmall: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.DuplicateCounter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.duplicatecounter@v1"))]
        pub duplicatecounter: Option<u64>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.IncorrectPacketLength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.incorrectpacketlength@v1"))]
        pub incorrectpacketlength: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.InvalidAeadTag`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.invalidaeadtag@v1"))]
        pub invalidaeadtag: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.InvalidCounter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.invalidcounter@v1"))]
        pub invalidcounter: Option<u64>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.InvalidMac`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.invalidmac@v1"))]
        pub invalidmac: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.InvalidPacket`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.invalidpacket@v1"))]
        pub invalidpacket: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.InvalidTai64nTimestamp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.invalidtai64ntimestamp@v1"))]
        pub invalidtai64ntimestamp: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.LockFailed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.lockfailed@v1"))]
        pub lockfailed: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.NoCurrentSession`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.nocurrentsession@v1"))]
        pub nocurrentsession: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.UnderLoad`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.underload@v1"))]
        pub underload: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.UnexpectedPacket`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.unexpectedpacket@v1"))]
        pub unexpectedpacket: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.WrongIndex`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.wrongindex@v1"))]
        pub wrongindex: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.WrongKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.wrongkey@v1"))]
        pub wrongkey: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.WrongPacketType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.wrongpackettype@v1"))]
        pub wrongpackettype: Option<String>,

        /// Discovered from Repomix path `enum.rs.WireGuardError.WrongTai64nTimestamp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.wg-opdbus.wrongtai64ntimestamp@v1"))]
        pub wrongtai64ntimestamp: Option<String>,

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

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[
    ];

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
