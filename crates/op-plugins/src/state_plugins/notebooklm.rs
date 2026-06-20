//! NotebookLM plugin — projects NotebookLM (knowledge notebooks) state into the
//! D-Bus tree at `/org/opdbus/v1/plugins/notebooklm`.
//!
//! Read-only mirror: reports auth status, the designated-corpus manifest, the
//! master notebook, and per-notebook source counts. Source ingestion / upload is
//! a runtime action driven through the knowledge pipeline (OD-23), not this mirror.
//!
//! Identity/accountability: notebook queries are sessions like any other —
//! stable session_id, subid-classified, trace_id-logged. This plugin only
//! projects the catalog; the chatbot reads across it via cognitive-mcp.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
#[cfg(test)]
use op_state_store::{Constraint, FieldSchema, FieldType};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::path::Path;

/// Manifest of the designated knowledge corpus.
const MANIFEST_PATH: &str = "knowledge/notebooks.manifest.json";
/// nlm CLI profile location — its presence implies a configured (not necessarily live) session.
const NLM_PROFILE: &str = ".notebooklm-mcp-cli/profiles/default";

/// NotebookLM authentication/profile state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.notebooklm.auth.schema@v1"))]
pub struct NotebookLmAuth {
    /// Whether the NotebookLM CLI profile is present.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.profile.configured@v1"))]
    pub profile_configured: bool,
    /// Path to the default CLI profile.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.profile.path@v1"))]
    pub profile_path: String,
    /// Note about session lifetime.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.profile.note@v1"))]
    pub note: String,
}

/// NotebookLM master notebook reference.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.notebooklm.master.schema@v1"))]
pub struct NotebookLmMaster {
    /// Master notebook identifier.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.master.id@v1"))]
    pub id: String,
    /// Human-readable notebook title.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.master.title@v1"))]
    pub title: Option<String>,
}

/// NotebookLM designated knowledge corpus.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.notebooklm.corpus.schema@v1"))]
pub struct NotebookLmCorpus {
    /// Path to the corpus manifest file.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.corpus.manifest@v1"))]
    pub manifest: String,
    /// Number of notebooks designated in the manifest.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.corpus.designated-count@v1"))]
    pub designated_notebooks: usize,
    /// Maximum source count allowed by the corpus.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.corpus.source-cap@v1"))]
    pub source_cap: usize,
    /// Destination sinks for the corpus.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.corpus.sinks@v1"))]
    pub sinks: Option<Vec<String>>,
}

/// NotebookLM transport/ingest configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.notebooklm.config.schema@v1"))]
pub struct NotebookLmConfig {
    /// CLI tool name.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.config.cli@v1"))]
    pub cli: String,
    /// Transport mechanism.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.config.transport@v1"))]
    pub transport: String,
    /// Ingest pipeline identifier.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.config.ingest-pipeline@v1"))]
    pub ingest_pipeline: String,
}

/// Runtime state of the NotebookLM plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.notebooklm.schema@v1"))]
pub struct NotebookLmState {
    /// NotebookLM configuration status.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.status.query@v1"))]
    pub status: String,
    /// Authentication/profile state.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.auth.query@v1"))]
    pub auth: NotebookLmAuth,
    /// Designated master notebook.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.master.query@v1"))]
    pub master_notebook: NotebookLmMaster,
    /// Designated knowledge corpus.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.corpus.query@v1"))]
    pub corpus: NotebookLmCorpus,
    /// Ingest/transport configuration.
    #[schemars(extend("x-oscal-subid" = "obs.service.notebooklm.config.query@v1"))]
    pub config: NotebookLmConfig,
}

pub struct NotebookLmPlugin;

impl Default for NotebookLmPlugin {
    fn default() -> Self {
        Self
    }
}

impl NotebookLmPlugin {
    pub fn new() -> Self {
        Self
    }

    fn profile_configured() -> bool {
        if let Some(home) = std::env::var_os("HOME") {
            return Path::new(&home).join(NLM_PROFILE).exists();
        }
        false
    }

    /// Read the designated-corpus manifest if present (best-effort).
    fn read_manifest() -> Option<Value> {
        let mut buf = std::fs::read(MANIFEST_PATH).ok()?;
        simd_json::to_owned_value(&mut buf).ok()
    }

