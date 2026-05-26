//! OVSDB event feed integration

use anyhow::Result;
use op_network::ovsdb::OvsdbClient;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::event::MirrorEvent;

/// Spawn OVSDB monitor and send events to broadcast channel
pub async fn spawn_ovsdb_monitor(
    ovsdb: Arc<OvsdbClient>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning OVSDB monitor for event feed");

    // Get initial snapshot from IDL
    let dump = ovsdb.dump_db("Open_vSwitch").await?;
    
    // Convert to MirrorEvents
    for (table_name, table_data) in dump.as_object().unwrap_or(&Default::default()) {
        if let Some(rows) = table_data.as_array() {
            for row in rows {
                let uuid = extract_uuid(row);
                let event = MirrorEvent::OvsdbRow {
                    table_name: table_name.clone(),
                    uuid,
                    delta: row.clone(),
                    sequence: 0, // Initial snapshot uses sequence 0
                };
                let _ = broadcast_tx.send(event);
            }
        }
    }

    // Start monitoring for changes
    let mut rx = ovsdb.monitor_db("Open_vSwitch").await?;
    
    tokio::spawn(async move {
        while let Some(_update) = rx.recv().await {
            // TODO: When monitor_db provides actual row change data,
            // convert to MirrorEvent::OvsdbRow
            // For now, we just re-read the full dump
            if let Ok(dump) = ovsdb.dump_db("Open_vSwitch").await {
                for (table_name, table_data) in dump.as_object().unwrap_or(&Default::default()) {
                    if let Some(rows) = table_data.as_array() {
                        for row in rows {
                            let uuid = extract_uuid(row);
                            let event = MirrorEvent::OvsdbRow {
                                table_name: table_name.clone(),
                                uuid,
                                delta: row.clone(),
                                sequence: 0, // TODO: Implement proper sequence tracking
                            };
                            let _ = broadcast_tx.send(event);
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

fn extract_uuid(row: &serde_json::Value) -> String {
    if let Some(uuid_val) = row.get("_uuid") {
        if let Some(uuid_arr) = uuid_val.as_array() {
            if uuid_arr.len() == 2 && uuid_arr[0].as_str() == Some("uuid") {
                if let Some(uuid_str) = uuid_arr[1].as_str() {
                    return uuid_str.to_string();
                }
            }
        }
    }
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
