use json_benchmark::JsonProcessor;
use simd_json::json;

fn main() {
    println!("=== SIMD-JSON Aggressive Migration Demo ===\n");

    let processor = JsonProcessor::new(8192);

    // Demo 1: Basic JSON operations
    println!("1. Basic JSON parsing and serialization:");
    let mut json_str = r#"{
        "status": "active",
        "data": {
            "users": 150,
            "orders": 2430,
            "revenue": 125000.50
        },
        "timestamp": 1703123456
    }"#.to_string();

    match processor.parse_json(&mut json_str) {
        Ok(parsed) => {
            println!("✓ Parsed JSON with {} users", parsed["data"]["users"]);
            
            // Serialize back
            match processor.serialize_json_string(&parsed) {
                Ok(serialized) => println!("✓ Serialized: {} bytes", serialized.len()),
                Err(e) => println!("✗ Serialization error: {}", e),
            }
        }
        Err(e) => println!("✗ Parsing error: {}", e),
    }

    // Demo 2: Large JSON processing
    println!("\n2. Large JSON processing:");
    let large_json = json!({
        "items": (0..10000).map(|i| {
            json!({
                "id": i,
                "name": format!("Item_{}", i),
                "price": (i as f64) * 1.99,
                "metadata": {
                    "category": "electronics",
                    "tags": ["new", "sale", "featured"],
                    "stock": i % 100
                }
            })
        }).collect::<Vec<_>>(),
        "total": 10000,
        "page": 1,
        "per_page": 10000
    });

    let mut large_json_str = simd_json::to_string(&large_json).unwrap();
    println!("Processing {} KB of JSON data...", large_json_str.len() / 1024);

    match processor.process_large_json(&mut large_json_str) {
        Ok(parsed) => {
            if let Some(total) = parsed.get("total") {
                println!("✓ Successfully processed {} items", total);
            }
        }
        Err(e) => println!("✗ Large JSON processing error: {}", e),
    }

    // Demo 3: Batch operations
    println!("\n3. Batch serialization:");
    let batch_data: Vec<_> = (0..100).map(|i| {
        json!({
            "id": i,
            "message": format!("Message {}", i),
            "priority": if i % 3 == 0 { "high" } else { "normal" },
            "timestamp": 1703123456 + i
        })
    }).collect();

    match processor.batch_serialize(&batch_data) {
        Ok(serialized_batch) => {
            let total_size: usize = serialized_batch.iter().map(|v| v.len()).sum();
            println!("✓ Batch serialized {} items ({} KB total)", serialized_batch.len(), total_size / 1024);
        }
        Err(e) => println!("✗ Batch serialization error: {}", e),
    }

    // Demo 4: Lazy parsing for memory efficiency
    println!("\n4. Lazy parsing demonstration:");
    let mut nested_json = r#"{
        "company": {
            "name": "TechCorp",
            "departments": {
                "engineering": {
                    "employees": 150,
                    "budget": 2500000
                },
                "sales": {
                    "employees": 75,
                    "budget": 1200000
                }
            }
        }
    }"#.to_string();

    match processor.lazy_parse(&mut nested_json) {
        Ok(lazy_value) => {
            // Access nested data without full parsing
            if let Some(eng_budget) = lazy_value.get("company")
                .and_then(|c| c.get("departments"))
                .and_then(|d| d.get("engineering"))
                .and_then(|e| e.get("budget")) {
                println!("✓ Engineering budget: {}", eng_budget);
            }
        }
        Err(e) => println!("✗ Lazy parsing error: {}", e),
    }

    println!("\n=== Migration Complete ===");
    println!("All JSON operations now use simd-json for maximum performance!");
}