    pub(crate) fn current_state() -> NotebookLmState {
        let configured = Self::profile_configured();

        let (master, corpus) = match Self::read_manifest() {
            Some(m) => {
                let obj = m.as_object();
                let master_id = obj
                    .and_then(|o| o.get("master_notebook"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| "unknown".to_string());
                let designated_count = obj
                    .and_then(|o| o.get("designated"))
                    .and_then(|d| d.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                (
                    NotebookLmMaster {
                        id: master_id,
                        title: Some("Ghostbridge Live!".to_string()),
                    },
                    NotebookLmCorpus {
                        manifest: MANIFEST_PATH.to_string(),
                        designated_notebooks: designated_count,
                        source_cap: 300,
                        sinks: Some(vec![
                            "semantic:qdrant".to_string(),
                            "graph:cozo".to_string(),
                        ]),
                    },
                )
            }
            None => (
                NotebookLmMaster {
                    id: "unknown".to_string(),
                    title: None,
                },
                NotebookLmCorpus {
                    manifest: MANIFEST_PATH.to_string(),
                    designated_notebooks: 0,
                    source_cap: 300,
                    sinks: None,
                },
            ),
        };

        NotebookLmState {
            status: if configured {
                "configured"
            } else {
                "unconfigured"
            }
            .to_string(),
            auth: NotebookLmAuth {
                profile_configured: configured,
                profile_path: NLM_PROFILE.to_string(),
                note: "cookie sessions are short-lived; long batches run laptop-side or in bursts"
                    .to_string(),
            },
            master_notebook: master,
            corpus,
            config: NotebookLmConfig {
                cli: "nlm".to_string(),
                transport: "nlm CLI / NotebookLM batchexecute".to_string(),
                ingest_pipeline: "OD-23 (Voyage->Qdrant + Cozo graph)".to_string(),
            },
        }
    }
}

#[async_trait]
impl StatePlugin for NotebookLmPlugin {
    fn name(&self) -> &str {
        "notebooklm"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(notebooklm_schema())
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

/// NotebookLM schema derived from the typed [`NotebookLmState`] struct via schemars.
pub(crate) fn notebooklm_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(NotebookLmState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "notebooklm",
        "1.0.0",
        "NotebookLM knowledge notebooks — auth status, designated corpus, master notebook",
        &root,
    )
}

/// Hand-rolled golden reference for the NotebookLM schema. Kept test-only so the
/// derived schema can be proven field-for-field equivalent to the original
/// hand-rolled contract.
#[cfg(test)]
pub(crate) fn notebooklm_schema_golden() -> PluginSchema {
    let mut auth_fields = std::collections::HashMap::new();
    auth_fields.insert(
        "profile_configured".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: true,
            description: "Whether the NotebookLM CLI profile is present.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    auth_fields.insert(
        "profile_path".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Path to the default CLI profile.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    auth_fields.insert(
        "note".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Note about session lifetime.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut master_fields = std::collections::HashMap::new();
    master_fields.insert(
        "id".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Master notebook identifier.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    master_fields.insert(
        "title".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Human-readable notebook title.".to_string(),
            default: Some(json!(null)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut corpus_fields = std::collections::HashMap::new();
    corpus_fields.insert(
        "manifest".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Path to the corpus manifest file.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    corpus_fields.insert(
        "designated_notebooks".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Number of notebooks designated in the manifest.".to_string(),
            default: None,
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    corpus_fields.insert(
        "source_cap".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Maximum source count allowed by the corpus.".to_string(),
            default: None,
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    corpus_fields.insert(
        "sinks".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Destination sinks for the corpus.".to_string(),
            default: Some(json!(null)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut config_fields = std::collections::HashMap::new();
    config_fields.insert(
        "cli".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "CLI tool name.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    config_fields.insert(
        "transport".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Transport mechanism.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    config_fields.insert(
        "ingest_pipeline".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Ingest pipeline identifier.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "status".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "NotebookLM configuration status.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "auth".to_string(),
        FieldSchema {
            field_type: FieldType::Object(auth_fields),
            required: true,
            description: "Authentication/profile state.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "master_notebook".to_string(),
        FieldSchema {
            field_type: FieldType::Object(master_fields),
            required: true,
            description: "Designated master notebook.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "corpus".to_string(),
        FieldSchema {
            field_type: FieldType::Object(corpus_fields),
            required: true,
            description: "Designated knowledge corpus.".to_string(),
            default: None,
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
            required: true,
            description: "Ingest/transport configuration.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schema = PluginSchema::builder("notebooklm")
        .version("1.0.0")
        .description(
            "NotebookLM knowledge notebooks — auth status, designated corpus, master notebook",
        )
        .build();
    schema.fields = fields;
    schema.subids = std::collections::HashMap::from([
        (
            "__schema__".to_string(),
            "sch.software.plugin.notebooklm.schema@v1".to_string(),
        ),
        (
            "status".to_string(),
            "obs.service.notebooklm.status.query@v1".to_string(),
        ),
        (
            "auth".to_string(),
            "obs.service.notebooklm.auth.query@v1".to_string(),
        ),
        (
            "master_notebook".to_string(),
            "obs.service.notebooklm.master.query@v1".to_string(),
        ),
        (
            "corpus".to_string(),
            "obs.service.notebooklm.corpus.query@v1".to_string(),
        ),
        (
            "config".to_string(),
            "obs.service.notebooklm.config.query@v1".to_string(),
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
        let hand = notebooklm_schema_golden();
        let derived = notebooklm_schema();
        let diffs = schema_diffs(&hand, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let root = serde_json::to_value(schemars::schema_for!(NotebookLmState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}
