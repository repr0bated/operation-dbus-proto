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
#[cfg(test)]
use op_state_store::{Constraint, FieldSchema, FieldType};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
#[cfg(test)]
use std::collections::HashMap;

const DEFAULT_VOYAGE_MODEL: &str = "voyage-4-lite";
const DEFAULT_COLLECTION: &str = "ctl_plane_reasoning_episodes";
const DEFAULT_VECTOR_DIMS: u32 = 1024;
const DEFAULT_DEDUP_WINDOW_HRS: u32 = 24;
const DEFAULT_QUEUE_ALERT_THRESHOLD: u32 = 50;
const DEFAULT_NESTING_POLICY: &str = "flat";

// ── Config (vectorization pipeline tuning) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CtlPlaneChatbotConfig {
    #[serde(default = "default_voyage_model")]
    pub voyage_model: String,
    #[serde(default = "default_collection")]
    pub qdrant_collection: String,
    #[serde(default)]
    pub vector_dims: u32,
    #[serde(default)]
    pub dedup_window_hrs: u32,
    #[serde(default)]
    pub queue_alert_threshold: u32,
    #[serde(default = "default_nesting_policy")]
    pub nesting_policy: String,
    #[serde(default = "default_true")]
    pub vectorization_enabled: bool,
}

fn default_voyage_model() -> String {
    DEFAULT_VOYAGE_MODEL.into()
}
fn default_collection() -> String {
    DEFAULT_COLLECTION.into()
}
fn default_nesting_policy() -> String {
    DEFAULT_NESTING_POLICY.into()
}
fn default_true() -> bool {
    true
}

impl Default for CtlPlaneChatbotConfig {
    fn default() -> Self {
        Self {
            voyage_model: default_voyage_model(),
            qdrant_collection: default_collection(),
            vector_dims: DEFAULT_VECTOR_DIMS,
            dedup_window_hrs: DEFAULT_DEDUP_WINDOW_HRS,
            queue_alert_threshold: DEFAULT_QUEUE_ALERT_THRESHOLD,
            nesting_policy: default_nesting_policy(),
            vectorization_enabled: true,
        }
    }
}

// ── Schema state (schemars-derived) ─────────────────────────────────────────

/// Interchangeable Voyage-4 embedding models.
///
/// `voyage-4-large`, `voyage-4` and `voyage-4-lite` share **one** embedding
/// space: vectors from any of the three are directly comparable, so all three
/// write to the *same* Qdrant collection at the *same* dimensionality
/// ([`VoyageModel::SHARED_EMBEDDING_DIMS`]). Switching models is a quality/cost
/// tradeoff only — it never requires re-vectorizing existing data.
///
/// `voyage-4-nano` is **deliberately excluded**: it does not share this space
/// (different dimensionality), so mixing it in would silently break cross-model
/// comparison and force a re-index. If it is ever needed it must get its own
/// collection, not this one.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VoyageModel {
    #[serde(rename = "voyage-4-large")]
    Voyage4Large,
    #[serde(rename = "voyage-4")]
    Voyage4,
    #[serde(rename = "voyage-4-lite")]
    Voyage4Lite,
}

impl Default for VoyageModel {
    fn default() -> Self {
        // POC target; cheapest of the shared-space trio.
        VoyageModel::Voyage4Lite
    }
}

impl VoyageModel {
    /// The single shared embedding dimensionality for the interchangeable
    /// voyage-4 trio. One collection, no re-index across models.
    pub const SHARED_EMBEDDING_DIMS: u32 = 1024;

    /// The interchangeable models, all in the shared embedding space.
    pub const SHARED_SPACE: [VoyageModel; 3] = [
        VoyageModel::Voyage4Large,
        VoyageModel::Voyage4,
        VoyageModel::Voyage4Lite,
    ];

