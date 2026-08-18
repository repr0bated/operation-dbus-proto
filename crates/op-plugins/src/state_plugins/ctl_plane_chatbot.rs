//! Control-plane chatbot reasoning episode plugin
//!
//! Declares the canonical schema for the chatbot's reasoning episodes —
//! every field, PII classification, significance, and vectorization contract.
//! THE PLUGIN IS THE SCHEMA: downstream (Qdrant, CozoDB, Accountability UI,
//! gRPC EventChainService) inherits from this definition.
//!
//! Related: REQ-1 through REQ-10 (Control-Plane Chatbot Reasoning Episode
//! Vectorization spec). This plugin covers REQ-2 (episode record fields)
//! and REQ-3 (plugin schema registration).

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

const DEFAULT_COLLECTION: &str = "ctl_plane_reasoning_episodes";
const DEFAULT_VECTOR_DIMS: u32 = 1024;
const DEFAULT_DEDUP_WINDOW_HRS: u32 = 24;
const DEFAULT_QUEUE_ALERT_THRESHOLD: u32 = 50;
const DEFAULT_NESTING_POLICY: &str = "flat";
const DEFAULT_OUTPUT_DTYPE: &str = "float";
const DEFAULT_INPUT_TYPE: &str = "document";

// ── Config (vectorization pipeline tuning) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CtlPlaneChatbotConfig {
    #[serde(default = "default_embedding_plugin")]
    pub embedding_plugin: String,
    #[serde(default = "default_collection")]
    pub qdrant_collection: String,
    #[serde(default)]
    pub vector_dims: u32,
    #[serde(default = "default_output_dtype")]
    pub output_dtype: String,
    #[serde(default = "default_input_type")]
    pub input_type: String,
    #[serde(default)]
    pub dedup_window_hrs: u32,
    #[serde(default)]
    pub queue_alert_threshold: u32,
    #[serde(default = "default_nesting_policy")]
    pub nesting_policy: String,
    #[serde(default = "default_true")]
    pub vectorization_enabled: bool,
}

fn default_embedding_plugin() -> String {
    "embedding_model".into()
}
fn default_collection() -> String {
    DEFAULT_COLLECTION.into()
}
fn default_chat_llm_plugin() -> String {
    "large_language_model".into()
}
fn default_nesting_policy() -> String {
    DEFAULT_NESTING_POLICY.into()
}
fn default_output_dtype() -> String {
    DEFAULT_OUTPUT_DTYPE.into()
}
fn default_input_type() -> String {
    DEFAULT_INPUT_TYPE.into()
}
fn default_true() -> bool {
    true
}

impl Default for CtlPlaneChatbotConfig {
    fn default() -> Self {
        Self {
            embedding_plugin: default_embedding_plugin(),
            qdrant_collection: default_collection(),
            vector_dims: DEFAULT_VECTOR_DIMS,
            output_dtype: default_output_dtype(),
            input_type: default_input_type(),
            dedup_window_hrs: DEFAULT_DEDUP_WINDOW_HRS,
            queue_alert_threshold: DEFAULT_QUEUE_ALERT_THRESHOLD,
            nesting_policy: default_nesting_policy(),
            vectorization_enabled: true,
        }
    }
}

// ── Schema state (schemars-derived) ─────────────────────────────────────────

/// Matryoshka output dimensions. A 2048-dim voyage-4 vector's leading *k* entries
/// (256/512/1024) are themselves a valid k-dim embedding, so a collection can be
/// truncated to a shorter dim later **without re-vectorizing** (leading-k +
/// re-normalize). Every vector in one Qdrant collection must still share one dim;
/// 1024 is the trio default.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
pub enum MatryoshkaDim {
    #[serde(rename = "256")]
    D256,
    #[serde(rename = "512")]
    D512,
    #[serde(rename = "1024")]
    D1024,
    #[serde(rename = "2048")]
    D2048,
}

impl Default for MatryoshkaDim {
    fn default() -> Self {
        MatryoshkaDim::D1024
    }
}

impl MatryoshkaDim {
    /// Numeric dimensionality.
    pub fn dims(&self) -> u32 {
        match self {
            MatryoshkaDim::D256 => 256,
            MatryoshkaDim::D512 => 512,
            MatryoshkaDim::D1024 => 1024,
            MatryoshkaDim::D2048 => 2048,
        }
    }
}

