use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_json::json;
use simd_json::prelude::ValueAsMutContainer;
use simd_json::OwnedValue as Value;
use std::net::TcpStream;
use std::time::Duration;

const DEFAULT_QDRANT_ENDPOINT: &str = "http://127.0.0.1:6334";
/// Knowledge plugin state.
/// See: https://qdrant.tech/
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct KnowledgeState {
    /// Plugin status
    pub status: String,
    /// Knowledge stores configuration
    #[schemars(with = "serde_json::Value")]
    pub stores: JsonValue,
    /// Embedding pipeline configuration
    #[schemars(with = "serde_json::Value")]
    pub embedding: JsonValue,
    /// Plugin configuration
    #[schemars(with = "serde_json::Value")]
    pub config: JsonValue,
}
pub struct KnowledgePlugin;
impl Default for KnowledgePlugin {
    fn default() -> Self {
        Self
    }
}
impl KnowledgePlugin {
    pub fn new() -> Self {
        Self
    }
    fn qdrant_endpoint() -> String {
        std::env::var("COGNITIVE_MCP_QDRANT_URL")
            .or_else(|_| std::env::var("QDRANT_URL"))
            .unwrap_or_else(|_| DEFAULT_QDRANT_ENDPOINT.to_string())
    }

    fn endpoint_tcp_addr(endpoint: &str) -> Option<String> {
        let stripped = endpoint
            .strip_prefix("http://")
            .or_else(|| endpoint.strip_prefix("https://"))
            .unwrap_or(endpoint);
        let host_port = stripped.split('/').next()?;
        if host_port.contains(':') {
            Some(host_port.to_string())
        } else {
            None
        }
    }

    fn qdrant_reachable(endpoint: &str) -> bool {
        let addr = match Self::endpoint_tcp_addr(endpoint) {
            Some(addr) => addr,
            None => return false,
        };
        let socket_addr = match addr.parse() {
            Ok(addr) => addr,
            Err(_) => return false,
        };
        TcpStream::connect_timeout(&socket_addr, Duration::from_millis(750)).is_ok()
    }

    pub(crate) fn current_state() -> KnowledgeState {
        let qdrant_endpoint = Self::qdrant_endpoint();
        KnowledgeState {
            status: "active".to_string(),
            stores: json!([{"name": "qdrant", "type": "vector", "status": "unknown", "endpoint": qdrant_endpoint, "collections": [], "dimensions": 1024}, {"name": "cozo", "type": "graph", "status": "active", "relations": 0, "stored_procedure_count": 0}, {"name": "sled", "type": "kv", "status": "active", "path": "/dev/shm/plugin_schema.dat"}]),
            embedding: json!({"pipeline": "default", "provider": "voyage", "model": "voyage-4", "queue_size": 0, "worker_status": "idle", "chunk_size": 512, "chunk_overlap": 50}),
            config: json!({"qdrant_endpoint": qdrant_endpoint, "cozo_db": "/var/lib/op-dbus/cognitive.db", "auto_capture": false, "suggest_on_query": true}),
        }
    }
}
#[async_trait]
impl StatePlugin for KnowledgePlugin {
    fn name(&self) -> &str {
        "knowledge"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(knowledge_schema())
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

pub(crate) fn knowledge_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(super::knowledge_plugin::KnowledgePlugin::current_state())
            .unwrap_or_else(|_| simd_json::json!({}));
    schema_from_state(
        "knowledge",
        "data",
        "1.0.0",
        "Knowledge stores — Qdrant, CozoDB, Sled, embedding pipeline",
        &state,
    )
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("knowledge", |_ctx| std::sync::Arc::new(KnowledgePlugin::new()))
}
