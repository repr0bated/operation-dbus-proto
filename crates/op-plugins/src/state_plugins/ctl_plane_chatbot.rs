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
        Some(super::plugin_schema_defs::ctl_plane_chatbot_plugin_schema())
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
