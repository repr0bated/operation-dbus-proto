//! Sled Reader: Zero-copy reads from The Sled (/dev/shm).
//!
//! This module implements a reader for the `IdentitySled` struct
//! located in shared memory, following the 3tched Architecture principles.

use crate::interfaces::{RawEntity, SourceReader};
use anyhow::Result;
use op_identity::{read_sled, IdentitySled};
use simd_json::json;
use tracing::{debug, warn};

/// Reader that extracts state from the Identity Sled in shared memory.
#[derive(Debug, Clone)]
pub struct IdentitySledReader {
    /// Source identifier
    source: String,
}

impl IdentitySledReader {
    /// Creates a new IdentitySledReader
    pub fn new() -> Self {
        Self {
            source: "identity-sled".to_string(),
        }
    }
}

impl Default for IdentitySledReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for IdentitySledReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        let mut entities = Vec::new();
        
        match self.read_sled_entity() {
            Ok(entity) => entities.push(entity),
            Err(e) => debug!("Sled not available: {}", e),
        }
        
        Ok(entities)
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        if entity_id == "current" {
            self.read_sled_entity()
        } else {
            Err(anyhow::anyhow!("Unknown sled entity: {}", entity_id))
        }
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        std::path::Path::new(op_identity::SHM_SLED_PATH).exists()
    }
}

impl IdentitySledReader {
    fn read_sled_entity(&self) -> Result<RawEntity> {
        let (ptr, _mmap) = read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
        let sled = unsafe { &*ptr };
        
        let footprint = hex::encode(sled.hashed_footprint);
        let pubkey = hex::encode(sled.wireguard_pubkey);
        
        Ok(RawEntity {
            entity_type: "identity.sled".to_string(),
            entity_id: "current".to_string(),
            data: json!({
                "mutation_index": sled.mutation_index,
                "hashed_footprint": footprint,
                "wireguard_pubkey": pubkey,
            }).into(),
            source: self.source.clone(),
        })
    }
}
