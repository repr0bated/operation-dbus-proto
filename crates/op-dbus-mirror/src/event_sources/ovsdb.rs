//! OVSDB event feed integration
//!
//! Uses `OvsdbClient::monitor_db()` which delivers full IDL snapshots (no
//! extra network connections) via `rovs_ovsdb::Client::wait()`.  The first
//! message received is the initial snapshot taken right after the monitoring
//! connection is established; subsequent messages arrive on every DB change.

use anyhow::Result;
use op_network::ovsdb::OvsdbClient;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::event::MirrorEvent;

/// Spawn OVSDB monitor and send events to the broadcast channel.
///
/// The monitor uses the IDL-based snapshot format from `monitor_db()`:
/// `{ "TableName": [{ "_uuid": ["uuid", "..."], col: val, … }, …], … }`.
/// Each snapshot is exploded into individual `MirrorEvent::OvsdbRow` events —
/// one per row, per table.
pub async fn spawn_ovsdb_monitor(
    ovsdb: Arc<OvsdbClient>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning OVSDB monitor for event feed");

    // monitor_db() returns a channel that delivers IDL snapshots.  The first
    // snapshot (sent immediately after connect) acts as the initial data load;
    // no separate dump_db() call is needed.
    let mut rx = ovsdb.monitor_db("Open_vSwitch").await?;

    tokio::spawn(async move {
        let mut sequence: u64 = 0;

        while let Some(snapshot) = rx.recv().await {
            sequence = sequence.wrapping_add(1);

            // snapshot = { "TableName": [ row, … ], … }
            // Each row has "_uuid": ["uuid", "…"] plus column values.
            let tables = match snapshot.as_object() {
                Some(t) => t,
                None => {
                    tracing::warn!("monitor_db: received non-object snapshot, skipping");
                    continue;
                }
            };

            for (table_name, rows_val) in tables {
                let rows = match rows_val.as_array() {
                    Some(r) => r,
                    None => continue,
                };

                for row in rows {
                    let uuid = extract_uuid(row);
                    let event = MirrorEvent::OvsdbRow {
                        table_name: table_name.clone(),
                        uuid,
                        delta: row.clone(),
                        sequence,
                    };
                    // A send error means all receivers dropped; the task keeps
                    // running so the monitoring connection stays alive.
                    let _ = broadcast_tx.send(event);
                }
            }
        }

        tracing::info!("OVSDB monitor_db channel closed, stopping event feed");
    });

    Ok(())
}

fn extract_uuid(row: &serde_json::Value) -> String {
    // Canonical OVSDB wire form: ["uuid", "<uuid-str>"]
    if let Some(uuid_val) = row.get("_uuid") {
        if let Some(uuid_arr) = uuid_val.as_array() {
            if uuid_arr.len() == 2 && uuid_arr[0].as_str() == Some("uuid") {
                if let Some(uuid_str) = uuid_arr[1].as_str() {
                    return uuid_str.to_string();
                }
            }
        }
    }
    // Fallback keys used by some callers
    if let Some(uuid) = row.get("uuid").and_then(|v| v.as_str()) {
        return uuid.to_string();
    }
    if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
        return id.to_string();
    }
    if let Some(s) = row.get("name").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    "unknown".to_string()
}
