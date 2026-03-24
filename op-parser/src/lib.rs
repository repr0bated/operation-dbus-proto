use simd_json::{prelude::*, Error as JsonError};
use regex::Regex;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static JSON_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\{[^{}]*\}"#).unwrap()
});

static ARRAY_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\[[^\[\]]*\]"#).unwrap()
});

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub original: String,
    pub parsed: simd_json::OwnedValue,
    pub metrics: ParseMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseMetrics {
    pub parse_time_ns: u64,
    pub object_count: usize,
    pub array_count: usize,
    pub string_count: usize,
    pub number_count: usize,
    pub bool_count: usize,
    pub null_count: usize,
    pub total_size_bytes: usize,
}

pub struct HighPerformanceJsonParser {
    buffer_size: usize,
    use_lazy_parsing: bool,
}

impl HighPerformanceJsonParser {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            use_lazy_parsing: true,
        }
    }

    pub fn parse_json(&self, json_str: &mut str) -> Result<ParseResult, JsonError> {
        let start = std::time::Instant::now();
        let original = json_str.to_string();
        
        let parsed = if self.use_lazy_parsing && json_str.len() > self.buffer_size {
            // Use lazy parsing for large JSON
            let borrowed: simd_json::BorrowedValue = json_str.parse()?;
            borrowed.into_owned()
        } else {
            json_str.parse()?
        };
        
        let parse_time = start.elapsed().as_nanos() as u64;
        let metrics = self.calculate_metrics(&parsed, parse_time, original.len());
        
        Ok(ParseResult {
            original,
            parsed,
            metrics,
        })
    }

    pub fn parse_json_bytes(&self, json_bytes: &mut [u8]) -> Result<ParseResult, JsonError> {
        let start = std::time::Instant::now();
        let original = String::from_utf8_lossy(json_bytes).to_string();
        
        let parsed: simd_json::OwnedValue = simd_json::from_slice(json_bytes)?;
        
        let parse_time = start.elapsed().as_nanos() as u64;
        let metrics = self.calculate_metrics(&parsed, parse_time, json_bytes.len());
        
        Ok(ParseResult {
            original,
            parsed,
            metrics,
        })
    }

    pub fn extract_json_from_text(&self, text: &str) -> Vec<Result<ParseResult, JsonError>> {
        JSON_PATTERN
            .find_iter(text)
            .filter_map(|mat| {
                let mut json_str = mat.as_str().to_string();
                match self.parse_json(&mut json_str) {
                    Ok(result) => Some(Ok(result)),
                    Err(e) => Some(Err(e)),
                }
            })
            .collect()
    }

    pub fn validate_json(&self, json_str: &mut str) -> Result<bool, JsonError> {
        match json_str.parse::<simd_json::BorrowedValue>() {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    pub fn normalize_json(&self, json_str: &mut str) -> Result<String, JsonError> {
        let parsed: simd_json::OwnedValue = json_str.parse()?;
        simd_json::to_string(&parsed)
    }

    pub fn compare_json_structures(&self, json1: &mut str, json2: &mut str) -> Result<HashMap<String, bool>, JsonError> {
        let parsed1: simd_json::OwnedValue = json1.parse()?;
        let parsed2: simd_json::OwnedValue = json2.parse()?;
        
        let mut comparison = HashMap::new();
        comparison.insert("same_type".to_string(), parsed1.type_string() == parsed2.type_string());
        comparison.insert("same_keys".to_string(), self.have_same_keys(&parsed1, &parsed2));
        comparison.insert("same_array_length".to_string(), self.have_same_array_length(&parsed1, &parsed2));
        
        Ok(comparison)
    }

    pub fn stream_parse_large_json<R: std::io::Read>(&self, reader: R) -> Result<Vec<ParseResult>, JsonError> {
        use std::io::{BufRead, BufReader};
        
        let mut results = Vec::new();
        let buf_reader = BufReader::with_capacity(self.buffer_size, reader);
        
        for line in buf_reader.lines() {
            if let Ok(mut line_str) = line {
                if let Ok(result) = self.parse_json(&mut line_str) {
                    results.push(result);
                }
            }
        }
        
        Ok(results)
    }

    fn calculate_metrics(&self, value: &simd_json::OwnedValue, parse_time: u64, size_bytes: usize) -> ParseMetrics {
        let mut metrics = ParseMetrics {
            parse_time_ns: parse_time,
            object_count: 0,
            array_count: 0,
            string_count: 0,
            number_count: 0,
            bool_count: 0,
            null_count: 0,
            total_size_bytes: size_bytes,
        };

        self.count_values(value, &mut metrics);
        metrics
    }

    fn count_values(&self, value: &simd_json::OwnedValue, metrics: &mut ParseMetrics) {
        match value {
            simd_json::OwnedValue::Object(map) => {
                metrics.object_count += 1;
                for (_, v) in map {
                    self.count_values(v, metrics);
                }
            }
            simd_json::OwnedValue::Array(arr) => {
                metrics.array_count += 1;
                for v in arr {
                    self.count_values(v, metrics);
                }
            }
            simd_json::OwnedValue::String(_) => metrics.string_count += 1,
            simd_json::OwnedValue::Number(_) => metrics.number_count += 1,
            simd_json::OwnedValue::Bool(_) => metrics.bool_count += 1,
            simd_json::OwnedValue::Null => metrics.null_count += 1,
        }
    }

    fn have_same_keys(&self, val1: &simd_json::OwnedValue, val2: &simd_json::OwnedValue) -> bool {
        match (val1, val2) {
            (simd_json::OwnedValue::Object(map1), simd_json::OwnedValue::Object(map2)) => {
                let keys1: Vec<_> = map1.keys().collect();
                let keys2: Vec<_> = map2.keys().collect();
                keys1 == keys2
            }
            _ => false,
        }
    }

    fn have_same_array_length(&self, val1: &simd_json::OwnedValue, val2: &simd_json::OwnedValue) -> bool {
        match (val1, val2) {
            (simd_json::OwnedValue::Array(arr1), simd_json::OwnedValue::Array(arr2)) => {
                arr1.len() == arr2.len()
            }
            _ => true, // Not arrays, so length comparison is irrelevant
        }
    }
}

// Migration helpers for parser operations
pub fn quick_parse(json_str: &mut str) -> Result<simd_json::OwnedValue, JsonError> {
    json_str.parse()
}

pub fn quick_validate(json_str: &mut str) -> bool {
    json_str.parse::<simd_json::BorrowedValue>().is_ok()
}

pub fn extract_metrics(json_str: &mut str) -> Result<ParseMetrics, JsonError> {
    let parser = HighPerformanceJsonParser::new(8192);
    let result = parser.parse_json(json_str)?;
    Ok(result.metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggressive_parser_migration() {
        let parser = HighPerformanceJsonParser::new(1024);
        
        let mut complex_json = r#"{
            "users": [
                {"id": 1, "name": "Alice", "active": true, "balance": 100.50},
                {"id": 2, "name": "Bob", "active": false, "balance": null}
            ],
            "metadata": {
                "version": "1.0",
                "count": 2
            }
        }"#.to_string();

        let result = parser.parse_json(&mut complex_json).unwrap();
        
        // Verify parsing worked
        assert_eq!(result.parsed["users"][0]["name"], "Alice");
        assert_eq!(result.parsed["users"][1]["active"], false);
        assert_eq!(result.parsed["metadata"]["version"], "1.0");
        
        // Verify metrics
        assert_eq!(result.metrics.object_count, 3); // root, users[0], users[1], metadata
        assert_eq!(result.metrics.array_count, 1); // users array
        assert_eq!(result.metrics.string_count, 5); // Alice, Bob, 1.0, etc.
        assert_eq!(result.metrics.number_count, 3); // id: 1, id: 2, count: 2
        assert_eq!(result.metrics.bool_count, 2); // active: true, active: false
        assert_eq!(result.metrics.null_count, 1); // balance: null
    }

    #[test]
    fn test_json_extraction() {
        let parser = HighPerformanceJsonParser::new(1024);
        
        let text = r#"
            Here is some text with {"embedded": "json"} inside it.
            And another {"array": [1, 2, 3], "nested": {"deep": "value"}} here.
        "#;

        let extracted = parser.extract_json_from_text(text);
        assert_eq!(extracted.len(), 2);
        
        let first = extracted[0].as_ref().unwrap();
        assert_eq!(first.parsed["embedded"], "json");
        
        let second = extracted[1].as_ref().unwrap();
        assert_eq!(second.parsed["array"][1], 2);
        assert_eq!(second.parsed["nested"]["deep"], "value");
    }
}