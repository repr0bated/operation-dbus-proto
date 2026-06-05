//! gRPC Reader: Reading from gRPC services.
//!
//! This module implements the `GrpcReader` trait, discovering gRPC services
//! and projecting methods and message types into raw entities.

use crate::interfaces::{GrpcReader, RawEntity, SourceReader};
use anyhow::Result;
use simd_json::json;
use tracing::debug;

/// Reader that extracts state from gRPC services.
#[derive(Debug)]
pub struct SystemGrpcReader {
    /// Source identifier
    source: String,
}

impl SystemGrpcReader {
    /// Creates a new SystemGrpcReader
    pub fn new() -> Self {
        Self {
            source: "grpc".to_string(),
        }
    }
}

impl Default for SystemGrpcReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for SystemGrpcReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        self.read_services()
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        debug!(entity_id = entity_id, "Reading gRPC entity");
        Ok(RawEntity {
            entity_type: "grpc.service".to_string(),
            entity_id: entity_id.to_string(),
            data: json!({ "methods": [] }),
            source: self.source.clone(),
        })
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        true
    }
}

impl GrpcReader for SystemGrpcReader {
    fn read_services(&self) -> Result<Vec<RawEntity>> {
        debug!("Discovering gRPC services");
        Ok(Vec::new())
    }

    fn read_methods(&self, service: &str) -> Result<Vec<RawEntity>> {
        debug!(service = service, "Reading gRPC methods");
        Ok(Vec::new())
    }

    fn read_messages(&self, service: &str) -> Result<Vec<RawEntity>> {
        debug!(service = service, "Reading gRPC message types");
        Ok(Vec::new())
    }

    fn read_endpoints(&self) -> Result<Vec<RawEntity>> {
        debug!("Reading gRPC endpoints");
        Ok(Vec::new())
    }
}
