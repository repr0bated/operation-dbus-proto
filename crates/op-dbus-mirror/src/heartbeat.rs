//! Heartbeat safety net module

use anyhow::Result;

use std::collections::HashMap;
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
    _broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!(
        "Spawning heartbeat task with {} second interval",
        HEARTBEAT_INTERVAL
    );

    let mut interval = time::interval(time::Duration::from_secs(HEARTBEAT_INTERVAL));

    tokio::spawn(async move {
        // Per-path sequence numbers as of the previous tick. A path whose
        // sequence has not advanced across a full interval is re-emitted so
        // peers that missed a delta (e.g. broadcast lag) converge again.
        let mut last_seen: HashMap<String, u64> = HashMap::new();

        // The first tick of tokio::time::interval fires immediately; use it
        // to take the initial sequence snapshot without resyncing anything.
        interval.tick().await;
        snapshot_sequences(&mirror, &mut last_seen);

        loop {
            interval.tick().await;
            resync_stale_objects(&mirror, &mut last_seen).await;
        }
    });

    Ok(())
}

/// Record the current per-path sequence numbers.
fn snapshot_sequences(mirror: &DbusMirror, last_seen: &mut HashMap<String, u64>) {
    for entry in mirror.current_data.iter() {
        last_seen.insert(entry.key().clone(), entry.value().1);
    }
}

/// Resync objects whose sequence numbers have not advanced since the last tick
async fn resync_stale_objects(mirror: &DbusMirror, last_seen: &mut HashMap<String, u64>) {
    // Collect first so no DashMap guard is held across an await.
    let current: Vec<(String, u64)> = mirror
        .current_data
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().1))
        .collect();

    for (path, sequence) in current {
        if last_seen.get(&path) == Some(&sequence) {
            tracing::info!("Resyncing stale object: {}", path);
            mirror.resync_object(&path).await;
        }
        last_seen.insert(path, sequence);
    }
}
