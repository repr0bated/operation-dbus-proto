//! Heartbeat safety net module

use anyhow::Result;

use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time;
use tracing::info;

use crate::event::MirrorEvent;
use crate::DbusMirror;

/// Heartbeat interval in seconds (300 seconds = 5 minutes)
const HEARTBEAT_INTERVAL: u64 = 300;

/// Spawn heartbeat task that resyncs objects with stale sequence numbers
pub async fn spawn_heartbeat_task(
    mirror: Arc<DbusMirror>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!(
        "Spawning heartbeat task with {} second interval",
        HEARTBEAT_INTERVAL
    );

    let mut interval = time::interval(time::Duration::from_secs(HEARTBEAT_INTERVAL));

    tokio::spawn(async move {
        loop {
            interval.tick().await;

            // Resync objects whose sequence numbers have not advanced
            resync_stale_objects(&mirror, &broadcast_tx).await;
        }
    });

    Ok(())
}

/// Resync objects whose sequence numbers have not advanced
async fn resync_stale_objects(mirror: &DbusMirror, _broadcast_tx: &broadcast::Sender<MirrorEvent>) {
    // Get current time
    let now = std::time::SystemTime::now();

    // Iterate over all published objects
    for entry in mirror.published_objects.iter() {
        let path = entry.key();

        // Check if there's a session for this path
        if let Some(session) = mirror.sessions.get(path) {
            let session = session.value();

            // Check if the session has been active recently
            let elapsed = match now.duration_since(session.created_at) {
                Ok(elapsed) => elapsed,
                Err(_) => continue,
            };

            // If session is older than heartbeat interval, trigger resync
            if elapsed > time::Duration::from_secs(HEARTBEAT_INTERVAL) {
                // TODO: Implement proper resync logic
                // For now, we'll just log the resync
                tracing::info!("Resyncing stale object: {}", path);

                // TODO: Send resync event to broadcast channel
                // let _ = broadcast_tx.send(MirrorEvent::Resync { path: path.clone() });
            }
        }
    }
}
