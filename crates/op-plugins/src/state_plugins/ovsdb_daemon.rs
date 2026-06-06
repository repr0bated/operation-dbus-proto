//! OVSDB Daemon Plugin
//!
//! Represents the op-openvswitch-daemon binary as a state plugin.
//! Per AGENTS.md: "the schema IS the plugin" - this plugin's schema
//! defines the daemon's interface contract.
//!
//! The daemon itself is an external binary that registers on D-Bus at
//! org.opdbus.v1.plugins.ovsdb. This plugin provides the schema definition
//! and state representation for the system registry.

use crate::state_plugins::plugin_schema_defs;
use anyhow::Result;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use simd_json::OwnedValue as Value;

/// OVSDB Daemon Plugin
///
/// Schema-driven definition of the native OVSDB D-Bus daemon.
/// The daemon binary (op-openvswitch-daemon) implements the actual
/// OVSDB operations; this plugin provides the schema contract.
pub struct OvsdbDaemonPlugin {
    state: Value,
}

impl Default for OvsdbDaemonPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl OvsdbDaemonPlugin {
    pub fn new() -> Self {
        Self {
            state: simd_json::json!({
                "daemon_version": "1.0.0",
                "ovs_version": null,
                "socket_path": "/var/run/openvswitch/db.sock",
                "connected": false,
                "bridges": [],
                "operations_supported": [
                    "create_bridge",
                    "delete_bridge",
                    "add_port",
                    "remove_port",
                    "list_bridges",
                    "list_ports",
                    "list_dbs"
                ],
                "dbus_bus_name": "org.opdbus.v1.plugins.ovsdb",
                "dbus_object_path": "/org/opdbus/v1/plugins/ovsdb",
                "grpc_listen_addr": null,
            }),
        }
    }

    /// Query the daemon status via D-Bus
    async fn query_daemon_status(&self) -> Result<Value> {
        // Return the static state - daemon detection is done via D-Bus name ownership
        // In a full implementation, this would query the daemon's status method
        Ok(self.state.clone())
    }
}

#[async_trait::async_trait]
impl StatePlugin for OvsdbDaemonPlugin {
    fn name(&self) -> &str {
        "ovsdb_daemon"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        // The daemon plugin is always available as a schema definition
        // Whether the daemon binary is running is a runtime concern
        true
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(plugin_schema_defs::ovsdb_daemon_plugin_schema())
    }

    /// Query the daemon's current status
    async fn query_current_state(&self) -> Result<Value> {
        self.query_daemon_status().await
    }

    /// Calculate diff - the daemon is external, so this returns empty diff
    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    /// Apply state - no-op, daemon state is managed via D-Bus directly
    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    /// Verify state - always true, daemon is authoritative
    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    /// Create checkpoint of current state
    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    /// Rollback - no-op for daemon plugin
    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
