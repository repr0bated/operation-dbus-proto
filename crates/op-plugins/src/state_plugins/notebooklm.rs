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
use op_state_store::SideEffect;
#[cfg(test)]
use op_state_store::{Constraint, FieldSchema, FieldType};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::path::Path;

use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
use super::plugin_scaffold_helpers::EmptyInput;

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
                ingest_pipeline: "OD-23 (embedding_model->Qdrant + Cozo graph)".to_string(),
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

// ── Typed method inputs/outputs for the NotebookLM sidecar bridge ──────────────
//
// These give `zcall notebooklm <method>` real, schema-validated arguments
// instead of the empty `()` input that produced echo-only responses. Field
// names mirror the NotebookLM MCP tool arguments; the dispatch in
// `op-grpc-bridge::notebooklm_dispatch` forwards them to the live sidecar.
// Outputs are the permissive [`NotebookLmResult`] envelope because NotebookLM
// tools return dynamic JSON.

/// Result envelope returned by every NotebookLM method (the sidecar's raw JSON
/// under `result`, or an `error` string on failure).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotebookLmResult {
    /// Whether the underlying NotebookLM tool call succeeded.
    pub success: bool,
    /// The plugin method that was invoked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// The resolved NotebookLM sidecar tool that serviced the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Raw JSON result returned by the NotebookLM MCP tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error message when the call failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Reference to a single notebook.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbNotebookRef {
    /// NotebookLM notebook identifier.
    pub notebook_id: String,
}

/// Free-text search across notebooks.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbSearch {
    /// Search query.
    pub query: String,
    /// Maximum number of results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Ask a question against one notebook.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbQuery {
    /// Notebook to query.
    pub notebook_id: String,
    /// Natural-language question.
    pub question: String,
}

/// Ask a question spanning multiple notebooks.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbCrossQuery {
    /// Natural-language question.
    pub question: String,
    /// Notebooks to include (defaults to the designated corpus).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notebook_ids: Option<Vec<String>>,
}

/// Reference to a source within a notebook.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbSourceRef {
    /// Owning notebook.
    pub notebook_id: String,
    /// Source identifier.
    pub source_id: String,
}

/// Add a URL source.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbAddUrl {
    /// Target notebook.
    pub notebook_id: String,
    /// Source URL.
    pub url: String,
}

/// Add a text source.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbAddText {
    /// Target notebook.
    pub notebook_id: String,
    /// Source title.
    pub title: String,
    /// Source body text.
    pub content: String,
}

/// Add a Google Drive source.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbAddDrive {
    /// Target notebook.
    pub notebook_id: String,
    /// Google Drive file/document URL.
    pub drive_url: String,
}

/// Add a local file source.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbAddFile {
    /// Target notebook.
    pub notebook_id: String,
    /// Path to the file to upload.
    pub file_path: String,
}

/// Sync a notebook's sources from a Google Drive folder.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbSyncDrive {
    /// Target notebook.
    pub notebook_id: String,
    /// Google Drive folder identifier to sync from.
    pub folder_id: String,
}

/// Create a source label.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbCreateLabel {
    /// Target notebook.
    pub notebook_id: String,
    /// Label name.
    pub name: String,
    /// Optional emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
}

/// Rename a source label.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbRenameLabel {
    /// Target notebook.
    pub notebook_id: String,
    /// Label identifier.
    pub label_id: String,
    /// New label name.
    pub name: String,
}

/// Set a label's emoji.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbLabelEmoji {
    /// Target notebook.
    pub notebook_id: String,
    /// Label identifier.
    pub label_id: String,
    /// Emoji to set.
    pub emoji: String,
}

/// Move a source to a label.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbMoveLabel {
    /// Target notebook.
    pub notebook_id: String,
    /// Source to move.
    pub source_id: String,
    /// Destination label.
    pub label_id: String,
}

/// Reference to a label within a notebook.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbLabelRef {
    /// Owning notebook.
    pub notebook_id: String,
    /// Label identifier.
    pub label_id: String,
}