    /// Canonical Voyage API model id.
    pub fn model_id(&self) -> &'static str {
        match self {
            VoyageModel::Voyage4Large => "voyage-4-large",
            VoyageModel::Voyage4 => "voyage-4",
            VoyageModel::Voyage4Lite => "voyage-4-lite",
        }
    }

    /// Output dimensionality. Identical across the trio so their vectors fuse
    /// and compare in the same Qdrant collection.
    pub fn dimensions(&self) -> u32 {
        Self::SHARED_EMBEDDING_DIMS
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

fn default_vector_dims() -> u32 {
    DEFAULT_VECTOR_DIMS
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
pub struct CtlPlaneChatbotState {
    #[serde(default)]
    #[schemars(
        description = "Voyage embedding model for reasoning episodes (REQ-4). The three voyage-4 models share one embedding space (1024-dim, one Qdrant collection) — switching never requires re-vectorizing. voyage-4-nano is excluded (different dim). POC target: voyage-4-lite",
        example = &"voyage-4",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.voyage-model@v1")
    )]
    pub voyage_model: VoyageModel,
    #[serde(default = "default_collection")]
    #[schemars(
        description = "Qdrant collection name (REQ-5). Separate from mutation/schema footprints",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.qdrant-collection@v1")
    )]
    pub qdrant_collection: String,
    #[serde(default = "default_vector_dims")]
    #[schemars(
        description = "Vector dimensions (1024 for voyage-4-lite, flexible post-POC)",
        extend("x-oscal-subid" = "mut.software.plugin.ctl-plane-chatbot.vector-dims@v1")
    )]
    pub vector_dims: u32,
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

    async fn query_current_state(&self) -> Result<Value> {
        let cfg = simd_json::serde::to_owned_value(CtlPlaneChatbotConfig::default())?;
        Ok(cfg)
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
        field_diff!(voyage_model, "voyage_model");
        field_diff!(qdrant_collection, "qdrant_collection");
        field_diff!(vector_dims, "vector_dims");
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

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let cur: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(current)?;
        let des: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(cur == des)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
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
    super::schemars_adapter::plugin_schema_from_json(
        "ctl_plane_chatbot",
        "1.0.0",
        "Control-plane chatbot reasoning episodes — THE PLUGIN IS THE SCHEMA. Declares every episode field (REQ-2), PII tagging (REQ-8), significance classification (REQ-3), and vectorization pipeline config (REQ-4/5/6/7). Downstream (Qdrant, CozoDB, Accountability UI, EventChainService) inherits.",
        &root,
    )
}

/// Frozen golden reference for the `ctl_plane_chatbot` schema.
#[cfg(test)]
pub(crate) fn ctl_plane_chatbot_schema_golden() -> PluginSchema {
    let mut schema = ctl_plane_chatbot_schema_inner();
    schema.subids.insert(
        "__schema__".to_string(),
        "sch.software.plugin.ctl-plane-chatbot.schema@v1".to_string(),
    );
    for (field, subid) in [
        (
            "voyage_model",
            "mut.software.plugin.ctl-plane-chatbot.voyage-model@v1",
        ),
        (
            "qdrant_collection",
            "mut.software.plugin.ctl-plane-chatbot.qdrant-collection@v1",
        ),
        (
            "vector_dims",
            "mut.software.plugin.ctl-plane-chatbot.vector-dims@v1",
        ),
        (
            "dedup_window_hrs",
            "mut.software.plugin.ctl-plane-chatbot.dedup-window-hrs@v1",
        ),
        (
            "queue_alert_threshold",
            "mut.software.plugin.ctl-plane-chatbot.queue-alert-threshold@v1",
        ),
        (
            "nesting_policy",
            "mut.software.plugin.ctl-plane-chatbot.nesting-policy@v1",
        ),
        (
            "vectorization_enabled",
            "mut.software.plugin.ctl-plane-chatbot.vectorization-enabled@v1",
        ),
        (
            "running",
            "obs.software.plugin.ctl-plane-chatbot.running@v1",
        ),
        (
            "reasoning_active",
            "obs.software.plugin.ctl-plane-chatbot.reasoning-active@v1",
        ),
        (
            "embedding_queue_depth",
            "obs.software.plugin.ctl-plane-chatbot.embedding-queue-depth@v1",
        ),
        (
            "last_vectorized_at",
            "obs.software.plugin.ctl-plane-chatbot.last-vectorized-at@v1",
        ),
        (
            "vector_id",
            "obs.software.plugin.ctl-plane-chatbot.vector-id@v1",
        ),
        (
            "reasoning_episode",
            "obs.software.plugin.ctl-plane-chatbot.reasoning-episode@v1",
        ),
        (
            "significance",
            "obs.software.plugin.ctl-plane-chatbot.significance@v1",
        ),
    ] {
        schema.subids.insert(field.to_string(), subid.to_string());
    }
    schema
}

