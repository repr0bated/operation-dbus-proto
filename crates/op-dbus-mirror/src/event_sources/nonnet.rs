//! NonNetDb watch integration

use anyhow::Result;
use op_jsonrpc::nonnet::NonNetDb;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::event::MirrorEvent;

/// NonNetChanged event for broadcast
#[derive(Debug, Clone)]
pub struct NonNetChanged {
    pub key: String,
    pub operation: NonNetOperation,
}

/// NonNet operation type
#[derive(Debug, Clone)]
pub enum NonNetOperation {
    Insert,
    Update,
    Delete,
}

/// Spawn NonNetDb watcher and send events to broadcast channel
pub async fn spawn_nonnet_watcher(
    nonnet: Arc<NonNetDb>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning NonNetDb watcher for event feed");

    let mut rx = nonnet.subscribe();

    tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            for row in update.rows {
                let delta = serde_json::to_value(&row).unwrap_or_default();
                let event = MirrorEvent::NonNet {
                    key: update.table.clone(),
                    delta,
                    sequence: 0,
                };
                let _ = broadcast_tx.send(event);
            }
        }
    });

    Ok(())
}
