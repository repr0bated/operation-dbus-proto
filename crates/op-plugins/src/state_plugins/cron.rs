use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

#[cfg(test)]
use op_state_store::{Constraint, FieldSchema, FieldType};
#[cfg(test)]
use simd_json::json;

/// A single scheduled cron job.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.cron.job.schema@v1"))]
pub struct CronJob {
    /// Stable job identifier.
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.job.id@v1"))]
    #[serde(default)]
    pub id: String,
    /// Human-readable job name.
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.job.name@v1"))]
    #[serde(default)]
    pub name: String,
    /// Cron expression (e.g. `*/30 * * * *`).
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.job.schedule@v1"))]
    #[serde(default)]
    pub schedule: String,
    /// Agent responsible for executing this job.
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.job.agent-id@v1"))]
    #[serde(default)]
    pub agent_id: String,
    /// Whether the job is enabled.
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.job.enabled@v1"))]
    #[serde(default)]
    pub enabled: bool,
    /// Last execution timestamp (ISO 8601), or null if never run.
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.job.last-run@v1"))]
    #[serde(default)]
    pub last_run: Option<String>,
    /// Next scheduled execution timestamp (ISO 8601), or null if not scheduled.
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.job.next-run@v1"))]
    #[serde(default)]
    pub next_run: Option<String>,
}

/// Global scheduling settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.cron.schedules.schema@v1"))]
pub struct CronSchedules {
    /// Timezone for schedule evaluation.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.schedules.timezone@v1"))]
    #[serde(default)]
    pub timezone: String,
    /// Maximum number of jobs that may run concurrently.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.schedules.max-concurrent@v1"))]
    #[serde(default)]
    pub max_concurrent: u32,
    /// Run missed jobs when the scheduler starts.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.schedules.catch-up-on-startup@v1"))]
    #[serde(default)]
    pub catch_up_on_startup: bool,
    /// Maximum number of recent run records to retain.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.schedules.max-run-history@v1"))]
    #[serde(default)]
    pub max_run_history: u32,
}

/// Runtime execution configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.cron.config.schema@v1"))]
pub struct CronConfig {
    /// Whether the scheduler is enabled.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.config.enabled@v1"))]
    #[serde(default)]
    pub enabled: bool,
    /// Timeout for a single job invocation.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.config.task-timeout-secs@v1"))]
    #[serde(default)]
    pub task_timeout_secs: u32,
    /// Number of retries on failure.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.config.retry-count@v1"))]
    #[serde(default)]
    pub retry_count: u32,
    /// Backoff between retries in milliseconds.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.config.backoff-ms@v1"))]
    #[serde(default)]
    pub backoff_ms: u32,
}

/// Runtime state of the cron scheduler.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.plugin.cron.schema@v1"))]
pub struct CronState {
    /// Operational status of the cron scheduler.
    #[schemars(extend("x-oscal-subid" = "obs.service.cron.status@v1"))]
    #[serde(default)]
    pub status: String,
    /// Scheduled jobs.
    #[schemars(extend("x-oscal-subid" = "mut.service.cron.manage-jobs@v1"))]
    #[serde(default)]
    pub jobs: Vec<CronJob>,
    /// Global scheduling settings.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.schedules@v1"))]
    #[serde(default)]
    pub schedules: CronSchedules,
    /// Runtime execution configuration.
    #[schemars(extend("x-oscal-subid" = "sch.service.cron.config@v1"))]
    #[serde(default)]
    pub config: CronConfig,
}

pub struct CronPlugin;

impl Default for CronPlugin {
    fn default() -> Self {
        Self
    }
}

impl CronPlugin {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn current_state() -> CronState {
        CronState {
            status: "active".to_string(),
            jobs: vec![
                CronJob {
                    id: "heartbeat".to_string(),
                    name: "System Heartbeat".to_string(),
                    schedule: "*/30 * * * *".to_string(),
                    agent_id: "system".to_string(),
                    enabled: true,
                    last_run: None,
                    next_run: None,
                },
                CronJob {
                    id: "memory-hygiene".to_string(),
                    name: "Memory Hygiene".to_string(),
                    schedule: "0 3 * * *".to_string(),
                    agent_id: "memory".to_string(),
                    enabled: true,
                    last_run: None,
                    next_run: None,
                },
                CronJob {
                    id: "audit-rotate".to_string(),
                    name: "Audit Log Rotation".to_string(),
                    schedule: "0 0 * * 0".to_string(),
                    agent_id: "audit".to_string(),
                    enabled: true,
                    last_run: None,
                    next_run: None,
                },
            ],
            schedules: CronSchedules {
                timezone: "UTC".to_string(),
                max_concurrent: 4,
                catch_up_on_startup: true,
                max_run_history: 50,
            },
            config: CronConfig {
                enabled: true,
                task_timeout_secs: 600,
                retry_count: 2,
                backoff_ms: 500,
            },
        }
    }
}

