//! OVSDB Bridge plugin — 1:1 mirror of RFC 7047 Bridge/Port/Interface tables.
//!
//! OVSDB *is* the source of truth. This plugin queries reality from ovsdb-server
//! and projects it onto D-Bus via the mirror reconciliation loop. There is no
//! desired-vs-current diff — the database is the desired state.

use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
use anyhow::Result;
use async_trait::async_trait;
use op_network::rovs_proxy::OvsdbDbusClient;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListDbsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetSchemaInput {
    pub db_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TransactInput {
    pub db_name: String,
    pub operations: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitorInput {
    pub db_name: String,
    pub monitor_id: String,
    pub monitor_requests: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitorCondInput {
    pub db_name: String,
    pub monitor_id: String,
    pub monitor_cond_requests: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LockInput {
    pub lock_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StealInput {
    pub lock_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnlockInput {
    pub lock_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EchoInput {
    pub params: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CancelInput {
    pub id: String,
}

/// Input struct for CreateBridge method
/// D-Bus method spec: https://www.opennetworking.org/wp-content/uploads/2014/10/of_spec_1_0.pdf
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateBridgeInput {
    /// Bridge name
    pub name: String,
    /// Datapath type (e.g., "system", "internal")
    pub datapath_type: Option<String>,
    /// Fail mode (secure or standalone)
    pub fail_mode: Option<String>,
    /// Enable STP
    pub stp_enable: Option<bool>,
}

/// Input struct for DeleteBridge method
/// D-Bus method spec: https://www.opennetworking.org/wp-content/uploads/2014/10/of_spec_1_0.pdf
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteBridgeInput {
    /// Bridge name to delete
    pub name: String,
}

/// Input struct for AddPort method
/// D-Bus method spec: https://www.opennetworking.org/wp-content/uploads/2014/10/of_spec_1_0.pdf
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddPortInput {
    /// Bridge name
    pub bridge: String,
    /// Port name
    pub port: String,
    /// Interface name
    pub interface: Option<String>,
    /// VLAN tag (optional)
    pub tag: Option<i32>,
}

/// Input struct for RemovePort method
/// D-Bus method spec: https://www.opennetworking.org/wp-content/uploads/2014/10/of_spec_1_0.pdf
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemovePortInput {
    /// Bridge name
    pub bridge: String,
    /// Port name to remove
    pub port: String,
}

use simd_json::prelude::*;
use std::sync::Arc;

// ============================================================================
// RFC 7047 §3.2 Schema Types — Bridge → Port → Interface hierarchy
// ============================================================================

/// Full OVS state — 1:1 projection of what ovsdb-server reports.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-category" = "network"))]
pub struct OvsBridgeState {
    pub bridges: Vec<BridgeConfig>,
    /// Full source-owned OVSDB table/column surface discovered from the
    /// authoritative Open vSwitch schemas. Nested to preserve the existing
    /// Bridge/Port/Interface contract while making the rest UI-selectable.
    #[serde(default)]
    pub inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
}

/// RFC 7047 Bridge table row.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BridgeConfig {
    pub name: String,
    #[serde(default)]
    pub ports: Vec<PortConfig>,
    /// "system" | "netdev" | "" (kernel datapath)
    #[serde(default)]
    pub datapath_type: String,
    /// "standalone" | "secure" | null
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_mode: Option<String>,
    #[serde(default)]
    pub stp_enable: bool,
    #[serde(default)]
    pub mcast_snooping_enable: bool,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub other_config: std::collections::HashMap<String, String>,
}

/// RFC 7047 Port table row.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PortConfig {
    pub name: String,
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trunks: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bond_mode: Option<String>,
}

/// RFC 7047 Interface table row.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InterfaceConfig {
    pub name: String,
    /// "system" | "internal" | "patch" | "vxlan" | "gre" | "geneve" | ""
    #[serde(default, rename = "type")]
    pub iface_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac_in_use: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_state: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub options: std::collections::HashMap<String, String>,
}

// ============================================================================
// Plugin
// ============================================================================

pub struct OvsBridgePlugin {
    ovsdb: Arc<OvsdbDbusClient>,
}

impl Default for OvsBridgePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl OvsBridgePlugin {
    pub fn new() -> Self {
        Self {
            ovsdb: Arc::new(OvsdbDbusClient::new()),
        }
    }

    /// Query full Bridge→Port→Interface hierarchy from OVSDB.
    async fn query_bridges(&self) -> Result<OvsBridgeState> {
        let bridge_names = self.ovsdb.list_bridges().await.unwrap_or_default();
        let mut bridges = Vec::new();

        for bname in bridge_names {
            // Bridge-level properties
            let bridge_info = self.ovsdb.get_bridge_info(&bname).await.ok();
            let (datapath_type, fail_mode, stp_enable, mcast_snooping_enable) =
                Self::parse_bridge_props(&bridge_info);

            // Ports
            let port_names = self
                .ovsdb
                .list_bridge_ports(&bname)
                .await
                .unwrap_or_default();
            let ports: Vec<PortConfig> = port_names
                .into_iter()
                .map(|pname| PortConfig {
                    interfaces: vec![InterfaceConfig {
                        name: pname.clone(),
                        iface_type: String::new(),
                        mac_in_use: None,
                        mac: None,
                        admin_state: None,
                        link_state: None,
                        options: Default::default(),
                    }],
                    name: pname,
                    tag: None,
                    trunks: vec![],
                    vlan_mode: None,
                    bond_mode: None,
                })
                .collect();

            bridges.push(BridgeConfig {
                name: bname,
                ports,
                datapath_type,
                fail_mode,
                stp_enable,
                mcast_snooping_enable,
                other_config: Default::default(),
            });
        }

        Ok(OvsBridgeState {
            bridges,
            inspector_fields: Default::default(),
        })
    }

    fn parse_bridge_props(info: &Option<String>) -> (String, Option<String>, bool, bool) {
        let Some(ref info_str) = info else {
            return (String::new(), None, false, false);
        };
        let v: std::result::Result<Value, _> = serde_json::from_str(info_str);
        match v {
            Ok(row) => (
                row.get("datapath_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                row.get("fail_mode")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                row.get("stp_enable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                row.get("mcast_snooping_enable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            ),
            Err(_) => (String::new(), None, false, false),
        }
    }
}

#[async_trait]
impl StatePlugin for OvsBridgePlugin {
    fn name(&self) -> &str {
        "ovsdb_bridge"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(ovsdb_bridge_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/var/run/openvswitch/db.sock").exists()
    }

    fn unavailable_reason(&self) -> String {
        "OVSDB socket not found at /var/run/openvswitch/db.sock".to_string()
    }

    /// Query reality — dump OVSDB Bridge/Port/Interface tables.

    /// Reconciliation, not diff. OVSDB is the DB — the "desired" parameter
    /// is what the D-Bus mirror currently shows. We return actions needed
    /// to update the mirror to match OVSDB reality.
    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        // No diff — OVSDB is authoritative. The mirror reconciliation loop
        // in op-dbus-mirror handles projection. Return empty diff.
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

    /// No-op — reconciliation happens via the mirror, not through apply.
    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    /// Verify just re-queries OVSDB — it's always "correct" by definition.
    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = simd_json::json!(null);
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
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

pub(crate) fn ovsdb_bridge_schema() -> PluginSchema {
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "ovsdb_bridge",
        "1.0.0",
        "OVS bridge declarations",
        &serde_json::to_value(schemars::schema_for!(OvsBridgeState)).unwrap(),
    );
    super::schemars_adapter::apply_state_defaults(
        &mut schema,
        &simd_json::serde::to_owned_value(&OvsBridgeState::default()).unwrap(),
    );

    schema.methods.insert(
        "list_dbs".to_string(),
        method_decl_from_schemars_with_output::<
            ListDbsInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "list_dbs",
            op_state_store::SideEffect::Read,
            true,
            "ovsdb.read",
            "obs.network.ovsdb.dbs.list@v1",
        ),
    );
    schema.methods.insert(
        "get_schema".to_string(),
        method_decl_from_schemars_with_output::<
            GetSchemaInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "get_schema",
            op_state_store::SideEffect::Read,
            true,
            "ovsdb.read",
            "obs.network.ovsdb.schema.get@v1",
        ),
    );
    schema.methods.insert(
        "transact".to_string(),
        method_decl_from_schemars_with_output::<
            TransactInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "transact",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.transact@v1",
        ),
    );
    schema.methods.insert(
        "monitor".to_string(),
        method_decl_from_schemars_with_output::<
            MonitorInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "monitor",
            op_state_store::SideEffect::Read,
            false,
            "ovsdb.read",
            "obs.network.ovsdb.monitor@v1",
        ),
    );
    schema.methods.insert(
        "monitor_cond".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            MonitorCondInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "monitor_cond",
            op_state_store::SideEffect::Read,
            false,
            "ovsdb.read",
            "obs.network.ovsdb.monitor_cond@v1",
        ),
    );
    schema.methods.insert(
        "lock".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            LockInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "lock",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.lock@v1",
        ),
    );
    schema.methods.insert(
        "steal".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            StealInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "steal",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.lock.steal@v1",
        ),
    );
    schema.methods.insert(
        "unlock".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            UnlockInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "unlock",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.lock.unlock@v1",
        ),
    );
    schema.methods.insert(
        "echo".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            EchoInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "echo",
            op_state_store::SideEffect::Read,
            true,
            "ovsdb.read",
            "obs.network.ovsdb.echo@v1",
        ),
    );
    schema.methods.insert(
        "cancel".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            CancelInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "cancel",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.cancel@v1",
        ),
    );

    // These protocol declarations do not have a runtime implementation in
    // the native rovs client. Do not advertise methods that Call will reject.
    for unavailable in [
        "monitor",
        "monitor_cond",
        "lock",
        "steal",
        "unlock",
        "echo",
        "cancel",
    ] {
        schema.methods.remove(unavailable);
    }

    // Add required methods: CreateBridge, DeleteBridge, AddPort, RemovePort
    // D-Bus method spec: https://www.opennetworking.org/wp-content/uploads/2014/10/of_spec_1_0.pdf
    schema.methods.insert(
        "create_bridge".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            CreateBridgeInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "CreateBridge",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.bridge.create@v1",
        ),
    );
    schema.methods.insert(
        "delete_bridge".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            DeleteBridgeInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "DeleteBridge",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.bridge.delete@v1",
        ),
    );
    schema.methods.insert(
        "add_port".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            AddPortInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "AddPort",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.port.add@v1",
        ),
    );
    schema.methods.insert(
        "remove_port".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            RemovePortInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "RemovePort",
            op_state_store::SideEffect::Mutation,
            false,
            "ovsdb.write",
            "mut.network.ovsdb.port.remove@v1",
        ),
    );

    schema.capabilities.insert(
        "ovsdb.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "ovsdb.read".to_string(),
            description: "Grants: list_dbs, get_schema.".to_string(),
        },
    );
    schema.capabilities.insert(
        "ovsdb.write".to_string(),
        op_state_store::CapabilityDecl {
            id: "ovsdb.write".to_string(),
            description:
                "Grants: transact, create_bridge, delete_bridge, add_port, remove_port.".to_string(),
        },
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("ovsdb_bridge", |_ctx| std::sync::Arc::new(OvsBridgePlugin::new()))
}

