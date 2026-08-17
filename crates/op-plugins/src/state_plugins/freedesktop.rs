//! FreeDesktop Plugin
//!
//! Implements FreeDesktop D-Bus standards support including:
//! - org.freedesktop.DBus.ObjectManager
//! - org.freedesktop.DBus.Properties
//! - org.freedesktop.DBus.Introspectable
//! - org.freedesktop.DBus.Peer
//!
//! This plugin follows the canonical path convention:
//! - D-Bus Path: /org/opdbus/v1/plugins/freedesktop
//! - Interface: org.opdbus.v1.PluginV1
//! - Schema: schemas/plugin/freedesktop.json
//!
//! ## Architecture
//!
//! The FreeDesktop plugin provides a schema-backed implementation of standard
//! FreeDesktop D-Bus interfaces. It acts as a reference implementation for how
//! plugins should be structured in this system.
//!
//! ## Canonical Path Compliance
//!
//! This plugin uses the canonical paths as defined in `crate::canonical`:
//! - Object path prefix: `/org/opdbus/v1/plugins/`
//! - Interface: `org.opdbus.v1.PluginV1`
//!
//! Legacy paths like `/opdbus/v1/plugins/` or `/org/opdbus/v1/plugin/plugins/`
//! are normalized to the canonical prefix by `crate::canonical::normalize_plugin_path`.

