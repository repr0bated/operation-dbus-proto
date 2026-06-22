//! EventDispatcher module for unified event dispatch

use anyhow::Result;
use op_network::rovs_proxy::OvsdbDbusClient;
use op_state::manager::StateManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::event::MirrorEvent;
use crate::event_sources::component_registry;
use crate::event_sources::ovsdb;
use crate::event_sources::procfs;
use crate::event_sources::state_manager;
use crate::DbusMirror;

/// Event dispatcher that wires all event sources to the broadcast channel
pub struct EventDispatcher {
    pub broadcast_tx: broadcast::Sender<MirrorEvent>,
    mirror: Arc<DbusMirror>,
    ovsdb_client: Arc<OvsdbDbusClient>,
    state_manager: Option<Arc<StateManager>>,
    grpc_server: Option<Arc<op_grpc_bridge::OperationGrpcServer>>,
    /// Sequence numbers per object path
    sequence_numbers: Arc<std::sync::Mutex<HashMap<String, u64>>>,
}

impl EventDispatcher {
    /// Create a new EventDispatcher
    pub fn new(
        mirror: Arc<DbusMirror>,
        ovsdb_client: Arc<OvsdbDbusClient>,
        state_manager: Option<Arc<StateManager>>,
        grpc_server: Option<Arc<op_grpc_bridge::OperationGrpcServer>>,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        Self {
            broadcast_tx,
            mirror,
            ovsdb_client,
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