#[async_trait]
impl StatePlugin for CronPlugin {
    fn name(&self) -> &str {
        "cron"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = cron_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }
    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(Self::current_state())?)
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

/// Cron schema derived from the typed [`CronState`] struct via schemars.
pub(crate) fn cron_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(CronState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "cron",
        "1.0.0",
        "Cron scheduler — jobs, schedules, execution history",
        &root,
    )
}

/// Hand-rolled golden reference for the cron schema. Kept test-only so the
/// derived schema can be proven field-for-field equivalent to the original
/// hand-rolled contract.
#[cfg(test)]
pub(crate) fn cron_schema_golden() -> PluginSchema {
    let mut job_fields = std::collections::HashMap::new();
    job_fields.insert(
        "id".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Stable job identifier.".to_string(),
            default: Some(json!("")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    job_fields.insert(
        "name".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Human-readable job name.".to_string(),
            default: Some(json!("")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    job_fields.insert(
        "schedule".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Cron expression (e.g. `*/30 * * * *`).".to_string(),
            default: Some(json!("")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    job_fields.insert(
        "agent_id".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Agent responsible for executing this job.".to_string(),
            default: Some(json!("")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    job_fields.insert(
        "enabled".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the job is enabled.".to_string(),
            default: Some(json!(false)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    job_fields.insert(
        "last_run".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Last execution timestamp (ISO 8601), or null if never run.".to_string(),
            default: Some(json!(null)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    job_fields.insert(
        "next_run".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Next scheduled execution timestamp (ISO 8601), or null if not scheduled."
                .to_string(),
            default: Some(json!(null)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schedule_fields = std::collections::HashMap::new();
    schedule_fields.insert(
        "timezone".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Timezone for schedule evaluation.".to_string(),
            default: Some(json!("")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    schedule_fields.insert(
        "max_concurrent".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Maximum number of jobs that may run concurrently.".to_string(),
            default: Some(json!(0)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    schedule_fields.insert(
        "catch_up_on_startup".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Run missed jobs when the scheduler starts.".to_string(),
            default: Some(json!(false)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    schedule_fields.insert(
        "max_run_history".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Maximum number of recent run records to retain.".to_string(),
            default: Some(json!(0)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );

    let mut config_fields = std::collections::HashMap::new();
    config_fields.insert(
        "enabled".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the scheduler is enabled.".to_string(),
            default: Some(json!(false)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    config_fields.insert(
        "task_timeout_secs".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Timeout for a single job invocation.".to_string(),
            default: Some(json!(0)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    config_fields.insert(
        "retry_count".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Number of retries on failure.".to_string(),
            default: Some(json!(0)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    config_fields.insert(
        "backoff_ms".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Backoff between retries in milliseconds.".to_string(),
            default: Some(json!(0)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );

    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "status".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Operational status of the cron scheduler.".to_string(),
            default: Some(json!("")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "jobs".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::Object(job_fields))),
            required: false,
            description: "Scheduled jobs.".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "schedules".to_string(),
        FieldSchema {
            field_type: FieldType::Object(schedule_fields),
            required: false,
            description: "Global scheduling settings.".to_string(),
            default: Some(json!({
                "catch_up_on_startup": false,
                "max_concurrent": 0,
                "max_run_history": 0,
                "timezone": ""
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "config".to_string(),
        FieldSchema {
            field_type: FieldType::Object(config_fields),
            required: false,
            description: "Runtime execution configuration.".to_string(),
            default: Some(json!({
                "backoff_ms": 0,
                "enabled": false,
                "retry_count": 0,
                "task_timeout_secs": 0
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schema = PluginSchema::builder("cron")
        .version("1.0.0")
        .description("Cron scheduler — jobs, schedules, execution history")
        .build();
    schema.fields = fields;
    schema.subids = std::collections::HashMap::from([
        (
            "__schema__".to_string(),
            "sch.service.plugin.cron.schema@v1".to_string(),
        ),
        (
            "status".to_string(),
            "obs.service.cron.status@v1".to_string(),
        ),
        (
            "jobs".to_string(),
            "mut.service.cron.manage-jobs@v1".to_string(),
        ),
        (
            "schedules".to_string(),
            "sch.service.cron.schedules@v1".to_string(),
        ),
        (
            "config".to_string(),
            "sch.service.cron.config@v1".to_string(),
        ),
    ]);
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
        let hand = cron_schema_golden();
        let derived = cron_schema();
        let diffs = schema_diffs(&hand, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let root = serde_json::to_value(schemars::schema_for!(CronState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}
