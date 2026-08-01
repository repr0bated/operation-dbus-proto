This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  bin/
    ovs-dbus-init.rs
    verify_performance.rs
  event_sources/
    component_registry.rs
    mod.rs
    nonnet.rs
    ovsdb.rs
    procfs.rs
    state_manager.rs
  dbus_interface.rs
  event_dispatcher.rs
  event.rs
  heartbeat.rs
  jsonrpc_interface.rs
  lib.rs
  lib.rs.orig
  managed_objects.rs
  object.rs
  plugin_interface.rs
  session.rs
  tree.rs
Cargo.toml
compare-op-dbus-mirror.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/bin/ovs-dbus-init.rs">
use anyhow::{Context, Result};
use op_core::types::BusType;
use op_dbus_mirror::DbusMirror;
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_plugins::default_registry::DefaultPluginRegistry;
use op_state::manager::StateManager;
use op_state_store::SqliteStore;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "ovs_dbus_init=info,info".to_string()),
        )
        .init();

    let bus_type = match env::var("OP_DBUS_MIRROR_BUS")
        .unwrap_or_else(|_| "system".to_string())
        .as_str()
    {
        "session" => BusType::Session,
        _ => BusType::System,
    };

    tracing::info!(bus = %bus_type, "starting op-dbus-mirror (event-driven)");

    let ovsdb = Arc::new(OvsdbClient::new());
    let nonnet = Arc::new(NonNetDb::new());
    let state_manager = Arc::new(StateManager::new());

    let state_store = Arc::new(
        SqliteStore::in_memory()
            .await
            .context("failed to create op-dbus-mirror in-memory state store")?,
    );
    let plugin_registry = DefaultPluginRegistry::new(state_store);
    let plugins = plugin_registry
        .load_default_plugins()
        .await
        .context("failed to load default PluginSchema-backed plugins")?;

    for plugin in plugins {
        state_manager.register_plugin(plugin.name().to_string(), plugin);
    }

    let plugin_state = state_manager
        .query_current_state()
        .await
        .context("failed to query PluginSchema-backed state for NonNet")?;
    nonnet.load_from_plugins(&plugin_state).await;

    let mirror = DbusMirror::new(bus_type, ovsdb, nonnet, None)
        .await
        .context("failed to create DbusMirror")?
        .with_state_manager(state_manager);

    Arc::new(mirror)
        .start()
        .await
        .context("DbusMirror event loop exited")?;

    Ok(())
}
</file>

<file path="src/bin/verify_performance.rs">
use anyhow::Result;
use op_dbus_mirror::object::MirrorObject;
use serde_json::json;
use std::time::Instant;
use tracing_subscriber::EnvFilter;
use zbus::connection::Builder;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let count = 16_000;
    println!(
        "🚀 Starting D-Bus performance verification with {} objects...",
        count
    );

    let conn = Builder::session()?
        .name("org.opdbus.mirror.perf_test")?
        .build()
        .await?;

    let start = Instant::now();

    for i in 0..count {
        let path = format!("/org/opdbus/mirror/perf/obj_{}", i);
        let dbus_path = zbus::zvariant::ObjectPath::try_from(path)?;

        let data = json!({
            "id": i,
            "uuid": format!("uuid_{}", i),
            "status": "active",
            "metadata": {
                "created_at": "2026-02-12T00:00:00Z",
                "owner": "perf-test",
                "tags": ["test", "performance", "heavy-load"]
            }
        });

        let obj = MirrorObject::new(data);
        conn.object_server().at(dbus_path, obj).await?;

        if (i + 1) % 1000 == 0 {
            println!("   Registered {}/{} objects...", i + 1, count);
        }
    }

    let duration = start.elapsed();
    let per_object = duration.as_micros() / count as u128;

    println!("\n✅ Performance Results:");
    println!("   Total Objects:    {}", count);
    println!("   Total Time:       {:?}", duration);
    println!("   Avg Per Object:   {} us", per_object);

    // Give zbus a moment to settle
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    #[zbus::proxy(
        interface = "org.opdbus.MirrorObjectV1",
        default_service = "org.opdbus.mirror.perf_test"
    )]
    trait MirrorObject {
        fn get_json(&self) -> zbus::Result<String>;
    }

    let test_path = "/org/opdbus/mirror/perf/obj_8000";
    let lookup_start = Instant::now();

    let proxy = MirrorObjectProxy::builder(&conn)
        .path(test_path)?
        .build()
        .await?;

    let reply = proxy.get_json().await?;

    let lookup_duration = lookup_start.elapsed();
    println!("   Single Lookup:    {:?} (obj_8000)", lookup_duration);

    if reply.contains("uuid_8000") {
        println!("   Data Integrity:   PASS");
    } else {
        println!("   Data Integrity:   FAIL");
    }

    println!("\nVerification complete. Press Ctrl+C to exit.");
    // Exit after a few seconds to avoid hanging CI-like runs
    tokio::select! {
        _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {},
        _ = tokio::signal::ctrl_c() => {},
    }

    Ok(())
}
</file>

<file path="src/event_sources/component_registry.rs">
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
</file>

<file path="src/event_sources/mod.rs">
//! Event sources module for wiring all data source feeds

pub mod component_registry;
pub mod nonnet;
pub mod ovsdb;
pub mod procfs;
pub mod state_manager;
</file>

<file path="src/event_sources/nonnet.rs">
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
</file>

<file path="src/event_sources/ovsdb.rs">
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
</file>

<file path="src/event_sources/procfs.rs">
//! Procfs event feed integration using inotify and procfs crate

use anyhow::Result;
use inotify::{Inotify, WatchMask};
use procfs::{Current, LoadAverage};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;
use tracing::{info, warn};

use crate::event::MirrorEvent;

