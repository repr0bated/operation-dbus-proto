//! rovs_commands Plugin — Command Definitions Schema
//!
//! Schema-only plugin (read-only StatePlugin — no apply_state mutations).
//! Serves as the source of truth for available OVS/rovs commands
//! and their D-Bus method mappings.
//!
//! Per AGENTS.md §4: D-Bus is the ONLY control plane. This plugin
//! defines the schema for commands that are executed via D-Bus
//! at `/org/opdbus/v1/plugins/rovs_commands`.
//!
//! Projected at: `/org/opdbus/v1/plugins/rovs_commands`

use op_state_store::{FieldSchema, FieldType, PluginSchema};
use simd_json::json;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// rovs_commands Plugin — Command definitions schema
///
/// This plugin exposes the schema of available OVS commands.
/// Commands are executed via D-Bus methods on the OVSDB daemon.
/// Implements StatePlugin as a read-only schema provider.
pub struct RovsCommandsPlugin;

impl Default for RovsCommandsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl RovsCommandsPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Get the current state of available commands
    pub fn current_state() -> Value {
        simd_json::json!({
            "commands": [
                {
                    "name": "create-bridge",
                    "description": "Create a new OVS bridge",
                    "dbus_method": "org.opdbus.v1.Ovsdb.JsonRpc.transact",
                    "args": ["bridge_name", "datapath_type"],
                },
                {
                    "name": "delete-bridge",
                    "description": "Delete an existing OVS bridge",
                    "dbus_method": "org.opdbus.v1.Ovsdb.JsonRpc.transact",
                    "args": ["bridge_name"],
                },
                {
                    "name": "add-port",
                    "description": "Add a port to an OVS bridge",
                    "dbus_method": "org.opdbus.v1.Ovsdb.JsonRpc.transact",
                    "args": ["bridge_name", "port_name", "interface_type"],
                },
                {
                    "name": "remove-port",
                    "description": "Remove a port from an OVS bridge",
                    "dbus_method": "org.opdbus.v1.Ovsdb.JsonRpc.transact",
                    "args": ["port_name"],
                },
                {
                    "name": "list-bridges",
                    "description": "List all OVS bridges",
                    "dbus_method": "org.opdbus.v1.Ovsdb.JsonRpc.transact",
                    "args": [],
                },
                {
                    "name": "list-ports",
                    "description": "List ports on a bridge",
                    "dbus_method": "org.opdbus.v1.Ovsdb.JsonRpc.transact",
                    "args": ["bridge_name"],
                },
                {
                    "name": "list-dbs",
                    "description": "List OVSDB databases",
                    "dbus_method": "org.opdbus.v1.Ovsdb.JsonRpc.transact",
                    "args": [],
                },
            ],
            "dbus_bus_name": "org.opdbus.v1.plugins.ovsdb",
            "dbus_object_path": "/org/opdbus/v1/plugins/ovsdb",
            "schema_version": "1.0.0",
        })
    }

    /// Return the plugin schema
    pub fn schema() -> Option<PluginSchema> {
        Some(rovs_commands_schema())
    }
}

#[async_trait::async_trait]
impl op_state::StatePlugin for RovsCommandsPlugin {
    fn name(&self) -> &str {
        "rovs_commands"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Self::schema()
    }

    async fn query_current_state(&self) -> anyhow::Result<Value> {
        Ok(Self::current_state())
    }

    async fn calculate_diff(
        &self,
        _current: &Value,
        _desired: &Value,
    ) -> anyhow::Result<op_state::StateDiff> {
        // Schema-only plugin: no state to apply
        Ok(op_state::StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: op_state::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-only".to_string(),
                desired_hash: "schema-only".to_string(),
            },
        })
    }

    async fn apply_state(
        &self,
        _diff: &op_state::StateDiff,
    ) -> anyhow::Result<op_state::ApplyResult> {
        // Schema-only plugin: no state to apply
        Ok(op_state::ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> anyhow::Result<op_state::Checkpoint> {
        Ok(op_state::Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Self::current_state(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &op_state::Checkpoint) -> anyhow::Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> op_state::PluginCapabilities {
        op_state::PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }
}

pub(crate) fn rovs_commands_schema() -> PluginSchema {
    PluginSchema::builder("rovs_commands")
        .version("1.0.0")
        .category("network")
        .description("rovs command definitions — schema source of truth for OVS bridge/port/flow operations via D-Bus")
        .dependency("net")
        .array_field(
            "commands",
            FieldType::Object(HashMap::from([
                ("name".to_string(), FieldSchema {
                    field_type: FieldType::String,
                    required: true,
                    description: "Command name".to_string(),
                    default: None,
                    example: Some(json!("create-bridge")),
                    constraints: Vec::new(),
                    read_only: true,
                    read_only_when: None,
                }),
                ("description".to_string(), FieldSchema {
                    field_type: FieldType::String,
                    required: true,
                    description: "Human-readable description".to_string(),
                    default: None,
                    example: Some(json!("Create a new OVS bridge")),
                    constraints: Vec::new(),
                    read_only: true,
                    read_only_when: None,
                }),
                ("dbus_method".to_string(), FieldSchema {
                    field_type: FieldType::String,
                    required: true,
                    description: "D-Bus method path for this command".to_string(),
                    default: None,
                    example: Some(json!("org.opdbus.v1.Ovsdb.JsonRpc.transact")),
                    constraints: Vec::new(),
                    read_only: true,
                    read_only_when: None,
                }),
            ])),
            true,
            "Available OVS/rovs commands exposed via D-Bus",
        )
        .build()
}
