//! Block events and plugin footprints for the streaming blockchain

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// A block event in the streaming blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub timestamp: u64,
    pub category: String,
    pub action: String,
    pub data: simd_json::OwnedValue,
    pub hash: String,
    pub vector: Vec<f32>,
}

impl BlockEvent {
    /// Create a new block event
    pub fn new(
        category: impl Into<String>,
        action: impl Into<String>,
        data: simd_json::OwnedValue,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        let category = category.into();
        let action = action.into();

        // Compute hash
        let hash_input = format!("{}:{}:{}:{}", timestamp, category, action, data);
        let mut hasher = Sha256::new();
        hasher.update(hash_input.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Self {
            timestamp,
            category,
            action,
            data,
            hash,
            vector: Vec::new(), // Empty vector, can be populated by ML
        }
    }

    /// Create with a pre-computed vector
    pub fn with_vector(mut self, vector: Vec<f32>) -> Self {
        self.vector = vector;
        self
    }
}

/// A plugin footprint representing a tracked operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFootprint {
    pub plugin_id: String,
    pub operation: String,
    pub timestamp: u64,
    pub data_hash: String,
    pub content_hash: String,
    pub metadata: HashMap<String, simd_json::OwnedValue>,
    pub vector_features: Vec<f32>,
}

impl PluginFootprint {
    /// Create a new plugin footprint
    pub fn new(
        plugin_id: impl Into<String>,
        operation: impl Into<String>,
        data: &simd_json::OwnedValue,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        // Hash the data
        let data_str = simd_json::to_string(data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(data_str.as_bytes());
        let data_hash = format!("{:x}", hasher.finalize());

        Self {
            plugin_id: plugin_id.into(),
            operation: operation.into(),
            timestamp,
            data_hash: data_hash.clone(),
            content_hash: data_hash,
            metadata: HashMap::new(),
            vector_features: Vec::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: simd_json::OwnedValue) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Convert to a BlockEvent
    pub fn to_block_event(&self) -> BlockEvent {
        let data = simd_json::json!({
            "plugin_id": self.plugin_id,
            "operation": self.operation,
            "data_hash": self.data_hash,
            "metadata": self.metadata
        });

        BlockEvent {
            timestamp: self.timestamp,
            category: self.plugin_id.clone(),
            action: self.operation.clone(),
            data,
            hash: self.data_hash.clone(),
            vector: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_event_creation() {
        let event = BlockEvent::new("test", "create", simd_json::json!({"key": "value"}));

        assert!(!event.hash.is_empty());
        assert_eq!(event.category, "test");
        assert_eq!(event.action, "create");
    }

    #[test]
    fn test_plugin_footprint_creation() {
        let footprint = PluginFootprint::new(
            "systemd",
            "unit_started",
            &simd_json::json!({"unit": "nginx.service"}),
        );

        assert_eq!(footprint.plugin_id, "systemd");
        assert!(!footprint.data_hash.is_empty());
    }
}
