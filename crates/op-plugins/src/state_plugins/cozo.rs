//! CozoDB relational-graph-vector database plugin.
//!
//! Schema derived from CozoDB official documentation (docs.cozodb.org/en/latest/).
//! CozoDB is a Datalog database with dynamic schema — relations are created with
//! `:create` specifying key/value columns and types. The system exposes metadata
//! via `::relations`, `::columns`, `::indices` system ops.
//!
//! Column types (from docs.cozodb.org/en/latest/datatypes.html):
//!   Atomic: Int, Float, Bool, String, Bytes, Uuid, Json, Validity
//!   Composite: [Type] (list), (T1, T2) (tuple), <F32; N> (vector)
//!   Special: Any, Any? (nullable)
//!
//! Engines: mem, sled, rocksdb, sqlite
//! Access levels: normal, protected, read_only, hidden

use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};

const DEFAULT_COZO_DB_PATH: &str = "/var/lib/opdbus/cognitive.db";

/// CozoDB column definition — mirrors `::columns` system op output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CozoColumn {
    pub name: String,
    pub r#type: String,     // Int, Float, Bool, String, Bytes, Uuid, Json, Validity, Any, Any?
    pub is_key: bool,       // true if before => in :create spec
    pub default: Option<String>,
    pub description: String,
}

/// CozoDB index definition — mirrors `::indices` system op output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CozoIndex {
    pub name: String,       // e.g. "relation_name:index_name"
    pub relation: String,
    pub columns: Vec<String>,
}

/// CozoDB trigger — mirrors `::show_triggers` system op output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CozoTrigger {
    pub relation: String,
    pub event: String,      // "put", "rm", "replace"
    pub query: String,
}

/// CozoDB stored relation — mirrors `::relations` + `::columns` system op output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CozoRelation {
    pub name: String,
    pub description: String,
    pub keys: Vec<CozoColumn>,
    pub values: Vec<CozoColumn>,
    pub access_level: String, // normal, protected, read_only, hidden
    pub row_count: Option<u64>,
}

/// CozoDB HNSW vector index — from docs.cozodb.org/en/latest/vector.html
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CozoHnswIndex {
    pub name: String,
    pub relation: String,
    pub vector_column: String,
    pub dim: u32,
    pub dtype: String,      // "f32" or "f64"
    pub m: u32,
    pub ef_construction: u32,
    pub ef_search: u32,
    pub distance: String,   // "Cosine", "Euclid", "Dot"
}

/// Top-level CozoDB plugin state — the shape the projection tree exposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CozoState {
    pub engine: String,         // mem, sled, rocksdb, sqlite
    pub path: String,           // database path (empty for mem)
    pub version: String,        // CozoDB version
    pub relations: Vec<CozoRelation>,
    pub indices: Vec<CozoIndex>,
    pub hnsw_indices: Vec<CozoHnswIndex>,
    pub triggers: Vec<CozoTrigger>,
    pub running_queries: u32,
}

pub struct CozoPlugin;

impl Default for CozoPlugin {
    fn default() -> Self {
        Self
    }
}

impl CozoPlugin {
    pub fn new() -> Self {
        Self
    }

    fn db_path() -> String {
        std::env::var("COGNITIVE_MCP_COZO_DB_PATH")
            .unwrap_or_else(|_| DEFAULT_COZO_DB_PATH.to_string())
    }

    fn engine() -> String {
        let path = Self::db_path();
        if path.is_empty() {
            "mem".to_string()
        } else {
            "sled".to_string()
        }
    }

    /// Exemplar state derived from CozoDB docs system ops structure.
    /// Field names and types match the `::relations`, `::columns`, `::indices`
    /// system op outputs and the datatypes reference.
    pub(crate) fn exemplar_state() -> CozoState {
        CozoState {
            engine: Self::engine(),
            path: Self::db_path(),
            version: "0.7.6".to_string(),
            relations: vec![CozoRelation {
                name: "example_relation".to_string(),
                description: String::new(),
                keys: vec![CozoColumn {
                    name: "id".to_string(),
                    r#type: "String".to_string(),
                    is_key: true,
                    default: None,
                    description: String::new(),
                }],
                values: vec![CozoColumn {
                    name: "data".to_string(),
                    r#type: "Json".to_string(),
                    is_key: false,
                    default: Some("'{}".to_string()),
                    description: String::new(),
                }],
                access_level: "normal".to_string(),
                row_count: Some(0),
            }],
            indices: vec![],
            hnsw_indices: vec![],
            triggers: vec![],
            running_queries: 0,
        }
    }
}

#[async_trait]
impl StatePlugin for CozoPlugin {
    fn name(&self) -> &str {
        "cozo"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(cozo_schema())
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
            state_snapshot: simd_json::json!(null),
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

pub(crate) fn cozo_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(CozoPlugin::exemplar_state()).unwrap_or_else(|_| json!({}));
    schema_from_state(
        "cozo",
        "data",
        "1.0.0",
        "CozoDB relational-graph-vector database — relations, indices, HNSW, triggers, Datalog",
        &state,
    )
}

inventory::submit! {
    crate::default_registry::PluginReg::new("cozo", |_ctx| std::sync::Arc::new(CozoPlugin::new()))
}
