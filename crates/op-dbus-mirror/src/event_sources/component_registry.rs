//! ComponentRegistry broadcast integration

use anyhow::Result;
use op_grpc_bridge::OperationGrpcServer;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::event::MirrorEvent;

/// Spawn ComponentRegistry watcher and send events to broadcast channel
pub async fn spawn_component_registry_watcher(
    grpc_server: Arc<OperationGrpcServer>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning ComponentRegistry watcher for event feed");

    let (_, mut rx) = grpc_server.registry_watch().await;

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let mirror_event = MirrorEvent::Registry {
                        event,
                        sequence: 0, // TODO: Implement proper sequence tracking
                    };
                    let _ = broadcast_tx.send(mirror_event);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("ComponentRegistry watcher lagged by {} events", n);
                    // TODO: Implement resync logic
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    Ok(())
}
