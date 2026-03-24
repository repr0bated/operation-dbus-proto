use crate::{json_handler::JsonProcessor, simd_json_handler::SimdJsonProcessor, sonic_json_handler::SonicJsonProcessor};
use simd_json::json;
use std::time::Instant;

pub struct JsonBenchmark {
    test_data: String,
}

impl JsonBenchmark {
    pub fn new() -> Self {
        let test_data = json!({
            "users": (0..1000).map(|i| {
                json!({
                    "id": i,
                    "name": format!("User {}", i),
                    "email": format!("user{}@example.com", i),
                    "metadata": {
                        "created": "2023-01-01T00:00:00Z",
                        "updated": "2023-12-31T23:59:59Z",
                        "tags": ["tag1", "tag2", "tag3"]
                    }
                })
            }).collect::<Vec<_>>(),
            "total": 1000,
            "page": 1,
            "per_page": 1000
        });

        Self {
            test_data: simd_json::to_string(&test_data).unwrap(),
        }
    }

    pub fn run_benchmarks(&self) {
        println!("Running JSON library benchmarks...");
        println!("Test data size: {} bytes\n", self.test_data.len());

        // Benchmark serde_json
        let processor = JsonProcessor::new(8192);
        self.benchmark_serde_json(&processor);

        // Benchmark simd-json
        let simd_processor = SimdJsonProcessor::new(8192);
        self.benchmark_simd_json(&simd_processor);

        // Benchmark sonic-rs
        let sonic_processor = SonicJsonProcessor::new(8192);
        self.benchmark_sonic_rs(&sonic_processor);
    }

    fn benchmark_serde_json(&self, processor: &JsonProcessor) {
        println!("=== serde_json Benchmark ===");
        
        let start = Instant::now();
        let parsed = processor.parse_json(&self.test_data).unwrap();
        let parse_time = start.elapsed();
        println!("Parsing: {:?}", parse_time);

        let start = Instant::now();
        let _serialized = processor.serialize_json(&parsed).unwrap();
        let serialize_time = start.elapsed();
        println!("Serialization: {:?}", serialize_time);
    }

    fn benchmark_simd_json(&self, processor: &SimdJsonProcessor) {
        println!("\n=== simd-json Benchmark ===");
        
        let mut test_data = self.test_data.clone();
        let start = Instant::now();
        let parsed = processor.parse_json(&mut test_data).unwrap();
        let parse_time = start.elapsed();
        println!("Parsing: {:?}", parse_time);

        let start = Instant::now();
        let _serialized = processor.serialize_json(&parsed).unwrap();
        let serialize_time = start.elapsed();
        println!("Serialization: {:?}", serialize_time);
    }

    fn benchmark_sonic_rs(&self, processor: &SonicJsonProcessor) {
        println!("\n=== sonic-rs Benchmark ===");
        
        let start = Instant::now();
        let parsed = processor.parse_json(&self.test_data).unwrap();
        let parse_time = start.elapsed();
        println!("Parsing: {:?}", parse_time);

        let start = Instant::now();
        let _serialized = processor.serialize_json(&parsed).unwrap();
        let serialize_time = start.elapsed();
        println!("Serialization: {:?}", serialize_time);
    }
}