/// Output precision / quantization for stored vectors. `float` (default, highest
/// accuracy) → `int8`/`uint8` (4× smaller) → `binary`/`ubinary` (32× smaller,
/// bit-packed). Qdrant stores and searches all of these natively. Changing dtype
/// is a precision change, **not** a re-vectorize.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutputDtype {
    Float,
    Int8,
    Uint8,
    Binary,
    Ubinary,
}

impl Default for OutputDtype {
    fn default() -> Self {
        OutputDtype::Float
    }
}

/// Retrieval `input_type` (Voyage input_type). Voyage prepends a retrieval prompt
/// based on this: documents embed as `document`, queries as `query`. The Voyage
/// FAQ says **not** to omit it / use `None` for retrieval, so `None` is
/// intentionally not offered here. Stored reasoning episodes are documents →
/// default `document`; the search side uses `query`. Embeddings with and without
/// input_type stay compatible, so this is a quality knob, not a space knob.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    Query,
    Document,
}

impl Default for InputType {
    fn default() -> Self {
        InputType::Document
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NestingPolicy {
    Flat,
    Nested,
}

impl Default for NestingPolicy {
    fn default() -> Self {
        NestingPolicy::Flat
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Goal,
    ToolResult,
    Interrupt,
    Replan,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    ToolCall,
    ResponseEmitted,
    DirectionChange,
    GoalAchieved,
    ConfigSet,
    TaskScheduled,
    Interrupt,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeClass {
    GoalAchieved,
    ConfigSet,
    TaskScheduled,
    Delegated,
    Interrupted,
    DirectionChanged,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
pub enum SignificanceLevel {
    Contextual,
    Signal,
}

impl Default for SignificanceLevel {
    fn default() -> Self {
        SignificanceLevel::Contextual
    }
}

fn default_dedup_window_hrs() -> u32 {
    DEFAULT_DEDUP_WINDOW_HRS
}
fn default_queue_alert_threshold() -> u32 {
    DEFAULT_QUEUE_ALERT_THRESHOLD
}
fn default_running() -> bool {
    true
}
fn default_reasoning_active() -> bool {
    false
}
fn default_embedding_queue_depth() -> u32 {
    0
}
fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReasoningEpisode {
    #[schemars(
        description = "Unique ID (UUID v7 for time-ordering)",
        example = &"01912abc-def0-7abc-8def-0123456789ab",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.episode-id@v1")
    )]
    pub episode_id: String,
    #[schemars(
        description = "ISO-8601 timestamp of reasoning entry",
        example = &"2025-05-29T14:30:00Z",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.started-at@v1")
    )]
    pub started_at: String,
    #[schemars(
        description = "ISO-8601 timestamp of reasoning exit",
        example = &"2025-05-29T14:30:05Z",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.ended-at@v1")
    )]
    pub ended_at: String,
    #[schemars(
        description = "Wall-clock duration in milliseconds",
        example = 5000,
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.duration-ms@v1")
    )]
    pub duration_ms: u64,
    #[schemars(
        description = "What caused reasoning to start",
        example = &"goal",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.trigger@v1")
    )]
    pub trigger: Trigger,
    #[schemars(
        description = "What ended reasoning",
        example = &"tool_call",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.exit-reason@v1")
    )]
    pub exit_reason: ExitReason,
    #[schemars(
        description = "High-level goal or prompt active at episode start [PII]",
        example = &"Configure VLAN isolation for tenant-3",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.goal-text@v1")
    )]
    pub goal_text: Option<String>,
    #[schemars(
        description = "Compact natural-language summary of reasoning — primary embedding input [PII]",
        example = &"Evaluated 3 bridge configs, chose br-tenant3 for isolation",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.reasoning-summary@v1")
    )]
    pub reasoning_summary: String,
    #[serde(default)]
    #[schemars(
        description = "Ordered list of tools/plugins/MCP calls made during the episode",
        example = &["ovs_list_bridges", "ovs_create_bridge"],
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.tools-consulted@v1")
    )]
    pub tools_consulted: Vec<String>,
    #[schemars(
        description = "The decision, plan, or action the episode produced [PII]",
        example = &"Create br-tenant3 with VLAN 103 tagged ports",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.decision-output@v1")
    )]
    pub decision_output: Option<String>,
    #[schemars(
        description = "Classification of episode outcome. goal_achieved/config_set/task_scheduled => Signal significance",
        example = &"config_set",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.outcome-class@v1")
    )]
    pub outcome_class: OutcomeClass,
    #[schemars(
        range(min = 0.0, max = 1.0),
        description = "Optional confidence 0.0-1.0 if the model emits one",
        example = 0.87,
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.confidence@v1")
    )]
    pub confidence: Option<f64>,
    #[schemars(
        description = "Plugin that owns the context being reasoned about",
        example = &"ovsdb_bridge",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.plugin-id@v1")
    )]
    pub plugin_id: Option<String>,
    #[schemars(
        description = "Groups episodes belonging to the same high-level task chain",
        example = &"vlan-isolation-task-3",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.conversation-id@v1")
    )]
    pub conversation_id: Option<String>,
    #[schemars(
        description = "SHA-256 of canonical serialized record — for exact dedup before upsert (REQ-7)",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.content-hash@v1")
    )]
    pub content_hash: String,
    #[serde(default = "default_false")]
    #[schemars(
        description = "If true, reasoning_summary and decision_output are redacted before vectorization (REQ-8)",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.pii-flagged@v1")
    )]
    pub pii_flagged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Significance {
    #[schemars(
        description = "Reasoning episodes are always at least Contextual. goal_achieved/config_set/task_scheduled => Signal",
        example = &"Signal",
        extend("readOnly" = true),
        extend("default" = &"Contextual"),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.significance-level@v1")
    )]
    pub level: SignificanceLevel,
    #[schemars(
        description = "Significance rule that was evaluated",
        example = &"outcome_class in [goal_achieved, config_set, task_scheduled]",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.significance-rule@v1")
    )]
    pub rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.ctl-plane-chatbot.schema@v1"))]
