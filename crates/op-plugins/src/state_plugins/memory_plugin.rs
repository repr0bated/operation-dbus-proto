use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_json::json;
use simd_json::OwnedValue as Value;

/// Memory plugin state.
/// See: https://github.com/memgraph/memgraph
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryState {
    /// Plugin status
    pub status: String,
    /// Backend type (sqlite, sled, etc.)
    pub backend: String,
    /// Memory namespaces
    #[schemars(with = "serde_json::Value")]
    pub namespaces: JsonValue,
    /// Memory statistics
    #[schemars(with = "serde_json::Value")]
    pub stats: JsonValue,
    /// Memory configuration
    #[schemars(with = "serde_json::Value")]
    pub config: JsonValue,
}
pub struct MemoryPlugin;
impl Default for MemoryPlugin {
    fn default() -> Self {
        Self
    }
}
impl MemoryPlugin {
    pub fn new() -> Self {
        Self
    }
    pub(crate) fn current_state() -> MemoryState {
        MemoryState {
            status: "active".to_string(),
            backend: "sqlite".to_string(),
            namespaces: json!([{"name": "default", "entries": 0, "max_entries": 0, "read_only": false},{"name": "soul", "entries": 0,"max_entries": 0}, {"name": "audit","entries": 0}]),
            stats: json!({"total_entries": 0, "total_namespaces": 3, "auto_save": true, "hygiene_enabled": true, "archive_after_days": 7, "purge_after_days": 30, "cosine_cache_size": 10000, "chunk_max_tokens": 512}),
            config: json!({"backend": "sqlite", "embedding_provider": "none", "embedding_model": "text-embedding-3-small", "embedding_dimensions": 1536, "search_mode": "hybrid", "min_relevance_score": 0.4, "vector_weight": 0.7, "keyword_weight": 0.3, "rerank_enabled": false}),
        }
    }
}
#[async_trait]
impl StatePlugin for MemoryPlugin {
    fn name(&self) -> &str {
        "memory"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(memory_schema())
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

pub(crate) fn memory_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(super::memory_plugin::MemoryPlugin::current_state())
            .unwrap_or_else(|_| simd_json::json!({}));
    schema_from_state(
        "memory",
        "data",
        "1.0.0",
        "Cognitive memory — namespaces, embeddings, search",
        &state,
    )
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("memory", |_ctx| std::sync::Arc::new(MemoryPlugin::new()))
}
