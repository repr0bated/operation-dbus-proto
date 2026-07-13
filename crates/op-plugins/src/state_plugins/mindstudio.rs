//! MindStudio Model Router plugin surface — the identity-gated D-Bus/gRPC
//! entry point for the MindStudio-hosted routing/UI-generation agent
//! (`RoutingAgent-UIGenerator`, grok-4.1-fast).
//!
//! This is distinct from `op-mindstudio-shim`, the loopback OpenAI-compat
//! proxy zeroclaw uses directly on `127.0.0.1:8093` (no identity, not on the
//! plugin surface). This plugin exposes the same underlying MindStudio
//! `apps/run` call, but through `PluginService.CallMethod` so it is reachable
//! via `zcall`/gRPC with Ghostbridge identity attached like every other
//! command, and notarized in the event chain.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// Top-level state for the MindStudio surface. Config only — MindStudio
/// itself is stateless per-run (no server-side session beyond `threadId`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.mindstudio.schema@v1"))]
pub struct MindstudioState {
    /// MindStudio app id being invoked.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.mindstudio.app-id@v1"))]
    pub app_id: String,
    /// MindStudio API base URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.mindstudio.endpoint@v1"))]
    pub endpoint: String,
    /// Reachability of the MindStudio API, as last observed by `run` ("online"/"offline").
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.mindstudio.status@v1"))]
    pub status: String,
}

pub struct MindstudioPlugin;

impl Default for MindstudioPlugin {
    fn default() -> Self {
        Self
    }
}

impl MindstudioPlugin {
    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    fn config_state() -> MindstudioState {
        MindstudioState {
            app_id: Self::env_or("MINDSTUDIO_APP_ID", ""),
            endpoint: Self::env_or(
                "MINDSTUDIO_RUN_URL",
                "https://api.mindstudio.ai/developer/v2/apps/run",
            ),
            status: "unknown".to_string(),
        }
    }
}

#[async_trait]
impl StatePlugin for MindstudioPlugin {
    fn name(&self) -> &str {
        "mindstudio"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = mindstudio_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-declared".to_string(),
                desired_hash: "schema-declared".to_string(),
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
            state_snapshot: simd_json::serde::to_owned_value(Self::config_state())?,
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

/// Canonical `mindstudio` schema derived from [`MindstudioState`] via schemars.
pub(crate) fn mindstudio_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(MindstudioState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "mindstudio",
        "1.0.0",
        "MindStudio Model Router surface (RoutingAgent-UIGenerator, grok-4.1-fast) reached via apps/run, identity-gated",
        &root,
    );

    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RunInput {
        /// Prompt/variables passed as the MindStudio launch variable(s).
        pub prompt: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RunOutput {
        pub success: bool,
        pub thread_id: Option<String>,
        pub result: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetConfigOutput {
        pub config: serde_json::Value,
    }

    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "run".to_string(),
        method_decl_from_schemars_with_output::<RunInput, RunOutput>(
            "run",
            SideEffect::Mutation,
            false,
            "mindstudio.invoke",
            "mut.service.mindstudio.run@v1",
        ),
    );
    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<(), GetConfigOutput>(
            "get_config",
            SideEffect::Read,
            true,
            "mindstudio.read",
            "obs.service.mindstudio.config.get@v1",
        ),
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("mindstudio", |_ctx| std::sync::Arc::new(MindstudioPlugin::new()))
}