#[schemars(extend("x-oscal-category" = "llm"))]
pub struct CtlPlaneChatbotState {
    #[serde(default = "default_chat_llm_plugin")]
    #[schemars(
        description = "Generation plugin the chatbot delegates to for chat completion (default large_language_model). Provider/endpoint/runtime live there; the chatbot only selects its own model via chat_model.",
        example = &"large_language_model",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.llm-plugin@v1")
    )]
    pub llm_plugin: String,
    #[serde(default)]
    #[schemars(
        description = "The chatbot's own selectable generation model id — chosen independently of the unified system model. Empty falls back to the generation surface's resolved model; a value overrides it for the chatbot only.",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.chat-model@v1")
    )]
    pub chat_model: String,
    #[serde(default = "default_embedding_plugin")]
    #[schemars(
        description = "Embedding surface this chatbot delegates to for vectorizing reasoning episodes (default embedding_model). The model/provider/dimensions live in that plugin; the chatbot holds no embedding model of its own.",
        example = &"embedding_model",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.embedding-plugin@v1")
    )]
    pub embedding_plugin: String,
    #[serde(default = "default_collection")]
    #[schemars(
        description = "Qdrant collection name (REQ-5). Separate from mutation/schema footprints",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.qdrant-collection@v1")
    )]
    pub qdrant_collection: String,
    #[serde(default)]
    #[schemars(
        description = "Matryoshka output dimension (Voyage output_dimension). One per Qdrant collection; can be truncated to a shorter dim later (leading-k) without re-vectorizing. Default 1024.",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.vector-dims@v1")
    )]
    pub vector_dims: MatryoshkaDim,
    #[serde(default)]
    #[schemars(
        description = "Output precision / quantization (Voyage output_dtype): float (default) → int8/uint8 (4x smaller) → binary/ubinary (32x). Qdrant stores all natively. Changing dtype is a precision change, not a re-vectorize.",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.output-dtype@v1")
    )]
    pub output_dtype: OutputDtype,
    #[serde(default)]
    #[schemars(
        description = "Retrieval input_type (Voyage). Documents embed as 'document', queries as 'query' — Voyage prepends a retrieval prompt. None/omitted is discouraged for retrieval so it is not offered. Stored episodes default to document.",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.input-type@v1")
    )]
    pub input_type: InputType,
    #[serde(default = "default_dedup_window_hrs")]
    #[schemars(
        description = "Content-hash dedup collision window in hours (REQ-7, default 24)",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.dedup-window-hrs@v1")
    )]
    pub dedup_window_hrs: u32,
    #[serde(default = "default_queue_alert_threshold")]
    #[schemars(
        description = "Alert if embedding queue depth exceeds this (REQ-10, default 50)",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.queue-alert-threshold@v1")
    )]
    pub queue_alert_threshold: u32,
    #[serde(default)]
    #[schemars(
        description = "REQ-1: flat = new trigger extends current episode; nested = opens new episode",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.nesting-policy@v1")
    )]
    pub nesting_policy: NestingPolicy,
    #[serde(default = "default_true")]
    #[schemars(
        description = "Enable Voyage embedding pipeline for reasoning episodes",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.vectorization-enabled@v1")
    )]
    pub vectorization_enabled: bool,
    #[serde(default = "default_running")]
    #[schemars(
        description = "Whether the chatbot is currently active",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.running@v1")
    )]
    pub running: bool,
    #[serde(default = "default_reasoning_active")]
    #[schemars(
        description = "Whether the chatbot is currently in reasoning state (REQ-1)",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.reasoning-active@v1")
    )]
    pub reasoning_active: bool,
    #[serde(default = "default_embedding_queue_depth")]
    #[schemars(
        description = "Current Voyage embedding queue depth (alert at queue_alert_threshold)",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.embedding-queue-depth@v1")
    )]
    pub embedding_queue_depth: u32,
    #[schemars(
        description = "ISO-8601 timestamp of last successful Qdrant upsert",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.last-vectorized-at@v1")
    )]
    pub last_vectorized_at: Option<String>,
    #[schemars(
        description = "Qdrant vector UUID on the identity sled — binds every vectorized episode to this identity",
        example = &"a1b2c3d4-e5f6-7890-abcd-ef0123456789",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.vector-id@v1")
    )]
    pub vector_id: Option<String>,
    #[schemars(
        description = "REQ-2: Structured record produced at reasoning exit. Primary unit of vectorization.",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.reasoning-episode@v1")
    )]
    pub reasoning_episode: Option<ReasoningEpisode>,
    #[schemars(
        description = "REQ-3: Always at least Contextual. goal_achieved/config_set/task_scheduled => Signal",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.ctl-plane-chatbot.significance@v1")
    )]
    pub significance: Option<Significance>,
}

