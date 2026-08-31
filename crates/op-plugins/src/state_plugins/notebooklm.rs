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
use op_state_store::SideEffect;
use op_state_store::{CapabilityDecl, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::path::Path;

use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
use super::plugin_scaffold_helpers::AckOutput;

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
#[schemars(extend("x-oscal-category" = "service"))]
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
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "list_notebooks",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.list@v1",
        ),
    );
    schema.methods.insert(
        "get_notebook".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "get_notebook",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.get@v1",
        ),
    );
    schema.methods.insert(
        "select_notebook".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "select_notebook",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.notebook.select@v1",
        ),
    );
    schema.methods.insert(
        "search_notebooks".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "search_notebooks",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.search@v1",
        ),
    );
    schema.methods.insert(
        "query_notebook".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "query_notebook",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.notebook.query@v1",
        ),
    );
    schema.methods.insert(
        "get_library_stats".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "get_library_stats",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.library.stats@v1",
        ),
    );
    schema.methods.insert(
        "cross_notebook_query".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
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
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "list_sources",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.source.list@v1",
        ),
    );
    schema.methods.insert(
        "get_source_content".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "get_source_content",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.source.content@v1",
        ),
    );
    schema.methods.insert(
        "add_source_url".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "add_source_url",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-url@v1",
        ),
    );
    schema.methods.insert(
        "add_source_text".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "add_source_text",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-text@v1",
        ),
    );
    schema.methods.insert(
        "add_source_drive".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "add_source_drive",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-drive@v1",
        ),
    );
    schema.methods.insert(
        "add_source_file".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "add_source_file",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.add-file@v1",
        ),
    );
    schema.methods.insert(
        "delete_source".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "delete_source",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.delete@v1",
        ),
    );
    schema.methods.insert(
        "sync_drive_sources".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
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
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "auto_label_sources",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-auto@v1",
        ),
    );
    schema.methods.insert(
        "create_label".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-create@v1",
        ),
    );
    schema.methods.insert(
        "rename_label".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "rename_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-rename@v1",
        ),
    );
    schema.methods.insert(
        "set_label_emoji".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "set_label_emoji",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-emoji@v1",
        ),
    );
    schema.methods.insert(
        "move_source_label".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "move_source_label",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.source.label-move@v1",
        ),
    );
    schema.methods.insert(
        "delete_label".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
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
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_audio",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.audio.create@v1",
        ),
    );
    schema.methods.insert(
        "create_video".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_video",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.video.create@v1",
        ),
    );
    schema.methods.insert(
        "create_report".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_report",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.report.create@v1",
        ),
    );
    schema.methods.insert(
        "create_quiz".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_quiz",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.quiz.create@v1",
        ),
    );
    schema.methods.insert(
        "create_flashcards".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_flashcards",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.flashcards.create@v1",
        ),
    );
    schema.methods.insert(
        "create_infographic".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_infographic",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.infographic.create@v1",
        ),
    );
    schema.methods.insert(
        "create_mindmap".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_mindmap",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.mindmap.create@v1",
        ),
    );
    schema.methods.insert(
        "create_slides".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_slides",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.slides.create@v1",
        ),
    );
    schema.methods.insert(
        "revise_slides".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "revise_slides",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.studio.slides.revise@v1",
        ),
    );
    schema.methods.insert(
        "describe_studio".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "describe_studio",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.describe@v1",
        ),
    );
    schema.methods.insert(
        "get_audio_status".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "get_audio_status",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.audio.status@v1",
        ),
    );
    schema.methods.insert(
        "list_artifacts".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "list_artifacts",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.studio.artifact.list@v1",
        ),
    );
    schema.methods.insert(
        "download_artifact".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
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
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "start_research",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.research.start@v1",
        ),
    );
    schema.methods.insert(
        "import_research".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "import_research",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.research.import@v1",
        ),
    );
    schema.methods.insert(
        "share_public".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "share_public",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.public@v1",
        ),
    );
    schema.methods.insert(
        "share_invite".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "share_invite",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.invite@v1",
        ),
    );
    schema.methods.insert(
        "get_share_settings".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "get_share_settings",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.share.settings@v1",
        ),
    );
    schema.methods.insert(
        "disable_share".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "disable_share",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.share.disable@v1",
        ),
    );
    schema.methods.insert(
        "batch_operation".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "batch_operation",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.batch.run@v1",
        ),
    );
    schema.methods.insert(
        "run_pipeline".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "run_pipeline",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.pipeline.run@v1",
        ),
    );
    schema.methods.insert(
        "list_pipelines".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "list_pipelines",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.pipeline.list@v1",
        ),
    );
    schema.methods.insert(
        "tag_add".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "tag_add",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.tag.add@v1",
        ),
    );
    schema.methods.insert(
        "tag_list".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "tag_list",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.tag.list@v1",
        ),
    );
    schema.methods.insert(
        "tag_smart_select".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
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
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "list_sessions",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.session.list@v1",
        ),
    );
    schema.methods.insert(
        "close_session".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "close_session",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.session.close@v1",
        ),
    );
    schema.methods.insert(
        "reset_session".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "reset_session",
            SideEffect::Mutation,
            false,
            "notebooklm.invoke",
            "mut.service.plugin.notebooklm.session.reset@v1",
        ),
    );
    schema.methods.insert(
        "get_health".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "get_health",
            SideEffect::Read,
            true,
            "notebooklm.read",
            "obs.service.plugin.notebooklm.health@v1",
        ),
    );
    schema.methods.insert(
        "setup_auth".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "setup_auth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.setup@v1",
        ),
    );
    schema.methods.insert(
        "refresh_auth".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "refresh_auth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.refresh@v1",
        ),
    );
    schema.methods.insert(
        "reauth".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "reauth",
            SideEffect::Mutation,
            false,
            "notebooklm.admin",
            "mut.service.plugin.notebooklm.auth.reauth@v1",
        ),
    );

    schema.capabilities.insert(
        "notebooklm.read".to_string(),
        CapabilityDecl {
            id: "notebooklm.read".to_string(),
            description: "Grants: list_notebooks, get_notebook, search_notebooks, query_notebook, get_library_stats, cross_notebook_query, list_sources, get_source_content.".to_string(),
        },
    );
    schema.capabilities.insert(
        "notebooklm.invoke".to_string(),
        CapabilityDecl {
            id: "notebooklm.invoke".to_string(),
            description: "Grants: select_notebook, add_source_url, add_source_text, add_source_drive, add_source_file, delete_source, sync_drive_sources, auto_label_sources, create_label, rename_label, set_label_emoji, move_source_label, delete_label, create_audio, create_video, create_report, create_quiz, create_flashcards, create_infographic, create_slides, revise_slides, describe_studio, get_audio_status, list_artifacts, download_artifact, start_research, import_research, share_public, share_invite, get_share_settings, disable_share, batch_operation, list_sessions, close_session, reset_session, get_health, setup_auth, refresh_auth, reauth.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
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
