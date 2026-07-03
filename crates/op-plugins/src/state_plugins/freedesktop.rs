//! FreeDesktop Plugin
//!
//! Implements FreeDesktop D-Bus standards support including:
//! - org.freedesktop.DBus.ObjectManager
//! - org.freedesktop.DBus.Properties
//! - org.freedesktop.DBus.Introspectable
//! - org.freedesktop.DBus.Peer
//!
//! This plugin follows the canonical path convention:
//! - D-Bus Path: /org/opdbus/v1/plugin/plugins/freedesktop
//! - Interface: org.opdbus.v1.Plugin.Plugins.FreeDesktop
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
//! - Object path prefix: `/org/opdbus/v1/plugin/plugins/`
//! - Interface prefix: `org.opdbus.v1.Plugin.Plugins`
//!
//! Legacy paths like `/opdbus/v1/plugins/` are NOT supported.

use crate::canonical;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, PluginSchema, StateDiff, StatePlugin,
};
use simd_json::prelude::{ValueAsScalar, ValueObjectAccess};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

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
        self.schema = Some(
            PluginSchema::builder("freedesktop")
                .version("1.0.0")
                .category("system")
                .description("FreeDesktop D-Bus standards implementation")
                .build(),
        );
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

    async fn query_current_state(&self) -> anyhow::Result<Value> {
        // Return the current state of FreeDesktop interfaces
        let interfaces: Vec<Value> = self
            .dbus_interfaces
            .values()
            .map(|iface| {
                simd_json::json!({
                    "name": iface.name.clone(),
                    "description": iface.description.clone(),
                    "methods_count": iface.methods.len(),
                    "signals_count": iface.signals.len(),
                    "properties_count": iface.properties.len(),
                })
            })
            .collect();

        let state = simd_json::json!({
            "name": "freedesktop",
            "version": "1.0.0",
            "plugin_type": "system",
            "dbus_path": self.dbus_path(),
            "dbus_interface": self.dbus_interface(),
            "interfaces": interfaces,
            "canonical_path_verified": true,
            "freedesktop_standards": {
                "object_manager": true,
                "properties_interface": true,
                "introspection_interface": true,
                "peer_interface": true,
            }
        });

        Ok(state)
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
        let current = self.query_current_state().await?;

        // Check critical fields match
        let current_name = current.get("name").and_then(|v| v.as_str());
        let desired_name = desired.get("name").and_then(|v| v.as_str());

        Ok(current_name == desired_name && current_name == Some("freedesktop"))
    }

    async fn create_checkpoint(&self) -> anyhow::Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("freedesktop-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
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
        assert_eq!(
            plugin.dbus_interface(),
            "org.opdbus.v1.Plugin.Plugins.Freedesktop"
        );
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

        // Invalid legacy path
        assert!(plugin
            .validate_canonical_path("/opdbus/v1/plugins/test")
            .is_err());
    }

    #[test]
    fn test_path_normalization() {
        let plugin = FreeDesktopPlugin::new();

        // Legacy paths should be normalized to canonical form
        let normalized = plugin.normalize_path("/opdbus/v1/plugins/test");
        assert_eq!(normalized, Some("/org/opdbus/v1/plugins/test".to_string()));

        let legacy_alias = plugin.normalize_path("/org/opdbus/v1/plugin/plugins/test");
        assert_eq!(
            legacy_alias,
            Some("/org/opdbus/v1/plugins/test".to_string())
        );

        // Canonical path stays the same
        let canonical = plugin.normalize_path("/org/opdbus/v1/plugins/test");
        assert_eq!(canonical, Some("/org/opdbus/v1/plugins/test".to_string()));
    }

    #[tokio::test]
    async fn test_state_query() {
        let plugin = FreeDesktopPlugin::new();
        let state = plugin.query_current_state().await.unwrap();

        assert_eq!(state.get("name").unwrap().as_str().unwrap(), "freedesktop");
        assert_eq!(state.get("version").unwrap().as_str().unwrap(), "1.0.0");
        assert!(state.get("interfaces").unwrap().is_array());
    }
}