// ── Plugin struct ────────────────────────────────────────────────────────────

pub struct CtlPlaneChatbotPlugin;

impl CtlPlaneChatbotPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CtlPlaneChatbotPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// ── StatePlugin impl ─────────────────────────────────────────────────────────

#[async_trait]
impl StatePlugin for CtlPlaneChatbotPlugin {
    fn name(&self) -> &str {
        "ctl_plane_chatbot"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = ctl_plane_chatbot_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    fn is_available(&self) -> bool {
        // Always available — the chatbot is the control plane itself
        true
    }

    fn unavailable_reason(&self) -> String {
        String::new()
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let cur: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(current.clone())?;
        let des: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();
        macro_rules! field_diff {
            ($field:ident, $key:expr) => {
                if cur.$field != des.$field {
                    actions.push(StateAction::Modify {
                        resource: $key.into(),
                        changes: simd_json::serde::to_owned_value(&des.$field)?,
                    });
                }
            };
        }
        field_diff!(embedding_plugin, "embedding_plugin");
        field_diff!(qdrant_collection, "qdrant_collection");
        field_diff!(vector_dims, "vector_dims");
        field_diff!(output_dtype, "output_dtype");
        field_diff!(input_type, "input_type");
        field_diff!(dedup_window_hrs, "dedup_window_hrs");
        field_diff!(queue_alert_threshold, "queue_alert_threshold");
        field_diff!(nesting_policy, "nesting_policy");
        field_diff!(vectorization_enabled, "vectorization_enabled");

        Ok(StateDiff {
            plugin: self.name().into(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        // Pipeline config changes are applied at next episode close — no service reload
        let changes: Vec<String> = diff
            .actions
            .iter()
            .map(|a| {
                format!(
                    "ctl_plane_chatbot.{} queued",
                    match a {
                        StateAction::Modify { resource, .. } => resource.clone(),
                        StateAction::Create { resource, .. } => resource.clone(),
                        StateAction::Delete { resource } => resource.clone(),
                        StateAction::NoOp { resource } => resource.clone(),
                    }
                )
            })
            .collect();
        Ok(ApplyResult {
            success: true,
            changes_applied: changes,
            errors: Vec::new(),
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = simd_json::json!(null);
        Ok(Checkpoint {
            id: format!("ctl_plane_chatbot-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let _ = checkpoint;
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

pub(crate) fn ctl_plane_chatbot_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(CtlPlaneChatbotState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "ctl_plane_chatbot",
        "1.0.0",
        "Control-plane chatbot reasoning episodes — THE PLUGIN IS THE SCHEMA. Declares every episode field (REQ-2), PII tagging (REQ-8), significance classification (REQ-3), and vectorization pipeline config (REQ-4/5/6/7). Downstream (Qdrant, CozoDB, Accountability UI, EventChainService) inherits.",
        &root,
    );

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetConfigOutput {
        pub config: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListEpisodesOutput {
        pub episodes: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetEpisodeOutput {
        pub episode: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct QueryContextOutput {
        pub results: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ClassifySignificanceOutput {
        pub significance: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct VectorizeOutput {
        pub vector_id: String,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<(), GetConfigOutput>(
            "get_config",
            SideEffect::Read,
            true,
            "chatbot.read",
            "obs.service.ctl-plane-chatbot.config.get@v1",
        ),
    );
    schema.methods.insert(
        "set_config".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "set_config",
            SideEffect::Mutation,
            false,
            "chatbot.invoke",
            "mut.service.ctl-plane-chatbot.config.set@v1",
        ),
    );
    schema.methods.insert(
        "list_episodes".to_string(),
        method_decl_from_schemars_with_output::<(), ListEpisodesOutput>(
            "list_episodes",
            SideEffect::Read,
            true,
            "chatbot.read",
            "obs.service.ctl-plane-chatbot.episode.list@v1",
        ),
    );
    schema.methods.insert(
        "get_episode".to_string(),
        method_decl_from_schemars_with_output::<(), GetEpisodeOutput>(
            "get_episode",
            SideEffect::Read,
            true,
            "chatbot.read",
            "obs.service.ctl-plane-chatbot.episode.get@v1",
        ),
    );
    schema.methods.insert(
        "query_context".to_string(),
        method_decl_from_schemars_with_output::<(), QueryContextOutput>(
            "query_context",
            SideEffect::Read,
            true,
            "chatbot.read",
            "obs.service.ctl-plane-chatbot.context.query@v1",
        ),
    );
    schema.methods.insert(
        "classify_significance".to_string(),
        method_decl_from_schemars_with_output::<(), ClassifySignificanceOutput>(
            "classify_significance",
            SideEffect::Mutation,
            false,
            "chatbot.invoke",
            "mut.service.ctl-plane-chatbot.classify@v1",
        ),
    );
    schema.methods.insert(
        "vectorize".to_string(),
        method_decl_from_schemars_with_output::<(), VectorizeOutput>(
            "vectorize",
            SideEffect::Mutation,
            false,
            "chatbot.invoke",
            "mut.service.ctl-plane-chatbot.vectorize@v1",
        ),
    );

    schema.capabilities.insert(
        "chatbot.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "chatbot.read".to_string(),
            description: "Grants: get_config, list_episodes, get_episode, query_context."
                .to_string(),
        },
    );
    schema.capabilities.insert(
        "chatbot.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "chatbot.invoke".to_string(),
            description: "Grants: set_config, classify_significance, vectorize.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(CtlPlaneChatbotState)).unwrap();
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
    crate::default_registry::PluginReg::new("ctl_plane_chatbot", |_ctx| std::sync::Arc::new(CtlPlaneChatbotPlugin::new()))
}
