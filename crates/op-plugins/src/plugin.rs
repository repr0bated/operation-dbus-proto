/// Core plugin trait and types
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use simd_json::ValueBuilder;
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::state::{DesiredState, StateChange, ValidationResult};
use op_core::state_publisher::StatePublisher;
pub use op_state::plugin::PluginMetadata;

/// Context provided to plugin during initialization
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Optional state publisher for authoritative updates
    pub publisher: Option<std::sync::Arc<dyn StatePublisher>>,
    /// Dedicated BTRFS subvolume path for this plugin's storage
    pub storage_path: PathBuf,
    /// Assigned NUMA node (if available)
    pub numa_node: Option<u32>,
    /// Plugin configuration
    pub config: Value,
}

impl Default for PluginContext {
    fn default() -> Self {
        Self {
            publisher: None,
            storage_path: PathBuf::from("/var/lib/op-dbus/plugins/default"),
            numa_node: None,
            config: Value::null(),
        }
    }
}

/// Plugin tunable parameters (runtime configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTunables {
    pub priority: i32,
    pub max_retries: u32,
    pub timeout_ms: u64,
    pub enabled: bool,
    #[serde(default)]
    pub config: Value,
}

impl Default for PluginTunables {
    fn default() -> Self {
        Self {
            priority: 0,
            max_retries: 3,
            timeout_ms: 30000,
            enabled: true,
            config: Value::null(),
        }
    }
}

/// Plugin capabilities
///
/// Re-exported from `op_state_store` — the single canonical definition.
/// The previous 8-field struct has been replaced by the 4-field guarantee
/// block (`supports_rollback`, `supports_checkpoints`, `supports_verification`,
/// `atomic_operations`).
pub use op_state_store::PluginCapabilities;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSchema {
    pub feature_type: String,
    pub version: String,
    pub config: Value,
    /// Capability tags (e.g. "immutable", "core", "optional")
    #[serde(default)]
    pub tags: Vec<String>,
    /// Specific JSON configuration paths that are immutable (e.g. ["/metadata/id"])
    #[serde(default)]
    pub immutable_paths: Vec<String>,
}

impl FeatureSchema {
    pub fn is_fully_immutable(&self) -> bool {
        self.tags.iter().any(|t| t == "immutable")
    }

    pub fn is_path_immutable(&self, path: &str) -> bool {
        self.is_fully_immutable() || self.immutable_paths.iter().any(|p| p == path)
    }
}

/// Core plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Unique name for this plugin
    fn name(&self) -> &str;

    /// Description of what this plugin does
    fn description(&self) -> &str;

    /// Version of the plugin
    fn version(&self) -> &str;

    /// Get the current state managed by this plugin
    async fn get_state(&self) -> Result<Value>;

    /// Get the desired state (target configuration)
    async fn get_desired_state(&self) -> Result<DesiredState>;

    /// Set the desired state
    async fn set_desired_state(&self, desired: DesiredState) -> Result<()>;

    /// Apply the desired state (reconcile current -> desired)
    async fn apply_state(&self) -> Result<Vec<StateChange>>;

    /// Calculate diff between current and desired state
    async fn diff(&self) -> Result<Vec<StateChange>>;

    /// Validate a configuration before applying
    async fn validate(&self, config: &Value) -> Result<ValidationResult>;

    /// Get plugin capabilities
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::default()
    }

    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: self.description().to_string(),
            author: None,
            license: None,
            dependencies: vec![],
            dbus_services: vec![],
            feature_schemas: vec![],
            object_schemas: HashMap::new(),
        }
    }

    /// Handle plugin-specific commands
    async fn handle_command(&self, command: &str, _args: Value) -> Result<Value> {
        Err(anyhow::anyhow!(
            "Command '{}' not supported by plugin '{}'",
            command,
            self.name()
        ))
    }

    /// Initialize the plugin with context
    async fn initialize(&mut self, _context: PluginContext) -> Result<()> {
        Ok(())
    }

    /// Cleanup when plugin is being removed
    async fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }

    /// Get hash of current state for blockchain footprint
    fn state_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        // Default implementation - plugins should override for accuracy
        let mut hasher = Sha256::new();
        hasher.update(self.name().as_bytes());
        hasher.update(self.version().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Boxed plugin type
pub type BoxedPlugin = Box<dyn Plugin>;

#[cfg(test)]
mod tests {
    use super::PluginCapabilities as ReExportedCaps;
    use op_state_store::PluginCapabilities as CanonicalCaps;

    /// VAL-DEDUP-003: op-plugins must not define its own PluginCapabilities
    /// type. The re-exported type must be the exact same type as the canonical
    /// one in op_state_store (type identity, not a duplicate definition).
    #[test]
    fn plugin_capabilities_is_reexport_from_op_state_store() {
        use std::any::TypeId;
        assert_eq!(
            TypeId::of::<ReExportedCaps>(),
            TypeId::of::<CanonicalCaps>(),
            "op_plugins::PluginCapabilities must be the same type as \
             op_state_store::PluginCapabilities, not a separate struct"
        );
    }

    /// VAL-DEDUP-004: callers that `use op_plugins::PluginCapabilities` are
    /// transitively importing from op_state_store.  The 4-field guarantee
    /// block must be the one they get.
    #[test]
    fn reexported_plugin_capabilities_has_four_bool_fields() {
        let caps = ReExportedCaps::default();
        assert!(!caps.supports_rollback);
        assert!(!caps.supports_checkpoints);
        assert!(!caps.supports_verification);
        assert!(!caps.atomic_operations);
    }

    /// The serialized form must contain exactly the four canonical keys.
    #[test]
    fn reexported_plugin_capabilities_serializes_four_keys() {
        let caps = ReExportedCaps {
            supports_rollback: true,
            supports_checkpoints: false,
            supports_verification: true,
            atomic_operations: false,
        };
        let json = serde_json::to_value(&caps).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 4);
        assert_eq!(obj["supports_rollback"], serde_json::json!(true));
        assert_eq!(obj["supports_checkpoints"], serde_json::json!(false));
        assert_eq!(obj["supports_verification"], serde_json::json!(true));
        assert_eq!(obj["atomic_operations"], serde_json::json!(false));
    }

    /// The old 8-field access-control fields (can_read, can_write, can_delete,
    /// requires_root, supported_platforms) must not be present in the
    /// serialized form.  They are superseded by MethodDecl.required_capability.
    #[test]
    fn reexported_plugin_capabilities_has_no_access_control_fields() {
        let caps = ReExportedCaps::default();
        let json = serde_json::to_string(&caps).unwrap();
        assert!(
            !json.contains("can_read"),
            "can_read must not exist in PluginCapabilities"
        );
        assert!(
            !json.contains("can_write"),
            "can_write must not exist in PluginCapabilities"
        );
        assert!(
            !json.contains("can_delete"),
            "can_delete must not exist in PluginCapabilities"
        );
        assert!(
            !json.contains("requires_root"),
            "requires_root must not exist in PluginCapabilities"
        );
        assert!(
            !json.contains("supported_platforms"),
            "supported_platforms must not exist in PluginCapabilities"
        );
    }
}
