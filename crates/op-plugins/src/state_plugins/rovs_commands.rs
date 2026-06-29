//! rovs_commands Plugin — Direct OVSDB Command Execution via D-Bus
//!
//! Schema-only plugin defining callable methods for OVSDB operations.
//! The grpc-bridge exposes these as D-Bus objects at `/org/opdbus/v1/plugins/rovs_commands`
//! that can be called directly via zbus/busctl.
//!
//! Per AGENTS.md §4: D-Bus is the ONLY control plane. Methods execute OVSDB transact
//! calls directly through the rovs proxy.
//!
//! Method input types use schemars::JsonSchema derive - this is the single source
//! of truth for both JSON Schema (exposed in PluginSchema.methods.args) and
//! gRPC proto generation [via `proto-from-methods` binary at build time].

use op_state_store::{FieldSchema, FieldType, MethodDecl, PluginSchema, SideEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

// =============================================================================
// Method input types - single source of truth via schemars
// =============================================================================

/// create_bridge method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateBridgeInput {
    /// Name of the bridge to create
    pub bridge_name: String,
    /// Datapath type (e.g. "system", "netdev")
    #[serde(default = "default_datapath_type")]
    pub datapath_type: String,
}

fn default_datapath_type() -> String {
    "system".to_string()
}

/// delete_bridge method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteBridgeInput {
    /// Name of the bridge to delete
    pub bridge_name: String,
}

/// add_port method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPortInput {
    /// Parent bridge name
    pub bridge_name: String,
    /// Port name to add
    pub port_name: String,
    /// Interface type (e.g. "internal", "system")
    #[serde(default = "default_interface_type")]
    pub interface_type: String,
}

fn default_interface_type() -> String {
    "internal".to_string()
}

/// remove_port method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemovePortInput {
    /// Bridge name containing the port (optional for backward compat)
    #[serde(default)]
    pub bridge_name: String,
    /// Port name to remove
    pub port_name: String,
}

/// list_ports method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListPortsInput {
    /// Bridge name to query
    pub bridge_name: String,
}

// =============================================================================
// Plugin implementation
// =============================================================================

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

    pub fn current_state() -> Value {
        simd_json::json!({
            "available": true,
            "schema_version": "1.0.0",
        })
    }

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

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> anyhow::Result<op_state::StateDiff> {
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

    async fn apply_state(&self, _diff: &op_state::StateDiff) -> anyhow::Result<op_state::ApplyResult> {
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
            state_snapshot: simd_json::json!(null),
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
    let mut methods = HashMap::new();
    methods.insert("create_bridge".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<CreateBridgeInput>(
        "create_bridge", SideEffect::Mutation, false,
        "cap.network.ovsdb.bridge.create@v1", "mut.network.ovsdb.bridge.create@v1"));
    methods.insert("delete_bridge".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<DeleteBridgeInput>(
        "delete_bridge", SideEffect::Mutation, false,
        "cap.network.ovsdb.bridge.delete@v1", "mut.network.ovsdb.bridge.delete@v1"));
    methods.insert("add_port".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<AddPortInput>(
        "add_port", SideEffect::Mutation, false,
        "cap.network.ovsdb.port.add@v1", "mut.network.ovsdb.port.add@v1"));
    methods.insert("remove_port".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<RemovePortInput>(
        "remove_port", SideEffect::Mutation, false,
        "cap.network.ovsdb.port.delete@v1", "mut.network.ovsdb.port.delete@v1"));
    // Use unit type () for methods with no arguments
    methods.insert("list_bridges".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<()>(
        "list_bridges", SideEffect::Read, true,
        "cap.network.ovsdb.bridge.list@v1", "obs.network.ovsdb.bridge.list@v1"));
    methods.insert("list_ports".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<ListPortsInput>(
        "list_ports", SideEffect::Read, true,
        "cap.network.ovsdb.port.list@v1", "obs.network.ovsdb.port.list@v1"));
    methods.insert("list_dbs".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<()>(
        "list_dbs", SideEffect::Read, true,
        "cap.network.ovsdb.db.list@v1", "obs.network.ovsdb.db.list@v1"));

    PluginSchema::builder("rovs_commands")
        .version("1.0.0")
        .category("network")
        .description("Direct OVSDB command execution methods exposed via D-Bus by grpc-bridge")
        .dependency("net")
        .field("available", FieldSchema {
            field_type: FieldType::Boolean,
            required: true,
            description: "Plugin is available".to_string(),
            default: Some(json!(true)),
            example: Some(json!(true)),
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("schema_version", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Schema version".to_string(),
            default: Some(json!("1.0.0")),
            example: Some(json!("1.0.0")),
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .methods(methods)
        .build()
}

inventory::submit! {
    crate::default_registry::PluginReg::new("rovs_commands", |_ctx| std::sync::Arc::new(RovsCommandsPlugin::new()))
}
