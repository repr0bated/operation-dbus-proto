use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// A configured unix-domain socket endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketEndpoint {
    /// Filesystem path to the socket (e.g. `/run/qdrant.sock`).
    pub path: String,
    /// Human-readable label (e.g. `"qdrant-grpc"`).
    pub label: String,
    /// Transport protocol carried over the socket (`"grpc"`, `"jsonrpc"`, …).
    pub protocol: String,
}

/// Runtime state: all declared socket endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnixSocketState {
    /// Declared unix socket endpoints visible to internal services.
    pub sockets: Vec<SocketEndpoint>,
}

pub struct UnixSocketPlugin;

impl UnixSocketPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnixSocketPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for UnixSocketPlugin {
    fn name(&self) -> &str {
        "unix_socket"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::unix_socket_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(UnixSocketState::default())?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
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
            state_snapshot: Value::null(),
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
