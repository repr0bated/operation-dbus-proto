use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::SideEffect;
use op_state_store::{FieldSchema, FieldType, CapabilityDecl, PluginSchema};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;

/// Memory plugin state.
/// See: https://github.com/memgraph/memgraph
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-category" = "data"))]
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
            config: json!({"backend": "sqlite", "embedding_plugin": "embedding_model", "search_mode": "hybrid", "min_relevance_score": 0.4, "vector_weight": 0.7, "keyword_weight": 0.3, "rerank_enabled": false}),
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
    async fn calculate_diff(
        &self,
        _current: &Value,
        _desired: &Value,
    ) -> anyhow::Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: Vec::new(),
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }
    async fn apply_state(&self, _diff: &StateDiff) -> anyhow::Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }
    async fn verify_state(&self, _desired: &Value) -> anyhow::Result<bool> {
        Ok(true)
    }
    async fn create_checkpoint(&self) -> anyhow::Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?,
            backend_checkpoint: None,
        })
    }
    async fn rollback(&self, _checkpoint: &Checkpoint) -> anyhow::Result<()> {
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

/// Request memory namespace creation or retrieval.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateNamespaceInput {
    /// Namespace identifier
    pub name: String,
    /// Maximum entries allowed
    pub max_entries: usize,
    /// Whether the namespace is read-only
    pub read_only: bool,
    /// Namespace configuration
    #[schemars(with = "serde_json::Value")]
    pub config: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateNamespaceOutput {
    /// Success status
    pub success: bool,
    /// Actual max entries configured
    pub actual_max_entries: usize,
    /// Current entry count
    pub current_entries: usize,
    /// Full namespace details
    #[schemars(with = "serde_json::Value")]
    pub namespace: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteNamespaceInput {
    /// Namespace identifier to delete
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteNamespaceOutput {
    /// Success status
    pub success: bool,
    /// Entries that were deleted
    pub deleted_entries: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QueryNamespaceInput {
    /// Namespace identifier to query
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QueryNamespaceOutput {
    /// Found namespace details
    #[schemars(with = "serde_json::Value")]
    pub namespace: JsonValue,
    /// Current entries count
    pub entries: usize,
    /// Namespace metadata
    #[schemars(with = "serde_json::Value")]
    pub metadata: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InsertEntryInput {
    /// Namespace identifier
    pub namespace: String,
    /// Key-value pair to insert
    #[schemars(with = "serde_json::Value")]
    pub kv: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InsertEntryOutput {
    /// Success status
    pub success: bool,
    /// Entry count after insertion
    pub new_entry_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateEntryInput {
    /// Namespace identifier
    pub namespace: String,
    /// Entry key to update
    pub key: String,
    /// New value
    #[schemars(with = "serde_json::Value")]
    pub value: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateEntryOutput {
    /// Success status
    pub success: bool,
    /// Previous value (if any)
    #[schemars(with = "serde_json::Value")]
    pub previous_value: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetEntryInput {
    /// Namespace identifier
    pub namespace: String,
    /// Entry key to retrieve
    pub key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetEntryOutput {
    /// Found entry value
    #[schemars(with = "serde_json::Value")]
    pub value: JsonValue,
    /// Whether the entry existed
    pub found: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteEntryInput {
    /// Namespace identifier
    pub namespace: String,
    /// Entry key to delete
    pub key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteEntryOutput {
    /// Success status
    pub success: bool,
    /// Whether entry was deleted
    pub deleted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClearNamespaceInput {
    /// Namespace identifier to clear
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClearNamespaceOutput {
    /// Success status
    pub success: bool,
    /// Entries that were deleted
    pub deleted_entries: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetStatsInput {
    #[schemars(with = "serde_json::Value")]
    pub options: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetStatsOutput {
    /// Memory statistics
    #[schemars(with = "serde_json::Value")]
    pub stats: JsonValue,
    /// System memory usage
    #[schemars(with = "serde_json::Value")]
    pub system_memory: JsonValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompactInput {
    /// Target namespace(s) for compaction
    pub namespace: Option<String>,
    /// Minimum compaction threshold
    pub min_threshold: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompactOutput {
    /// Success status
    pub success: bool,
    /// Bytes freed during compaction
    pub freed_bytes: usize,
    /// New fragmentation level
    pub fragmentation_level: f64,
}

pub(crate) fn memory_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(MemoryState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "memory",
        "1.0.0",
        "Cognitive memory — namespaces, entries, search",
        &root,
    );

    // Add methods to schema with typed returns
    schema.methods.insert(
        "create_namespace".to_string(),
        method_decl_from_schemars_with_output::<CreateNamespaceInput, CreateNamespaceOutput>(
            "create_namespace",
            SideEffect::Mutation,
            false,
            "memory.namespace.create",
            "mut.memory.memory.create-namespace@v1",
        ),
    );
    schema.methods.insert(
        "delete_namespace".to_string(),
        method_decl_from_schemars_with_output::<DeleteNamespaceInput, DeleteNamespaceOutput>(
            "delete_namespace",
            SideEffect::Mutation,
            true,
            "memory.namespace.delete",
            "mut.memory.memory.delete-namespace@v1",
        ),
    );
    schema.methods.insert(
        "query_namespace".to_string(),
        method_decl_from_schemars_with_output::<QueryNamespaceInput, QueryNamespaceOutput>(
            "query_namespace",
            SideEffect::Read,
            false,
            "memory.namespace.read",
            "mut.memory.memory.query-namespace@v1",
        ),
    );
    schema.methods.insert(
        "insert_entry".to_string(),
        method_decl_from_schemars_with_output::<InsertEntryInput, InsertEntryOutput>(
            "insert_entry",
            SideEffect::Mutation,
            false,
            "memory.entry.create",
            "mut.memory.memory.insert-entry@v1",
        ),
    );
    schema.methods.insert(
        "update_entry".to_string(),
        method_decl_from_schemars_with_output::<UpdateEntryInput, UpdateEntryOutput>(
            "update_entry",
            SideEffect::Mutation,
            false,
            "memory.entry.write",
            "mut.memory.memory.update-entry@v1",
        ),
    );
    schema.methods.insert(
        "get_entry".to_string(),
        method_decl_from_schemars_with_output::<GetEntryInput, GetEntryOutput>(
            "get_entry",
            SideEffect::Read,
            false,
            "memory.entry.read",
            "mut.memory.memory.get-entry@v1",
        ),
    );
    schema.methods.insert(
        "delete_entry".to_string(),
        method_decl_from_schemars_with_output::<DeleteEntryInput, DeleteEntryOutput>(
            "delete_entry",
            SideEffect::Mutation,
            true,
            "memory.entry.delete",
            "mut.memory.memory.delete-entry@v1",
        ),
    );
    schema.methods.insert(
        "clear_namespace".to_string(),
        method_decl_from_schemars_with_output::<ClearNamespaceInput, ClearNamespaceOutput>(
            "clear_namespace",
            SideEffect::Mutation,
            true,
            "memory.namespace.clear",
            "mut.memory.memory.clear-namespace@v1",
        ),
    );
    schema.methods.insert(
        "get_stats".to_string(),
        method_decl_from_schemars_with_output::<GetStatsInput, GetStatsOutput>(
            "get_stats",
            SideEffect::Read,
            false,
            "memory.stats.read",
            "mut.memory.memory.get-stats@v1",
        ),
    );
    schema.methods.insert(
        "compact".to_string(),
        method_decl_from_schemars_with_output::<CompactInput, CompactOutput>(
            "compact",
            SideEffect::Mutation,
            true,
            "memory.memory.compact",
            "mut.memory.memory.compact@v1",
        ),
    );

    schema.capabilities.insert(
        "memory.namespace.create".to_string(),
        CapabilityDecl {
            id: "memory.namespace.create".to_string(),
            description: "Grants: create_namespace.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.namespace.delete".to_string(),
        CapabilityDecl {
            id: "memory.namespace.delete".to_string(),
            description: "Grants: delete_namespace.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.namespace.read".to_string(),
        CapabilityDecl {
            id: "memory.namespace.read".to_string(),
            description: "Grants: query_namespace.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.entry.create".to_string(),
        CapabilityDecl {
            id: "memory.entry.create".to_string(),
            description: "Grants: insert_entry.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.entry.write".to_string(),
        CapabilityDecl {
            id: "memory.entry.write".to_string(),
            description: "Grants: update_entry.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.entry.read".to_string(),
        CapabilityDecl {
            id: "memory.entry.read".to_string(),
            description: "Grants: get_entry.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.entry.delete".to_string(),
        CapabilityDecl {
            id: "memory.entry.delete".to_string(),
            description: "Grants: delete_entry.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.namespace.clear".to_string(),
        CapabilityDecl {
            id: "memory.namespace.clear".to_string(),
            description: "Grants: clear_namespace.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.stats.read".to_string(),
        CapabilityDecl {
            id: "memory.stats.read".to_string(),
            description: "Grants: get_stats.".to_string(),
        },
    );
    schema.capabilities.insert(
        "memory.memory.compact".to_string(),
        CapabilityDecl {
            id: "memory.memory.compact".to_string(),
            description: "Grants: compact.".to_string(),
        },
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("memory", |_ctx| std::sync::Arc::new(MemoryPlugin::new()))
}
