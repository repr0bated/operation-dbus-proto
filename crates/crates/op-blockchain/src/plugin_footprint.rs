//! Plugin footprint mechanism with hash for blockchain vectorization

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use simd_json::prelude::*;
use std::collections::HashMap;

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
    #[allow(dead_code)]
    pub fn new(plugin_id: String, operation: String, metadata: simd_json::OwnedValue) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let data_str = simd_json::to_string(&metadata).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(data_str.as_bytes());
        let data_hash = format!("{:x}", hasher.finalize());

        let content = format!("{}:{}:{}", plugin_id, operation, timestamp);
        let mut content_hasher = Sha256::new();
        content_hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", content_hasher.finalize());

        let mut metadata_map = HashMap::new();
        if let simd_json::OwnedValue::Object(obj) = metadata {
            for (k, v) in obj.iter() {
                metadata_map.insert(k.clone(), v.clone());
            }
        }

        Self {
            plugin_id,
            operation,
            timestamp,
            data_hash,
            content_hash,
            metadata: metadata_map,
            vector_features: vec![0.0; 64], // Default 64-dimensional vector
        }
    }
}

pub struct FootprintGenerator {
    plugin_id: String,
}

impl FootprintGenerator {
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
        }
    }

    /// Create footprint upon plugin operation
    pub fn create_footprint(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<PluginFootprint> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| anyhow::anyhow!("System clock error: {}", e))?
            .as_secs();

        // Hash the data content
        let data_str =
            simd_json::to_string(data).context("Failed to serialize data for hashing")?;
        let data_hash = format!("{:x}", Sha256::digest(data_str.as_bytes()));

        // Hash the entire operation context
        let context = format!(
            "{}:{}:{}:{}",
            self.plugin_id, operation, timestamp, data_hash
        );
        let content_hash = format!("{:x}", Sha256::digest(context.as_bytes()));

        // Generate vector features for blockchain
        let vector_features = self.generate_vector_features(operation, data, &metadata)?;

        Ok(PluginFootprint {
            plugin_id: self.plugin_id.clone(),
            operation: operation.to_string(),
            timestamp,
            data_hash,
            content_hash,
            metadata: metadata.unwrap_or_default(),
            vector_features,
        })
    }

    /// Generate vector features for blockchain vectorization
    fn generate_vector_features(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: &Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<Vec<f32>> {
        // Check vectorization level
        #[cfg(feature = "ml")]
        {
            let model_manager = crate::ml::ModelManager::global();

            if model_manager.is_enabled() {
                // Use transformer-based vectorization
                return self.generate_transformer_features(operation, data, metadata);
            }
        }

        // Fall back to heuristic vectorization
        self.generate_heuristic_features(operation, data, metadata)
    }

    /// Generate transformer-based vector features
    #[cfg(feature = "ml")]
    fn generate_transformer_features(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: &Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<Vec<f32>> {
        // Prepare text for embedding
        let text = self.prepare_text_for_embedding(operation, data, metadata);

        // Get model manager and embed
        let model_manager = crate::ml::ModelManager::global();

        match model_manager.embed(&text) {
            Ok(vector) => Ok(vector),
            Err(e) => {
                log::warn!(
                    "Transformer embedding failed: {}, falling back to heuristic",
                    e
                );
                // Fall back to heuristic if transformer fails
                self.generate_heuristic_features(operation, data, metadata)
            }
        }
    }

    /// Prepare text from footprint data for embedding
    #[cfg(feature = "ml")]
    fn prepare_text_for_embedding(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: &Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> String {
        let mut parts = Vec::new();

        // Add plugin and operation context
        parts.push(format!("plugin: {}", self.plugin_id));
        parts.push(format!("operation: {}", operation));

        // Add data summary
        match data {
            simd_json::OwnedValue::Object(obj) => {
                for (key, value) in obj {
                    let value_str = match value {
                        simd_json::OwnedValue::String(s) => s.clone(),
                        _ => value.to_string(),
                    };
                    parts.push(format!("{}: {}", key, value_str));
                }
            }
            simd_json::OwnedValue::String(s) => {
                parts.push(format!("data: {}", s));
            }
            _ => {
                parts.push(format!("data: {}", data));
            }
        }

        // Add metadata
        if let Some(meta) = metadata {
            for (key, value) in meta {
                let value_str = match value {
                    simd_json::OwnedValue::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                parts.push(format!("meta.{}: {}", key, value_str));
            }
        }

        // Join all parts with newlines
        parts.join("\n")
    }

    /// Generate heuristic-based features (original method, now extracted)
    fn generate_heuristic_features(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: &Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<Vec<f32>> {
        let mut features = Vec::with_capacity(64);

        // Plugin ID hash feature
        features.push(self.hash_string(&self.plugin_id) as f32 / u32::MAX as f32);

        // Operation type features
        let op_features = match operation {
            "create" => vec![1.0, 0.0, 0.0, 0.0],
            "update" => vec![0.0, 1.0, 0.0, 0.0],
            "delete" => vec![0.0, 0.0, 1.0, 0.0],
            "query" => vec![0.0, 0.0, 0.0, 1.0],
            _ => vec![0.5, 0.5, 0.5, 0.5],
        };
        features.extend(op_features);

        // Data structure features
        match data {
            simd_json::OwnedValue::Object(obj) => {
                features.push(1.0); // is_object
                features.push(obj.len() as f32 / 100.0); // normalized size

                // Key diversity (unique first chars)
                let unique_chars: std::collections::HashSet<char> =
                    obj.keys().filter_map(|k| k.chars().next()).collect();
                features.push(unique_chars.len() as f32 / 26.0);

                // Value type distribution
                let mut string_count = 0;
                let mut number_count = 0;
                let mut bool_count = 0;
                let mut null_count = 0;

                for value in obj.values() {
                    if value.is_str() {
                        string_count += 1;
                    } else if value.is_i64() || value.is_u64() || value.is_f64() {
                        number_count += 1;
                    } else if value.is_bool() {
                        bool_count += 1;
                    } else if value.is_null() {
                        null_count += 1;
                    }
                }

                let total = obj.len() as f32;
                features.push(string_count as f32 / total);
                features.push(number_count as f32 / total);
                features.push(bool_count as f32 / total);
                features.push(null_count as f32 / total);
            }
            simd_json::OwnedValue::Array(arr) => {
                features.push(0.0); // not_object
                features.push(arr.len() as f32 / 100.0);
                features.extend(vec![0.0; 6]); // padding
            }
            simd_json::OwnedValue::String(s) => {
                features.push(0.0); // not_object
                features.push(s.len() as f32 / 1000.0);
                features.push(self.hash_string(s) as f32 / u32::MAX as f32);
                features.extend(vec![0.0; 5]); // padding
            }
            _ => {
                features.extend(vec![0.0; 8]); // padding for other types
            }
        }

        // Metadata features
        if let Some(meta) = metadata {
            features.push(meta.len() as f32 / 50.0);

            // Common metadata keys
            let common_keys = ["user", "host", "process", "version", "source"];
            for key in &common_keys {
                features.push(if meta.contains_key(*key) { 1.0 } else { 0.0 });
            }
        } else {
            features.extend(vec![0.0; 6]); // no metadata
        }

        // Temporal features (time-based patterns)
        let hour = (self.get_current_timestamp() / 3600) % 24;
        let day_of_week = (self.get_current_timestamp() / 86400) % 7;
        features.push(hour as f32 / 24.0);
        features.push(day_of_week as f32 / 7.0);

        // Pad to fixed size
        features.resize(64, 0.0);
        Ok(features)
    }

    fn hash_string(&self, s: &str) -> u32 {
        s.bytes()
            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))
    }

    fn get_current_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Plugin trait with footprint mechanism
#[allow(dead_code)]
pub trait FootprintPlugin {
    fn plugin_id(&self) -> &str;

    /// Create footprint and send to blockchain
    fn create_and_record_footprint(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<PluginFootprint> {
        let generator = FootprintGenerator::new(self.plugin_id());
        let footprint = generator.create_footprint(operation, data, metadata)?;

        // Send to blockchain for vectorization
        self.send_to_blockchain(&footprint)?;

        Ok(footprint)
    }

    /// Send footprint to blockchain (implemented by each plugin)
    fn send_to_blockchain(&self, footprint: &PluginFootprint) -> Result<()>;
}

/// Network plugin with footprint
#[allow(dead_code)]
pub struct NetworkPlugin {
    footprint_gen: FootprintGenerator,
    blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
}

impl NetworkPlugin {
    #[allow(dead_code)]
    pub fn new(blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>) -> Self {
        Self {
            footprint_gen: FootprintGenerator::new("network"),
            blockchain_sender,
        }
    }

    #[allow(dead_code)]
    pub async fn interface_created(
        &self,
        interface: &str,
        config: simd_json::OwnedValue,
    ) -> Result<()> {
        let mut metadata = HashMap::new();
        metadata.insert(
            "interface".to_string(),
            simd_json::OwnedValue::String(interface.to_string()),
        );
        metadata.insert(
            "host".to_string(),
            simd_json::OwnedValue::String(gethostname::gethostname().to_string_lossy().to_string()),
        );

        let footprint = self
            .footprint_gen
            .create_footprint("create", &config, Some(metadata))?;
        self.blockchain_sender.send(footprint)?;
        Ok(())
    }
}

impl FootprintPlugin for NetworkPlugin {
    fn plugin_id(&self) -> &str {
        "network"
    }

    fn send_to_blockchain(&self, footprint: &PluginFootprint) -> Result<()> {
        self.blockchain_sender.send(footprint.clone())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_footprint_generation() {
        let generator = FootprintGenerator::new("test_plugin");
        let data = simd_json::json!({
            "interface": "eth0",
            "ip": "192.168.1.100",
            "status": "active"
        });

        let footprint = generator.create_footprint("create", &data, None).unwrap();

        assert_eq!(footprint.plugin_id, "test_plugin");
        assert_eq!(footprint.operation, "create");
        assert!(!footprint.data_hash.is_empty());
        assert!(!footprint.content_hash.is_empty());
        assert_eq!(footprint.vector_features.len(), 64);
    }

    #[test]
    fn test_vector_features() {
        let generator = FootprintGenerator::new("test");
        let data = simd_json::json!({"key": "value"});

        let footprint = generator.create_footprint("create", &data, None).unwrap();

        // Should have create operation features
        assert_eq!(footprint.vector_features[1], 1.0); // create = [1,0,0,0]
        assert_eq!(footprint.vector_features[2], 0.0);
        assert_eq!(footprint.vector_features[3], 0.0);
        assert_eq!(footprint.vector_features[4], 0.0);

        // Should have object features
        assert_eq!(footprint.vector_features[5], 1.0); // is_object
    }
}
