use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessDeclState {
    #[serde(default)]
    pub sessions: Vec<SessionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionConfig {
    pub id: String,
    pub user: String,
}

/// Schema-only state for the `sess_decl` plugin. The runtime state is fully
/// typed (`SessDeclState`), but the published schema preserves the original
/// opaque `sessions` field to avoid changing the downstream contract.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.sessdecl.schema@v1"))]
#[schemars(extend("x-oscal-category" = "software"))]
pub struct SessDeclSchemaState {
    #[schemars(
        description = "Session declarations",
        extend("default" = serde_json::json!([]), "x-oscal-subid" = "obs.software.plugin.sessdecl.sessions@v1")
    )]
    pub sessions: JsonValue,
}

pub struct SessDeclPlugin;

impl Default for SessDeclPlugin {
    fn default() -> Self {
        Self
    }
}

impl SessDeclPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for SessDeclPlugin {
    fn name(&self) -> &str {
        "sess_decl"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(sess_decl_schema())
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

pub(crate) fn sess_decl_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(SessDeclSchemaState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "sess_decl",
        "1.0.0",
        "Session declaration state",
        &root,
    );

    use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, EmptyInput};
    use op_state_store::SideEffect;

    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListSessionsOutput {
        pub sessions: Vec<SessionConfig>,
    }

    // No dispatch wired yet: calculate_diff/apply_state are both no-ops today
    // (nothing durably stores session declarations), so there is no real
    // backend to call into. Declared for UI/gRPC discovery, matching the xray
    // precedent for schema-declared-but-not-yet-implemented methods.
    schema.methods.insert(
        "list_sessions".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, ListSessionsOutput>(
            "list_sessions",
            SideEffect::Read,
            true,
            "sess_decl.read",
            "obs.software.plugin.sessdecl.sessions.list@v1",
        ),
    );
    schema.methods.insert(
        "declare_session".to_string(),
        method_decl_from_schemars_with_output::<SessionConfig, ListSessionsOutput>(
            "declare_session",
            SideEffect::Mutation,
            false,
            "sess_decl.write",
            "mut.software.plugin.sessdecl.session.declare@v1",
        ),
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(SessDeclSchemaState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        for subid in subids {
            assert!(
                crate::state_plugins::common::oscal::validate_subid(&subid).is_ok(),
                "invalid subid: {subid}"
            );
        }
    }

    fn collect_subids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(subid) = obj.get("x-oscal-subid").and_then(|v| v.as_str()) {
                out.push(subid.to_string());
            }
            for v in obj.values() {
                collect_subids(v, out);
            }
        }
        if let Some(arr) = value.as_array() {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("sess_decl", |_ctx| std::sync::Arc::new(SessDeclPlugin::new()))
}
