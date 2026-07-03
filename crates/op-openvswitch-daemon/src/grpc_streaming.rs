//! gRPC Streaming Service for OVSDB and OpenFlow updates
//!
//! Implements server-streaming subscriptions that act as mutate triggers
#![allow(dead_code, clippy::result_large_err)]
//! into the SchemaEngine. Uses `tokio::sync::broadcast` for internal event
//! distribution, allowing multiple subscribers to receive updates.
//!
//! Per AGENTS.md §4: D-Bus = authority/registration; gRPC = typed transport + subscriptions

use anyhow::Result;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::dbus::DaemonState;

// Generated protobuf types
pub mod proto {
    tonic::include_proto!("ovsdaemon.v1");
}

use proto::ovsdb_stream_service_server::{OvsdbStreamService, OvsdbStreamServiceServer};
use proto::{FlowUpdate, OvsdbUpdate, SubscribeRequest, TopologyEvent};

/// Size of the broadcast channel buffer for events
const EVENT_BUFFER_SIZE: usize = 1024;

/// Global event bus for OVSDB/OpenFlow updates
#[derive(Clone)]
pub struct EventBus {
    /// OVSDB update events
    pub ovsdb_tx: broadcast::Sender<OvsdbUpdate>,
    /// OpenFlow flow events
    pub flow_tx: broadcast::Sender<FlowUpdate>,
    /// Topology change events
    pub topology_tx: broadcast::Sender<TopologyEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let (ovsdb_tx, _) = broadcast::channel(EVENT_BUFFER_SIZE);
        let (flow_tx, _) = broadcast::channel(EVENT_BUFFER_SIZE);
        let (topology_tx, _) = broadcast::channel(EVENT_BUFFER_SIZE);

        Self {
            ovsdb_tx,
            flow_tx,
            topology_tx,
        }
    }

    /// Publish an OVSDB update to all subscribers
    #[allow(dead_code)]
    pub fn publish_ovsdb(&self, update: OvsdbUpdate) {
        let _ = self.ovsdb_tx.send(update);
    }

    /// Publish a flow update to all subscribers
    #[allow(dead_code)]
    pub fn publish_flow(&self, update: FlowUpdate) {
        let _ = self.flow_tx.send(update);
    }

    /// Publish a topology event to all subscribers
    #[allow(dead_code)]
    pub fn publish_topology(&self, event: TopologyEvent) {
        let _ = self.topology_tx.send(event);
    }
}

/// gRPC streaming service implementation
pub struct StreamingService {
    state: DaemonState,
    event_bus: EventBus,
    active_subscriptions: Arc<RwLock<u64>>,
}

impl StreamingService {
    pub fn new(state: DaemonState, event_bus: EventBus) -> Self {
        Self {
            state,
            event_bus,
            active_subscriptions: Arc::new(RwLock::new(0)),
        }
    }

    pub fn into_server(self) -> OvsdbStreamServiceServer<Self> {
        OvsdbStreamServiceServer::new(self)
    }

    /// Increment subscription count
    async fn inc_subs(&self) {
        let mut subs = self.active_subscriptions.write().await;
        *subs += 1;
    }

    /// Decrement subscription count
    #[allow(dead_code)]
    async fn dec_subs(&self) {
        let mut subs = self.active_subscriptions.write().await;
        *subs -= 1;
    }
}

#[tonic::async_trait]
impl OvsdbStreamService for StreamingService {
    type SubscribeOvsdbUpdatesStream =
        Pin<Box<dyn Stream<Item = Result<OvsdbUpdate, Status>> + Send>>;