/// Generate a studio artifact (audio, video, report, quiz, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbStudioCreate {
    /// Target notebook.
    pub notebook_id: String,
    /// Optional generation instructions / focus prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Revise an existing slides artifact.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbReviseSlides {
    /// Target notebook.
    pub notebook_id: String,
    /// Slides artifact to revise.
    pub artifact_id: String,
    /// Revision instructions.
    pub instructions: String,
}

/// Reference to a studio artifact.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbArtifactRef {
    /// Owning notebook.
    pub notebook_id: String,
    /// Artifact identifier.
    pub artifact_id: String,
}

/// Query audio-generation status.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbAudioStatus {
    /// Owning notebook.
    pub notebook_id: String,
    /// Specific artifact (defaults to the latest audio job).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
}

/// Start a research task.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbResearchStart {
    /// Target notebook.
    pub notebook_id: String,
    /// Research topic / prompt.
    pub topic: String,
}

/// Import research results.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbResearchImport {
    /// Target notebook.
    pub notebook_id: String,
    /// Research job identifier to import.
    pub research_id: String,
}

/// Invite a collaborator.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbShareInvite {
    /// Notebook to share.
    pub notebook_id: String,
    /// Invitee email address.
    pub email: String,
}

/// Run a batch of operations.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbBatch {
    /// Ordered list of operation descriptors.
    pub operations: Vec<serde_json::Value>,
}

/// Run a named pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbRunPipeline {
    /// Pipeline identifier.
    pub pipeline_id: String,
    /// Optional pipeline parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Tag a source.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbTagAdd {
    /// Owning notebook.
    pub notebook_id: String,
    /// Source to tag.
    pub source_id: String,
    /// Tag value.
    pub tag: String,
}

/// Smart-select sources by query.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbTagSmartSelect {
    /// Owning notebook.
    pub notebook_id: String,
    /// Selection query.
    pub query: String,
}

/// Reference to a session.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbSessionRef {
    /// Session identifier.
    pub session_id: String,
}

/// Reset a session (or all sessions when omitted).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbResetSession {
    /// Specific session to reset (defaults to the active session).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Configure authentication.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NbSetupAuth {
    /// NotebookLM auth cookie (falls back to the `NOTEBOOKLM_COOKIE` env).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
}