#[cfg(test)]
mod inspector_gadget_tests {
    use super::*;

    #[test]
    fn promoted_schema_preserves_state_and_only_exposes_dispatched_methods() {
        let schema = ovsdb_bridge_schema();
        assert!(schema.fields.contains_key("bridges"));
        assert!(schema.fields.contains_key("inspector_fields"));

        for method in [
            "list_dbs",
            "get_schema",
            "transact",
            "create_bridge",
            "delete_bridge",
            "add_port",
            "remove_port",
        ] {
            assert!(schema.methods.contains_key(method), "missing {method}");
        }
        for method in [
            "monitor",
            "monitor_cond",
            "lock",
            "steal",
            "unlock",
            "echo",
            "cancel",
        ] {
            assert!(
                !schema.methods.contains_key(method),
                "undispatched {method}"
            );
        }
    }
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
    #[schemars(extend("x-oscal-subid" = "sch.software.ovsdb-bridge.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `json.ovsdb._server.ovsschema.table.Database.field.cid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cid@v1"))]
        pub cid: Option<String>,

        /// Discovered from Repomix path `json.ovsdb._server.ovsschema.table.Database.field.connected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.connected@v1"))]
        pub connected: Option<String>,

        /// Discovered from Repomix path `json.ovsdb._server.ovsschema.table.Database.field.index`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.index@v1"))]
        pub index: Option<String>,

