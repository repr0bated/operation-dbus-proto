pub mod json_handler;

// Re-export commonly used functions for easy migration
pub use json_handler::{
    JsonProcessor, ApiResponse, ConfigData,
    to_vec, to_string, from_str, from_slice
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub library: String,
    pub operation: String,
    pub duration_ns: u64,
    pub data_size_bytes: usize,
}

pub trait JsonProcessorTrait {
    fn parse(&self, data: &mut str) -> Result<Box<dyn std::any::Any>, Box<dyn std::error::Error>>;
    fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}