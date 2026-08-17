use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// Endpoint declaration state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.endpoint.schema@v1"))]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct EndpointState {
    /// Declared endpoints.
    #[serde(default)]
    #[schemars(
        description = "Declared endpoints",
        example = &["192.168.1.100:8080"],
        extend("x-oscal-subid" = "exp.service.endpoint.endpoints.render@v1")
    )]
    pub endpoints: Vec<String>,
}

pub struct EndpointPlugin;

impl Default for EndpointPlugin {
    fn default() -> Self {
        Self
    }
}

impl EndpointPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for EndpointPlugin {
    fn name(&self) -> &str {
        "endpoint"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(endpoint_schema())
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
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

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
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
            atomic_operations: false,
        }
    }
}

/// Derived `endpoint` schema from the `EndpointState` struct.
pub fn endpoint_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(EndpointState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "endpoint",
        "1.0.0",
        "Endpoint configuration",
        &root,
    );

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListEndpointsOutput {
        pub endpoints: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetEndpointOutput {
        pub endpoint: Option<serde_json::Value>,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "list_endpoints".to_string(),
        method_decl_from_schemars_with_output::<(), ListEndpointsOutput>(
            "list_endpoints",
            SideEffect::Read,
            true,
            "endpoint.read",
            "obs.service.endpoint.list@v1",
        ),
    );
    schema.methods.insert(
        "get_endpoint".to_string(),
        method_decl_from_schemars_with_output::<(), GetEndpointOutput>(
            "get_endpoint",
            SideEffect::Read,
            true,
            "endpoint.read",
            "obs.service.endpoint.get@v1",
        ),
    );
    schema.methods.insert(
        "add_endpoint".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "add_endpoint",
            SideEffect::Mutation,
            false,
            "endpoint.invoke",
            "mut.service.endpoint.add@v1",
        ),
    );
    schema.methods.insert(
        "remove_endpoint".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "remove_endpoint",
            SideEffect::Mutation,
            true,
            "endpoint.invoke",
            "mut.service.endpoint.remove@v1",
        ),
    );

    schema.capabilities.insert(
        "endpoint.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "endpoint.read".to_string(),
            description: "Grants: list_endpoints, get_endpoint.".to_string(),
        },
    );
    schema.capabilities.insert(
        "endpoint.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "endpoint.invoke".to_string(),
            description: "Grants: add_endpoint, remove_endpoint.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_schemars_seeded_and_typed() {
        let schema = endpoint_schema();
        assert_eq!(schema.name, "endpoint");
        assert_eq!(schema.version, "1.0.0");
        assert!(schema.fields.contains_key("endpoints"));
        assert!(schema.methods.contains_key("list_endpoints"));
        assert!(schema.methods.contains_key("get_endpoint"));
        assert!(schema.methods.contains_key("add_endpoint"));
        assert!(schema.methods.contains_key("remove_endpoint"));
    }

    /// Every `x-oscal-subid` annotation in the derived schema must be a valid
    /// OSCAL subid according to the canonical taxonomy.
    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(EndpointState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            crate::state_plugins::common::oscal::validate_subid(&subid)
                .unwrap_or_else(|e| panic!("invalid subid {subid}: {e}"));
        }
    }

    fn collect_subids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let serde_json::Value::Object(map) = value {
            if let Some(subid) = map.get("x-oscal-subid").and_then(|v| v.as_str()) {
                out.push(subid.to_string());
            }
            for v in map.values() {
                collect_subids(v, out);
            }
        } else if let serde_json::Value::Array(arr) = value {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("endpoint", |_ctx| std::sync::Arc::new(EndpointPlugin::new()))
}
