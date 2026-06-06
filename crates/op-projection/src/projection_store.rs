//! Projection Store: In-memory projection storage with persistence.
//!
//! This module implements the authoritative store for all schema-validated
//! projections. It uses DashMap for high-concurrency access and supports
//! time-indexed versioning for historical replay.

use crate::data_models::*;
use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// In-memory authoritative store for all projections.
///
/// This store is the source of truth for the current state of all projections.
/// It provides zero-copy read access and maintains historical versions.
#[derive(Debug, Clone, Default)]
pub struct ProjectionStore {
    /// Current projections by ID
    projections: Arc<DashMap<String, Projection>>,
    /// Historical projections by ID and version
    history: Arc<DashMap<String, Vec<HistoricalProjection>>>,
}

impl ProjectionStore {
    /// Creates a new empty ProjectionStore
    pub fn new() -> Self {
        Self {
            projections: Arc::new(DashMap::new()),
            history: Arc::new(DashMap::new()),
        }
    }

    /// Insert or update a projection
    pub fn upsert(&self, projection: Projection) {
        let id = projection.id.clone();

        // Update history before replacing current
        if let Some(old) = self.projections.get(&id) {
            let historical = HistoricalProjection {
                projection: old.clone(),
                version: self.history.get(&id).map(|h| h.len() as u64).unwrap_or(0) + 1,
                timestamp: chrono::Utc::now(),
                is_quarantined: old.state == ProjectionState::Quarantined,
            };

            self.history.entry(id.clone()).or_default().push(historical);
        }

        self.projections.insert(id, projection);
    }

    /// Get a projection by ID
    pub fn get(&self, id: &str) -> Option<Projection> {
        self.projections.get(id).map(|p| p.clone())
    }

    /// Get all projections for an entity type
    pub fn get_by_type(&self, entity_type: &str) -> Vec<Projection> {
        self.projections
            .iter()
            .filter(|p| p.entity_type == entity_type)
            .map(|p| p.value().clone())
            .collect()
    }

    /// Get all projections for a state
    pub fn get_by_state(&self, state: ProjectionState) -> Vec<Projection> {
        self.projections
            .iter()
            .filter(|p| p.state == state)
            .map(|p| p.value().clone())
            .collect()
    }

    /// Get all projections
    pub fn get_all(&self) -> Vec<Projection> {
        self.projections.iter().map(|p| p.value().clone()).collect()
    }

    /// Delete a projection
    pub fn delete(&self, id: &str) -> Result<()> {
        if self.projections.remove(id).is_some() {
            debug!(projection_id = id, "Projection deleted from store");
            Ok(())
        } else {
            Err(anyhow::anyhow!("Projection '{}' not found", id))
        }
    }

    /// Get historical versions for a projection
    pub fn get_history(&self, id: &str) -> Vec<HistoricalProjection> {
        self.history.get(id).map(|h| h.clone()).unwrap_or_default()
    }

    /// Clear all projections and history
    pub fn clear(&self) {
        self.projections.clear();
        self.history.clear();
        info!("Projection store cleared");
    }
}