/// Spawn procfs inotify watchers for meminfo and stat
pub async fn spawn_procfs_inotify_watchers(
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning procfs inotify watchers");

    let mut inotify = Inotify::init()?;

    let wd_meminfo = inotify.watches().add("/proc/meminfo", WatchMask::ACCESS)?;
    let wd_stat = inotify.watches().add("/proc/stat", WatchMask::ACCESS)?;

    tokio::spawn(async move {
        let mut buffer = [0; 4096];
        loop {
            let events = inotify.read_events(&mut buffer).ok();
            if let Some(events) = events {
                for event in events {
                    let path = if event.wd == wd_meminfo {
                        Some("/proc/meminfo")
                    } else if event.wd == wd_stat {
                        Some("/proc/stat")
                    } else {
                        None
                    };

                    if let Some(path) = path {
                        if path == "/proc/meminfo" {
                            if let Ok(meminfo) = procfs::Meminfo::current() {
                                let event = MirrorEvent::ProcMem {
                                    delta: serde_json::to_value(meminfo).unwrap_or_default(),
                                    sequence: 0,
                                };
                                let _ = broadcast_tx.send(event);
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    Ok(())
}

/// Spawn procfs timer for /proc/loadavg
pub async fn spawn_procfs_loadavg_timer(
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning procfs loadavg timer");

    let mut interval = time::interval(Duration::from_secs(5));

    tokio::spawn(async move {
        loop {
            interval.tick().await;
            if let Ok(loadavg) = LoadAverage::current() {
                let event = MirrorEvent::ProcLoad {
                    delta: serde_json::to_value(loadavg).unwrap_or_default(),
                    sequence: 0,
                };
                let _ = broadcast_tx.send(event);
            }
        }
    });

    Ok(())
}
</file>

<file path="src/event_sources/state_manager.rs">
//! StateManager watch integration

use anyhow::Result;
use op_state::manager::StateManager;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::event::MirrorEvent;

/// PluginEvent for broadcast
#[derive(Debug, Clone)]
pub struct PluginEvent {
    pub plugin_id: String,
    pub operation: PluginOperation,
}

/// Plugin operation type
#[derive(Debug, Clone)]
pub enum PluginOperation {
    Register,
    Deregister,
    Update,
}

/// Spawn StateManager watcher and send events to broadcast channel
pub async fn spawn_state_manager_watcher(
    state_manager: Arc<StateManager>,
    broadcast_tx: broadcast::Sender<MirrorEvent>,
) -> Result<()> {
    info!("Spawning StateManager watcher for event feed");

    // TODO: Implement StateManager::watch() method
    // For now, we'll use a polling approach as a placeholder
    // The actual implementation should use a broadcast channel

    tokio::spawn(async move {
        // Placeholder implementation
        // In the real implementation, this would subscribe to StateManager's watch channel
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            // TODO: Read from watch channel and send events
        }
    });

    Ok(())
}
</file>

<file path="src/dbus_interface.rs">
//! D-Bus interface for the publication service.

use crate::DbusMirror;
use std::sync::Arc;
use zbus::interface;

pub struct DbusMirrorInterface {
    mirror: Arc<DbusMirror>,
}

impl DbusMirrorInterface {
    pub fn new(mirror: Arc<DbusMirror>) -> Self {
        Self { mirror }
    }
}

#[interface(name = "org.opdbus.MirrorV1")]
impl DbusMirrorInterface {
    /// Publish a fresh snapshot from authoritative stores.
    async fn publish_snapshot(&self) -> zbus::fdo::Result<()> {
        self.mirror
            .publish_snapshot()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Compatibility alias for older callers that still use the old term.
    async fn reconcile(&self) -> zbus::fdo::Result<()> {
        self.publish_snapshot().await
    }

    /// Get current publication statistics.
    async fn get_stats(&self) -> zbus::fdo::Result<String> {
        let stats = serde_json::json!({
            "published_objects": self.mirror.published_count(),
            "projected_objects": self.mirror.projected_count(),
        });
        Ok(serde_json::to_string(&stats).unwrap_or_default())
    }

    /// Get list of all published object paths.
    async fn list_paths(&self) -> zbus::fdo::Result<Vec<String>> {
        Ok(self.mirror.list_published_paths())
    }
}
</file>

<file path="src/event_dispatcher.rs">
//! EventDispatcher module for unified event dispatch

use anyhow::Result;
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_state::manager::StateManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::event::MirrorEvent;
use crate::event_sources::component_registry;
use crate::event_sources::nonnet;
use crate::event_sources::ovsdb;
use crate::event_sources::procfs;
use crate::event_sources::state_manager;
use crate::DbusMirror;

/// Event dispatcher that wires all event sources to the broadcast channel
pub struct EventDispatcher {
    pub broadcast_tx: broadcast::Sender<MirrorEvent>,
    mirror: Arc<DbusMirror>,
    ovsdb_client: Arc<OvsdbClient>,
    nonnet_db: Arc<NonNetDb>,
    state_manager: Option<Arc<StateManager>>,
    grpc_server: Option<Arc<op_grpc_bridge::OperationGrpcServer>>,
    /// Sequence numbers per object path
    sequence_numbers: Arc<std::sync::Mutex<HashMap<String, u64>>>,
}

impl EventDispatcher {
    /// Create a new EventDispatcher
    pub fn new(
        mirror: Arc<DbusMirror>,
        ovsdb_client: Arc<OvsdbClient>,
        nonnet_db: Arc<NonNetDb>,
        state_manager: Option<Arc<StateManager>>,
        grpc_server: Option<Arc<op_grpc_bridge::OperationGrpcServer>>,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        Self {
            broadcast_tx,
            mirror,
            ovsdb_client,
            nonnet_db,
            state_manager,
            grpc_server,
            sequence_numbers: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Spawn all event sources
    pub async fn spawn_event_sources(&self) -> Result<()> {
        info!("Spawning all event sources");

        // Spawn OVSDB monitor
        ovsdb::spawn_ovsdb_monitor(self.ovsdb_client.clone(), self.broadcast_tx.clone()).await?;

        // Spawn NonNetDb watcher
        nonnet::spawn_nonnet_watcher(self.nonnet_db.clone(), self.broadcast_tx.clone()).await?;

        // Spawn procfs watchers
        procfs::spawn_procfs_inotify_watchers(self.broadcast_tx.clone()).await?;
        procfs::spawn_procfs_loadavg_timer(self.broadcast_tx.clone()).await?;

        // Spawn StateManager watcher
        if let Some(sm) = &self.state_manager {
            state_manager::spawn_state_manager_watcher(sm.clone(), self.broadcast_tx.clone())
                .await?;
        }

        // Spawn ComponentRegistry watcher
        if let Some(grpc) = &self.grpc_server {
            component_registry::spawn_component_registry_watcher(
                grpc.clone(),
                self.broadcast_tx.clone(),
            )
            .await?;
        }

        Ok(())
    }

    /// Run the event loop
    pub async fn run_event_loop(&self) -> Result<()> {
        info!("Starting event loop");

        let mut rx = self.broadcast_tx.subscribe();

        loop {
            match rx.recv().await {
                Ok(event) => {
                    self.publish_delta(&event).await?;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Event dispatcher lagged by {} events", n);
                    // TODO: Implement resync logic
                }
                Err(broadcast::error::RecvError::Closed) => {
                    warn!("Event broadcast channel closed");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Publish delta for an event
    async fn publish_delta(&self, event: &MirrorEvent) -> Result<()> {
        if let Some(path) = event.target_path() {
            // Get current sequence number and increment (scoped to avoid holding guard across .await)
            let _current_seq = {
                let mut seq_map = self.sequence_numbers.lock().unwrap();
                let sequence = seq_map.entry(path.clone()).or_insert(0);
                *sequence += 1;
                *sequence
            };

            // Update current_data with new value and sequence
            let event_seq = event.sequence();
            if let Some(mut entry) = self.mirror.current_data.get_mut(&path) {
                let (data, seq) = &mut *entry;
                *data = event.delta();
                *seq = event_seq;
            } else {
                self.mirror
                    .current_data
                    .insert(path.clone(), (event.delta(), event_seq));
            }

            // Update session pending queues
            let mut sessions_to_drop: Vec<String> = Vec::new();
            for mut session_entry in self.mirror.sessions.iter_mut() {
                session_entry.value_mut().add_event(event.clone());

                if session_entry.value().is_queue_full() {
                    warn!(
                        "Session {} queue full, dropping",
                        session_entry.value().peer_name
                    );
                    sessions_to_drop.push(session_entry.key().clone());
                }
            }
            for key in sessions_to_drop {
                self.mirror.sessions.remove(&key);
            }

            // Emit PropertiesChanged with only changed fields
            self.mirror.publish_object(&path, event.delta()).await?;
        }

        Ok(())
    }
}

/// Extension trait for MirrorEvent to get delta
trait MirrorEventDelta {
    fn delta(&self) -> serde_json::Value;
}

impl MirrorEventDelta for MirrorEvent {
    fn delta(&self) -> serde_json::Value {
        match self {
            MirrorEvent::OvsdbRow { delta, .. }
            | MirrorEvent::NonNet { delta, .. }
            | MirrorEvent::Plugin { delta, .. }
            | MirrorEvent::ProcMem { delta, .. }
            | MirrorEvent::ProcLoad { delta, .. }
            | MirrorEvent::ProcStatic { data: delta, .. } => delta.clone(),
            MirrorEvent::Registry { event, .. } => {
                serde_json::json!({
                    "event_type": event.event_type,
                    "component_id": event.component.as_ref().map(|c| c.component_id.as_str()).unwrap_or(""),
                })
            }
        }
    }
}
</file>

<file path="src/event.rs">
//! MirrorEvent module for unified event enum

use serde_json::Value;

/// Unified event enum representing all data source changes
#[derive(Debug, Clone)]
pub enum MirrorEvent {
    /// OVSDB row change event
    OvsdbRow {
        table_name: String,
        uuid: String,
        delta: Value,
        sequence: u64,
    },
    /// NonNetDb key change event
    NonNet {
        key: String,
        delta: Value,
        sequence: u64,
    },
    /// StateManager plugin event
    Plugin {
        plugin_id: String,
        delta: Value,
        sequence: u64,
    },
    /// ComponentRegistry event
    Registry {
        event: op_grpc_bridge::proto::registry::RegistryEvent,
        sequence: u64,
    },
    /// Procfs memory info event
    ProcMem { delta: Value, sequence: u64 },
    /// Procfs load average event
    ProcLoad { delta: Value, sequence: u64 },
    /// Procfs static section event
    ProcStatic {
        section: String,
        data: Value,
        sequence: u64,
    },
}

impl MirrorEvent {
    /// Get the target path for this event
    pub fn target_path(&self) -> Option<String> {
        match self {
            MirrorEvent::OvsdbRow {
                table_name, uuid, ..
            } => Some(format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, uuid)),
            MirrorEvent::NonNet { key, .. } => Some(format!("/org/opdbus/v1/nonnet/{}", key)),
            MirrorEvent::Plugin { plugin_id, .. } => {
                Some(format!("/org/opdbus/v1/plugins/{}", plugin_id))
            }
            MirrorEvent::Registry { event, .. } => {
                let component = event.component.as_ref()?;
                let safe = component.component_id.replace(['.', '-', ':'], "_");
                Some(format!("/org/opdbus/v1/registry/{}", safe))
            }
            MirrorEvent::ProcMem { .. } => Some("/org/opdbus/v1/host/meminfo".to_string()),
            MirrorEvent::ProcLoad { .. } => Some("/org/opdbus/v1/host/loadavg".to_string()),
            MirrorEvent::ProcStatic { section, .. } => {
                Some(format!("/org/opdbus/v1/host/{}", section))
            }
        }
    }

    /// Get the sequence number for this event
    pub fn sequence(&self) -> u64 {
        match self {
            MirrorEvent::OvsdbRow { sequence, .. }
            | MirrorEvent::NonNet { sequence, .. }
            | MirrorEvent::Plugin { sequence, .. }
            | MirrorEvent::Registry { sequence, .. }
            | MirrorEvent::ProcMem { sequence, .. }
            | MirrorEvent::ProcLoad { sequence, .. }
            | MirrorEvent::ProcStatic { sequence, .. } => *sequence,
        }
    }
}
</file>

<file path="src/heartbeat.rs">
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
async fn resync_stale_objects(mirror: &DbusMirror, broadcast_tx: &broadcast::Sender<MirrorEvent>) {
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
</file>

<file path="src/jsonrpc_interface.rs">
//! JSON-RPC D-Bus Interfaces
//!
//! Exposes OVSDB and NonNet JSON-RPC methods as D-Bus interfaces
//! for a true 1:1 mirror of the JSON-RPC API.
//!
//! Authoritative Path: D-Bus method → SchemaEngine.mutate → RCP Database → EventChain

use op_grpc_bridge::{ChangeType, SchemaEngine};
use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::protocol::JsonRpcRequest;
use op_network::ovsdb::OvsdbClient;
use serde_json::Value;
use std::sync::Arc;
use zbus::interface;

fn to_simd(val: &Value) -> simd_json::OwnedValue {
    serde_json::to_string(val)
        .ok()
        .and_then(|s| {
            let mut bytes = s.into_bytes();
            simd_json::to_owned_value(&mut bytes).ok()
        })
        .unwrap_or(simd_json::OwnedValue::Static(simd_json::StaticNode::Null))
}

fn str_to_simd(s: &str) -> Result<simd_json::OwnedValue, zbus::fdo::Error> {
    let mut bytes = s.as_bytes().to_vec();
    simd_json::to_owned_value(&mut bytes).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))
}

/// OVSDB D-Bus interface - mirrors JSON-RPC methods
pub struct OvsdbInterface {
    pub client: Arc<OvsdbClient>,
    pub schema_engine: Option<Arc<SchemaEngine>>,
}

impl OvsdbInterface {
    pub fn new(client: Arc<OvsdbClient>, schema_engine: Option<Arc<SchemaEngine>>) -> Self {
        Self {
            client,
            schema_engine,
        }
    }
}

#[interface(name = "org.opdbus.OvsdbV1")]
impl OvsdbInterface {
    /// Execute JSON-RPC transact on OVSDB
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String> {
        let operations_val = str_to_simd(&operations)?;

        // Route through SchemaEngine for authoritative recording if available
        if let Some(engine) = &self.schema_engine {
            match engine
                .mutate(
                    "net".to_string(),
                    "/org/opdbus/v1/ovsdb".to_string(),
                    ChangeType::MethodCall,
                    Some("transact".to_string()),
                    operations_val,
                    "dbus-client".to_string(),
                    None,
                )
                .await
            {
                Ok(result) => Ok(serde_json::to_string(&result.result).unwrap_or_default()),
                Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
            }
        } else {
            match self.client.transact_simd(operations_val).await {
                Ok(result) => Ok(serde_json::to_string(&result).unwrap_or_default()),
                Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
            }
        }
    }

    /// Get OVSDB schema (returns list of databases as a proxy for schema info)
    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        match self.client.list_dbs().await {
            Ok(dbs) => Ok(serde_json::to_string(&dbs).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// List OVSDB databases
    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        match self.client.list_dbs().await {
            Ok(dbs) => Ok(serde_json::to_string(&dbs).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Dump entire database
    async fn dump_db(&self) -> zbus::fdo::Result<String> {
        match self.client.dump_db("Open_vSwitch").await {
            Ok(result) => Ok(serde_json::to_string(&result).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Create bridge
    async fn create_bridge(&self, name: String) -> zbus::fdo::Result<()> {
        if let Some(engine) = &self.schema_engine {
            engine
                .mutate(
                    "net".to_string(),
                    "/org/opdbus/v1/ovsdb".to_string(),
                    ChangeType::MethodCall,
                    Some("create_bridge".to_string()),
                    simd_json::json!(name),
                    "dbus-client".to_string(),
                    None,
                )
                .await
                .map(|_| ())
                .map_err(|e: anyhow::Error| zbus::fdo::Error::Failed(e.to_string()))
        } else {
            self.client
                .create_bridge(&name)
                .await
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
        }
    }

    /// Delete bridge
    async fn delete_bridge(&self, name: String) -> zbus::fdo::Result<()> {
        self.client
            .delete_bridge(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Add port to bridge
    async fn add_port(&self, bridge: String, port: String) -> zbus::fdo::Result<()> {
        if let Some(engine) = &self.schema_engine {
            engine
                .mutate(
                    "net".to_string(),
                    "/org/opdbus/v1/ovsdb".to_string(),
                    ChangeType::MethodCall,
                    Some("add_port".to_string()),
                    simd_json::json!([bridge, port]),
                    "dbus-client".to_string(),
                    None,
                )
                .await
                .map(|_| ())
                .map_err(|e: anyhow::Error| zbus::fdo::Error::Failed(e.to_string()))
        } else {
            self.client
                .add_port(&bridge, &port)
                .await
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
        }
    }

    /// List bridges
    async fn list_bridges(&self) -> zbus::fdo::Result<String> {
        match self.client.list_bridges().await {
            Ok(bridges) => Ok(serde_json::to_string(&bridges).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// List ports on a bridge
    async fn list_ports(&self, bridge: String) -> zbus::fdo::Result<String> {
        match self.client.list_bridge_ports(&bridge).await {
            Ok(ports) => Ok(serde_json::to_string(&ports).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }
}

/// NonNet D-Bus interface - mirrors JSON-RPC methods
pub struct NonNetInterface {
    pub nonnet: Arc<NonNetDb>,
    pub schema_engine: Option<Arc<SchemaEngine>>,
}

impl NonNetInterface {
    pub fn new(nonnet: Arc<NonNetDb>, schema_engine: Option<Arc<SchemaEngine>>) -> Self {
        Self {
            nonnet,
            schema_engine,
        }
    }
}

#[interface(name = "org.opdbus.NonNetV1")]
impl NonNetInterface {
    /// Execute JSON-RPC transact on NonNet
    async fn transact(&self, request: String) -> zbus::fdo::Result<String> {
        let req_simd = str_to_simd(&request)?;
        let req_serde: Value = serde_json::from_str(&request)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let json_req: JsonRpcRequest = serde_json::from_value(req_serde.clone())
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        if json_req.method == "mutate"
            || json_req.method == "update"
            || json_req.method == "insert"
            || json_req.method == "delete"
        {
            if let Some(engine) = &self.schema_engine {
                match engine
                    .mutate(
                        "nonnet".to_string(),
                        "/org/opdbus/v1/nonnet".to_string(),
                        ChangeType::MethodCall,
                        Some(json_req.method.clone()),
                        req_simd,
                        "dbus-client".to_string(),
                        None,
                    )
                    .await
                {
                    Ok(result) => Ok(serde_json::to_string(&result.result).unwrap_or_default()),
                    Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
                }
            } else {
                let response = self.nonnet.handle_request(json_req).await;
                Ok(serde_json::to_string(&response).unwrap_or_default())
            }
        } else {
            let response = self.nonnet.handle_request(json_req).await;
            Ok(serde_json::to_string(&response).unwrap_or_default())
        }
    }

    /// Get NonNet schema
    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        let request =
            op_jsonrpc::protocol::JsonRpcRequest::new("get_schema", simd_json::json!(["OpNonNet"]));
        let response = self.nonnet.handle_request(request).await;
        Ok(serde_json::to_string(&response.result).unwrap_or_default())
    }

    /// List NonNet databases
    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", simd_json::json!([]));
        let response = self.nonnet.handle_request(request).await;
        Ok(serde_json::to_string(&response.result).unwrap_or_default())
    }
}
</file>

<file path="src/lib.rs">
//! op-dbus-mirror: 1:1 D-Bus publication of internal databases
//!
//! This crate publishes the internal OVSDB and NonNet database structures as a
//! D-Bus object hierarchy without introducing a second source of truth.

use anyhow::Result;
use dashmap::DashMap;
use managed_objects::{
    build_interface_map, ManagedObjectRegistry, ObjectManagerInterface, OBJECT_MANAGER_PATH,
    PROJECTED_IFACE,
};
use op_core::types::BusType;
use op_grpc_bridge::{OperationGrpcServer, SchemaEngine};
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_state::manager::StateManager;
use procfs::Current as _;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::sync::Arc;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};
use zbus::{connection::Builder, Connection};

pub mod dbus_interface;
pub mod event;
pub mod event_dispatcher;
pub mod event_sources;
pub mod heartbeat;
pub mod jsonrpc_interface;
pub mod managed_objects;
pub mod object;
pub mod plugin_interface;
pub mod session;
pub mod tree;

/// D-Bus publication service.
///
/// Responsible for maintaining a 1:1 D-Bus object view of authoritative
/// internal databases.
pub struct DbusMirror {
    ovsdb: Arc<OvsdbClient>,
    nonnet: Arc<NonNetDb>,
    schema_engine: Option<Arc<SchemaEngine>>,
    connection: Connection,
    /// Published D-Bus object paths managed by this service.
    published_objects: DashMap<String, ()>,
    /// Registry backing the org.freedesktop.DBus.ObjectManager at OBJECT_MANAGER_PATH.
    /// Tracks every plugin object so GetManagedObjects can enumerate them all.
    plugin_registry: ManagedObjectRegistry,
    /// Optional handle to the gRPC server so the ComponentRegistry can be
    /// mirrored into the D-Bus tree under /org/opdbus/v1/registry/.
    grpc_server: Option<Arc<OperationGrpcServer>>,
    /// StateManager for enumerating all registered plugins (active or not).
    state_manager: Option<Arc<StateManager>>,
    /// Current data and sequence numbers per object path
    current_data: DashMap<String, (Value, u64)>,
    /// Per-session state keyed by peer name
    pub sessions: DashMap<String, session::MirrorSession>,
}

impl DbusMirror {
    /// Create a new D-Bus publication service.
    pub async fn new(
        bus_type: BusType,
        ovsdb: Arc<OvsdbClient>,
        nonnet: Arc<NonNetDb>,
        schema_engine: Option<Arc<SchemaEngine>>,
    ) -> Result<Self> {
        let connection = match bus_type {
            BusType::System => Builder::system()?.name("org.opdbus.v1")?.build().await?,
            BusType::Session => Builder::session()?.name("org.opdbus.v1")?.build().await?,
        };

        Ok(Self {
            ovsdb,
            nonnet,
            schema_engine,
            connection,
            published_objects: DashMap::new(),
            plugin_registry: Arc::new(DashMap::new()),
            grpc_server: None,
            state_manager: None,
            current_data: DashMap::new(),
            sessions: DashMap::new(),
        })
    }

    /// Attach a gRPC server so the ComponentRegistry is mirrored into D-Bus.
    pub fn with_grpc_server(mut self, grpc_server: Arc<OperationGrpcServer>) -> Self {
        self.grpc_server = Some(grpc_server);
        self
    }

    /// Attach the StateManager so all registered plugins are always visible in
    /// the managed objects tree (active or not).
    pub fn with_state_manager(mut self, state_manager: Arc<StateManager>) -> Self {
        self.state_manager = Some(state_manager);
        self
    }

    /// Start the mirror service.
    ///
    /// Performs an initial full-tree publication and then enters an event-driven
    /// loop to publish deltas from all data sources.
    pub async fn start(self: Arc<Self>) -> Result<()> {
        tracing::info!("Starting D-Bus mirror publication service...");

        // Initial full sync
        if let Err(e) = self.refresh_full_tree().await {
            tracing::error!("Initial D-Bus mirror sync failed: {}", e);
        }

        // Register ObjectManager at the root to manage EVERYTHING.
        let om = ObjectManagerInterface::new(self.plugin_registry.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1", om)
            .await?;

        // Register PluginsV1 at the plugins path.
        let plugin_iface = plugin_interface::PluginInterface::new();
        let plugin_snap = plugin_iface.snapshot_handle();
        self.connection
            .object_server()
            .at("/org/opdbus/v1/plugins", plugin_iface)
            .await?;

        // Register mirror-management interface
        let interface = dbus_interface::DbusMirrorInterface::new(self.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1", interface)
            .await?;

        // Register OVSDB JSON-RPC interface at /org/opdbus/v1/ovsdb
        let ovsdb_interface =
            jsonrpc_interface::OvsdbInterface::new(self.ovsdb.clone(), self.schema_engine.clone());
        self.connection
            .object_server()
            .at("/org/opdbus/v1/ovsdb", ovsdb_interface)
            .await?;

        // Register NonNet JSON-RPC interface at /org/opdbus/v1/nonnet
        let nonnet_interface = jsonrpc_interface::NonNetInterface::new(
            self.nonnet.clone(),
            self.schema_engine.clone(),
        );
        self.connection
            .object_server()
            .at("/org/opdbus/v1/nonnet", nonnet_interface)
            .await?;

        // Create event dispatcher
        let dispatcher = crate::event_dispatcher::EventDispatcher::new(
            self.clone(),
            self.ovsdb.clone(),
            self.nonnet.clone(),
            self.state_manager.clone(),
            self.grpc_server.clone(),
        );

        // Spawn all event sources
        if let Err(e) = dispatcher.spawn_event_sources().await {
            tracing::error!("Failed to spawn event sources: {}", e);
        }

        // Spawn heartbeat task
        if let Err(e) =
            crate::heartbeat::spawn_heartbeat_task(self.clone(), dispatcher.broadcast_tx.clone())
                .await
        {
            tracing::error!("Failed to spawn heartbeat task: {}", e);
        }

        // Run event loop
        if let Err(e) = dispatcher.run_event_loop().await {
            tracing::error!("Event loop failed: {}", e);
        }

        // Populate fixed objects immediately on startup.
        self.refresh_plugin_snapshot(&plugin_snap).await;

        // Watch ComponentRegistry for live register/deregister events
        if let Some(grpc) = &self.grpc_server {
            let (_, mut rx) = grpc.registry_watch().await;
            let mirror = self.clone();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => mirror.apply_registry_event(event).await,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "ComponentRegistry watcher lagged by {} events, resyncing",
                                n
                            );
                            if let Err(e) = mirror.refresh_full_tree().await {
                                tracing::error!("Registry resync failed: {}", e);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        std::future::pending::<()>().await;
        Ok(())
    }

    /// Compatibility method for dbus_interface
    pub async fn publish_snapshot(&self) -> Result<()> {
        self.refresh_full_tree().await
    }

    pub fn published_count(&self) -> u64 {
        self.published_objects.len() as u64
    }

    pub fn projected_count(&self) -> u64 {
        self.published_objects.len() as u64 // Currently 1:1
    }

    /// Perform a full scan of all authoritative databases and ensure
    /// the D-Bus tree exactly matches the database state.
    pub async fn refresh_full_tree(&self) -> Result<()> {
        let mut active_paths = HashSet::new();
        tracing::info!("DEBUG: Starting full tree refresh");

        // 1. Scan OVSDB (Authoritative for Network)
        tracing::info!("DEBUG: Publishing OVSDB snapshot");
        if let Err(e) = self.publish_ovsdb_snapshot(&mut active_paths).await {
            tracing::warn!("OVSDB snapshot failed: {}", e);
        }

        // 2. Scan Procfs (Host state)
        tracing::info!("DEBUG: Publishing host snapshot");
        if let Err(e) = self.publish_host_snapshot(&mut active_paths).await {
            tracing::warn!("Procfs snapshot failed: {}", e);
        }

        // 3. Scan NonNet (Authoritative for Plugins not in Enterprise DB)
        tracing::info!("DEBUG: Publishing NonNet snapshot");
        if let Err(e) = self.publish_nonnet_snapshot(&mut active_paths).await {
            tracing::warn!("NonNet snapshot failed: {}", e);
        }

        // 4. gRPC ComponentRegistry snapshot (host/plugin now use fixed objects)
        if let Err(e) = self.publish_registry_snapshot(&mut active_paths).await {
            tracing::warn!("ComponentRegistry snapshot failed: {}", e);
        }

        // 6. Scan freedesktop system services
        if let Err(e) = self.publish_system_services(&mut active_paths).await {
            tracing::warn!("System services snapshot failed: {}", e);
        }

        // 5. Network interfaces and WireGuard peers
        if let Err(e) = self.publish_network_snapshot(&mut active_paths).await {
            tracing::warn!("Network snapshot failed: {}", e);
        }

        // 7. Running processes
        if let Err(e) = self.publish_process_snapshot(&mut active_paths).await {
            tracing::warn!("Process snapshot failed: {}", e);
        }

        // 8. Keep plugin objects published even when a plugin is currently inactive.
        if let Err(e) = self.publish_plugin_snapshot(&mut active_paths).await {
            tracing::warn!("Plugin snapshot failed: {}", e);
        }

        // 9. Remove any D-Bus objects that no longer exist in any authority
        self.remove_stale_publications(&active_paths).await?;

        Ok(())
    }

    async fn publish_host_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let sections = vec![
            "cpuinfo",
            "meminfo",
            "loadavg",
            "uptime",
            "stat",
            "vmstat",
            "diskstats",
            "mounts",
            "version",
        ];

        for section in sections {
            let path = format!("/org/opdbus/v1/host/{}", section);
            let data = match section {
                "meminfo" => self.gather_meminfo().await?,
                "cpuinfo" => self.gather_cpuinfo().await?,
                "loadavg" => self.gather_loadavg().await?,
                _ => serde_json::json!({ "status": "available" }),
            };
            self.publish_object(&path, data).await?;
            active_paths.insert(path);
        }

        Ok(())
    }

    async fn gather_meminfo(&self) -> Result<Value> {
        match procfs::Meminfo::current() {
            Ok(meminfo) => Ok(serde_json::to_value(meminfo).unwrap_or_default()),
            Err(e) => {
                tracing::warn!("Failed to read /proc/meminfo: {}", e);
                Ok(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    async fn gather_cpuinfo(&self) -> Result<Value> {
        match procfs::CpuInfo::current() {
            Ok(cpuinfo) => Ok(serde_json::to_value(cpuinfo).unwrap_or_default()),
            Err(e) => {
                tracing::warn!("Failed to read /proc/cpuinfo: {}", e);
                Ok(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    async fn gather_loadavg(&self) -> Result<Value> {
        match procfs::LoadAverage::current() {
            Ok(loadavg) => Ok(serde_json::to_value(loadavg).unwrap_or_default()),
            Err(e) => {
                tracing::warn!("Failed to read /proc/loadavg: {}", e);
                Ok(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    async fn publish_ovsdb_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        tracing::debug!("Scanning OVSDB for projection...");
        self.publish_object(
            "/org/opdbus/v1/ovsdb",
            serde_json::json!({
                "kind": "database",
                "database": "Open_vSwitch",
                "source": "ovsdb",
            }),
        )
        .await?;
        active_paths.insert("/org/opdbus/v1/ovsdb".to_string());
        self.publish_object(
            "/org/opdbus/v1/ovs",
            serde_json::json!({
                "kind": "database_alias",
                "database": "Open_vSwitch",
                "source": "ovsdb",
                "canonical_path": "/org/opdbus/v1/ovsdb",
            }),
        )
        .await?;
        active_paths.insert("/org/opdbus/v1/ovs".to_string());

        let dump_serde = self.ovsdb.dump_db("Open_vSwitch").await?;
        tracing::info!("DEBUG: OVSDB dump retrieved");
        // Convert serde_json::Value → serde_json::Value for compatibility
        let dump: Value = {
            let s = serde_json::to_string(&dump_serde)
                .map_err(|e| anyhow::anyhow!("dump_db serialize error: {}", e))?;
            serde_json::from_str(&s).map_err(|e| anyhow::anyhow!("dump_db parse error: {}", e))?
        };

        if let Value::Object(tables) = dump {
            tracing::info!("DEBUG: OVSDB dump contains {} tables", tables.len());
            let table_list: Vec<_> = tables
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            for (table_name, table_data) in table_list {
                let rows = table_data
                    .get("rows")
                    .and_then(|r| r.as_array())
                    .cloned()
                    .unwrap_or_default();
                let table_payload = serde_json::json!({
                    "kind": "table",
                    "database": "Open_vSwitch",
                    "table": table_name,
                    "row_count": rows.len(),
                });
                let table_path = format!("/org/opdbus/v1/ovsdb/{}", table_name);
                self.publish_object(&table_path, table_payload.clone())
                    .await?;
                active_paths.insert(table_path);

                let ovs_table_path = format!("/org/opdbus/v1/ovs/{}", table_name);
                self.publish_object(&ovs_table_path, table_payload).await?;
                active_paths.insert(ovs_table_path);

                if let Some(rows) = table_data.get("rows").and_then(|r| r.as_array()) {
                    tracing::info!("DEBUG: OVSDB table {} has {} rows", table_name, rows.len());
                    let rows_vec: Vec<_> = rows.iter().cloned().collect();
                    for (idx, row_data) in rows_vec.into_iter().enumerate() {
                        let row_id = Self::extract_uuid(&row_data);
                        let id = if row_id == "unknown" {
                            idx.to_string()
                        } else {
                            row_id
                        };
                        let path = format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, id);
                        self.publish_object(&path, row_data).await?;
                        active_paths.insert(path.clone());

                        let ovs_path = format!("/org/opdbus/v1/ovs/{}/{}", table_name, id);
                        self.publish_object(
                            &ovs_path,
                            serde_json::json!({
                                "canonical_path": path,
                                "database": "Open_vSwitch",
                                "table": table_name,
                            }),
                        )
                        .await?;
                        active_paths.insert(ovs_path);
                    }
                }
            }
        }

        Ok(())
    }

    async fn publish_nonnet_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        tracing::info!("DEBUG: NonNet snapshot started");
        self.publish_object(
            "/org/opdbus/v1/nonnet",
            serde_json::json!({
                "kind": "database_root",
                "source": "nonnet",
            }),
        )
        .await?;
        active_paths.insert("/org/opdbus/v1/nonnet".to_string());

        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", simd_json::json!([]));
        let response = self.nonnet.handle_request(request).await;

        let dbs: Vec<Value> = response
            .result
            .and_then(|v| serde_json::to_value(&v).ok())
            .and_then(|v| v.as_array().map(|a| a.to_vec()))
            .unwrap_or_default();

        tracing::info!("DEBUG: NonNet has {} databases", dbs.len());

        for db_name_val in dbs {
            if let Some(db_name) = db_name_val.as_str() {
                tracing::info!("DEBUG: Scanning NonNet DB: {}", db_name);
                let db_path = format!(
                    "/org/opdbus/v1/nonnet/{}",
                    Self::sanitize_dbus_path_segment(db_name)
                );
                self.publish_object(
                    &db_path,
                    serde_json::json!({
                        "kind": "database",
                        "database": db_name,
                        "source": "nonnet",
                    }),
                )
                .await?;
                active_paths.insert(db_path.clone());

                let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new(
                    "get_schema",
                    simd_json::json!([db_name]),
                );
                let schema_resp = self.nonnet.handle_request(schema_req).await;

                let schema_serde: Option<Value> = schema_resp
                    .result
                    .and_then(|v| serde_json::to_value(&v).ok());
                if let Some(tables) =
                    schema_serde.and_then(|s| s.get("tables").and_then(|v| v.as_object().cloned()))
                {
                    tracing::info!("DEBUG: NonNet DB {} has {} tables", db_name, tables.len());
                    let table_names: Vec<String> = tables.keys().map(|k| k.to_string()).collect();
                    for table_name in table_names {
                        let table_path = format!(
                            "{}/{}",
                            db_path,
                            Self::sanitize_dbus_path_segment(&table_name)
                        );
                        let dump_req = op_jsonrpc::protocol::JsonRpcRequest::new(
                            "transact",
                            simd_json::json!([
                                db_name,
                                {
                                    "op": "select",
                                    "table": table_name,
                                    "where": []
                                }
                            ]),
                        );
                        let dump_resp = self.nonnet.handle_request(dump_req).await;

                        let rows = dump_resp.result.and_then(|v| serde_json::to_value(&v).ok());
                        let rows = rows
                            .as_ref()
                            .and_then(|r| r.as_array())
                            .and_then(|results| results.first())
                            .and_then(|result| result.get("rows"))
                            .and_then(|v| v.as_array().map(|a| a.to_vec()))
                            .unwrap_or_default();

                        self.publish_object(
                            &table_path,
                            serde_json::json!({
                                "kind": "table",
                                "database": db_name,
                                "table": table_name,
                                "row_count": rows.len(),
                            }),
                        )
                        .await?;
                        active_paths.insert(table_path.clone());

                        tracing::info!(
                            "DEBUG: NonNet DB {} table {} has {} rows",
                            db_name,
                            table_name,
                            rows.len()
                        );
                        for (idx, row) in rows.into_iter().enumerate() {
                            let id = Self::extract_uuid(&row);
                            let id = if id == "unknown" { idx.to_string() } else { id };
                            let path =
                                format!("{}/{}", table_path, Self::sanitize_dbus_path_segment(&id));
                            self.publish_object(&path, row).await?;
                            active_paths.insert(path);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn publish_network_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        use std::process::Command;

        // Read network interfaces from /proc/net/dev
        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                let name = parts[0].trim_end_matches(':');
                let path = format!(
                    "/org/opdbus/v1/network/interface/{}",
                    Self::sanitize_path_segment(name)
                );
                let data = serde_json::json!({ "name": name, "source": "/proc/net/dev" });
                self.publish_object(&path, data).await?;
                active_paths.insert(path);
            }
        }

        // Read WireGuard peers from `wg show all dump`
        if let Ok(out) = Command::new("wg").args(["show", "all", "dump"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut current_iface = String::new();
            for line in text.lines() {
                let cols: Vec<&str> = line.split('\t').collect();
                if cols.len() == 5 {
                    // Interface line: iface pubkey privkey listen_port fwmark
                    current_iface = cols[0].to_string();
                    let path = format!(
                        "/org/opdbus/v1/network/wireguard/{}",
                        Self::sanitize_path_segment(&current_iface)
                    );
                    let data = serde_json::json!({
                        "interface": current_iface,
                        "public_key": cols[1],
                        "listen_port": cols[3],
                        "fwmark": cols[4],
                    });
                    self.publish_object(&path, data).await?;
                    active_paths.insert(path);
                } else if cols.len() >= 8 && !current_iface.is_empty() {
                    // Peer line: iface pubkey preshared endpoint allowed_ips latest_handshake rx tx keepalive
                    let peer_key = cols[1];
                    let safe_key = peer_key
                        .replace('/', "_")
                        .replace('+', "_")
                        .replace('=', "");
                    let path = format!(
                        "/org/opdbus/v1/network/wireguard/{}/peer/{}",
                        Self::sanitize_path_segment(&current_iface),
                        safe_key
                    );
                    let data = serde_json::json!({
                        "interface": current_iface,
                        "public_key": peer_key,
                        "endpoint": cols[3],
                        "allowed_ips": cols[4],
                        "latest_handshake": cols[5].parse::<u64>().unwrap_or(0),
                        "transfer_rx": cols[6].parse::<u64>().unwrap_or(0),
                        "transfer_tx": cols[7].parse::<u64>().unwrap_or(0),
                    });
                    self.publish_object(&path, data).await?;
                    active_paths.insert(path);
                }
            }
        }

        Ok(())
    }

    async fn publish_process_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        use procfs::process::all_processes;

        if let Ok(procs) = all_processes() {
            for proc in procs.flatten() {
                let pid = proc.pid();
                let stat = match proc.stat() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let path = format!("/org/opdbus/v1/process/_{}", pid);
                let data = serde_json::json!({
                    "pid": pid,
                    "name": stat.comm,
                    "state": stat.state().map(|s| format!("{:?}", s)).unwrap_or_default(),
                    "ppid": stat.ppid,
                    "threads": stat.num_threads,
                });
                self.publish_object(&path, data).await?;
                active_paths.insert(path);
            }
        }

        Ok(())
    }

    /// Services that are too large or ephemeral to project, or that op-dbus replaces.
    const SKIP_SERVICES: &'static [&'static str] = &[
        "org.freedesktop.systemd1",       // thousands of unit objects
        "org.freedesktop.NetworkManager", // replaced by op-dbus, huge ephemeral tree
        "fi.w1.wpa_supplicant1",          // ephemeral BSS/scan results
        "org.freedesktop.DBus",           // meta bus service
    ];

    async fn publish_system_services(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let system_conn = zbus::Connection::system().await?;
        let proxy = zbus::fdo::DBusProxy::new(&system_conn).await?;
        let names = proxy.list_names().await?;

        for name in names {
            let name_str = name.as_str();
            if name_str.starts_with(':')
                || name_str.starts_with("org.opdbus")
                || Self::SKIP_SERVICES.iter().any(|s| *s == name_str)
            {
                continue;
            }

            // Walk the full object tree for this service recursively
            let mut queue: Vec<String> = vec!["/".to_string()];
            while let Some(obj_path) = queue.pop() {
                let proxy = match zbus::fdo::IntrospectableProxy::builder(&system_conn)
                    .destination(name_str)
                    .and_then(|b| b.path(obj_path.as_str()))
                {
                    Ok(b) => match b.build().await {
                        Ok(p) => p,
                        Err(_) => continue,
                    },
                    Err(_) => continue,
                };

                let xml = match proxy.introspect().await {
                    Ok(x) => x,
                    Err(_) => continue,
                };

                let node = match zbus_xml::Node::try_from(xml.as_str()) {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                // Enqueue child paths for recursive walk
                for child in node.nodes() {
                    if let Some(child_name) = child.name() {
                        let child_path = if obj_path == "/" {
                            format!("/{}", child_name)
                        } else {
                            format!("{}/{}", obj_path.trim_end_matches('/'), child_name)
                        };
                        queue.push(child_path);
                    }
                }

                // Only publish nodes that have meaningful interfaces
                let mut interfaces = Vec::new();
                let mut methods = Vec::new();
                let mut properties = Vec::new();
                let mut signals = Vec::new();

                for iface in node.interfaces() {
                    let iface_name: String = iface.name().to_string();
                    if iface_name == "org.freedesktop.DBus.Introspectable"
                        || iface_name == "org.freedesktop.DBus.Peer"
                        || iface_name == "org.freedesktop.DBus.Properties"
                    {
                        continue;
                    }
                    interfaces.push(Value::from(iface_name));
                    for m in iface.methods() {
                        methods.push(Value::from(m.name().to_string()));
                    }
                    for p in iface.properties() {
                        properties.push(Value::from(p.name().to_string()));
                    }
                    for s in iface.signals() {
                        signals.push(Value::from(s.name().to_string()));
                    }
                }

                if interfaces.is_empty() {
                    continue;
                }

                // Map the real object path into our namespace
                let safe_obj = obj_path
                    .trim_start_matches('/')
                    .replace('/', "_")
                    .replace('-', "_");
                let safe_svc = name_str.replace('.', "/").replace('-', "_");
                let mirror_path = if safe_obj.is_empty() {
                    format!("/org/opdbus/v1/system/{}", safe_svc)
                } else {
                    format!("/org/opdbus/v1/system/{}/{}", safe_svc, safe_obj)
                };

                let data = serde_json::json!({
                    "service": name_str,
                    "path": obj_path,
                    "interfaces": interfaces,
                    "methods": methods,
                    "properties": properties,
                    "signals": signals,
                });

                self.publish_object(&mirror_path, data).await?;
                active_paths.insert(mirror_path);
            }
        }

        Ok(())
    }

    async fn publish_registry_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let grpc = match &self.grpc_server {
            Some(g) => g,
            None => return Ok(()),
        };
        let (components, _) = grpc.registry_watch().await;
        for info in components {
            let path = Self::registry_dbus_path(&info.component_id);
            let data = Self::component_info_to_value(&info);
            self.publish_object(&path, data).await?;
            active_paths.insert(path);
        }
        Ok(())
    }

    /// Handle a single live ComponentRegistry event from the broadcast channel.
    async fn apply_registry_event(&self, event: op_grpc_bridge::proto::registry::RegistryEvent) {
        use op_grpc_bridge::proto::registry::RegistryEventType;
        let event_type = RegistryEventType::try_from(event.event_type)
            .unwrap_or(RegistryEventType::RegistryEventRegistered);

        match event_type {
            RegistryEventType::RegistryEventRegistered
            | RegistryEventType::RegistryEventUpdated => {
                if let Some(info) = event.component {
                    let path = Self::registry_dbus_path(&info.component_id);
                    let data = Self::component_info_to_value(&info);
                    if let Err(e) = self.publish_object(&path, data).await {
                        tracing::warn!("registry publish failed for {}: {}", path, e);
                    }
                }
            }
            RegistryEventType::RegistryEventDeregistered => {
                if let Some(info) = event.component {
                    let path = Self::registry_dbus_path(&info.component_id);
                    let op = match ObjectPath::try_from(path.as_str()) {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let _ = self
                        .connection
                        .object_server()
                        .remove::<object::MirrorObject, _>(op)
                        .await;
                    self.published_objects.remove(&path);
                }
            }
            _ => {}
        }
    }

    fn registry_dbus_path(component_id: &str) -> String {
        let safe = component_id.replace(['.', '-', ':'], "_");
        format!("/org/opdbus/v1/registry/{}", safe)
    }

    fn plugin_dbus_path(plugin_id: &str) -> String {
        format!(
            "/org/opdbus/v1/plugins/{}",
            Self::sanitize_dbus_path_segment(plugin_id)
        )
    }

    fn is_permanent_plugin_path(path: &str) -> bool {
        if !path.starts_with("/org/opdbus/v1/plugins/") {
            return false;
        }

        let remainder = &path["/org/opdbus/v1/plugins/".len()..];
        !remainder.is_empty() && !remainder.contains('/')
    }

    fn sanitize_dbus_path_segment(segment: &str) -> String {
        let mut out = String::with_capacity(segment.len());
        for ch in segment.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                out.push(ch);
            } else {
                out.push('_');
            }
        }

        if out.is_empty() {
            "_".to_string()
        } else {
            out
        }
    }

    fn component_info_to_value(info: &op_grpc_bridge::proto::registry::ComponentInfo) -> Value {
        use serde_json::Map;
        let mut map = Map::new();
        map.insert(
            "component_id".into(),
            Value::from(info.component_id.clone()),
        );
        map.insert(
            "component_type".into(),
            Value::from(info.component_type.clone()),
        );
        map.insert("name".into(), Value::from(info.name.clone()));
        map.insert("description".into(), Value::from(info.description.clone()));
        map.insert("endpoint".into(), Value::from(info.endpoint.clone()));
        map.insert("version".into(), Value::from(info.version.clone()));
        map.insert("status".into(), Value::from(info.status as i64));
        map.insert("schema_json".into(), Value::from(info.schema_json.clone()));
        let caps: Vec<Value> = info
            .capabilities
            .iter()
            .map(|s| Value::from(s.clone()))
            .collect();
        map.insert("capabilities".into(), Value::Array(caps));
        let mut meta = Map::new();
        for (k, v) in &info.metadata {
            meta.insert(k.clone(), Value::from(v.clone()));
        }
        map.insert("metadata".into(), Value::Object(meta));
        Value::Object(map)
    }

    /// Push current plugin state into the fixed PluginInterface object.
    async fn refresh_plugin_snapshot(&self, handle: &plugin_interface::PluginSnapshot) {
        let sm = match &self.state_manager {
            Some(sm) => sm.clone(),
            None => return,
        };
        let live_state_raw = sm.query_current_state().await.unwrap_or_default();
        let live_state: std::collections::HashMap<String, Value> = live_state_raw
            .into_iter()
            .map(|(k, v)| (k, serde_json::to_value(&v).unwrap_or_default()))
            .collect();
        let mut map = std::collections::HashMap::new();
        for name in sm.list_plugins() {
            let json = match live_state.get(&name) {
                Some(state) => {
                    let mut obj = match state {
                        Value::Object(o) => o.clone(),
                        other => {
                            let mut m = Map::new();
                            m.insert("state".into(), other.clone());
                            m
                        }
                    };
                    obj.insert("active".into(), Value::from(true));
                    serde_json::to_string(&Value::Object(obj)).unwrap_or_default()
                }
                None => format!("{{\"active\":false,\"name\":{:?}}}", name),
            };
            map.insert(name, json);
        }
        *handle.write().await = map;
    }

    /// Publish every registered plugin as a stable D-Bus object.
    ///
    /// If a plugin cannot currently provide state, it still appears in the tree
    /// with `active: false`.
    async fn publish_plugin_snapshot(&self, active_paths: &mut HashSet<String>) -> Result<()> {
        let sm = match &self.state_manager {
            Some(sm) => sm.clone(),
            None => return Ok(()),
        };

        let live_state_raw = sm.query_current_state().await.unwrap_or_default();
        let live_state: std::collections::HashMap<String, Value> = live_state_raw
            .into_iter()
            .map(|(k, v)| (k, serde_json::to_value(&v).unwrap_or_default()))
            .collect();

        for plugin_name in sm.list_plugins() {
            let path = Self::plugin_dbus_path(&plugin_name);
            let data = match live_state.get(&plugin_name) {
                Some(state) => {
                    let mut obj = match state {
                        Value::Object(o) => o.clone(),
                        other => {
                            let mut m = Map::new();
                            m.insert("state".into(), other.clone());
                            m
                        }
                    };
                    obj.insert("active".into(), Value::from(true));
                    obj.insert("name".into(), Value::from(plugin_name.clone()));
                    Value::Object(obj)
                }
                None => serde_json::json!({
                    "active": false,
                    "name": plugin_name.clone(),
                }),
            };

            self.publish_object(&path, data.clone()).await?;
            active_paths.insert(path.clone());

            if live_state.contains_key(&plugin_name) {
                self.publish_plugin_children(&path, &data, active_paths)
                    .await?;
            }
        }

        Ok(())
    }

    async fn publish_plugin_children(
        &self,
        root_path: &str,
        data: &Value,
        active_paths: &mut HashSet<String>,
    ) -> Result<()> {
        let mut projected = Vec::new();
        self.collect_plugin_children(root_path, data, &mut projected);

        for (path, value) in projected {
            self.publish_object(&path, value.clone()).await?;
            active_paths.insert(path.clone());
        }

        Ok(())
    }

    fn collect_plugin_children(
        &self,
        root_path: &str,
        data: &Value,
        out: &mut Vec<(String, Value)>,
    ) {
        match data {
            Value::Object(map) => {
                for (key, value) in map.iter() {
                    let child_path = format!(
                        "{}/{}",
                        root_path,
                        Self::sanitize_dbus_path_segment(key.as_str())
                    );
                    out.push((child_path.clone(), Self::child_value_payload(value)));
                    self.collect_plugin_children(&child_path, value, out);
                }
            }
            Value::Array(items) => {
                for (idx, value) in items.iter().enumerate() {
                    let child_path = format!("{}/{}", root_path, idx);
                    out.push((child_path.clone(), Self::child_value_payload(value)));
                    self.collect_plugin_children(&child_path, value, out);
                }
            }
            _ => {}
        }
    }

    fn child_value_payload(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(map.clone()),
            Value::Array(items) => {
                let mut payload = serde_json::Map::new();
                payload.insert("items".into(), Value::Array(items.clone()));
                Value::Object(payload)
            }
            scalar => {
                let mut payload = serde_json::Map::new();
                payload.insert("value".into(), scalar.clone());
                Value::Object(payload)
            }
        }
    }

    async fn publish_object(&self, path: &str, data: Value) -> Result<()> {
        // Get current data and sequence from the store
        let mut entry = self
            .current_data
            .entry(path.to_string())
            .or_insert_with(|| (serde_json::json!({}), 0u64));
        let (current_data, sequence) = &mut *entry;

        // Check if data has changed
        let changed = *current_data != data;

        if changed {
            // Increment sequence number
            *sequence += 1;

            // Update stored data
            *current_data = data.clone();

            if self.published_objects.contains_key(path) {
                // Object already registered — update data in-place and signal if changed.
                if let Ok(iface_ref) = self
                    .connection
                    .object_server()
                    .interface::<_, object::MirrorObject>(path)
                    .await
                {
                    let _ = iface_ref.get_mut().await.update_data(data.clone());
                    // Emit PropertiesChanged with only changed fields
                    let ctxt = iface_ref.signal_context();
                    let _ = iface_ref.get().await.data_updated(ctxt).await;
                    // Ensure the ObjectManager knows the properties changed as well
                    self.register_in_object_manager(path, &data).await;
                }
            } else {
                let obj = object::MirrorObject::new(data.clone());
                self.connection.object_server().at(path, obj).await?;
                self.published_objects.insert(path.to_string(), ());

                self.register_in_object_manager(path, &data).await;
            }
        }

        Ok(())
    }

    /// Load plugin state into the mirror (Seeding).
    ///
    /// Each plugin gets both a `MirrorObject` at `/org/opdbus/v1/plugins/{id}`
    /// and an entry in the `ObjectManagerInterface` registry so that
    /// `GetManagedObjects` immediately returns all loaded plugins.
    pub async fn load_plugin_state(&self, plugins: &std::collections::HashMap<String, Value>) {
        for (plugin_id, state) in plugins {
            let path = Self::plugin_dbus_path(plugin_id);
            if let Err(e) = self.publish_object(&path, state.clone()).await {
                tracing::error!("Failed to seed mirror for {}: {}", plugin_id, e);
                continue;
            }
            let mut active_paths = HashSet::new();
            if let Err(e) = self
                .publish_plugin_children(&path, state, &mut active_paths)
                .await
            {
                tracing::warn!(
                    "Failed to seed child plugin objects for {}: {}",
                    plugin_id,
                    e
                );
            }
        }
    }

    /// Insert a plugin object into the ObjectManager registry and emit
    /// `InterfacesAdded` if the ObjectManager interface is already up.
    async fn register_in_object_manager(&self, path: &str, data: &Value) {
        let op = match OwnedObjectPath::try_from(path.to_string()) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("register_in_object_manager: invalid path {path}: {e}");
                return;
            }
        };

        let json_str = serde_json::to_string(data).unwrap_or_default();
        let existed = self
            .plugin_registry
            .insert(op.clone(), build_interface_map(&json_str))
            .is_some();

        // Best-effort: emit InterfacesAdded only when the object first appears.
        if existed {
            return;
        }

        match self
            .connection
            .object_server()
            .interface::<_, ObjectManagerInterface>(OBJECT_MANAGER_PATH)
            .await
        {
            Ok(iface_ref) => {
                let ctxt = iface_ref.signal_context();
                if let Err(e) = ObjectManagerInterface::interfaces_added(
                    ctxt,
                    op,
                    build_interface_map(&json_str),
                )
                .await
                {
                    tracing::warn!("InterfacesAdded signal failed for {path}: {e}");
                }
            }
            Err(_) => {
                // ObjectManager not yet registered — registry is already updated,
                // so GetManagedObjects will return this entry once it comes up.
            }
        }
    }

    /// Remove a plugin object from the ObjectManager registry and emit
    /// `InterfacesRemoved`.
    async fn deregister_from_object_manager(&self, path: &str) {
        let op = match OwnedObjectPath::try_from(path.to_string()) {
            Ok(p) => p,
            Err(_) => return,
        };

        if self.plugin_registry.remove(&op).is_none() {
            return; // was not a managed plugin object
        }

        match self
            .connection
            .object_server()
            .interface::<_, ObjectManagerInterface>(OBJECT_MANAGER_PATH)
            .await
        {
            Ok(iface_ref) => {
                let ctxt = iface_ref.signal_context();
                let interfaces = vec![PROJECTED_IFACE.to_string()];
                if let Err(e) =
                    ObjectManagerInterface::interfaces_removed(ctxt, op, interfaces).await
                {
                    tracing::warn!("InterfacesRemoved signal failed for {path}: {e}");
                }
            }
            Err(_) => {}
        }
    }

    async fn remove_stale_publications(&self, active_paths: &HashSet<String>) -> Result<()> {
        let mut to_remove = Vec::new();
        for entry in self.published_objects.iter() {
            let key = entry.key();
            // Plugin objects are permanent — they stay in the tree with active:false
            // rather than being removed when inactive.
            if Self::is_permanent_plugin_path(key) {
                continue;
            }
            if !active_paths.contains(key) {
                to_remove.push(key.clone());
            }
        }

        for path in to_remove {
            let op = ObjectPath::try_from(path.as_str())?;
            self.connection
                .object_server()
                .remove::<object::MirrorObject, _>(op)
                .await?;
            self.published_objects.remove(&path);

            // If this was a plugin-managed object, remove it from the registry
            // and emit InterfacesRemoved.
            if path.starts_with("/org/opdbus/v1/plugins/") {
                self.deregister_from_object_manager(&path).await;
            }
        }

        Ok(())
    }

    pub fn list_published_paths(&self) -> Vec<String> {
        self.published_objects
            .iter()
            .map(|e| entry_path_to_dbus(e.key()))
            .collect()
    }

    /// Expose the underlying D-Bus connection so callers can register
    /// additional interfaces on the same bus name.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    fn extract_uuid(row: &Value) -> String {
        if let Some(uuid_val) = row.get("_uuid") {
            if let Some(uuid_arr) = uuid_val.as_array() {
                if uuid_arr.len() == 2 && uuid_arr[0].as_str() == Some("uuid") {
                    if let Some(uuid_str) = uuid_arr[1].as_str() {
                        return Self::sanitize_path_segment(uuid_str);
                    }
                }
            }
        }
        if let Some(uuid) = row.get("uuid").and_then(|v| v.as_str()) {
            return Self::sanitize_path_segment(uuid);
        }
        if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
            return Self::sanitize_path_segment(id);
        }
        if let Some(s) = row.get("name").and_then(|v| v.as_str()) {
            return Self::sanitize_path_segment(s);
        }
        "unknown".to_string()
    }

    fn sanitize_path_segment(raw: &str) -> String {
        raw.chars()
            .map(|ch| match ch {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => ch,
                '-' | '.' | ':' | ' ' | '/' => '_',
                _ => '_',
            })
            .collect()
    }
}

fn entry_path_to_dbus(path: &str) -> String {
    path.to_string()
}

pub mod prelude {
    pub use super::DbusMirror;
}
</file>

<file path="src/lib.rs.orig">
//! op-dbus-mirror: 1:1 D-Bus projection of internal databases
//!
//! This crate provides a mechanism to mirror the internal OVSDB and NonNet
//! database structures into a D-Bus object hierarchy.

use anyhow::Result;
use op_core::types::BusType;
use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::ovsdb::OvsdbClient;
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::sync::Arc;
use zbus::{connection::Builder, Connection};
use dashmap::DashMap;
use sqlx::{sqlite::SqlitePool, Row};

pub mod tree;
pub mod object;
pub mod dbus_interface;

/// D-Bus Mirror Service
///
/// Responsible for maintaining a 1:1 D-Bus projection of internal databases.
pub struct DbusMirror {
    ovsdb: Arc<OvsdbClient>,
    nonnet: Arc<NonNetDb>,
    connection: Connection,
    /// Map of registered D-Bus object paths
    projected_objects: DashMap<String, ()>,
    /// Enterprise state database pool
    db_pool: Option<SqlitePool>,
}

impl DbusMirror {
    /// Create a new D-Bus mirror
    pub async fn new(
        bus_type: BusType,
        ovsdb: Arc<OvsdbClient>,
        nonnet: Arc<NonNetDb>,
    ) -> Result<Self> {
        let connection = match bus_type {
            BusType::System => Builder::system()?.name("org.opdbus.v1")?.build().await?,
            BusType::Session => Builder::session()?.name("org.opdbus.v1")?.build().await?,
        };

        // Initialize Enterprise DB pool if it exists
        let db_path = "/var/lib/op-dbus/state.db";
        let db_pool = if std::path::Path::new(db_path).exists() {
            Some(SqlitePool::connect(&format!("sqlite://{}", db_path)).await?)
        } else {
            None
        };

        Ok(Self {
            ovsdb,
            nonnet,
            connection,
            projected_objects: DashMap::new(),
            db_pool,
        })
    }

    /// Reconcile the D-Bus tree with the current database state
    ///
    /// This function performs a full mirror sync:
    /// 1. Dumps OVSDB and NonNet
    /// 2. Maps tables/rows to D-Bus paths
    /// 3. Registers/unregisters objects to match the DB
    pub async fn reconcile(&self) -> Result<()> {
        tracing::info!("Starting 1:1 D-Bus mirror reconciliation");

        // 1. Mirror OVSDB
        self.mirror_ovsdb().await?;

        // 2. Mirror NonNet
        self.mirror_nonnet().await?;

        // 3. Mirror Enterprise Namespace (org.opdbus.*)
        self.mirror_enterprise().await?;

        tracing::info!("D-Bus mirror reconciliation complete");
        Ok(())
    }

    /// Mirror Enterprise Namespace into their respective paths
    async fn mirror_enterprise(&self) -> Result<()> {
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(()),
        };

        // Get all live objects
        let rows = sqlx::query("SELECT object_path, state FROM live_objects")
            .fetch_all(pool)
            .await?;

        for row in rows {
            let path: String = row.get("object_path");
            let mut state_str: String = row.get("state");
            
            let state_val: Value = unsafe { 
                simd_json::from_str(state_str.as_mut_str())? 
            };
            
            self.register_projected_object(&path, state_val).await?;
        }

        // Also ensure we request names for all pre-populated services
        let services = sqlx::query("SELECT service_name FROM namespace_services WHERE enabled = 1")
            .fetch_all(pool)
            .await?;

        for s_row in services {
            let service_name: String = s_row.get("service_name");
            // Request the name on the bus so we own the namespace
            if let Err(e) = self.connection.request_name(service_name.clone()).await {
                tracing::debug!("Could not request name {}: {}", service_name, e);
            }
        }

        Ok(())
    }

    /// Mirror OVSDB into /org/opdbus/v1/ovsdb/
    async fn mirror_ovsdb(&self) -> Result<()> {
        let db_name = "Open_vSwitch";
        let dump = self.ovsdb.dump_db(db_name).await?;
        
        if let Value::Object(tables) = dump {
            for (table_name, rows) in tables.iter() {
                if let Some(row_arr) = rows.as_array() {
                    for row in row_arr {
                        let uuid = self.extract_uuid(row).unwrap_or_else(|| "unknown".to_string());
                        let path = format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, uuid.replace('-', "_"));
                        
                        self.register_projected_object(&path, row.clone()).await?;
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Mirror NonNet into /org/opdbus/v1/nonnet/
    async fn mirror_nonnet(&self) -> Result<()> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", Value::Array(vec![]));
        let response = self.nonnet.handle_request(request).await;
        
        if let Some(dbs) = response.result.and_then(|v: Value| v.as_array().map(|a| a.to_vec())) {
            for db_val in dbs {
                if let Some(db_name) = db_val.as_str() {
                    // Get schema to find tables
                    let schema_req = op_jsonrpc::protocol::JsonRpcRequest::new("get_schema", Value::Array(vec![Value::from(db_name)]));
                    let schema_resp = self.nonnet.handle_request(schema_req).await;
                    
                    if let Some(schema) = schema_resp.result {
                        if let Some(tables) = schema.get("tables").and_then(|v: &Value| v.as_object()) {
                            for (table_name, _) in tables.iter() {
                                let select_req = op_jsonrpc::protocol::JsonRpcRequest::new("transact", simd_json::json!([
                                    db_name,
                                    {
                                        "op": "select",
                                        "table": table_name,
                                        "where": []
                                    }
                                ]));
                                let select_resp = self.nonnet.handle_request(select_req).await;
                                
                                if let Some(results) = select_resp.result.and_then(|v: Value| v.as_array().map(|a| a.to_vec())) {
                                    if let Some(rows) = results.get(0).and_then(|r: &Value| r.get("rows")).and_then(|v: &Value| v.as_array()) {
                                        for row in rows {
                                            let uuid = self.extract_uuid(row).unwrap_or_else(|| "unknown".to_string());
                                            let path = format!("/org/opdbus/v1/nonnet/{}/{}/{}", db_name, table_name, uuid.replace('-', "_"));
                                            self.register_projected_object(&path, row.clone()).await?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn register_projected_object(&self, path: &str, data: Value) -> Result<()> {
        let path_owned = path.to_string();
        
        // Ensure path is valid D-Bus path
        let dbus_path = zbus::zvariant::ObjectPath::try_from(path_owned.clone())
            .map_err(|e| anyhow::anyhow!("Invalid D-Bus path {}: {}", path_owned, e))?;

        if self.projected_objects.contains_key(&path_owned) {
            // Signal property change on existing object
            let server = self.connection.object_server();
            match server.interface::<_, object::MirrorObject>(dbus_path).await {
                Ok(iface_ref) => {
                    let mut obj = iface_ref.get_mut().await;
                    if obj.update_data(data) {
                        tracing::debug!("Emitting property change signal for {}", path_owned);
                        // Emit the signal
                        let ctxt = iface_ref.signal_context();
                        iface_ref.get().await.data_updated(ctxt).await?;
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to get interface for {}: {}", path_owned, e);
                }
            }
            return Ok(())
        }

        tracing::info!("Registering new projected object: {}", path_owned);
        let obj = object::MirrorObject::new(data);
        self.connection.object_server().at(dbus_path, obj).await?;
        self.projected_objects.insert(path_owned, ());
        
        Ok(())
    }

    fn extract_uuid(&self, row: &Value) -> Option<String> {
        // OVSDB rows usually have a _uuid field which is ["uuid", "actual-uuid-string"]
        if let Some(uuid_val) = row.get("_uuid") {
            if let Some(arr) = uuid_val.as_array() {
                if arr.len() == 2 && arr[0] == "uuid" {
                    return arr[1].as_str().map(|s: &str| s.to_string());
                }
            }
            if let Some(s) = uuid_val.as_str() {
                return Some(s.to_string());
            }
        }
        
        // Fallback to 'name' or other identity fields if _uuid is missing
        row.get("name").and_then(|v: &Value| v.as_str()).map(|s: &str| s.to_string())
    }

    pub fn projected_count(&self) -> usize {
        self.projected_objects.len()
    }

    /// Start the mirror reconciliation loop
    pub async fn start(self: Arc<Self>) -> Result<()> {
        // Register the main mirror interface
        let interface = dbus_interface::DbusMirrorInterface::new(self.clone());
        self.connection.object_server().at("/org/opdbus/v1", interface).await?;

        // 1. Start NonNet update listener
        let mut nonnet_rx = self.nonnet.subscribe();
        let mirror_clone = self.clone();
        tokio::spawn(async move {
            while let Ok(update) = nonnet_rx.recv().await {
                for row in update.rows {
                    let uuid = mirror_clone.extract_uuid(&row).unwrap_or_else(|| "unknown".to_string());
                    let path = format!("/org/opdbus/v1/nonnet/OpNonNet/{}/{}", update.table, uuid.replace('-', "_"));
                    if let Err(e) = mirror_clone.register_projected_object(&path, row).await {
                        tracing::error!("Failed to register NonNet object {}: {}", path, e);
                    }
                }
            }
        });

        // 2. Start OVSDB monitor listener
        let ovsdb_clone = self.ovsdb.clone();
        let mirror_ovs_clone = self.clone();
        tokio::spawn(async move {
            if let Ok(mut rx) = ovsdb_clone.monitor_db("Open_vSwitch").await {
                while let Some(update) = rx.recv().await {
                    // Update format: ["update", null, {table: {uuid: {new: row}}}]
                    if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
                        if params.len() >= 3 {
                            if let Some(tables) = params[2].as_object() {
                                for (table_name, table_update) in tables.iter() {
                                    if let Some(uuids) = table_update.as_object() {
                                        for (uuid, row_update) in uuids.iter() {
                                            if let Some(new_row) = row_update.get("new") {
                                                let path = format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, uuid.replace('-', "_"));
                                                if let Err(e) = mirror_ovs_clone.register_projected_object(&path, new_row.clone()).await {
                                                    tracing::error!("Failed to register OVSDB object {}: {}", path, e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        // 3. Start periodic full reconciliation
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = self.reconcile().await {
                tracing::error!("D-Bus mirror reconciliation failed: {}", e);
            }
        }
    }
}

pub mod prelude {
    pub use super::DbusMirror;
}
</file>

<file path="src/managed_objects.rs">
//! org.freedesktop.DBus.ObjectManager implementation
//!
//! Provides `GetManagedObjects` so any D-Bus client can enumerate every object
//! created by a plugin in a single round-trip call.
//!
//! The interface is registered at `/org/opdbus/v1/plugins`.  Every plugin
//! object published under that path is reflected in the registry; the
//! `InterfacesAdded` / `InterfacesRemoved` signals are emitted as objects
//! come and go.
//!
//! D-Bus signature of GetManagedObjects: `a{oa{sa{sv}}}`
//!   ObjectPath  →  interface-name  →  property-name  →  variant

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use zbus::interface;
use zbus::zvariant::OwnedObjectPath;

/// Properties for a single interface: `a{ss}` — property name → JSON-encoded value.
/// Using String instead of OwnedValue keeps the type Clone + simple.
pub type PropertyMap = HashMap<String, String>;

/// All interfaces (with their properties) for one object: `a{sa{ss}}`
pub type InterfaceMap = HashMap<String, PropertyMap>;

/// Registry: ObjectPath → InterfaceMap.  Shared between DbusMirror and the
/// ObjectManagerInterface so writes are visible immediately to readers.
pub type ManagedObjectRegistry = Arc<DashMap<OwnedObjectPath, InterfaceMap>>;

/// D-Bus path where the ObjectManager is registered.
pub const OBJECT_MANAGER_PATH: &str = "/org/opdbus/v1";

/// Interface name exposed on every projected plugin object.
pub const PROJECTED_IFACE: &str = "org.opdbus.ProjectedObjectV1";

// ── Interface ──────────────────────────────────────────────────────────────

pub struct ObjectManagerInterface {
    registry: ManagedObjectRegistry,
}

impl ObjectManagerInterface {
    pub fn new(registry: ManagedObjectRegistry) -> Self {
        Self { registry }
    }
}

#[interface(name = "org.freedesktop.DBus.ObjectManager")]
impl ObjectManagerInterface {
    /// Return every managed object with all their interface properties.
    fn get_managed_objects(&self) -> HashMap<OwnedObjectPath, InterfaceMap> {
        self.registry
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect()
    }

    /// Emitted when a new object (or new interfaces on an existing object)
    /// appears under this manager.
    #[zbus(signal)]
    pub async fn interfaces_added(
        ctxt: &zbus::object_server::SignalContext<'_>,
        object_path: OwnedObjectPath,
        interfaces_and_properties: InterfaceMap,
    ) -> zbus::Result<()>;

    /// Emitted when an object (or some of its interfaces) is removed.
    #[zbus(signal)]
    pub async fn interfaces_removed(
        ctxt: &zbus::object_server::SignalContext<'_>,
        object_path: OwnedObjectPath,
        interfaces: Vec<String>,
    ) -> zbus::Result<()>;
}

// ── Helper ─────────────────────────────────────────────────────────────────

/// Build the `InterfaceMap` for a plugin object whose state is a raw JSON blob.
///
/// The single interface `org.opdbus.ProjectedObjectV1` is exposed with a
/// `JsonData` property that carries the serialised JSON.
pub fn build_interface_map(json_str: &str) -> InterfaceMap {
    let mut props = PropertyMap::new();
    props.insert("JsonData".to_string(), json_str.to_string());
    let mut iface_map = InterfaceMap::new();
    iface_map.insert(PROJECTED_IFACE.to_string(), props);
    iface_map
}
</file>

<file path="src/object.rs">
//! Mirror Object D-Bus Interface

use serde_json::Value;
use zbus::interface;

/// A generic D-Bus object representing a database row
pub struct MirrorObject {
    data: Value,
}

impl MirrorObject {
    pub fn new(data: Value) -> Self {
        Self { data }
    }

    pub fn update_data(&mut self, new_data: Value) -> bool {
        if self.data == new_data {
            return false;
        }
        tracing::debug!("Updating MirrorObject data");
        self.data = new_data;
        true
    }
}

#[interface(name = "org.opdbus.ProjectedObjectV1")]
impl MirrorObject {
    /// Get the full JSON representation of the row
    #[zbus(property)]
    async fn json_data(&self) -> String {
        serde_json::to_string(&self.data).unwrap_or_default()
    }

    /// Get a specific property value by key
    async fn get_property(&self, key: String) -> String {
        self.data
            .get(&key)
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Signal emitted when json_data changes
    #[zbus(signal)]
    pub async fn data_updated(
        &self,
        ctxt: &zbus::object_server::SignalContext<'_>,
    ) -> zbus::Result<()>;
}
</file>

<file path="src/plugin_interface.rs">
//! Fixed D-Bus object at /org/opdbus/v1/plugins
//!
//! Exposes all registered plugins (active or inactive) through methods.
//! Sits alongside the org.freedesktop.DBus.ObjectManager interface on the
//! same path so clients can use either GetManagedObjects or these helpers.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zbus::interface;

/// Snapshot of all plugins: name → JSON state (includes "active" bool).
pub type PluginSnapshot = Arc<RwLock<HashMap<String, String>>>;

pub struct PluginInterface {
    plugins: PluginSnapshot,
}

impl PluginInterface {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn snapshot_handle(&self) -> PluginSnapshot {
        self.plugins.clone()
    }
}

#[interface(name = "org.opdbus.PluginsV1")]
impl PluginInterface {
    /// Names of all registered plugins (active and inactive).
    async fn list(&self) -> Vec<String> {
        self.plugins.read().await.keys().cloned().collect()
    }

    /// Full state JSON for a single plugin. Returns "{}" if unknown.
    async fn get(&self, name: String) -> String {
        self.plugins
            .read()
            .await
            .get(&name)
            .cloned()
            .unwrap_or_else(|| "{}".to_string())
    }

    /// All plugins and their state as a map of name → JSON.
    async fn get_all(&self) -> HashMap<String, String> {
        self.plugins.read().await.clone()
    }
}
</file>

<file path="src/session.rs">
//! MirrorSession module for per-peer session management

use crate::event::MirrorEvent;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::broadcast;

/// Per-peer session tracking state
#[derive(Debug)]
pub struct MirrorSession {
    /// Peer's UniqueName on D-Bus
    pub peer_name: String,
    /// Set of subscribed object paths
    pub subscribed_paths: HashSet<String>,
    /// Last acknowledged sequence number per path
    pub last_acked_sequence: HashMap<String, u64>,
    /// Pending events queue (max 500 events)
    pub pending_events: Vec<MirrorEvent>,
    /// Session creation time
    pub created_at: SystemTime,
    /// Total event count for this session
    pub event_count: usize,
}

impl MirrorSession {
    /// Create a new session for a peer
    pub fn new(peer_name: String) -> Self {
        Self {
            peer_name,
            subscribed_paths: HashSet::new(),
            last_acked_sequence: HashMap::new(),
            pending_events: Vec::new(),
            created_at: SystemTime::now(),
            event_count: 0,
        }
    }

    /// Subscribe to an object path
    pub fn subscribe_path(&mut self, path: String) {
        self.subscribed_paths.insert(path);
    }

    /// Unsubscribe from an object path
    pub fn unsubscribe_path(&mut self, path: &str) {
        self.subscribed_paths.remove(path);
    }

    /// Check if session has exceeded event queue limit
    pub fn is_queue_full(&self) -> bool {
        self.pending_events.len() >= 500
    }

    /// Add an event to the pending queue
    pub fn add_event(&mut self, event: MirrorEvent) {
        if !self.is_queue_full() {
            self.pending_events.push(event);
            self.event_count += 1;
        }
    }

    /// Get and remove all pending events
    pub fn take_events(&mut self) -> Vec<MirrorEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Update last acknowledged sequence number for a path
    pub fn update_ack_sequence(&mut self, path: &str, sequence: u64) {
        self.last_acked_sequence.insert(path.to_string(), sequence);
    }
}
</file>

<file path="src/tree.rs">
//! Tree-walking and path management for D-Bus mirror

use serde_json::Value;
use std::collections::HashMap;

/// Represents a node in the D-Bus hierarchy
pub struct MirrorNode {
    pub name: String,
    pub children: HashMap<String, MirrorNode>,
    pub data: Option<Value>,
}

impl MirrorNode {
    pub fn new(name: String) -> Self {
        Self {
            name,
            children: HashMap::new(),
            data: None,
        }
    }

    /// Insert a path into the tree
    pub fn insert(&mut self, path: &str, data: Value) {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        self.insert_recursive(&parts, data);
    }

    fn insert_recursive(&mut self, parts: &[&str], data: Value) {
        if parts.is_empty() {
            self.data = Some(data);
            return;
        }

        let first = parts[0];
        let remaining = &parts[1..];

        let entry = self
            .children
            .entry(first.to_string())
            .or_insert_with(|| MirrorNode::new(first.to_string()));

        entry.insert_recursive(remaining, data);
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-dbus-mirror"
version = "1.0.0"
edition = "2021"
description = "1:1 D-Bus projection of internal databases (OVSDB, NonNet)"

[dependencies]
op-core = { path = "../op-core" }
op-state = { path = "../op-state" }
op-state-store = { path = "../op-state-store" }
op-plugins = { path = "../op-plugins" }
op-jsonrpc = { path = "../op-jsonrpc" }
op-grpc-bridge = { path = "../op-grpc-bridge" }
op-network = { path = "../op-network" }
anyhow = "1"
tokio = { version = "1", features = ["full"] }
zbus = { workspace = true }
serde = { version = "1", features = ["derive"] }
serde_json = { workspace = true }
simd-json = { workspace = true }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
futures = "0.3"
async-trait = "0.1"
dashmap = "5.0"
zbus_xml = { workspace = true }
procfs = { version = "0.17", features = ["serde1"] }
inotify = "0.10"
</file>

<file path="compare-op-dbus-mirror.md">
# compare-op-dbus-mirror

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 6 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 1 |
| Spec-listed source files | 5 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- 1:1 D-Bus projection of internal databases (OVSDB, NonNet)
- Internal crate integrations: op-core, op-jsonrpc.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/lib.rs` | ⚠️ Partial | Declared in source inventory from spec/design docs | src/lib.rs; partial artifacts: src/lib.rs.orig |
| `src/object.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/object.rs |
| `src/tree.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tree.rs |
| `src/dbus_interface.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dbus_interface.rs |
| `src/bin/verify_performance.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/bin/verify_performance.rs |
| `bin` | ✅ Present | bin group | src/bin/verify_performance.rs |
| `root` | ✅ Present | root source group | src/dbus_interface.rs, src/jsonrpc_interface.rs, src/lib.rs, src/object.rs, src/tree.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| object | ✅ Implemented | src/object.rs | SPEC main module |
| tree | ✅ Implemented | src/tree.rs | SPEC main module |
| dbus_interface | ✅ Implemented | src/dbus_interface.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-jsonrpc` - documented in SPEC

### External Runtime Dependencies
- `anyhow` - documented in SPEC
- `tokio` - documented in SPEC
- `zbus` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `sqlx` - documented in SPEC
- `tracing` - documented in SPEC
- `tracing-subscriber` - documented in SPEC
- `futures` - documented in SPEC
- `async-trait` - documented in SPEC
- `dashmap` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Transitional or partial artifacts detected: src/lib.rs.orig.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: dbus_interface, jsonrpc_interface, object, tree.
</file>

<file path="SPEC.md">
# op-dbus-mirror - Specification

## Overview
**Crate**: `op-dbus-mirror`  
**Location**: `crates/op-dbus-mirror`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-dbus-mirror"
version = "1.0.0"
edition = "2021"
description = "1:1 D-Bus projection of internal databases (OVSDB, NonNet)"
```

### Source Structure
```
op-dbus-mirror/src/lib.rs
op-dbus-mirror/src/object.rs
op-dbus-mirror/src/tree.rs
op-dbus-mirror/src/dbus_interface.rs
op-dbus-mirror/src/bin/verify_performance.rs
```

### Key Dependencies
```toml
op-core = { path = "../op-core" }
op-jsonrpc = { path = "../op-jsonrpc" }
anyhow = "1"
tokio = { version = "1", features = ["full"] }
zbus = { version = "4.0", features = ["tokio"] }
serde = { version = "1", features = ["derive"] }
simd-json = { version = "0.13", features = ["serde"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "json"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
futures = "0.3"
async-trait = "0.1"
dashmap = "5.0"
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       5 Rust source files

### Main Modules
object
tree
dbus_interface

## Purpose
1:1 D-Bus projection of internal databases (OVSDB, NonNet)

## Build Information
- **Edition**: 2021
- **Version**: 1.0.0
- **License**: 

## Related Crates
Internal dependencies:
- op-core
- op-jsonrpc

---
*Generated from crate analysis*
</file>

</files>