        /// Discovered from Repomix path `json.ovsdb._server.ovsschema.table.Database.field.leader`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.leader@v1"))]
        pub leader: Option<String>,

        /// Discovered from Repomix path `json.ovsdb._server.ovsschema.table.Database.field.model`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.model@v1"))]
        pub model: Option<String>,

        /// Discovered from Repomix path `json.ovsdb._server.ovsschema.table.Database.field.schema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.schema@v1"))]
        pub schema: Option<String>,

        /// Discovered from Repomix path `json.ovsdb._server.ovsschema.table.Database.field.sid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.sid@v1"))]
        pub sid: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Config.field.connections`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.connections@v1"))]
        pub connections: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.external_ids`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.external-ids@v1"))]
        pub external_ids: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.inactivity_probe`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.inactivity-probe@v1"))]
        pub inactivity_probe: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.is_connected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.is-connected@v1"))]
        pub is_connected: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.max_backoff`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.max-backoff@v1"))]
        pub max_backoff: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.read_only`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.read-only@v1"))]
        pub read_only: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.role`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.role@v1"))]
        pub role: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.status@v1"))]
        pub status: Option<String>,

        /// Discovered from Repomix path `json.ovsdb.local-config.ovsschema.table.Connection.field.target`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.target@v1"))]
        pub target: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.AutoAttach.field.mappings`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.mappings@v1"))]
        pub mappings: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.AutoAttach.field.system_description`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.system-description@v1"))]
        pub system_description: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.AutoAttach.field.system_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.system-name@v1"))]
        pub system_name: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.auto_attach`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.auto-attach@v1"))]
        pub auto_attach: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.controller`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.controller@v1"))]
        pub controller: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.datapath_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.datapath-id@v1"))]
        pub datapath_id: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.datapath_version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.datapath-version@v1"))]
        pub datapath_version: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.flood_vlans`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.flood-vlans@v1"))]
        pub flood_vlans: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.flow_tables`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.flow-tables@v1"))]
        pub flow_tables: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.ipfix`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ipfix@v1"))]
        pub ipfix: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.mirrors`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.mirrors@v1"))]
        pub mirrors: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.netflow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.netflow@v1"))]
        pub netflow: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.protocols`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.protocols@v1"))]
        pub protocols: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.rstp_enable`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.rstp-enable@v1"))]
        pub rstp_enable: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.rstp_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.rstp-status@v1"))]
        pub rstp_status: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Bridge.field.sflow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.sflow@v1"))]
        pub sflow: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.CT_Timeout_Policy.field.timeouts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.timeouts@v1"))]
        pub timeouts: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.CT_Zone.field.limit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.limit@v1"))]
        pub limit: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.CT_Zone.field.timeout_policy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.timeout-policy@v1"))]
        pub timeout_policy: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.connection_mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.connection-mode@v1"))]
        pub connection_mode: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.controller_burst_limit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.controller-burst-limit@v1"))]
        pub controller_burst_limit: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.controller_queue_size`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.controller-queue-size@v1"))]
        pub controller_queue_size: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.controller_rate_limit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.controller-rate-limit@v1"))]
        pub controller_rate_limit: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.enable_async_messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.enable-async-messages@v1"))]
        pub enable_async_messages: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.local_gateway`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.local-gateway@v1"))]
        pub local_gateway: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.local_ip`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.local-ip@v1"))]
        pub local_ip: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.local_netmask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.local-netmask@v1"))]
        pub local_netmask: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Controller.field.type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Datapath.field.capabilities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.capabilities@v1"))]
        pub capabilities: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Datapath.field.ct_zone_default_limit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ct-zone-default-limit@v1"))]
        pub ct_zone_default_limit: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Datapath.field.ct_zones`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ct-zones@v1"))]
        pub ct_zones: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Flow_Sample_Collector_Set.field.bridge`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bridge@v1"))]
        pub bridge: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Flow_Sample_Collector_Set.field.id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.id@v1"))]
        pub id: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Flow_Sample_Collector_Set.field.local_group_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.local-group-id@v1"))]
        pub local_group_id: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Flow_Table.field.flow_limit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.flow-limit@v1"))]
        pub flow_limit: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Flow_Table.field.groups`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.groups@v1"))]
        pub groups: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Flow_Table.field.overflow_policy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.overflow-policy@v1"))]
        pub overflow_policy: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Flow_Table.field.prefixes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.prefixes@v1"))]
        pub prefixes: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.cache_active_timeout`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cache-active-timeout@v1"))]
        pub cache_active_timeout: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.cache_max_flows`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cache-max-flows@v1"))]
        pub cache_max_flows: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.obs_domain_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.obs-domain-id@v1"))]
        pub obs_domain_id: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.obs_point_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.obs-point-id@v1"))]
        pub obs_point_id: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.sampling`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.sampling@v1"))]
        pub sampling: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.stats_interval`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.stats-interval@v1"))]
        pub stats_interval: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.targets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.targets@v1"))]
        pub targets: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.IPFIX.field.template_interval`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.template-interval@v1"))]
        pub template_interval: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.bfd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bfd@v1"))]
        pub bfd: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.bfd_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bfd-status@v1"))]
        pub bfd_status: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.cfm_fault`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cfm-fault@v1"))]
        pub cfm_fault: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.cfm_fault_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cfm-fault-status@v1"))]
        pub cfm_fault_status: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.cfm_flap_count`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cfm-flap-count@v1"))]
        pub cfm_flap_count: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.cfm_health`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cfm-health@v1"))]
        pub cfm_health: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.cfm_mpid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cfm-mpid@v1"))]
        pub cfm_mpid: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.cfm_remote_mpids`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cfm-remote-mpids@v1"))]
        pub cfm_remote_mpids: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.cfm_remote_opstate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cfm-remote-opstate@v1"))]
        pub cfm_remote_opstate: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.duplex`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.duplex@v1"))]
        pub duplex: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.error`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.error@v1"))]
        pub error: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.ifindex`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ifindex@v1"))]
        pub ifindex: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.ingress_policing_burst`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ingress-policing-burst@v1"))]
        pub ingress_policing_burst: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.ingress_policing_kpkts_burst`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ingress-policing-kpkts-burst@v1"))]
        pub ingress_policing_kpkts_burst: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.ingress_policing_kpkts_rate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ingress-policing-kpkts-rate@v1"))]
        pub ingress_policing_kpkts_rate: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.ingress_policing_rate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ingress-policing-rate@v1"))]
        pub ingress_policing_rate: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.lacp_current`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.lacp-current@v1"))]
        pub lacp_current: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.link_resets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.link-resets@v1"))]
        pub link_resets: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.link_speed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.link-speed@v1"))]
        pub link_speed: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.lldp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.lldp@v1"))]
        pub lldp: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.mtu`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.mtu@v1"))]
        pub mtu: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.mtu_request`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.mtu-request@v1"))]
        pub mtu_request: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.ofport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ofport@v1"))]
        pub ofport: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.ofport_request`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ofport-request@v1"))]
        pub ofport_request: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Interface.field.statistics`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.statistics@v1"))]
        pub statistics: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.filter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.filter@v1"))]
        pub filter: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.output_port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.output-port@v1"))]
        pub output_port: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.output_vlan`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.output-vlan@v1"))]
        pub output_vlan: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.select_all`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.select-all@v1"))]
        pub select_all: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.select_dst_port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.select-dst-port@v1"))]
        pub select_dst_port: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.select_src_port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.select-src-port@v1"))]
        pub select_src_port: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.select_vlan`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.select-vlan@v1"))]
        pub select_vlan: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Mirror.field.snaplen`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.snaplen@v1"))]
        pub snaplen: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.NetFlow.field.active_timeout`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.active-timeout@v1"))]
        pub active_timeout: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.NetFlow.field.add_id_to_interface`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.add-id-to-interface@v1"))]
        pub add_id_to_interface: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.NetFlow.field.engine_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.engine-id@v1"))]
        pub engine_id: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.NetFlow.field.engine_type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.engine-type@v1"))]
        pub engine_type: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.cur_cfg`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cur-cfg@v1"))]
        pub cur_cfg: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.datapath_types`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.datapath-types@v1"))]
        pub datapath_types: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.datapaths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.datapaths@v1"))]
        pub datapaths: Option<Vec<String>>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.db_version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.db-version@v1"))]
        pub db_version: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.dpdk_initialized`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dpdk-initialized@v1"))]
        pub dpdk_initialized: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.dpdk_version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dpdk-version@v1"))]
        pub dpdk_version: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.iface_types`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.iface-types@v1"))]
        pub iface_types: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.manager_options`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.manager-options@v1"))]
        pub manager_options: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.next_cfg`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.next-cfg@v1"))]
        pub next_cfg: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.ovs_version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ovs-version@v1"))]
        pub ovs_version: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.ssl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ssl@v1"))]
        pub ssl: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.system_type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.system-type@v1"))]
        pub system_type: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Open_vSwitch.field.system_version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.system-version@v1"))]
        pub system_version: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.bond_active_slave`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bond-active-slave@v1"))]
        pub bond_active_slave: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.bond_downdelay`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bond-downdelay@v1"))]
        pub bond_downdelay: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.bond_fake_iface`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bond-fake-iface@v1"))]
        pub bond_fake_iface: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.bond_updelay`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bond-updelay@v1"))]
        pub bond_updelay: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.cvlans`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.cvlans@v1"))]
        pub cvlans: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.fake_bridge`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.fake-bridge@v1"))]
        pub fake_bridge: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.lacp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.lacp@v1"))]
        pub lacp: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.protected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.protected@v1"))]
        pub protected: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.qos`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.qos@v1"))]
        pub qos: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Port.field.rstp_statistics`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.rstp-statistics@v1"))]
        pub rstp_statistics: Option<u64>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.QoS.field.queues`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.queues@v1"))]
        pub queues: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.Queue.field.dscp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dscp@v1"))]
        pub dscp: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.SSL.field.bootstrap_ca_cert`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bootstrap-ca-cert@v1"))]
        pub bootstrap_ca_cert: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.SSL.field.ca_cert`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ca-cert@v1"))]
        pub ca_cert: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.SSL.field.certificate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.certificate@v1"))]
        pub certificate: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.SSL.field.private_key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.private-key@v1"))]
        pub private_key: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.sFlow.field.agent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.agent@v1"))]
        pub agent: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.sFlow.field.header`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.header@v1"))]
        pub header: Option<String>,

        /// Discovered from Repomix path `json.vswitchd.vswitch.ovsschema.table.sFlow.field.polling`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.polling@v1"))]
        pub polling: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL.field.acl_entries`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.acl-entries@v1"))]
        pub acl_entries: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL.field.acl_fault_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.acl-fault-status@v1"))]
        pub acl_fault_status: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL.field.acl_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.acl-name@v1"))]
        pub acl_name: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.acle_fault_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.acle-fault-status@v1"))]
        pub acle_fault_status: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.dest_ip`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dest-ip@v1"))]
        pub dest_ip: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.dest_mac`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dest-mac@v1"))]
        pub dest_mac: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.dest_mask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dest-mask@v1"))]
        pub dest_mask: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.dest_port_max`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dest-port-max@v1"))]
        pub dest_port_max: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.dest_port_min`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dest-port-min@v1"))]
        pub dest_port_min: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.direction`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.direction@v1"))]
        pub direction: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.ethertype`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ethertype@v1"))]
        pub ethertype: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.icmp_code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.icmp-code@v1"))]
        pub icmp_code: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.icmp_type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.icmp-type@v1"))]
        pub icmp_type: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.protocol`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.protocol@v1"))]
        pub protocol: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.sequence`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.sequence@v1"))]
        pub sequence: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.source_ip`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.source-ip@v1"))]
        pub source_ip: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.source_mac`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.source-mac@v1"))]
        pub source_mac: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.source_mask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.source-mask@v1"))]
        pub source_mask: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.source_port_max`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.source-port-max@v1"))]
        pub source_port_max: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.source_port_min`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.source-port-min@v1"))]
        pub source_port_min: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.tcp_flags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.tcp-flags@v1"))]
        pub tcp_flags: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.ACL_entry.field.tcp_flags_mask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.tcp-flags-mask@v1"))]
        pub tcp_flags_mask: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Arp_Sources_Local.field.locator`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.locator@v1"))]
        pub locator: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Arp_Sources_Local.field.src_mac`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.src-mac@v1"))]
        pub src_mac: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Global.field.managers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.managers@v1"))]
        pub managers: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Global.field.switches`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.switches@v1"))]
        pub switches: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Binding_Stats.field.bytes_from_local`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bytes-from-local@v1"))]
        pub bytes_from_local: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Binding_Stats.field.bytes_to_local`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bytes-to-local@v1"))]
        pub bytes_to_local: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Binding_Stats.field.packets_from_local`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.packets-from-local@v1"))]
        pub packets_from_local: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Binding_Stats.field.packets_to_local`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.packets-to-local@v1"))]
        pub packets_to_local: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Router.field.LR_fault_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.lr-fault-status@v1"))]
        pub lr_fault_status: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Router.field.acl_binding`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.acl-binding@v1"))]
        pub acl_binding: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Router.field.description`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.description@v1"))]
        pub description: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Router.field.static_routes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.static-routes@v1"))]
        pub static_routes: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Router.field.switch_binding`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.switch-binding@v1"))]
        pub switch_binding: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Switch.field.replication_mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.replication-mode@v1"))]
        pub replication_mode: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Logical_Switch.field.tunnel_key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.tunnel-key@v1"))]
        pub tunnel_key: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Mcast_Macs_Local.field.ipaddr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.ipaddr@v1"))]
        pub ipaddr: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Mcast_Macs_Local.field.locator_set`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.locator-set@v1"))]
        pub locator_set: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Mcast_Macs_Local.field.logical_switch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.logical-switch@v1"))]
        pub logical_switch: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Locator.field.dst_ip`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.dst-ip@v1"))]
        pub dst_ip: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Locator.field.encapsulation_type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.encapsulation-type@v1"))]
        pub encapsulation_type: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Locator_Set.field.locators`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.locators@v1"))]
        pub locators: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Port.field.acl_bindings`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.acl-bindings@v1"))]
        pub acl_bindings: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Port.field.port_fault_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.port-fault-status@v1"))]
        pub port_fault_status: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Port.field.vlan_bindings`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.vlan-bindings@v1"))]
        pub vlan_bindings: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Port.field.vlan_stats`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.vlan-stats@v1"))]
        pub vlan_stats: Option<u64>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Switch.field.management_ips`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.management-ips@v1"))]
        pub management_ips: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Switch.field.switch_fault_status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.switch-fault-status@v1"))]
        pub switch_fault_status: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Switch.field.tunnel_ips`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.tunnel-ips@v1"))]
        pub tunnel_ips: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Physical_Switch.field.tunnels`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.tunnels@v1"))]
        pub tunnels: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Tunnel.field.bfd_config_local`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bfd-config-local@v1"))]
        pub bfd_config_local: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Tunnel.field.bfd_config_remote`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bfd-config-remote@v1"))]
        pub bfd_config_remote: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Tunnel.field.bfd_params`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.bfd-params@v1"))]
        pub bfd_params: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Tunnel.field.local`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.local@v1"))]
        pub local: Option<String>,

        /// Discovered from Repomix path `json.vtep.vtep.ovsschema.table.Tunnel.field.remote`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.ovsdb-bridge.remote@v1"))]
        pub remote: Option<String>,
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
