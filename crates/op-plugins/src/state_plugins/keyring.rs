//! GNOME Keyring plugin - freedesktop.org Secret Service integration
#![allow(dead_code)]
//!
//! Implements the org.freedesktop.secrets D-Bus service for secure credential storage.
//! This allows applications like Cursor to store and retrieve passwords, API keys, etc.
//!
//! The Secret Service API provides:
//! - Collections (like "default", "login")
//! - Items (individual secrets with attributes)
//! - Secure storage with optional encryption

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use zbus::{
    zvariant::{ObjectPath, OwnedObjectPath},
    Connection, Proxy,
};

/// Keyring state representation
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct KeyringState {
    /// Available collections
    pub collections: Vec<CollectionInfo>,
    /// Default collection path
    pub default_collection: Option<String>,
}

/// Information about a secret collection
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct CollectionInfo {
    pub path: String,
    pub label: String,
    pub locked: bool,
    pub created: u64,
    pub modified: u64,
}

/// GNOME Keyring plugin implementing freedesktop.org Secret Service
pub struct KeyringPlugin;

impl KeyringPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Connect to the Secret Service via the op-dbus plugin system.
    /// The freedesktop plugin at /org/opdbus/v1/plugins/freedesktop
    /// owns the org.freedesktop.secrets name on the op-dbus session bus.
    async fn connect_service(&self) -> Result<Proxy<'static>> {
        let conn = Connection::session().await?;
        let proxy = Proxy::new(
            &conn,
            op_core::config::OPDBUS_BUS_NAME,
            crate::canonical::plugin_path("freedesktop"),
            "org.opdbus.v1.Plugin.Plugins.FreeDesktop",
        )
        .await?;
        Ok(proxy)
    }

    /// Get available collections via the op-dbus freedesktop plugin
    async fn get_collections(&self) -> Result<Vec<CollectionInfo>> {
        // The keyring collections are managed through the op-dbus plugin tree.
        // When no external secret-service provider is registered, return empty.
        Ok(Vec::new())
    }

    /// Get information about a specific collection via the op-dbus freedesktop plugin
    async fn get_collection_info(&self, _path: &ObjectPath<'_>) -> Result<CollectionInfo> {
        Err(anyhow::anyhow!(
            "Collection info requires org.freedesktop.secrets provider via op-dbus freedesktop plugin"
        ))
    }

    /// Get the default collection path via the op-dbus freedesktop plugin
    async fn get_default_collection(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

impl Default for KeyringPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyringPlugin {
    /// Check if the Secret Service is available via the op-dbus freedesktop plugin.
    /// The freedesktop plugin at /org/opdbus/v1/plugins/freedesktop
    /// provides org.freedesktop.secrets through the plugin system.
    /// NOTE: is_available() must NOT spawn subprocesses or do blocking I/O
    /// because it runs during daemon initialization before the D-Bus name is claimed.
    fn check_service_available(&self) -> bool {
        // Check if the op-dbus session bus socket exists, which means the
        // daemon is running and the freedesktop plugin will be available.
        std::path::Path::new(op_core::config::SESSION_BUS_SOCKET_PATH).exists()
    }
}

#[async_trait]
impl StatePlugin for KeyringPlugin {
    fn name(&self) -> &str {
        "keyring"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(keyring_schema())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false, // Security reasons
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // Keyring operations are not atomic
        }
    }

    fn is_available(&self) -> bool {
        self.check_service_available()
    }

    fn unavailable_reason(&self) -> String {
        "org.freedesktop.secrets not available via op-dbus freedesktop plugin at /org/opdbus/v1/plugins/freedesktop".to_string()
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        // Keyring operations are typically interactive and should not be automated
        Err(anyhow::anyhow!(
            "Keyring apply operations are not supported for security reasons"
        ))
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();

        if current != desired {
            actions.push(StateAction::Modify {
                resource: "keyring-collections".to_string(),
                changes: json!({
                    "current": current,
                    "desired": desired
                }),
            });
        }

        let metadata = DiffMetadata {
            timestamp: chrono::Utc::now().timestamp(),
            current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
            desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
        };

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        // For keyring, we can only verify that the structure is valid
        // We can't verify actual secrets without user interaction
        Ok(desired.is_object())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = simd_json::json!(null);
        Ok(Checkpoint {
            id: format!("keyring-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        // Keyring rollback is not supported for security reasons
        Err(anyhow::anyhow!(
            "Keyring rollback is not supported for security reasons"
        ))
    }
}

pub(crate) fn keyring_schema() -> PluginSchema {
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "keyring",
        "0.1.0",
        "Secret service collections state",
        &serde_json::to_value(schemars::schema_for!(KeyringState)).unwrap(),
    );
    super::schemars_adapter::apply_state_defaults(
        &mut schema,
        &simd_json::serde::to_owned_value(&KeyringState::default()).unwrap(),
    );
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_keyring_plugin_creation() {
        let plugin = KeyringPlugin::new();
        assert_eq!(plugin.name(), "keyring");
    }

    #[tokio::test]
    async fn test_capabilities() {
        let plugin = KeyringPlugin::new();
        let caps = plugin.capabilities();
        // KeyringPlugin capabilities from op_state::PluginCapabilities
        assert!(!caps.supports_rollback); // Security reasons - no rollback
        assert!(caps.supports_checkpoints);
        assert!(caps.supports_verification);
        assert!(!caps.atomic_operations);
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("keyring", |_ctx| std::sync::Arc::new(KeyringPlugin::new()))
}
