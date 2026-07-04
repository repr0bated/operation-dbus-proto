//! StateManager watch integration

use anyhow::Result;
use op_state::manager::{PluginOperation, StateManager};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::event::MirrorEvent;

/// Spawn StateManager watcher and send events to broadcast channel
pub async fn spawn_state_manager_watcher(
    state_manager: Arc<StateManager>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning StateManager watcher for event feed");

    let Some(mut rx) = state_manager.watch() else {
        warn!("StateManager watch channel unavailable; plugin event feed disabled");
        return Ok(());
    };

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(plugin_event) => {
                    let operation = match plugin_event.operation {
                        PluginOperation::Register => "register",
                        PluginOperation::Deregister => "deregister",
                        PluginOperation::Update => "update",
                    };
                    let delta = serde_json::json!({
                        "plugin_id": plugin_event.plugin_id,
                        "operation": operation,
                    });
                    let event = MirrorEvent::Plugin {
                        plugin_id: plugin_event.plugin_id,
                        delta,
                        sequence: 0,
                    };
                    let _ = broadcast_tx.send(event);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("StateManager watcher lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    warn!("StateManager watch channel closed, stopping event feed");
                    break;
                }
            }
        }
    });

    Ok(())
}
