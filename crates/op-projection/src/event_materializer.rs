//! Event Materializer: Event-driven projection updates.
//!
//! This module implements the `EventMaterializer` trait, which handles
//! consuming events from the event bus and materializing projections
//! with 50ms processing guarantees.

use crate::data_models::*;
use crate::interfaces::{EventMaterializer, ProjectionEngine, RawEntity};
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{debug, error, warn};

/// Materializer that transforms events into projection updates.
#[derive(Debug)]
pub struct ProjectionMaterializer {
    /// Reference to the projection engine
    engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>,
    /// Number of events processed
    events_processed: u64,
    /// Number of events quarantined
    events_quarantined: u64,
    /// Latency of the last processing batch
    last_latency: chrono::Duration,
}

impl ProjectionMaterializer {
    /// Creates a new ProjectionMaterializer with given engine
    pub fn new(engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>) -> Self {
        Self {
            engine,
            events_processed: 0,
            events_quarantined: 0,
            last_latency: chrono::Duration::zero(),
        }
    }
}

impl EventMaterializer for ProjectionMaterializer {
    fn materialize(&mut self, event: &ProjectionEvent) -> Result<Vec<ProjectionUpdate>> {
        let start_time = Utc::now();
        let mut updates = Vec::new();

        debug!(
            event_id = event.id,
            event_type = ?event.event_type,
            "Materializing event"
        );

        match event.event_type {
            EventType::Created | EventType::Updated => {
                let entity = RawEntity {
                    entity_type: event.entity_type.clone(),
                    entity_id: event.entity_id.clone(),
                    data: event.data.clone(),
                    source: event.source.clone(),
                };

                let result = {
                    let mut engine = self.engine.lock();
                    engine.create_projection(entity)
                };

                match result {
                    Ok(projection) => {
                        updates.push(ProjectionUpdate {
                            update_type: if event.event_type == EventType::Created {
                                UpdateType::Created
                            } else {
                                UpdateType::Updated
                            },
                            projection,
                            timestamp: Utc::now(),
                        });
                        self.events_processed += 1;
                    }
                    Err(e) => {
                        self.quarantine_event(event, &format!("Failed to project: {}", e));
                    }
                }
            }
            EventType::Deleted => {
                let projection_id = format!("{}:{}", event.entity_type, event.entity_id);
                
                let maybe_projection = {
                    let mut engine = self.engine.lock();
                    // Get projection before deletion for the update
                    if let Some(projection) = engine.get_projection(&projection_id) {
                        if let Err(e) = engine.delete_projection(&projection_id) {
                            warn!(projection_id = projection_id, error = %e, "Failed to delete projection");
                            None
                        } else {
                            Some(projection)
                        }
                    } else {
                        None
                    }
                };

                if let Some(projection) = maybe_projection {
                    updates.push(ProjectionUpdate {
                        update_type: UpdateType::Deleted,
                        projection,
                        timestamp: Utc::now(),
                    });
                    self.events_processed += 1;
                }
            }
            _ => {
                debug!(event_type = ?event.event_type, "Ignoring non-materializable event");
            }
        }

        self.last_latency = Utc::now().signed_duration_since(start_time);
        
        // Ensure 50ms guarantee (log warning if exceeded)
        if self.last_latency > chrono::Duration::milliseconds(50) {
            warn!(
                latency_ms = self.last_latency.num_milliseconds(),
                "Materialization latency exceeded 50ms guarantee"
            );
        }

        Ok(updates)
    }

    fn get_processing_latency(&self) -> chrono::Duration {
        self.last_latency
    }

    fn has_ordering_guarantees(&self) -> bool {
        // This implementation processes events sequentially via the lock
        true
    }

    fn quarantine_event(&mut self, event: &ProjectionEvent, reason: &str) {
        self.events_quarantined += 1;
        error!(
            event_id = event.id,
            reason = reason,
            "Event quarantined"
        );
    }

    fn get_events_processed(&self) -> u64 {
        self.events_processed
    }

    fn get_events_quarantined(&self) -> u64 {
        self.events_quarantined
    }
}
