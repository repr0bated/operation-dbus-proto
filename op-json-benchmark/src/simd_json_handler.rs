use simd_json::{prelude::*, Error as SimdError};
use std::time::Instant;

pub struct SimdJsonProcessor {
    buffer_size: usize,
}

impl SimdJsonProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    pub fn parse_json(&self, json_str: &mut str) -> Result<simd_json::BorrowedValue, SimdError> {
        let start = Instant::now();
        let result = json_str.parse();
        let elapsed = start.elapsed();
        println!("SIMD JSON parsing took: {:?}", elapsed);
        result
    }

    pub fn serialize_json<T: serde::Serialize>(&self, data: &T) -> Result<Vec<u8>, SimdError> {
        let start = Instant::now();
        let result = simd_json::to_vec(data);
        let elapsed = start.elapsed();
        println!("SIMD JSON serialization took: {:?}", elapsed);
        result
    }

    pub fn process_large_json(&self, json_data: &mut str) -> Result<simd_json::BorrowedValue, SimdError> {
        let parsed = self.parse_json(json_data)?;
        
        if let Some(items) = parsed.get("items").and_then(|v| v.as_array()) {
            let processed_count = items.len();
            println!("SIMD Processing {} items", processed_count);
        }
        
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_json_parsing() {
        let processor = SimdJsonProcessor::new(1024);
        let mut json_str = r#"{"status": "ok", "data": {"count": 42}}"#.to_string();
        
        let result = processor.parse_json(&mut json_str).unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["data"]["count"], 42);
    }
}