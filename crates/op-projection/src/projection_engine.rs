//! Projection Engine: Authoritative state transformation engine.
//!
//! This module implements the `ProjectionEngine` trait, providing the core
//! logic for transforming authoritative state into schema-validated projections.

use crate::data_models::*;
use crate::interfaces::{ProjectionEngine, RawEntity};
use crate::projection_store::ProjectionStore;
use crate::schema_engine::SchemaValidator;
use anyhow::Result;
use chrono::Utc;
use tracing::{debug, info, warn};

/// The core engine for state transformation.
///
/// This engine manages the lifecycle of projections, ensuring all data is
/// validated against the authoritative schema registry.
#[derive(Debug, Clone)]
pub struct ProjectionSystemEngine {
    /// In-memory projection store
    store: ProjectionStore,
    /// Schema validator for data validation
    validator: SchemaValidator,
}

impl ProjectionSystemEngine {
    /// Creates a new ProjectionSystemEngine with given store and validator
    pub fn new(store: ProjectionStore, validator: SchemaValidator) -> Self {
        Self { store, validator }
    }

    /// Internal helper to create a projection from raw entity
    fn build_projection(&self, entity: RawEntity) -> Projection {
        let timestamp = Utc::now();
        let entity_type = entity.entity_type.clone();
        
        // Validate entity against schema
        let validation_result = self.validator.validate_entity(&entity);

        // Default projection
        let mut projection = Projection {
            id: format!("{}:{}", entity.entity_type, entity.entity_id),
            entity_type: entity.entity_type,
            entity_id: entity.entity_id,
            state: ProjectionState::Valid,
            schema_version: "0.0.0".to_string(),
            data: entity.data,
            validation_errors: Vec::new(),
            quarantine_reason: None,
            degradation_reason: None,
            affected_dependencies: Vec::new(),
            created_at: timestamp,
            updated_at: timestamp,
        };

        match validation_result {
            Ok(result) => {
                projection.validation_errors = result.errors;
                if !result.valid {
                    projection.state = ProjectionState::Quarantined;
                    projection.quarantine_reason = Some("Schema validation failed".to_string());
                }
                
                // Set schema version from registry
                if let Some(schema) = self.validator.get_schema_for_entity(&entity_type) {
                    projection.schema_version = schema.version.clone();
                }
            }
            Err(e) => {
                projection.state = ProjectionState::Quarantined;
                projection.quarantine_reason = Some(format!("Validation error: {}", e));
                warn!(
                    entity_type = entity_type,
                    entity_id = projection.entity_id,
                    error = %e,
                    "Failed to validate entity"
                );
            }
        }

        projection
    }
}

impl ProjectionEngine for ProjectionSystemEngine {
    fn create_projection(&mut self, entity: RawEntity) -> Result<Projection> {
        let projection = self.build_projection(entity);
        let projection_clone = projection.clone();
        self.store.upsert(projection);
        
        debug!(
            id = projection_clone.id,
            state = ?projection_clone.state,
            "Projection created"
        );
        
        Ok(projection_clone)
    }

    fn update_projection(&mut self, _projection_id: &str, entity: RawEntity) -> Result<Projection> {
        // In this implementation, ID is derived from entity_type and entity_id
        // So update is same as create (upsert)
        let projection = self.build_projection(entity);
        let projection_clone = projection.clone();
        self.store.upsert(projection);
        
        debug!(
            id = projection_clone.id,
            state = ?projection_clone.state,
            "Projection updated"
        );
        
        Ok(projection_clone)
    }

    fn get_projection(&self, projection_id: &str) -> Option<Projection> {
        self.store.get(projection_id)
    }

    fn get_projections_by_type(&self, entity_type: &str) -> Vec<Projection> {
        self.store.get_by_type(entity_type)
    }

    fn get_projections_by_state(&self, state: ProjectionState) -> Vec<Projection> {
        self.store.get_by_state(state)
    }

    fn quarantine_projection(&mut self, projection_id: &str, reason: &str) {
        if let Some(mut projection) = self.store.get(projection_id) {
            projection.state = ProjectionState::Quarantined;
            projection.quarantine_reason = Some(reason.to_string());
            projection.updated_at = Utc::now();
            self.store.upsert(projection);
            
            info!(
                projection_id = projection_id,
                reason = reason,
                "Projection quarantined"
            );
        }
    }

    fn degrade_projection(
        &mut self,
        projection_id: &str,
        reason: &str,
        affected_dependencies: Vec<String>,
    ) {
        if let Some(mut projection) = self.store.get(projection_id) {
            projection.state = ProjectionState::Degraded;
            projection.degradation_reason = Some(reason.to_string());
            projection.affected_dependencies = affected_dependencies;
            projection.updated_at = Utc::now();
            self.store.upsert(projection);
            
            info!(
                projection_id = projection_id,
                reason = reason,
                "Projection degraded"
            );
        }
    }

    fn revalidate_projections(&mut self, schema_name: &str, _old_version: &str) {
        let projections = self.store.get_by_type(schema_name);
        for mut projection in projections {
            // Re-validate against current schema
            let entity = RawEntity {
                entity_type: projection.entity_type.clone(),
                entity_id: projection.entity_id.clone(),
                data: projection.data.clone(),
                source: "revalidation".to_string(),
            };
            
            let updated = self.build_projection(entity);
            projection.state = updated.state;
            projection.validation_errors = updated.validation_errors;
            projection.quarantine_reason = updated.quarantine_reason;
            projection.schema_version = updated.schema_version;
            projection.updated_at = Utc::now();
            
            self.store.upsert(projection);
        }
    }

    fn get_all_projections(&self) -> Vec<Projection> {
        self.store.get_all()
    }

    fn delete_projection(&mut self, projection_id: &str) -> Result<()> {
        self.store.delete(projection_id)
    }

    fn get_projections_by_source(&self, source: &str) -> Vec<Projection> {
        // Filter projections by source if we had source field,
        // for now just return empty or placeholder filter
        self.store
            .get_all()
            .into_iter()
            .filter(|p| p.id.contains(source))
            .collect()
    }
}
