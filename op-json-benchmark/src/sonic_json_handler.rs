use sonic_rs::{JsonValue, Error as SonicError};
use std::time::Instant;

pub struct SonicJsonProcessor {
    buffer_size: usize,
}

impl SonicJsonProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    pub fn parse_json(&self, json_str: &str) -> Result<JsonValue, SonicError> {
        let start = Instant::now();
        let result = sonic_rs::from_str(json_str);
        let elapsed = start.elapsed();
        println!("Sonic JSON parsing took: {:?}", elapsed);
        result
    }

    pub fn serialize_json<T: serde::Serialize>(&self, data: &T) -> Result<String, SonicError> {
        let start = Instant::now();
        let result = sonic_rs::to_string(data);
        let elapsed = start.elapsed();
        println!("Sonic JSON serialization took: {:?}", elapsed);
        result
    }

    pub fn process_large_json(&self, json_data: &str) -> Result<JsonValue, SonicError> {
        let parsed = self.parse_json(json_data)?;
        
        if let Some(items) = parsed.get("items").and_then(|v| v.as_array()) {
            let processed_count = items.len();
            println!("Sonic Processing {} items", processed_count);
        }
        
        Ok(parsed)
    }

    pub fn lazy_parse(&self, json_data: &str) -> sonic_rs::LazyValue {
        sonic_rs::LazyValue::new(json_data.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sonic_json_parsing() {
        let processor = SonicJsonProcessor::new(1024);
        let json_str = r#"{"status": "ok", "data": {"count": 42}}"#;
        
        let result = processor.parse_json(json_str).unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["data"]["count"], 42);
    }
}