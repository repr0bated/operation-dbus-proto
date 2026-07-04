//! OVSDB event feed integration
//!
//! Subscribes to the SchemaEngine's `change_tx` broadcast channel for
//! OVSDB-related state changes. The SchemaEngine is the authoritative
//! source: all mutations flow through it and are recorded in the Event
//! Chain. State changes arrive as `StateChange` structs carrying the
//! new value from shared memory — no polling, no JSON-RPC monitor.

use anyhow::Result;
use op_grpc_bridge::{MutationEngine, StateChange};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::event::MirrorEvent;

/// Spawn OVSDB monitor via SchemaEngine change_tx subscription.
///
/// Filters `StateChange` events whose `plugin_id` is `"net"` or whose
/// `object_path` starts with `/org/opdbus/v1/ovsdb` and converts them
/// to `MirrorEvent::OvsdbRow` for the event dispatcher.
pub async fn spawn_ovsdb_monitor(
    schema_engine: Arc<MutationEngine>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Subscribing to SchemaEngine change_tx for OVSDB event feed");

    let mut rx = schema_engine.change_tx().subscribe();

    tokio::spawn(async move {
        let mut sequence: u64 = 0;

        loop {
            match rx.recv().await {
                Ok(change) => {
                    if !is_ovsdb_change(&change) {
                        continue;
                    }

                    sequence = sequence.wrapping_add(1);

                    // Convert SchemaEngine value → serde_json::Value via Display
                    let delta = change.new_value.to_string();
                    let delta: serde_json::Value =
                        serde_json::from_str(&delta).unwrap_or(serde_json::Value::Null);

                    let table_name = change
                        .object_path
                        .rsplit('/')
                        .next()
                        .unwrap_or("unknown")
                        .to_string();

                    let uuid = change.change_id.clone();

                    let event = MirrorEvent::OvsdbRow {
                        table_name,
                        uuid,
                        delta,
                        sequence,
                    };

                    let _ = broadcast_tx.send(event);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("OVSDB change_tx subscription lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!("SchemaEngine change_tx closed, stopping OVSDB event feed");
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Check whether a StateChange is OVSDB-related.
fn is_ovsdb_change(change: &StateChange) -> bool {
    change.plugin_id == "net"
        || change.object_path.starts_with("/org/opdbus/v1/ovsdb")
        || change.object_path.starts_with("/org/opdbus/v1/ovs")
        || change
            .tags_touched
            .iter()
            .any(|t| t == "ovsdb" || t == "net")
}