/// NotebookLM schema derived from the typed [`NotebookLmState`] struct via schemars.
pub(crate) fn notebooklm_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(NotebookLmState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "notebooklm",
        "1.0.0",
        "NotebookLM knowledge notebooks — auth status, designated corpus, master notebook",
        &root,
    );

    // Add methods to schema with typed returns
    schema.methods.insert(
        "list_notebooks".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "list_notebooks",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.list@v1",
        ),
    );
    schema.methods.insert(
        "get_notebook".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "get_notebook",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.get@v1",
        ),
    );
    schema.methods.insert(
        "select_notebook".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "select_notebook",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.notebook.select@v1",
        ),
    );
    schema.methods.insert(
        "search_notebooks".to_string(),
        method_decl_from_schemars_with_output::<NbSearch, NotebookLmResult>(
            "search_notebooks",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.search@v1",
        ),
    );
    schema.methods.insert(
        "query_notebook".to_string(),
        method_decl_from_schemars_with_output::<NbQuery, NotebookLmResult>(
            "query_notebook",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.query@v1",
        ),
    );
    schema.methods.insert(
        "get_library_stats".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "get_library_stats",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.library.stats@v1",
        ),
    );
    schema.methods.insert(
        "cross_notebook_query".to_string(),
        method_decl_from_schemars_with_output::<NbCrossQuery, NotebookLmResult>(
            "cross_notebook_query",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.cross.query@v1",
        ),
    );

    // === Source methods (R/M) ===
    schema.methods.insert(
        "list_sources".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "list_sources",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.source.list@v1",
        ),
    );
    schema.methods.insert(
        "get_source_content".to_string(),
        method_decl_from_schemars_with_output::<NbSourceRef, NotebookLmResult>(
            "get_source_content",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.source.content@v1",
        ),
    );
    schema.methods.insert(
        "add_source_url".to_string(),
        method_decl_from_schemars_with_output::<NbAddUrl, NotebookLmResult>(
            "add_source_url",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-url@v1",
        ),
    );
    schema.methods.insert(
        "add_source_text".to_string(),
        method_decl_from_schemars_with_output::<NbAddText, NotebookLmResult>(
            "add_source_text",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-text@v1",
        ),
    );
    schema.methods.insert(
        "add_source_drive".to_string(),
        method_decl_from_schemars_with_output::<NbAddDrive, NotebookLmResult>(
            "add_source_drive",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-drive@v1",
        ),
    );
    schema.methods.insert(
        "add_source_file".to_string(),
        method_decl_from_schemars_with_output::<NbAddFile, NotebookLmResult>(
            "add_source_file",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-file@v1",
        ),
    );
    schema.methods.insert(
        "delete_source".to_string(),
        method_decl_from_schemars_with_output::<NbSourceRef, NotebookLmResult>(
            "delete_source",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.delete@v1",
        ),
    );
    schema.methods.insert(
        "sync_drive_sources".to_string(),
        method_decl_from_schemars_with_output::<NbSyncDrive, NotebookLmResult>(
            "sync_drive_sources",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.sync-drive@v1",
        ),
    );

    // === Label methods (M) ===
    schema.methods.insert(
        "auto_label_sources".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "auto_label_sources",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-auto@v1",
        ),
    );
    schema.methods.insert(
        "create_label".to_string(),
        method_decl_from_schemars_with_output::<NbCreateLabel, NotebookLmResult>(
            "create_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-create@v1",
        ),
    );
    schema.methods.insert(
        "rename_label".to_string(),
        method_decl_from_schemars_with_output::<NbRenameLabel, NotebookLmResult>(
            "rename_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-rename@v1",
        ),
    );
    schema.methods.insert(
        "set_label_emoji".to_string(),
        method_decl_from_schemars_with_output::<NbLabelEmoji, NotebookLmResult>(
            "set_label_emoji",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-emoji@v1",
        ),
    );
    schema.methods.insert(
        "move_source_label".to_string(),
        method_decl_from_schemars_with_output::<NbMoveLabel, NotebookLmResult>(
            "move_source_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-move@v1",
        ),
    );
    schema.methods.insert(
        "delete_label".to_string(),
        method_decl_from_schemars_with_output::<NbLabelRef, NotebookLmResult>(
            "delete_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-delete@v1",
        ),
    );

    // === Studio content methods (M) ===
    schema.methods.insert(
        "create_audio".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_audio",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.audio.create@v1",
        ),
    );
    schema.methods.insert(
        "create_video".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_video",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.video.create@v1",
        ),
    );
    schema.methods.insert(
        "create_report".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_report",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.report.create@v1",
        ),
    );
    schema.methods.insert(
        "create_quiz".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_quiz",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.quiz.create@v1",
        ),
    );
    schema.methods.insert(
        "create_flashcards".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_flashcards",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.flashcards.create@v1",
        ),
    );
    schema.methods.insert(
        "create_infographic".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_infographic",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.infographic.create@v1",
        ),
    );
    schema.methods.insert(
        "create_mindmap".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "create_mindmap",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.mindmap.create@v1",
        ),
    );
    schema.methods.insert(
        "create_slides".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_slides",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.slides.create@v1",
        ),
    );
    schema.methods.insert(
        "revise_slides".to_string(),
        method_decl_from_schemars_with_output::<NbReviseSlides, NotebookLmResult>(
            "revise_slides",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.slides.revise@v1",
        ),
    );
    schema.methods.insert(
        "describe_studio".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "describe_studio",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.describe@v1",
        ),
    );
    schema.methods.insert(
        "get_audio_status".to_string(),
        method_decl_from_schemars_with_output::<NbAudioStatus, NotebookLmResult>(
            "get_audio_status",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.audio.status@v1",
        ),
    );
    schema.methods.insert(
        "list_artifacts".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "list_artifacts",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.artifact.list@v1",
        ),
    );
    schema.methods.insert(
        "download_artifact".to_string(),
        method_decl_from_schemars_with_output::<NbArtifactRef, NotebookLmResult>(
            "download_artifact",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.artifact.download@v1",
        ),
    );

    // === Research / share / batch (M) ===
    schema.methods.insert(
        "start_research".to_string(),
        method_decl_from_schemars_with_output::<NbResearchStart, NotebookLmResult>(
            "start_research",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.research.start@v1",
        ),
    );
    schema.methods.insert(
        "import_research".to_string(),
        method_decl_from_schemars_with_output::<NbResearchImport, NotebookLmResult>(
            "import_research",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.research.import@v1",
        ),
    );
    schema.methods.insert(
        "share_public".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "share_public",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.public@v1",
        ),
    );
    schema.methods.insert(
        "share_invite".to_string(),
        method_decl_from_schemars_with_output::<NbShareInvite, NotebookLmResult>(
            "share_invite",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.invite@v1",
        ),
    );
    schema.methods.insert(
        "get_share_settings".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "get_share_settings",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.share.settings@v1",
        ),
    );
    schema.methods.insert(
        "disable_share".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "disable_share",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.disable@v1",
        ),
    );
    schema.methods.insert(
        "batch_operation".to_string(),
        method_decl_from_schemars_with_output::<NbBatch, NotebookLmResult>(
            "batch_operation",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.batch.run@v1",
        ),
    );
    schema.methods.insert(
        "run_pipeline".to_string(),
        method_decl_from_schemars_with_output::<NbRunPipeline, NotebookLmResult>(
            "run_pipeline",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.pipeline.run@v1",
        ),
    );
    schema.methods.insert(
        "list_pipelines".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "list_pipelines",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.pipeline.list@v1",
        ),
    );
    schema.methods.insert(
        "tag_add".to_string(),
        method_decl_from_schemars_with_output::<NbTagAdd, NotebookLmResult>(
            "tag_add",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.tag.add@v1",
        ),
    );
    schema.methods.insert(
        "tag_list".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "tag_list",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.tag.list@v1",
        ),
    );
    schema.methods.insert(
        "tag_smart_select".to_string(),
        method_decl_from_schemars_with_output::<NbTagSmartSelect, NotebookLmResult>(
            "tag_smart_select",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.tag.select@v1",
        ),
    );

    // === Sessions / auth / health (R/M) ===
    schema.methods.insert(
        "list_sessions".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "list_sessions",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.session.list@v1",
        ),
    );
    schema.methods.insert(
        "close_session".to_string(),
        method_decl_from_schemars_with_output::<NbSessionRef, NotebookLmResult>(
            "close_session",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.session.close@v1",
        ),
    );
    schema.methods.insert(
        "reset_session".to_string(),
        method_decl_from_schemars_with_output::<NbResetSession, NotebookLmResult>(
            "reset_session",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.session.reset@v1",
        ),
    );
    schema.methods.insert(
        "get_health".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "get_health",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.health@v1",
        ),
    );
    schema.methods.insert(
        "setup_auth".to_string(),
        method_decl_from_schemars_with_output::<NbSetupAuth, NotebookLmResult>(
            "setup_auth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.setup@v1",
        ),
    );
    schema.methods.insert(
        "refresh_auth".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "refresh_auth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.refresh@v1",
        ),
    );
    schema.methods.insert(
        "reauth".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "reauth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.reauth@v1",
        ),
    );

    schema
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

    // === Notebook methods (R) ===
    schema.methods.insert(
        "list_notebooks".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "list_notebooks",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.list@v1",
        ),
    );
    schema.methods.insert(
        "get_notebook".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "get_notebook",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.get@v1",
        ),
    );
    schema.methods.insert(
        "select_notebook".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "select_notebook",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.notebook.select@v1",
        ),
    );
    schema.methods.insert(
        "search_notebooks".to_string(),
        method_decl_from_schemars_with_output::<NbSearch, NotebookLmResult>(
            "search_notebooks",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.search@v1",
        ),
    );
    schema.methods.insert(
        "query_notebook".to_string(),
        method_decl_from_schemars_with_output::<NbQuery, NotebookLmResult>(
            "query_notebook",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.query@v1",
        ),
    );
    schema.methods.insert(
        "get_library_stats".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "get_library_stats",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.library.stats@v1",
        ),
    );
    schema.methods.insert(
        "cross_notebook_query".to_string(),
        method_decl_from_schemars_with_output::<NbCrossQuery, NotebookLmResult>(
            "cross_notebook_query",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.cross.query@v1",
        ),
    );

    // === Source methods (R/M) ===
    schema.methods.insert(
        "list_sources".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "list_sources",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.source.list@v1",
        ),
    );
    schema.methods.insert(
        "get_source_content".to_string(),
        method_decl_from_schemars_with_output::<NbSourceRef, NotebookLmResult>(
            "get_source_content",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.source.content@v1",
        ),
    );
    schema.methods.insert(
        "add_source_url".to_string(),
        method_decl_from_schemars_with_output::<NbAddUrl, NotebookLmResult>(
            "add_source_url",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-url@v1",
        ),
    );
    schema.methods.insert(
        "add_source_text".to_string(),
        method_decl_from_schemars_with_output::<NbAddText, NotebookLmResult>(
            "add_source_text",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-text@v1",
        ),
    );
    schema.methods.insert(
        "add_source_drive".to_string(),
        method_decl_from_schemars_with_output::<NbAddDrive, NotebookLmResult>(
            "add_source_drive",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-drive@v1",
        ),
    );
    schema.methods.insert(
        "add_source_file".to_string(),
        method_decl_from_schemars_with_output::<NbAddFile, NotebookLmResult>(
            "add_source_file",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-file@v1",
        ),
    );
    schema.methods.insert(
        "delete_source".to_string(),
        method_decl_from_schemars_with_output::<NbSourceRef, NotebookLmResult>(
            "delete_source",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.delete@v1",
        ),
    );
    schema.methods.insert(
        "sync_drive_sources".to_string(),
        method_decl_from_schemars_with_output::<NbSyncDrive, NotebookLmResult>(
            "sync_drive_sources",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.sync-drive@v1",
        ),
    );

    // === Label methods (M) ===
    schema.methods.insert(
        "auto_label_sources".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "auto_label_sources",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-auto@v1",
        ),
    );
    schema.methods.insert(
        "create_label".to_string(),
        method_decl_from_schemars_with_output::<NbCreateLabel, NotebookLmResult>(
            "create_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-create@v1",
        ),
    );
    schema.methods.insert(
        "rename_label".to_string(),
        method_decl_from_schemars_with_output::<NbRenameLabel, NotebookLmResult>(
            "rename_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-rename@v1",
        ),
    );
    schema.methods.insert(
        "set_label_emoji".to_string(),
        method_decl_from_schemars_with_output::<NbLabelEmoji, NotebookLmResult>(
            "set_label_emoji",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-emoji@v1",
        ),
    );
    schema.methods.insert(
        "move_source_label".to_string(),
        method_decl_from_schemars_with_output::<NbMoveLabel, NotebookLmResult>(
            "move_source_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-move@v1",
        ),
    );
    schema.methods.insert(
        "delete_label".to_string(),
        method_decl_from_schemars_with_output::<NbLabelRef, NotebookLmResult>(
            "delete_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-delete@v1",
        ),
    );

    // === Studio content methods (M) ===
    schema.methods.insert(
        "create_audio".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_audio",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.audio.create@v1",
        ),
    );
    schema.methods.insert(
        "create_video".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_video",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.video.create@v1",
        ),
    );
    schema.methods.insert(
        "create_report".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_report",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.report.create@v1",
        ),
    );
    schema.methods.insert(
        "create_quiz".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_quiz",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.quiz.create@v1",
        ),
    );
    schema.methods.insert(
        "create_flashcards".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_flashcards",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.flashcards.create@v1",
        ),
    );
    schema.methods.insert(
        "create_infographic".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_infographic",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.infographic.create@v1",
        ),
    );
    schema.methods.insert(
        "create_mindmap".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "create_mindmap",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.mindmap.create@v1",
        ),
    );
    schema.methods.insert(
        "create_slides".to_string(),
        method_decl_from_schemars_with_output::<NbStudioCreate, NotebookLmResult>(
            "create_slides",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.slides.create@v1",
        ),
    );
    schema.methods.insert(
        "revise_slides".to_string(),
        method_decl_from_schemars_with_output::<NbReviseSlides, NotebookLmResult>(
            "revise_slides",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.slides.revise@v1",
        ),
    );
    schema.methods.insert(
        "describe_studio".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "describe_studio",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.describe@v1",
        ),
    );
    schema.methods.insert(
        "get_audio_status".to_string(),
        method_decl_from_schemars_with_output::<NbAudioStatus, NotebookLmResult>(
            "get_audio_status",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.audio.status@v1",
        ),
    );
    schema.methods.insert(
        "list_artifacts".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "list_artifacts",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.artifact.list@v1",
        ),
    );
    schema.methods.insert(
        "download_artifact".to_string(),
        method_decl_from_schemars_with_output::<NbArtifactRef, NotebookLmResult>(
            "download_artifact",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.artifact.download@v1",
        ),
    );

    // === Research / share / batch (M) ===
    schema.methods.insert(
        "start_research".to_string(),
        method_decl_from_schemars_with_output::<NbResearchStart, NotebookLmResult>(
            "start_research",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.research.start@v1",
        ),
    );
    schema.methods.insert(
        "import_research".to_string(),
        method_decl_from_schemars_with_output::<NbResearchImport, NotebookLmResult>(
            "import_research",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.research.import@v1",
        ),
    );
    schema.methods.insert(
        "share_public".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "share_public",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.public@v1",
        ),
    );
    schema.methods.insert(
        "share_invite".to_string(),
        method_decl_from_schemars_with_output::<NbShareInvite, NotebookLmResult>(
            "share_invite",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.invite@v1",
        ),
    );
    schema.methods.insert(
        "get_share_settings".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "get_share_settings",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.share.settings@v1",
        ),
    );
    schema.methods.insert(
        "disable_share".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "disable_share",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.disable@v1",
        ),
    );
    schema.methods.insert(
        "batch_operation".to_string(),
        method_decl_from_schemars_with_output::<NbBatch, NotebookLmResult>(
            "batch_operation",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.batch.run@v1",
        ),
    );
    schema.methods.insert(
        "run_pipeline".to_string(),
        method_decl_from_schemars_with_output::<NbRunPipeline, NotebookLmResult>(
            "run_pipeline",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.pipeline.run@v1",
        ),
    );
    schema.methods.insert(
        "list_pipelines".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "list_pipelines",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.pipeline.list@v1",
        ),
    );
    schema.methods.insert(
        "tag_add".to_string(),
        method_decl_from_schemars_with_output::<NbTagAdd, NotebookLmResult>(
            "tag_add",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.tag.add@v1",
        ),
    );
    schema.methods.insert(
        "tag_list".to_string(),
        method_decl_from_schemars_with_output::<NbNotebookRef, NotebookLmResult>(
            "tag_list",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.tag.list@v1",
        ),
    );
    schema.methods.insert(
        "tag_smart_select".to_string(),
        method_decl_from_schemars_with_output::<NbTagSmartSelect, NotebookLmResult>(
            "tag_smart_select",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.tag.select@v1",
        ),
    );

    // === Sessions / auth / health (R/M) ===
    schema.methods.insert(
        "list_sessions".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "list_sessions",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.session.list@v1",
        ),
    );
    schema.methods.insert(
        "close_session".to_string(),
        method_decl_from_schemars_with_output::<NbSessionRef, NotebookLmResult>(
            "close_session",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.session.close@v1",
        ),
    );
    schema.methods.insert(
        "reset_session".to_string(),
        method_decl_from_schemars_with_output::<NbResetSession, NotebookLmResult>(
            "reset_session",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.session.reset@v1",
        ),
    );
    schema.methods.insert(
        "get_health".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "get_health",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.health@v1",
        ),
    );
    schema.methods.insert(
        "setup_auth".to_string(),
        method_decl_from_schemars_with_output::<NbSetupAuth, NotebookLmResult>(
            "setup_auth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.setup@v1",
        ),
    );
    schema.methods.insert(
        "refresh_auth".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "refresh_auth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.refresh@v1",
        ),
    );
    schema.methods.insert(
        "reauth".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, NotebookLmResult>(
            "reauth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.reauth@v1",
        ),
    );

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

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("notebooklm", |_ctx| std::sync::Arc::new(NotebookLmPlugin::new()))
}