    async fn subscribe_ovsdb_updates(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeOvsdbUpdatesStream>, Status> {
        let req = request.into_inner();
        info!(
            "gRPC SubscribeOvsdbUpdates: database={}, tables={:?}",
            req.database, req.tables
        );

        self.inc_subs().await;

        // Subscribe to the broadcast channel
        let rx = self.event_bus.ovsdb_tx.subscribe();

        // Create a stream that filters by database/tables
        let database = req.database.clone();
        let tables: std::collections::HashSet<String> = req.tables.iter().cloned().collect();
        let filter_tables = !tables.is_empty();

        let stream = BroadcastStream::new(rx).filter_map(move |result| {
            match result {
                Ok(update) => {
                    // Filter by database if specified
                    if !database.is_empty() && update.database != database {
                        return None;
                    }
                    // Filter by table if specified
                    if filter_tables && !tables.contains(&update.table) {
                        return None;
                    }
                    Some(Ok(update))
                }
                Err(e) => {
                    warn!("Broadcast lagged: {}", e);
                    None
                }
            }
        });

        let service = self.clone();
        #[allow(clippy::result_large_err)]
        let stream = stream.map(move |item| {
            // This is a bit of a hack - we need to keep the service alive
            // In practice, we'd use a proper shutdown signal
            let _ = &service;
            item
        });

        Ok(Response::new(Box::pin(stream)))
    }

    type SubscribeFlowUpdatesStream =
        Pin<Box<dyn Stream<Item = Result<FlowUpdate, Status>> + Send>>;

    async fn subscribe_flow_updates(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeFlowUpdatesStream>, Status> {
        let req = request.into_inner();
        info!("gRPC SubscribeFlowUpdates: connection_id={}", req.database);

        self.inc_subs().await;

        let rx = self.event_bus.flow_tx.subscribe();
        let connection_filter = req.database.clone();

        let stream = BroadcastStream::new(rx).filter_map(move |result| match result {
            Ok(update) => {
                if !connection_filter.is_empty() && update.connection_id != connection_filter {
                    return None;
                }
                Some(Ok(update))
            }
            Err(e) => {
                warn!("Flow broadcast lagged: {}", e);
                None
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }

    type SubscribeTopologyStream =
        Pin<Box<dyn Stream<Item = Result<TopologyEvent, Status>> + Send>>;

    async fn subscribe_topology(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeTopologyStream>, Status> {
        let req = request.into_inner();
        info!("gRPC SubscribeTopology: filter={}", req.filter);

        self.inc_subs().await;

        let rx = self.event_bus.topology_tx.subscribe();

        let stream = BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(event) => Some(Ok(event)),
            Err(e) => {
                warn!("Topology broadcast lagged: {}", e);
                None
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }
}

impl Clone for StreamingService {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            event_bus: self.event_bus.clone(),
            active_subscriptions: self.active_subscriptions.clone(),
        }
    }
}

/// Bridge between D-Bus signals and gRPC streaming events
///
/// This module provides functions to convert OVSDB monitor events
/// and OpenFlow messages into gRPC stream events that trigger
/// SchemaEngine mutations.
pub mod bridge {
    use super::*;
    use chrono::Utc;

    /// Convert OVSDB monitor JSON into OvsdbUpdate protobuf
    #[allow(dead_code)]
    pub fn ovsdb_json_to_update(
        database: &str,
        table: &str,
        json: &serde_json::Value,
    ) -> OvsdbUpdate {
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);

        // Determine update type from JSON structure
        let update_type = if json.get("insert").is_some() {
            proto::ovsdb_update::UpdateType::Insert as i32
        } else if json.get("delete").is_some() {
            proto::ovsdb_update::UpdateType::Delete as i32
        } else {
            proto::ovsdb_update::UpdateType::Modify as i32
        };

        let row_uuid = json
            .get("uuid")
            .and_then(|u| u.as_array())
            .and_then(|a| a.get(1))
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        OvsdbUpdate {
            timestamp,
            database: database.to_string(),
            table: table.to_string(),
            row_uuid,
            update_type,
            row_json: json.to_string(),
            previous_row_json: String::new(),
        }
    }

    /// Create a topology event for bridge changes
    #[allow(dead_code)]
    pub fn topology_bridge_event(
        event_type: proto::topology_event::EventType,
        bridge_name: &str,
    ) -> TopologyEvent {
        TopologyEvent {
            timestamp: Utc::now().timestamp_nanos_opt().unwrap_or(0),
            event_type: event_type as i32,
            object_id: bridge_name.to_string(),
            parent_id: String::new(),
            details_json: serde_json::json!({"bridge": bridge_name}).to_string(),
        }
    }

    /// Create a topology event for port changes
    #[allow(dead_code)]
    pub fn topology_port_event(
        event_type: proto::topology_event::EventType,
        port_name: &str,
        bridge_name: &str,
    ) -> TopologyEvent {
        TopologyEvent {
            timestamp: Utc::now().timestamp_nanos_opt().unwrap_or(0),
            event_type: event_type as i32,
            object_id: port_name.to_string(),
            parent_id: bridge_name.to_string(),
            details_json: serde_json::json!({
                "port": port_name,
                "bridge": bridge_name
            })
            .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_creation() {
        let bus = EventBus::new();
        let mut rx = bus.ovsdb_tx.subscribe();

        let update = OvsdbUpdate {
            timestamp: 12345,
            database: "Open_vSwitch".to_string(),
            table: "Bridge".to_string(),
            row_uuid: "test-uuid".to_string(),
            update_type: proto::ovsdb_update::UpdateType::Insert as i32,
            row_json: r#"{"name":"br0"}"#.to_string(),
            previous_row_json: String::new(),
        };

        bus.publish_ovsdb(update.clone());

        // Should receive the update
        let received = rx.try_recv();
        assert!(received.is_ok());
        let received = received.unwrap();
        assert_eq!(received.row_uuid, "test-uuid");
    }

    #[test]
    fn test_ovsdb_json_to_update() {
        let json = serde_json::json!({
            "uuid": ["uuid", "test-uuid-123"],
            "name": "br0",
            "insert": true
        });

        let update = bridge::ovsdb_json_to_update("Open_vSwitch", "Bridge", &json);

        assert_eq!(update.database, "Open_vSwitch");
        assert_eq!(update.table, "Bridge");
        assert_eq!(update.row_uuid, "test-uuid-123");
        assert_eq!(
            update.update_type,
            proto::ovsdb_update::UpdateType::Insert as i32
        );
    }
}
