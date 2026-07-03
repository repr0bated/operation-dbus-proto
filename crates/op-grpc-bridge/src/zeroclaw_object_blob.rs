//! Zeroclaw D-Bus + gRPC object blob prototype.
//!
//! This is the copy-forward shape for coupling D-Bus identity, plugin schema
//! identity, and tonic reflection identity without making D-Bus passthrough part
//! of the new public API surface.

use crate::plugin_object_blob::{
    blobify_plugin_schema, DbusObjectIdentity, PluginObjectBlob,
};
use op_state::StatePlugin;
use op_state_store::PluginSchema;

const ZEROCLAW_PLUGIN_ID: &str = "zeroclaw";
const ZEROCLAW_DBUS_BUS: &str = "org.opdbus.v1.plugins";
const ZEROCLAW_DBUS_OBJECT: &str = "/org/opdbus/v1/plugins/zeroclaw";
const ZEROCLAW_DBUS_INTERFACE: &str = "org.opdbus.v1.Plugin";
const ZEROCLAW_GRPC_PACKAGE: &str = "operation.plugin.v1";
const ZEROCLAW_GRPC_SERVICE: &str = "operation.plugin.v1.ZeroclawPluginMethods";

/// Immutable bridge-local identity for the Zeroclaw object projection.
pub type ZeroclawObjectBlob = PluginObjectBlob;

impl ZeroclawObjectBlob {
    /// Build the immutable object blob from the plugin-owned Zeroclaw schema.
    /// Uses the uniform StatePlugin::schema() (recoverable from the schemars-driven current_state pipeline)
    pub fn from_plugin_schema() -> Self {
        let plugin = op_plugins::state_plugins::ZeroclawPlugin::new();
        let schema = StatePlugin::schema(&plugin)
            .expect("zeroclaw publishes PluginSchema (uniform plugin contract + schema_from_state)");
        Self::from_schema(schema)
    }

    pub fn from_schema(schema: PluginSchema) -> Self {
        blobify_plugin_schema(
            ZEROCLAW_PLUGIN_ID,
            schema,
            DbusObjectIdentity {
                bus_name: ZEROCLAW_DBUS_BUS.to_string(),
                object_path: ZEROCLAW_DBUS_OBJECT.to_string(),
                interface_name: ZEROCLAW_DBUS_INTERFACE.to_string(),
            },
            ZEROCLAW_GRPC_PACKAGE,
            ZEROCLAW_GRPC_SERVICE,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeroclaw_blob_couples_dbus_schema_and_grpc_identity() {
        let blob = ZeroclawObjectBlob::from_plugin_schema();

        assert_eq!(blob.plugin_id, "zeroclaw");
        assert_eq!(blob.dbus.object_path, "/org/opdbus/v1/plugins/zeroclaw");
        assert_eq!(
            blob.grpc.service_name,
            "operation.plugin.v1.ZeroclawPluginMethods"
        );
        assert!(!blob.schema_json.is_empty());
        assert_eq!(blob.schema_hash.len(), 64);
        assert!(!blob.grpc.descriptor_set.is_empty());
        // Zeroclaw schema is recovered via uniform current_state + schema_from_state pipeline;
        // it will contain methods derived from ZeroclawState actions + declared chat etc.
        assert!(blob.methods.iter().any(|m| m.schema_name.contains("state") || m.schema_name == "query_current_state" || m.schema_name == "chat"));
        assert!(blob.methods.iter().any(|m| m.grpc_name.contains("State") || m.grpc_name == "QueryCurrentState" || m.grpc_name == "Chat"));
        assert!(blob.methods.iter().all(|method| !method.args_schema.is_empty()));
        assert!(blob.methods.iter().all(|method| !method.grpc_path.is_empty()));
    }
}
