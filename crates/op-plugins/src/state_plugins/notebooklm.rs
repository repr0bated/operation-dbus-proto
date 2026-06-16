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
use serde::{Deserialize, Serialize};
use simd_json::prelude::ValueAsContainer;
use simd_json::{json, OwnedValue as Value};
use std::path::Path;

use super::plugin_schema_defs::schema_from_state;

/// Manifest of the designated knowledge corpus.
const MANIFEST_PATH: &str = "knowledge/notebooks.manifest.json";
/// nlm CLI profile location — its presence implies a configured (not necessarily live) session.
const NLM_PROFILE: &str = ".notebooklm-mcp-cli/profiles/default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookLmState {
    pub status: String,
    pub auth: Value,
    pub master_notebook: Value,
    pub corpus: Value,
    pub config: Value,
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
                let master = obj
                    .and_then(|o| o.get("master_notebook"))
                    .cloned()
                    .unwrap_or_else(|| Value::from("unknown"));
                let designated_count = obj
                    .and_then(|o| o.get("designated"))
                    .and_then(|d| d.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                (
                    json!({ "id": master, "title": "Ghostbridge Live!" }),
                    json!({
                        "manifest": MANIFEST_PATH,
                        "designated_notebooks": designated_count,
                        "source_cap": 300,
                        "sinks": ["semantic:qdrant", "graph:cozo"]
                    }),
                )
            }
            None => (
                json!({ "id": "unknown", "title": null }),
                json!({ "manifest": MANIFEST_PATH, "designated_notebooks": 0 }),
            ),
        };

        NotebookLmState {
            status: if configured {
                "configured"
            } else {
                "unconfigured"
            }
            .to_string(),
            auth: json!({
                "profile_configured": configured,
                "profile_path": NLM_PROFILE,
                "note": "cookie sessions are short-lived; long batches run laptop-side or in bursts"
            }),
            master_notebook: master,
            corpus,
            config: json!({
                "cli": "nlm",
                "transport": "nlm CLI / NotebookLM batchexecute",
                "ingest_pipeline": "OD-23 (Voyage->Qdrant + Cozo graph)"
            }),
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

pub(crate) fn notebooklm_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(NotebookLmPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "notebooklm",
        "data",
        "1.0.0",
        "NotebookLM knowledge notebooks — auth status, designated corpus, master notebook",
        &state,
    )
}
