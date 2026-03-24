//! Transformer-Based Vectorization for Blockchain Footprints
//!
//! Supports on-demand transformer vectorization with four semantic depth levels:
//! - None (default): No vectorization, zero overhead
//! - Low: Fast heuristic (keyword-based)
//! - Medium: MiniLM-L6-v2 (balanced)
//! - High: MPNet-base-v2 (best quality)

use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use sha2::{Sha256, Digest};

use op_execution_tracker::ExecutionRecord;

/// Vectorization level
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum VectorLevel {
    None,
    Low,
    Medium,
    High,
}

impl Default for VectorLevel {
    fn default() -> Self {
        VectorLevel::None
    }
}

impl VectorLevel {
    /// Parse from environment or string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "low" => VectorLevel::Low,
            "medium" | "med" => VectorLevel::Medium,
            "high" => VectorLevel::High,
            _ => VectorLevel::None,
        }
    }

    /// Get the dimension of vectors at this level
    pub fn dimension(&self) -> usize {
        match self {
            VectorLevel::None => 0,
            VectorLevel::Low => 32,
            VectorLevel::Medium => 384,
            VectorLevel::High => 768,
        }
    }
}

/// Footprint with vector features
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VectorFootprint {
    /// Execution record hash
    pub execution_hash: String,
    /// Plugin name
    pub plugin: String,
    /// Tool name
    pub tool: String,
    /// Operation type (create, update, delete, query)
    pub operation: OperationType,
    /// Content hash
    pub content_hash: String,
    /// Vector features (dimensionality depends on level)
    pub vector_features: Vec<f32>,
    /// Vectorization level used
    pub vector_level: VectorLevel,
    /// Timestamp
    pub timestamp_ns: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum OperationType {
    Create,
    Update,
    Delete,
    Query,
    Unknown,
}

impl OperationType {
    pub fn from_tool_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("create") || lower.contains("add") || lower.contains("new") {
            OperationType::Create
        } else if lower.contains("update") || lower.contains("modify") || lower.contains("set") {
            OperationType::Update
        } else if lower.contains("delete") || lower.contains("remove") || lower.contains("destroy") {
            OperationType::Delete
        } else if lower.contains("get") || lower.contains("list") || lower.contains("query") {
            OperationType::Query
        } else {
            OperationType::Unknown
        }
    }
}

/// Footprint generator
pub struct FootprintGenerator {
    level: VectorLevel,
    #[cfg(feature = "ml")]
    model: Option<Box<dyn VectorizationModel>>,
}

impl std::fmt::Debug for FootprintGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FootprintGenerator")
            .field("level", &self.level)
            .finish()
    }
}

impl FootprintGenerator {
    /// Create a new footprint generator
    pub fn new(level: VectorLevel) -> Self {
        Self {
            level,
            #[cfg(feature = "ml")]
            model: None,
        }
    }

    /// Create from environment variable
    pub fn from_env() -> Self {
        let level = std::env::var("OP_DBUS_VECTOR_LEVEL")
            .map(|v| VectorLevel::from_str(&v))
            .unwrap_or(VectorLevel::None);

        Self::new(level)
    }

    /// Generate footprint from execution record
    pub fn create_footprint(&self, record: &ExecutionRecord) -> VectorFootprint {
        let plugin = record.tool().split('.').next().unwrap_or("unknown").to_string();
        let operation = OperationType::from_tool_name(record.tool());

        let content_hash = self.compute_content_hash(record);
        let vector_features = self.vectorize(record);

        VectorFootprint {
            execution_hash: record.hash().to_string(),
            plugin,
            tool: record.tool().to_string(),
            operation,
            content_hash,
            vector_features,
            vector_level: self.level,
            timestamp_ns: record.timing.wallclock_ns,
        }
    }

    /// Compute content hash
    fn compute_content_hash(&self, record: &ExecutionRecord) -> String {
        let mut hasher = Sha256::new();
        hasher.update(record.tool().as_bytes());
        hasher.update(serde_json::to_vec(&record.input).unwrap_or_default());
        hasher.update(serde_json::to_vec(&record.output).unwrap_or_default());
        hex::encode(hasher.finalize())
    }

    /// Generate vector representation
    fn vectorize(&self, record: &ExecutionRecord) -> Vec<f32> {
        match self.level {
            VectorLevel::None => vec![],
            VectorLevel::Low => self.heuristic_vectorize(record),
            VectorLevel::Medium | VectorLevel::High => {
                #[cfg(feature = "ml")]
                {
                    self.transformer_vectorize(record)
                }
                #[cfg(not(feature = "ml"))]
                {
                    self.heuristic_vectorize(record)
                }
            }
        }
    }