use crate::canonical;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, PluginSchema, StateDiff, StatePlugin,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::{ValueAsScalar, ValueObjectAccess};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetManagedObjectsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetPropertyInput {
    pub interface_name: String,
    pub property_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetAllPropertiesInput {
    pub interface_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetPropertyInput {
    pub interface_name: String,
    pub property_name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntrospectInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PingInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetMachineIdInput {}

/// FreeDesktop plugin implementation
pub struct FreeDesktopPlugin {
    schema: Option<PluginSchema>,
    dbus_interfaces: HashMap<String, DbusInterface>,
}

/// D-Bus interface definition
#[derive(Debug, Clone)]
pub struct DbusInterface {
    pub name: String,
    pub description: String,
    pub methods: Vec<DbusMethod>,
    pub signals: Vec<DbusSignal>,
    pub properties: Vec<DbusProperty>,
}

/// D-Bus method definition
#[derive(Debug, Clone)]
pub struct DbusMethod {
    pub name: String,
    pub signature: String,
    pub description: String,
}

/// D-Bus signal definition
#[derive(Debug, Clone)]
pub struct DbusSignal {
    pub name: String,
    pub signature: String,
    pub description: String,
}

/// D-Bus property definition
#[derive(Debug, Clone)]
pub struct DbusProperty {
    pub name: String,
    pub property_type: String,
    pub access: PropertyAccess,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum PropertyAccess {
    Read,
    Write,
    ReadWrite,
}

impl FreeDesktopPlugin {
    /// Create a new FreeDesktop plugin with default configuration
    pub fn new() -> Self {
        let mut plugin = Self {
            schema: None,
            dbus_interfaces: HashMap::new(),
        };

        // Initialize schema from embedded definition
        plugin.initialize_schema();

        // Register standard FreeDesktop interfaces
        plugin.register_standard_interfaces();

        plugin
    }

    /// Initialize the plugin schema
    fn initialize_schema(&mut self) {
        // In a full implementation, this would load from schemas/plugin/freedesktop.json
        // For now, we define the schema programmatically following the canonical structure
        let mut schema = PluginSchema::builder("freedesktop")
            .category("service")
            .version("1.0.0")
            .category("system")
            .description("FreeDesktop D-Bus standards implementation")
            .method(
                super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
                    GetManagedObjectsInput,
                    super::plugin_scaffold_helpers::AckOutput,
                >(
                    "GetManagedObjects",
                    op_state_store::SideEffect::Read,
                    true,
                    "freedesktop.read",
                    "obs.software.freedesktop.managed_objects.get@v1",
                ),
            )
            .method(
                super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
                    GetPropertyInput,
                    super::plugin_scaffold_helpers::AckOutput,
                >(
                    "Get",
                    op_state_store::SideEffect::Read,
                    true,
                    "freedesktop.read",
                    "obs.software.freedesktop.property.get@v1",
                ),
            )
            .method(
                super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
                    GetAllPropertiesInput,
                    super::plugin_scaffold_helpers::AckOutput,
                >(
                    "GetAll",
                    op_state_store::SideEffect::Read,
                    true,
                    "freedesktop.read",
                    "obs.software.freedesktop.properties.getall@v1",
                ),
            )
            .method(
                super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
                    SetPropertyInput,
                    super::plugin_scaffold_helpers::AckOutput,
                >(
                    "Set",
                    op_state_store::SideEffect::Mutation,
                    true,
                    "freedesktop.write",
                    "mut.software.freedesktop.property.set@v1",
                ),
            )
            .method(
                super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
                    IntrospectInput,
                    super::plugin_scaffold_helpers::AckOutput,
                >(
                    "Introspect",
                    op_state_store::SideEffect::Read,
                    true,
                    "freedesktop.read",
                    "obs.software.freedesktop.introspect@v1",
                ),
            )
            .method(
                super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
                    PingInput,
                    super::plugin_scaffold_helpers::AckOutput,
                >(
                    "Ping",
                    op_state_store::SideEffect::Read,
                    true,
                    "freedesktop.read",
                    "obs.software.freedesktop.ping@v1",
                ),
            )
            .method(
                super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
                    GetMachineIdInput,
                    super::plugin_scaffold_helpers::AckOutput,
                >(
                    "GetMachineId",
                    op_state_store::SideEffect::Read,
                    true,
                    "freedesktop.read",
                "obs.software.freedesktop.machine_id.get@v1",
            ),
        )
        .capability(op_state_store::CapabilityDecl {
            id: "freedesktop.read".to_string(),
            description: "Grants: GetManagedObjects, Get, GetAll, Introspect, Ping, GetMachineId.".to_string(),
        })
        .capability(op_state_store::CapabilityDecl {
            id: "freedesktop.write".to_string(),
            description: "Grants: Set.".to_string(),
        })
        .build();

        #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
        struct FreeDesktopInspectorSchema {
            #[serde(default)]
            #[schemars(extend(
                "x-oscal-subid" = "sch.software.plugin.freedesktop.inspector-fields@v1"
            ))]
            inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
        }
        let root = serde_json::to_value(schemars::schema_for!(FreeDesktopInspectorSchema))
            .expect("FreeDesktop Inspector schema serializes to JSON");
        let mut inspector = super::schemars_adapter::plugin_schema_from_json(
            "freedesktop_inspector",
            "1.0.0",
            "FreeDesktop upstream Inspector fields",
            &root,
        );
        if let Some(field) = inspector.fields.remove("inspector_fields") {
            schema.fields.insert("inspector_fields".to_string(), field);
        }
        self.schema = Some(schema);
    }

    /// Register standard FreeDesktop D-Bus interfaces
    fn register_standard_interfaces(&mut self) {
        // ObjectManager interface
        self.dbus_interfaces.insert(
            canonical::OBJECT_MANAGER_INTERFACE.to_string(),
            DbusInterface {
                name: canonical::OBJECT_MANAGER_INTERFACE.to_string(),
                description: "Interface for enumerating managed objects".to_string(),
                methods: vec![DbusMethod {
                    name: "GetManagedObjects".to_string(),
                    signature: "a{oa{sa{sv}}}".to_string(),
                    description: "Get all managed objects with their interfaces".to_string(),
                }],
                signals: vec![
                    DbusSignal {
                        name: "InterfacesAdded".to_string(),
                        signature: "oa{sa{sv}}".to_string(),
                        description: "Emitted when interfaces are added".to_string(),
                    },
                    DbusSignal {
                        name: "InterfacesRemoved".to_string(),
                        signature: "oas".to_string(),
                        description: "Emitted when interfaces are removed".to_string(),
                    },
                ],
                properties: vec![],
            },
        );

        // Properties interface
        self.dbus_interfaces.insert(
            canonical::PROPERTIES_INTERFACE.to_string(),
            DbusInterface {
                name: canonical::PROPERTIES_INTERFACE.to_string(),
                description: "Standard D-Bus properties interface".to_string(),
                methods: vec![
                    DbusMethod {
                        name: "Get".to_string(),
                        signature: "v".to_string(),
                        description: "Get a property value".to_string(),
                    },
                    DbusMethod {
                        name: "GetAll".to_string(),
                        signature: "a{sv}".to_string(),
                        description: "Get all properties of an interface".to_string(),
                    },
                    DbusMethod {
                        name: "Set".to_string(),
                        signature: "".to_string(),
                        description: "Set a property value".to_string(),
                    },
                ],
                signals: vec![DbusSignal {
                    name: "PropertiesChanged".to_string(),
                    signature: "sa{sv}as".to_string(),
                    description: "Emitted when properties change".to_string(),
                }],
                properties: vec![],
            },
        );

        // Introspectable interface
        self.dbus_interfaces.insert(
            canonical::INTROSPECTABLE_INTERFACE.to_string(),
            DbusInterface {
                name: canonical::INTROSPECTABLE_INTERFACE.to_string(),
                description: "Standard D-Bus introspection interface".to_string(),
                methods: vec![DbusMethod {
                    name: "Introspect".to_string(),
                    signature: "s".to_string(),
                    description: "Return XML introspection data".to_string(),
                }],
                signals: vec![],
                properties: vec![],
            },
        );

        // Peer interface
        self.dbus_interfaces.insert(
            canonical::PEER_INTERFACE.to_string(),
            DbusInterface {
                name: canonical::PEER_INTERFACE.to_string(),
                description: "Standard D-Bus peer interface".to_string(),
                methods: vec![
                    DbusMethod {
                        name: "Ping".to_string(),
                        signature: "".to_string(),
                        description: "Ping the peer".to_string(),
                    },
                    DbusMethod {
                        name: "GetMachineId".to_string(),
                        signature: "s".to_string(),
                        description: "Get the machine ID".to_string(),
                    },
                ],
                signals: vec![],
                properties: vec![],
            },
        );
    }

    /// Get the canonical D-Bus object path for this plugin
    pub fn dbus_path(&self) -> String {
        canonical::plugin_path("freedesktop")
    }

    /// Get the canonical D-Bus interface name for this plugin
    pub fn dbus_interface(&self) -> String {
        canonical::plugin_interface("freedesktop")
    }

    /// Get a registered D-Bus interface by name
    pub fn get_interface(&self, name: &str) -> Option<&DbusInterface> {
        self.dbus_interfaces.get(name)
    }

    /// Get all registered interfaces
    pub fn all_interfaces(&self) -> &HashMap<String, DbusInterface> {
        &self.dbus_interfaces
    }

    /// Validate that a D-Bus path follows the canonical convention
    pub fn validate_canonical_path(&self, path: &str) -> Result<()> {
        if canonical::is_canonical_plugin_path(path) {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Path '{}' is not in canonical form. Expected prefix: {}",
                path,
                canonical::PLUGIN_BASE_PATH
            ))
        }
    }

    /// Normalize a legacy path to canonical form
    pub fn normalize_path(&self, path: &str) -> Option<String> {
        canonical::normalize_plugin_path(path)
    }
}