#[cfg(test)]
fn ctl_plane_chatbot_schema_inner() -> PluginSchema {
    // ── REQ-2: Reasoning Episode Record sub-object ──────────────────────────
    let reasoning_episode_fields = {
        let mut fields = HashMap::new();
        // Core identity
        fields.insert(
            "episode_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unique ID (UUID v7 for time-ordering)".to_string(),
                default: None,
                example: Some(json!("01912abc-def0-7abc-8def-0123456789ab")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "started_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "ISO-8601 timestamp of reasoning entry".to_string(),
                default: None,
                example: Some(json!("2025-05-29T14:30:00Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "ended_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "ISO-8601 timestamp of reasoning exit".to_string(),
                default: None,
                example: Some(json!("2025-05-29T14:30:05Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "duration_ms".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Wall-clock duration in milliseconds".to_string(),
                default: None,
                example: Some(json!(5000)),
                constraints: vec![Constraint::Min { value: 0.0 }],
                read_only: true,
                read_only_when: None,
            },
        );
        // Lifecycle
        fields.insert(
            "trigger".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "goal".to_string(),
                    "tool_result".to_string(),
                    "interrupt".to_string(),
                    "replan".to_string(),
                    "system_event".to_string(),
                ]),
                required: true,
                description: "What caused reasoning to start".to_string(),
                default: None,
                example: Some(json!("goal")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "exit_reason".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "tool_call".to_string(),
                    "response_emitted".to_string(),
                    "direction_change".to_string(),
                    "goal_achieved".to_string(),
                    "config_set".to_string(),
                    "task_scheduled".to_string(),
                    "interrupt".to_string(),
                ]),
                required: true,
                description: "What ended reasoning".to_string(),
                default: None,
                example: Some(json!("tool_call")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        // Content — PII-tagged per REQ-8
        fields.insert(
            "goal_text".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "High-level goal or prompt active at episode start [PII]".to_string(),
                default: None,
                example: Some(json!("Configure VLAN isolation for tenant-3")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "reasoning_summary".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description:
                    "Compact natural-language summary of reasoning — primary embedding input [PII]"
                        .to_string(),
                default: None,
                example: Some(json!(
                    "Evaluated 3 bridge configs, chose br-tenant3 for isolation"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "tools_consulted".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Ordered list of tools/plugins/MCP calls made during the episode"
                    .to_string(),
                default: Some(json!([])),
                example: Some(json!(["ovs_list_bridges", "ovs_create_bridge"])),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "decision_output".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "The decision, plan, or action the episode produced [PII]".to_string(),
                default: None,
                example: Some(json!("Create br-tenant3 with VLAN 103 tagged ports")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        // Outcome
        fields.insert("outcome_class".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec![
                "goal_achieved".to_string(), "config_set".to_string(),
                "task_scheduled".to_string(), "delegated".to_string(),
                "interrupted".to_string(), "direction_changed".to_string(),
                "inconclusive".to_string(),
            ]),
            required: true,
            description: "Classification of episode outcome. goal_achieved/config_set/task_scheduled => Signal significance".to_string(),
            default: None, example: Some(json!("config_set")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert(
            "confidence".to_string(),
            FieldSchema {
                field_type: FieldType::Float,
                required: false,
                description: "Optional confidence 0.0-1.0 if the model emits one".to_string(),
                default: None,
                example: Some(json!(0.87)),
                constraints: vec![
                    Constraint::Min { value: 0.0 },
                    Constraint::Max { value: 1.0 },
                ],
                read_only: true,
                read_only_when: None,
            },
        );
        // Grouping
        fields.insert(
            "plugin_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Plugin that owns the context being reasoned about".to_string(),
                default: None,
                example: Some(json!("ovsdb_bridge")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "conversation_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Groups episodes belonging to the same high-level task chain"
                    .to_string(),
                default: None,
                example: Some(json!("vlan-isolation-task-3")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        // Integrity + Privacy
        fields.insert(
            "content_hash".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description:
                    "SHA-256 of canonical serialized record — for exact dedup before upsert (REQ-7)"
                        .to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert("pii_flagged".to_string(), FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "If true, reasoning_summary and decision_output are redacted before vectorization (REQ-8)".to_string(),
            default: Some(json!(false)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields
    };

    // ── Significance classification sub-object (REQ-3) ───────────────────────
    let significance_fields = {
        let mut fields = HashMap::new();
        fields.insert("level".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec!["Contextual".to_string(), "Signal".to_string()]),
            required: true,
            description: "Reasoning episodes are always at least Contextual. goal_achieved/config_set/task_scheduled => Signal".to_string(),
            default: Some(json!("Contextual")), example: Some(json!("Signal")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert(
            "rule".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Significance rule that was evaluated".to_string(),
                default: None,
                example: Some(json!(
                    "outcome_class in [goal_achieved, config_set, task_scheduled]"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields
    };

    let schema = PluginSchema::builder("ctl_plane_chatbot")
        .version("1.0.0")
        .description("Control-plane chatbot reasoning episodes — THE PLUGIN IS THE SCHEMA. Declares every episode field (REQ-2), PII tagging (REQ-8), significance classification (REQ-3), and vectorization pipeline config (REQ-4/5/6/7). Downstream (Qdrant, CozoDB, Accountability UI, EventChainService) inherits.")
        // ── Pipeline config (tunable) ──────────────────────────────────────
        .field("voyage_model", FieldSchema {
            field_type: FieldType::Enum(vec![
                "voyage-4-large".to_string(), "voyage-4".to_string(), "voyage-4-lite".to_string(),
            ]),
            required: false,
            description: "Voyage embedding model for reasoning episodes (REQ-4). The three voyage-4 models share one embedding space (1024-dim, one Qdrant collection) — switching never requires re-vectorizing. voyage-4-nano is excluded (different dim). POC target: voyage-4-lite".to_string(),
            default: Some(json!("voyage-4-lite")), example: Some(json!("voyage-4")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("qdrant_collection", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Qdrant collection name (REQ-5). Separate from mutation/schema footprints".to_string(),
            default: Some(json!("ctl_plane_reasoning_episodes")), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("vector_dims", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Vector dimensions (1024 for voyage-4-lite, flexible post-POC)".to_string(),
            default: Some(json!(1024)), example: None,
            constraints: vec![Constraint::Min { value: 0.0 }], read_only: false, read_only_when: None,
        })
        .field("dedup_window_hrs", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Content-hash dedup collision window in hours (REQ-7, default 24)".to_string(),
            default: Some(json!(24)), example: None,
            constraints: vec![Constraint::Min { value: 0.0 }], read_only: false, read_only_when: None,
        })
        .field("queue_alert_threshold", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Alert if embedding queue depth exceeds this (REQ-10, default 50)".to_string(),
            default: Some(json!(50)), example: None,
            constraints: vec![Constraint::Min { value: 0.0 }], read_only: false, read_only_when: None,
        })
        .field("nesting_policy", FieldSchema {
            field_type: FieldType::Enum(vec!["flat".to_string(), "nested".to_string()]),
            required: false,
            description: "REQ-1: flat = new trigger extends current episode; nested = opens new episode".to_string(),
            default: Some(json!("flat")), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("vectorization_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable Voyage embedding pipeline for reasoning episodes".to_string(),
            default: Some(json!(true)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        // ── Observed state (read-only from pipeline) ───────────────────────
        .field("running", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the chatbot is currently active".to_string(),
            default: Some(json!(true)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("reasoning_active", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the chatbot is currently in reasoning state (REQ-1)".to_string(),
            default: Some(json!(false)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("embedding_queue_depth", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Current Voyage embedding queue depth (alert at queue_alert_threshold)".to_string(),
            default: Some(json!(0)), example: None,
            constraints: vec![Constraint::Min { value: 0.0 }], read_only: true, read_only_when: None,
        })
        .field("last_vectorized_at", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "ISO-8601 timestamp of last successful Qdrant upsert".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── Vector ID on sled (identity-bound) ───────────────────────────
        .field("vector_id", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Qdrant vector UUID on the identity sled — binds every vectorized episode to this identity".to_string(),
            default: None, example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef0123456789")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── REQ-2: Reasoning Episode Record ────────────────────────────────
        .field("reasoning_episode", FieldSchema {
            field_type: FieldType::Object(reasoning_episode_fields), required: false,
            description: "REQ-2: Structured record produced at reasoning exit. Primary unit of vectorization.".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── REQ-3: Significance classification ─────────────────────────────
        .field("significance", FieldSchema {
            field_type: FieldType::Object(significance_fields), required: false,
            description: "REQ-3: Always at least Contextual. goal_achieved/config_set/task_scheduled => Signal".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .build();
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let diffs = crate::state_plugins::schemars_adapter::schema_diffs(
            &ctl_plane_chatbot_schema_golden(),
            &ctl_plane_chatbot_schema(),
        );
        assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
    }

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