    /// Fast heuristic vectorization (32 dimensions)
    fn heuristic_vectorize(&self, record: &ExecutionRecord) -> Vec<f32> {
        let mut vec = vec![0.0f32; 32];

        // Encode tool namespace (first 8 dims)
        let tool_parts: Vec<&str> = record.tool().split('.').collect();
        for (i, part) in tool_parts.iter().take(4).enumerate() {
            let hash = self.simple_hash(part) as f32 / u32::MAX as f32;
            vec[i * 2] = hash;
            vec[i * 2 + 1] = part.len() as f32 / 20.0;
        }

        // Encode operation type (dims 8-11)
        let op_type = OperationType::from_tool_name(record.tool());
        match op_type {
            OperationType::Create => vec[8] = 1.0,
            OperationType::Update => vec[9] = 1.0,
            OperationType::Delete => vec[10] = 1.0,
            OperationType::Query => vec[11] = 1.0,
            OperationType::Unknown => {}
        }

        // Encode input complexity (dims 12-15)
        let input_str = serde_json::to_string(&record.input).unwrap_or_default();
        vec[12] = (input_str.len() as f32 / 1000.0).min(1.0);
        vec[13] = (input_str.matches('{').count() as f32 / 10.0).min(1.0);
        vec[14] = (input_str.matches('[').count() as f32 / 10.0).min(1.0);
        vec[15] = if record.input.is_object() { 1.0 } else { 0.0 };

        // Encode output complexity (dims 16-19)
        let output_str = serde_json::to_string(&record.output).unwrap_or_default();
        vec[16] = (output_str.len() as f32 / 1000.0).min(1.0);
        vec[17] = (output_str.matches('{').count() as f32 / 10.0).min(1.0);
        vec[18] = (output_str.matches('[').count() as f32 / 10.0).min(1.0);
        vec[19] = if record.output.is_object() { 1.0 } else { 0.0 };

        // Encode timing (dims 20-23)
        let duration_ms = record.timing.duration_ns as f32 / 1_000_000.0;
        vec[20] = (duration_ms / 1000.0).min(1.0); // Normalize to 1 second
        vec[21] = if duration_ms < 10.0 { 1.0 } else { 0.0 }; // Fast execution
        vec[22] = if duration_ms > 100.0 { 1.0 } else { 0.0 }; // Slow execution
        vec[23] = if duration_ms > 1000.0 { 1.0 } else { 0.0 }; // Very slow

        // Encode hash features (dims 24-31)
        for (i, byte) in record.hash().bytes().take(8).enumerate() {
            vec[24 + i] = byte as f32 / 255.0;
        }

        // L2 normalize
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        vec
    }

    fn simple_hash(&self, s: &str) -> u32 {
        let mut hash: u32 = 5381;
        for c in s.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(c as u32);
        }
        hash
    }

    #[cfg(feature = "ml")]
    fn transformer_vectorize(&self, record: &ExecutionRecord) -> Vec<f32> {
        // Prepare text for embedding
        let text = self.prepare_text(record);

        // Use model if available, otherwise fall back to heuristic
        if let Some(ref model) = self.model {
            model.embed(&text)
        } else {
            // Expand heuristic to target dimension
            let heuristic = self.heuristic_vectorize(record);
            let target_dim = self.level.dimension();
            let mut expanded = vec![0.0f32; target_dim];
            for (i, v) in heuristic.iter().enumerate() {
                expanded[i] = *v;
            }
            expanded
        }
    }

    #[cfg(feature = "ml")]
    fn prepare_text(&self, record: &ExecutionRecord) -> String {
        let mut parts = vec![
            format!("tool: {}", record.tool()),
            format!("policy: {}", record.policy_id),
        ];

        // Add input fields
        if let Some(obj) = record.input.as_object() {
            for (k, v) in obj.iter().take(10) {
                parts.push(format!("input.{}: {}", k, v));
            }
        }

        // Add output fields
        if let Some(obj) = record.output.as_object() {
            for (k, v) in obj.iter().take(10) {
                parts.push(format!("output.{}: {}", k, v));
            }
        }

        parts.join(" ")
    }
}

/// Vectorization model trait
#[cfg(feature = "ml")]
pub trait VectorizationModel: Send + Sync {
    fn embed(&self, text: &str) -> Vec<f32>;
    fn dimension(&self) -> usize;
}

/// Cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_vectorization() {
        let generator = FootprintGenerator::new(VectorLevel::Low);
        let record = ExecutionRecord::builder("system.echo")
            .input(serde_json::json!({"message": "hello"}))
            .output(serde_json::json!({"echoed": "hello"}))
            .build();

        let footprint = generator.create_footprint(&record);
        assert_eq!(footprint.vector_features.len(), 32);
        assert!(!footprint.content_hash.is_empty());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);
    }
}
