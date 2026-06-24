use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// Runtime state of the workflow automation plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.workflows.schema@v1"))]
pub struct WorkflowsState {
    /// Operational status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.workflows.status@v1"))]
    pub status: String,
    /// Declared workflow pipelines.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.workflows.workflows@v1"))]
    pub workflows: serde_json::Value,
    /// Workflow scheduler configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.workflows.config@v1"))]
    pub config: serde_json::Value,
}

impl Default for WorkflowsState {
    fn default() -> Self {
        Self {
            status: "active".to_string(),
            workflows: serde_json::json!([{"id": "deploy", "name": "Deploy Pipeline", "steps": ["build", "test", "deploy"], "triggers": ["push", "manual"], "status": "idle"}, {"id": "backup", "name": "System Backup", "steps": ["snapshot", "archive", "verify"], "triggers": ["cron"], "status": "idle"}, {"id": "embedding-sync", "name": "Embedding Pipeline Sync", "steps": ["ingest", "chunk", "embed", "store"], "triggers": ["webhook"], "status": "idle"}]),
            config: serde_json::json!({"max_concurrent": 4, "timeout_secs": 3600, "retry_count": 2, "notification_channel": "telegram"}),
        }
    }
}

pub struct WorkflowsPlugin;
impl Default for WorkflowsPlugin {
    fn default() -> Self {
        Self
    }
}
impl WorkflowsPlugin {
    pub fn new() -> Self {
        Self
    }
    pub(crate) fn current_state() -> WorkflowsState {
        WorkflowsState {
            status: "active".to_string(),
            workflows: serde_json::json!([{"id": "deploy", "name": "Deploy Pipeline", "steps": ["build", "test", "deploy"], "triggers": ["push", "manual"], "status": "idle"}, {"id": "backup", "name": "System Backup", "steps": ["snapshot", "archive", "verify"], "triggers": ["cron"], "status": "idle"}, {"id": "embedding-sync", "name": "Embedding Pipeline Sync", "steps": ["ingest", "chunk", "embed", "store"], "triggers": ["webhook"], "status": "idle"}]),
            config: serde_json::json!({"max_concurrent": 4, "timeout_secs": 3600, "retry_count": 2, "notification_channel": "telegram"}),
        }
    }
}
#[async_trait]
impl StatePlugin for WorkflowsPlugin {
    fn name(&self) -> &str {
        "workflows"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(workflows_schema())
    }
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
            state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?,
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

/// Derived `workflows` schema from the typed [`WorkflowsState`] struct via schemars.
pub(crate) fn workflows_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(WorkflowsState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "workflows",
        "1.0.0",
        "Workflow automation — pipelines, triggers, execution",
        &root,
    );
    let state = simd_json::serde::to_owned_value(&WorkflowsState::default())
        .expect("WorkflowsState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);
    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use crate::state_plugins::schemars_adapter::schema_diffs;
    use serde_json::Value as JVal;

    fn collect_subids(value: &JVal, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(JVal::String(subid)) = obj.get("x-oscal-subid") {
                out.push(subid.clone());
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

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = super::workflows_schema_golden();
        let derived = super::workflows_schema();
        let diffs = schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let root = serde_json::to_value(schemars::schema_for!(WorkflowsState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}

#[cfg(test)]
pub(crate) fn workflows_schema_golden() -> PluginSchema {
    use op_state_store::{FieldSchema, FieldType};
    use simd_json::json;

    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "status".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Operational status.".to_string(),
            default: Some(json!("active")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "workflows".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Declared workflow pipelines.".to_string(),
            default: Some(json!([
                {"id": "deploy", "name": "Deploy Pipeline", "steps": ["build", "test", "deploy"], "triggers": ["push", "manual"], "status": "idle"},
                {"id": "backup", "name": "System Backup", "steps": ["snapshot", "archive", "verify"], "triggers": ["cron"], "status": "idle"},
                {"id": "embedding-sync", "name": "Embedding Pipeline Sync", "steps": ["ingest", "chunk", "embed", "store"], "triggers": ["webhook"], "status": "idle"}
            ])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "config".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Workflow scheduler configuration.".to_string(),
            default: Some(json!({"max_concurrent": 4, "timeout_secs": 3600, "retry_count": 2, "notification_channel": "telegram"})),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schema = PluginSchema::builder("workflows")
        .version("1.0.0")
        .description("Workflow automation — pipelines, triggers, execution")
        .build();
    schema.fields = fields;
    schema.subids = std::collections::HashMap::from([
        (
            "__schema__".to_string(),
            "sch.software.plugin.workflows.schema@v1".to_string(),
        ),
        (
            "status".to_string(),
            "obs.software.plugin.workflows.status@v1".to_string(),
        ),
        (
            "workflows".to_string(),
            "exp.software.plugin.workflows.workflows@v1".to_string(),
        ),
        (
            "config".to_string(),
            "sch.software.plugin.workflows.config@v1".to_string(),
        ),
    ]);
    let state = simd_json::serde::to_owned_value(&WorkflowsState::default())
        .expect("WorkflowsState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("workflows", |_ctx| std::sync::Arc::new(WorkflowsPlugin::new()))
}
