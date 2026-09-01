//! `wg_opdbus` plugin — hypervisor WireGuard identity interface contract.
//!
//! This plugin does not define a second WireGuard model. It binds an op-dbus
//! identity role to an interface described by the existing `wireguard` plugin.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{CapabilityDecl, PluginSchema, SideEffect};
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

pub const ANNA_SCRIBE_PRINCIPAL_ID: &str = "did:op:service:anna-scribe";
pub const ANNA_SCRIBE_PERSONA_ID: &str = "anna-scribe";
pub const ANNA_SCRIBE_EMAIL: &str = "anna.scribe@3tched.com";
pub const ANNA_SCRIBE_PLUGIN_REF: &str = PLUGIN_NAME;

/// Persistent service-persona identity represented by this plugin.
///
/// The fields are private and deserialization accepts only the canonical
/// values, so state input cannot rename Anna, move her to another plugin, or
/// turn the descriptive email address into a caller-selected authority key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.anna-scribe.service-persona@v1"))]
pub struct AnnaScribeServicePersona {
    /// Stable authorization and audit identity; never derived from email.
    #[schemars(extend("x-oscal-subid" = "src.service.anna-scribe.principal-id@v1"))]
    principal_id: String,
    /// Stable persona catalog identifier.
    #[schemars(extend("x-oscal-subid" = "src.service.anna-scribe.persona-id@v1"))]
    persona_id: String,
    /// Descriptive contact address; never an authority or grant key.
    #[schemars(extend("x-oscal-subid" = "src.service.anna-scribe.email@v1"))]
    email: String,
    /// Plugin that projects this service persona.
    #[schemars(extend("x-oscal-subid" = "src.service.anna-scribe.plugin-ref@v1"))]
    plugin_ref: String,
}

impl Default for AnnaScribeServicePersona {
    fn default() -> Self {
        Self {
            principal_id: ANNA_SCRIBE_PRINCIPAL_ID.to_string(),
            persona_id: ANNA_SCRIBE_PERSONA_ID.to_string(),
            email: ANNA_SCRIBE_EMAIL.to_string(),
            plugin_ref: ANNA_SCRIBE_PLUGIN_REF.to_string(),
        }
    }
}

impl<'de> Deserialize<'de> for AnnaScribeServicePersona {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireIdentity {
            principal_id: String,
            persona_id: String,
            email: String,
            plugin_ref: String,
        }

        let supplied = WireIdentity::deserialize(deserializer)?;
        if supplied.principal_id != ANNA_SCRIBE_PRINCIPAL_ID
            || supplied.persona_id != ANNA_SCRIBE_PERSONA_ID
            || supplied.email != ANNA_SCRIBE_EMAIL
            || supplied.plugin_ref != ANNA_SCRIBE_PLUGIN_REF
        {
            return Err(serde::de::Error::custom(
                "A.N.N.A. Scribe service-persona identity is immutable",
            ));
        }
        Ok(Self::default())
    }
}

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
#[schemars(extend("x-immutable-paths" = ["/service_persona"]))]
pub struct WgOpdbusState {
    /// Immutable service-persona metadata. Authorization remains bound to the
    /// stable principal and its referenced host credential, never to email.
    #[serde(default)]
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "src.service.wg-opdbus.service-persona@v1")
    )]
    pub service_persona: AnnaScribeServicePersona,
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
            service_persona: AnnaScribeServicePersona::default(),
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
    /// Canonical read-only A.N.N.A. Scribe service-persona identity.
    #[serde(default)]
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.wg-opdbus.service-persona@v1")
    )]
    pub service_persona: AnnaScribeServicePersona,
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
            service_persona: state.service_persona,
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
    schema.capabilities.insert(
        "wg_opdbus.invoke".to_string(),
        CapabilityDecl {
            id: "wg_opdbus.invoke".to_string(),
            description: "Grants: create_interface.".to_string(),
        },
    );
    schema.capabilities.insert(
        "wg_opdbus.read".to_string(),
        CapabilityDecl {
            id: "wg_opdbus.read".to_string(),
            description: "Grants: get_status.".to_string(),
        },
    );

    schema
}

inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(WgOpdbusPlugin::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anna_scribe_service_persona_is_canonical_and_immutable() {
        let identity = serde_json::to_value(AnnaScribeServicePersona::default())
            .expect("service persona serializes");
        assert_eq!(identity["principal_id"], ANNA_SCRIBE_PRINCIPAL_ID);
        assert_eq!(identity["persona_id"], ANNA_SCRIBE_PERSONA_ID);
        assert_eq!(identity["email"], ANNA_SCRIBE_EMAIL);
        assert_eq!(identity["plugin_ref"], ANNA_SCRIBE_PLUGIN_REF);

        serde_json::from_value::<AnnaScribeServicePersona>(identity.clone())
            .expect("canonical identity deserializes");
        let mut changed = identity;
        changed["email"] = serde_json::json!("caller-selected@example.invalid");
        assert!(serde_json::from_value::<AnnaScribeServicePersona>(changed).is_err());
    }

    #[test]
    fn wg_opdbus_schema_projects_anna_scribe_identity() {
        let schema = wg_opdbus_schema();
        assert_eq!(schema.immutable_paths, vec!["/service_persona"]);
        let example = schema.example.expect("wg_opdbus schema has an example");
        assert_eq!(
            example["service_persona"]["principal_id"],
            ANNA_SCRIBE_PRINCIPAL_ID
        );
        assert_eq!(
            example["service_persona"]["persona_id"],
            ANNA_SCRIBE_PERSONA_ID
        );
        assert_eq!(example["service_persona"]["email"], ANNA_SCRIBE_EMAIL);
        assert_eq!(
            example["service_persona"]["plugin_ref"],
            ANNA_SCRIBE_PLUGIN_REF
        );
    }
}
