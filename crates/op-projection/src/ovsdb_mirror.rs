//! OVSDB Mirror: Specialized projection for Open vSwitch.
//!
//! This module implements a dedicated projection for OVSDB tables,
//! allowing 1:1 mirroring of OVSDB state into the projection system.

use crate::data_models::*;
use crate::interfaces::{OvsdbMirrorProjection, ProjectionEngine, RawEntity};
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{debug, warn};

/// Specialized projection for Open vSwitch OVSDB mirroring.
#[derive(Debug, Clone)]
pub struct OvsdbMirrorProjectionImpl {
    /// Reference to the projection engine
    engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>,
}

impl OvsdbMirrorProjectionImpl {
    /// Creates a new OvsdbMirrorProjection
    pub fn new(engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>) -> Self {
        Self { engine }
    }
}

impl OvsdbMirrorProjection for OvsdbMirrorProjectionImpl {
    fn mirror_table(&mut self, table_name: &str, entities: Vec<RawEntity>) -> Result<()> {
        debug!(
            table_name = table_name,
            count = entities.len(),
            "Mirroring OVSDB table"
        );

        let mut engine = self.engine.lock();
        for entity in entities {
            engine.create_projection(entity)?;
        }

        Ok(())
    }

    fn update_entry(&mut self, entity: RawEntity) -> Result<Projection> {
        let mut engine = self.engine.lock();
        engine.create_projection(entity)
    }

    fn handle_disconnect(&mut self) {
        warn!("OVSDB disconnected; degrading projections");
        let mut engine = self.engine.lock();
        for p in engine.get_all_projections() {
            if p.entity_type.starts_with("ovsdb.") {
                engine.degrade_projection(&p.id, "OVSDB connection lost", Vec::new());
            }
        }
    }
}