impl Default for FreeDesktopPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for FreeDesktopPlugin {
    fn name(&self) -> &str {
        "freedesktop"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn unavailable_reason(&self) -> String {
        String::new()
    }

    fn schema(&self) -> Option<PluginSchema> {
        self.schema.clone()
    }

    async fn calculate_diff(&self, _current: &Value, desired: &Value) -> anyhow::Result<StateDiff> {
        // FreeDesktop plugin is informational - we only check if desired state is valid
        let actions = vec![];

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> anyhow::Result<ApplyResult> {
        // FreeDesktop plugin is primarily informational/structural
        // No actual system changes are made
        Ok(ApplyResult {
            success: true,
            changes_applied: vec!["FreeDesktop configuration is informational only".to_string()],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> anyhow::Result<bool> {
        // Verify the desired state has the correct structure
        let current_name = desired.get("name").and_then(|v| v.as_str());
        Ok(current_name == Some("freedesktop"))
    }

    async fn create_checkpoint(&self) -> anyhow::Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("freedesktop-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!(null),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> anyhow::Result<()> {
        // FreeDesktop plugin is informational, nothing to rollback
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

/// FreeDesktopPlugin configuration
#[derive(Debug, Clone)]
pub struct FreeDesktopConfig {
    pub enable_object_manager: bool,
    pub enable_properties: bool,
    pub enable_introspection: bool,
    pub enable_peer: bool,
}

impl Default for FreeDesktopConfig {
    fn default() -> Self {
        Self {
            enable_object_manager: true,
            enable_properties: true,
            enable_introspection: true,
            enable_peer: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::prelude::TypedContainerValue;

    #[test]
    fn test_freedesktop_plugin_creation() {
        let plugin = FreeDesktopPlugin::new();
        assert_eq!(plugin.name(), "freedesktop");
        assert_eq!(plugin.version(), "1.0.0");
        assert!(plugin.is_available());
    }

    #[test]
    fn test_canonical_paths() {
        let plugin = FreeDesktopPlugin::new();

        // Verify canonical paths
        assert_eq!(plugin.dbus_path(), "/org/opdbus/v1/plugins/freedesktop");
        assert_eq!(plugin.dbus_interface(), "org.opdbus.v1.PluginV1");
    }

    #[test]
    fn test_registered_interfaces() {
        let plugin = FreeDesktopPlugin::new();

        // Check all standard interfaces are registered
        assert!(plugin
            .get_interface(canonical::OBJECT_MANAGER_INTERFACE)
            .is_some());
        assert!(plugin
            .get_interface(canonical::PROPERTIES_INTERFACE)
            .is_some());
        assert!(plugin
            .get_interface(canonical::INTROSPECTABLE_INTERFACE)
            .is_some());
        assert!(plugin.get_interface(canonical::PEER_INTERFACE).is_some());
    }

    #[test]
    fn test_path_validation() {
        let plugin = FreeDesktopPlugin::new();

        // Valid canonical path
        assert!(plugin
            .validate_canonical_path("/org/opdbus/v1/plugins/test")
            .is_ok());

        // Invalid legacy path (missing /org prefix)
        assert!(plugin
            .validate_canonical_path("/opdbus/v1/plugins/test")
            .is_err());
    }

    #[test]
    fn test_path_normalization() {
        let plugin = FreeDesktopPlugin::new();

        // Legacy path should be normalized to canonical
        let normalized = plugin.normalize_path("/opdbus/v1/plugins/test");
        assert_eq!(normalized, Some("/org/opdbus/v1/plugins/test".to_string()));

        // Legacy alias also normalizes to canonical
        let canonical = plugin.normalize_path("/org/opdbus/v1/plugin/plugins/test");
        assert_eq!(canonical, Some("/org/opdbus/v1/plugins/test".to_string()));
    }

    #[test]
    fn schema_preserves_standard_methods_and_promotes_inspector_fields() {
        let schema = FreeDesktopPlugin::new().schema().expect("schema");
        assert!(schema.fields.contains_key("inspector_fields"));
        for method in [
            "GetManagedObjects",
            "Get",
            "GetAll",
            "Set",
            "Introspect",
            "Ping",
            "GetMachineId",
        ] {
            assert!(schema.methods.contains_key(method), "missing {method}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("freedesktop", |_ctx| std::sync::Arc::new(FreeDesktopPlugin::new()))
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
    #[schemars(extend("x-oscal-subid" = "sch.software.freedesktop.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `enum.rs.Error.AccessDenied`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.accessdenied@v1"))]
        pub accessdenied: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.AddressInUse`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.addressinuse@v1"))]
        pub addressinuse: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.AdtAuditDataUnknown`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.adtauditdataunknown@v1"))]
        pub adtauditdataunknown: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.AuthFailed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.authfailed@v1"))]
        pub authfailed: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.BadAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.badaddress@v1"))]
        pub badaddress: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.Disconnected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.disconnected@v1"))]
        pub disconnected: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.Failed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.failed@v1"))]
        pub failed: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.FileExists`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.fileexists@v1"))]
        pub fileexists: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.FileNotFound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.filenotfound@v1"))]
        pub filenotfound: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.IOError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.ioerror@v1"))]
        pub ioerror: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.InconsistentMessage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.inconsistentmessage@v1"))]
        pub inconsistentmessage: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.InteractiveAuthorizationRequired`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.interactiveauthorizationrequired@v1"))]
        pub interactiveauthorizationrequired: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.InvalidArgs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.invalidargs@v1"))]
        pub invalidargs: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.InvalidFileContent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.invalidfilecontent@v1"))]
        pub invalidfilecontent: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.InvalidSignature`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.invalidsignature@v1"))]
        pub invalidsignature: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.LimitsExceeded`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.limitsexceeded@v1"))]
        pub limitsexceeded: Option<u64>,

        /// Discovered from Repomix path `enum.rs.Error.MatchRuleInvalid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.matchruleinvalid@v1"))]
        pub matchruleinvalid: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.MatchRuleNotFound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.matchrulenotfound@v1"))]
        pub matchrulenotfound: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.NameHasNoOwner`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.namehasnoowner@v1"))]
        pub namehasnoowner: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.NoMemory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.nomemory@v1"))]
        pub nomemory: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.NoNetwork`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.nonetwork@v1"))]
        pub nonetwork: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.NoReply`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.noreply@v1"))]
        pub noreply: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.NoServer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.noserver@v1"))]
        pub noserver: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.NotContainer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.notcontainer@v1"))]
        pub notcontainer: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.NotSupported`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.notsupported@v1"))]
        pub notsupported: Option<u64>,

        /// Discovered from Repomix path `enum.rs.Error.ObjectPathInUse`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.objectpathinuse@v1"))]
        pub objectpathinuse: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.PropertyReadOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.propertyreadonly@v1"))]
        pub propertyreadonly: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SELinuxSecurityContextUnknown`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.selinuxsecuritycontextunknown@v1"))]
        pub selinuxsecuritycontextunknown: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.ServiceUnknown`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.serviceunknown@v1"))]
        pub serviceunknown: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnChildExited`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnchildexited@v1"))]
        pub spawnchildexited: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnChildSignaled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnchildsignaled@v1"))]
        pub spawnchildsignaled: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnConfigInvalid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnconfiginvalid@v1"))]
        pub spawnconfiginvalid: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnExecFailed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnexecfailed@v1"))]
        pub spawnexecfailed: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnFailed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnfailed@v1"))]
        pub spawnfailed: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnFailedToSetup`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnfailedtosetup@v1"))]
        pub spawnfailedtosetup: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnFileInvalid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnfileinvalid@v1"))]
        pub spawnfileinvalid: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnForkFailed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnforkfailed@v1"))]
        pub spawnforkfailed: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnNoMemory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnnomemory@v1"))]
        pub spawnnomemory: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnPermissionsInvalid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnpermissionsinvalid@v1"))]
        pub spawnpermissionsinvalid: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnServiceNotFound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnservicenotfound@v1"))]
        pub spawnservicenotfound: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.SpawnServiceNotValid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.spawnservicenotvalid@v1"))]
        pub spawnservicenotvalid: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.TimedOut`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.timedout@v1"))]
        pub timedout: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.Timeout`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.timeout@v1"))]
        pub timeout: Option<u64>,

        /// Discovered from Repomix path `enum.rs.Error.UnixProcessIdUnknown`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unixprocessidunknown@v1"))]
        pub unixprocessidunknown: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.UnknownInterface`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unknowninterface@v1"))]
        pub unknowninterface: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.UnknownMethod`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unknownmethod@v1"))]
        pub unknownmethod: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.UnknownObject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unknownobject@v1"))]
        pub unknownobject: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.UnknownProperty`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unknownproperty@v1"))]
        pub unknownproperty: Option<String>,

        /// Discovered from Repomix path `enum.rs.Error.ZBus`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.zbus@v1"))]
        pub zbus: Option<String>,

        /// Discovered from Repomix path `enum.rs.ReleaseNameReply.NonExistent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.nonexistent@v1"))]
        pub nonexistent: Option<String>,

        /// Discovered from Repomix path `enum.rs.ReleaseNameReply.NotOwner`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.notowner@v1"))]
        pub notowner: Option<String>,

        /// Discovered from Repomix path `enum.rs.ReleaseNameReply.Released`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.released@v1"))]
        pub released: Option<String>,

        /// Discovered from Repomix path `enum.rs.RequestNameFlags.AllowReplacement`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.allowreplacement@v1"))]
        pub allowreplacement: Option<String>,

        /// Discovered from Repomix path `enum.rs.RequestNameFlags.DoNotQueue`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.donotqueue@v1"))]
        pub donotqueue: Option<String>,

        /// Discovered from Repomix path `enum.rs.RequestNameFlags.ReplaceExisting`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.replaceexisting@v1"))]
        pub replaceexisting: Option<String>,

        /// Discovered from Repomix path `enum.rs.RequestNameReply.AlreadyOwner`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.alreadyowner@v1"))]
        pub alreadyowner: Option<String>,

        /// Discovered from Repomix path `enum.rs.RequestNameReply.Exists`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.exists@v1"))]
        pub exists: Option<String>,

        /// Discovered from Repomix path `enum.rs.RequestNameReply.InQueue`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.inqueue@v1"))]
        pub inqueue: Option<String>,

        /// Discovered from Repomix path `enum.rs.RequestNameReply.PrimaryOwner`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.primaryowner@v1"))]
        pub primaryowner: Option<String>,

        /// Discovered from Repomix path `enum.rs.StartServiceReply.AlreadyRunning`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.alreadyrunning@v1"))]
        pub alreadyrunning: Option<String>,

        /// Discovered from Repomix path `enum.rs.StartServiceReply.Success`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.success@v1"))]
        pub success: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionCredentials.linux_security_label`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.linux-security-label@v1"))]
        pub linux_security_label: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionCredentials.process_fd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.process-fd@v1"))]
        pub process_fd: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionCredentials.process_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.process-id@v1"))]
        pub process_id: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionCredentials.unix_group_ids`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unix-group-ids@v1"))]
        pub unix_group_ids: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionCredentials.unix_user_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unix-user-id@v1"))]
        pub unix_user_id: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionCredentials.windows_sid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.windows-sid@v1"))]
        pub windows_sid: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.bus_names`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.bus-names@v1"))]
        pub bus_names: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.incoming_bytes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.incoming-bytes@v1"))]
        pub incoming_bytes: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.incoming_fds`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.incoming-fds@v1"))]
        pub incoming_fds: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.incoming_messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.incoming-messages@v1"))]
        pub incoming_messages: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.match_rules`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.match-rules@v1"))]
        pub match_rules: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.outgoing_bytes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.outgoing-bytes@v1"))]
        pub outgoing_bytes: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.outgoing_fds`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.outgoing-fds@v1"))]
        pub outgoing_fds: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.outgoing_messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.outgoing-messages@v1"))]
        pub outgoing_messages: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_bus_names`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-bus-names@v1"))]
        pub peak_bus_names: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_incoming_bytes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-incoming-bytes@v1"))]
        pub peak_incoming_bytes: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_incoming_fds`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-incoming-fds@v1"))]
        pub peak_incoming_fds: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_incoming_messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-incoming-messages@v1"))]
        pub peak_incoming_messages: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_match_rules`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-match-rules@v1"))]
        pub peak_match_rules: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_outgoing_bytes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-outgoing-bytes@v1"))]
        pub peak_outgoing_bytes: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_outgoing_fds`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-outgoing-fds@v1"))]
        pub peak_outgoing_fds: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.peak_outgoing_messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-outgoing-messages@v1"))]
        pub peak_outgoing_messages: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.rest`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.rest@v1"))]
        pub rest: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.serial`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.serial@v1"))]
        pub serial: Option<String>,

        /// Discovered from Repomix path `struct.rs.ConnectionStats.unique_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.unique-name@v1"))]
        pub unique_name: Option<String>,

        /// Discovered from Repomix path `struct.rs.Stats.active_connections`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.active-connections@v1"))]
        pub active_connections: Option<String>,

        /// Discovered from Repomix path `struct.rs.Stats.incomplete_connections`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.incomplete-connections@v1"))]
        pub incomplete_connections: Option<String>,

        /// Discovered from Repomix path `struct.rs.Stats.peak_bus_names_per_connection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-bus-names-per-connection@v1"))]
        pub peak_bus_names_per_connection: Option<String>,

        /// Discovered from Repomix path `struct.rs.Stats.peak_match_rules_per_connection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.freedesktop.peak-match-rules-per-connection@v1"))]
        pub peak_match_rules_per_connection: Option<String>,
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
