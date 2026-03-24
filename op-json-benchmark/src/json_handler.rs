use simd_json::{prelude::*, Error as JsonError};
use std::time::Instant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse {
    pub status: String,
    pub data: simd_json::OwnedValue,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigData {
    pub name: String,
    pub settings: simd_json::OwnedValue,
    pub enabled: bool,
}

pub struct JsonProcessor {
    buffer_size: usize,
}

impl JsonProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    pub fn parse_json(&self, json_str: &mut str) -> Result<simd_json::OwnedValue, JsonError> {
        let start = Instant::now();
        let result = json_str.parse();
        let elapsed = start.elapsed();
        println!("SIMD JSON parsing took: {:?}", elapsed);
        result
    }

    pub fn serialize_json<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, JsonError> {
        let start = Instant::now();
        let result = simd_json::to_vec(data);
        let elapsed = start.elapsed();
        println!("SIMD JSON serialization took: {:?}", elapsed);
        result
    }

    pub fn serialize_json_string<T: Serialize>(&self, data: &T) -> Result<String, JsonError> {
        let bytes = self.serialize_json(data)?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    pub fn process_large_json(&self, json_data: &mut str) -> Result<simd_json::OwnedValue, JsonError> {
        let parsed = self.parse_json(json_data)?;
        
        if let Some(items) = parsed.get("items").and_then(|v| v.as_array()) {
            let processed_count = items.len();
            println!("Processing {} items", processed_count);
        }
        
        Ok(parsed)
    }

    pub fn batch_serialize<T: Serialize>(&self, items: &[T]) -> Result<Vec<Vec<u8>>, JsonError> {
        items.iter()
            .map(|item| self.serialize_json(item))
            .collect()
    }

    pub fn lazy_parse<'a>(&self, json_data: &'a mut str) -> Result<simd_json::BorrowedValue<'a>, JsonError> {
        json_data.parse()
    }

    pub fn get_value<'a>(parsed: &'a simd_json::BorrowedValue<'a>, path: &[&str]) -> Option<&'a simd_json::BorrowedValue<'a>> {
        let mut current = parsed;
        for key in path {
            current = current.get(key)?;
        }
        Some(current)
    }
}

// Migration helper functions for drop-in replacement
pub fn to_vec<T: Serialize>(data: &T) -> Result<Vec<u8>, JsonError> {
    simd_json::to_vec(data)
}

pub fn to_string<T: Serialize>(data: &T) -> Result<String, JsonError> {
    simd_json::to_string(data)
}

pub fn from_str<'a, T: Deserialize<'a>>(data: &'a mut str) -> Result<T, JsonError> {
    simd_json::from_str(data)
}

pub fn from_slice<'a, T: Deserialize<'a>>(data: &'a mut [u8]) -> Result<T, JsonError> {
    simd_json::from_slice(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_json_migration() {
        let processor = JsonProcessor::new(1024);
        let mut json_str = r#"{"status": "ok", "data": {"count": 42}, "timestamp": 1234567890}"#.to_string();
        
        // Test parsing
        let parsed = processor.parse_json(&mut json_str).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["data"]["count"], 42);
        assert_eq!(parsed["timestamp"], 1234567890);
        
        // Test serialization
        let api_response = ApiResponse {
            status: "success".to_string(),
            data: simd_json::json!({"result": "test"}),
            timestamp: 1234567890,
        };
        
        let serialized = processor.serialize_json(&api_response).unwrap();
        let serialized_str = String::from_utf8_lossy(&serialized);
        assert!(serialized_str.contains("success"));
        assert!(serialized_str.contains("test"));
    }

    #[test]
    fn test_lazy_parsing() {
        let processor = JsonProcessor::new(1024);
        let mut json_str = r#"{"users": [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]}"#.to_string();
        
        let lazy_value = processor.lazy_parse(&mut json_str).unwrap();
        assert_eq!(lazy_value["users"][0]["name"], "Alice");
        assert_eq!(lazy_value["users"][1]["name"], "Bob");
    }
}