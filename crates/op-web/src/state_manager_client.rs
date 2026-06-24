//! Plugin state client — routes reads/writes through the gRPC MutationEngine
//! (the single write door) instead of the excised D-Bus StateManager.
//!
//! Reads go to `RemoteOperationClient::get_state` (state_cache, the mutation
//! fold). Writes go to `RemoteOperationClient::set_state` (ApplyPatch through
//! the MutationEngine → EventChain → shm projection). No second write door,
//! no StateManager, no drift.

use anyhow::{Context, Result};
use op_grpc_bridge::{GrpcClientPool, RemoteOperationClient};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::{Arc, OnceLock};

static GRPC_CLIENT: OnceLock<RemoteOperationClient> = OnceLock::new();

/// Lazily initialize a singleton RemoteOperationClient backed by the
/// MutationEngine's gRPC endpoint (env `OP_DBUS_GRPC_ADDR`, default
/// `127.0.0.1:50051`).
fn client() -> &'static RemoteOperationClient {
    GRPC_CLIENT.get_or_init(|| {
        let grpc_addr =
            std::env::var("OP_DBUS_GRPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".to_string());
        let pool = Arc::new(GrpcClientPool::new());
        RemoteOperationClient::new(pool, &grpc_addr, "op-web")
    })
}

pub async fn query_plugin_state<T>(plugin_id: &str) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    match client().get_state(plugin_id, "").await {
        Ok(state) => {
            match simd_json::serde::from_owned_value::<T>(state) {
                Ok(value) => Ok(Some(value)),
                Err(_) => Ok(None),
            }
        }
        Err(_) => Ok(None),
    }
}

pub async fn apply_plugin_state<T>(plugin_id: &str, value: &T) -> Result<()>
where
    T: Serialize,
{
    let state = simd_json::serde::to_owned_value(value)
        .with_context(|| format!("serialize {} plugin state", plugin_id))?;
    client()
        .set_state(plugin_id, "", state, "op-web", "")
        .await
        .map_err(|e| anyhow::anyhow!("apply {} state via gRPC: {}", plugin_id, e))?;
    Ok(())
}
