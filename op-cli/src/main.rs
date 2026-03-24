use clap::{Parser, Subcommand};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use json_benchmark::JsonProcessor;
use op_parser::{HighPerformanceJsonParser, create_batch_work};
use simd_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "simd-json-cli")]
#[command(about = "High-performance JSON processing CLI using simd-json")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and validate JSON files
    Parse {
        #[arg(short, long)]
        input: PathBuf,
        
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        #[arg(short, long)]
        validate: bool,
        
        #[arg(short, long)]
        pretty: bool,
    },
    
    /// Benchmark JSON parsing performance
    Benchmark {
        #[arg(short, long)]
        file: PathBuf,
        
        #[arg(short, long, default_value = "1000")]
        iterations: u32,
    },
    
    /// Transform JSON data
    Transform {
        #[arg(short, long)]
        input: PathBuf,
        
        #[arg(short, long)]
        operation: String,
        
        #[arg(short, long)]
        output: PathBuf,
    },
    
    /// Compare JSON files
    Compare {
        #[arg(short, long)]
        file1: PathBuf,
        
        #[arg(short, long)]
        file2: PathBuf,
    },
    
    /// Extract JSON from text files
    Extract {
        #[arg(short, long)]
        input: PathBuf,
        
        #[arg(short, long)]
        output: PathBuf,
    },
    
    /// Process large JSON files in streaming mode
    Stream {
        #[arg(short, long)]
        input: PathBuf,
        
        #[arg(short, long)]
        chunk_size: Option<usize>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    if cli.verbose {
        println!("{}", "simd-json CLI - High Performance JSON Processing".bright_green().bold());
        println!("{}", "=" .repeat(50));
    }

    let start_time = Instant::now();

    match cli.command {
        Commands::Parse { input, output, validate, pretty } => {
            handle_parse(input, output, validate, pretty, cli.verbose).await;
        }
        Commands::Benchmark { file, iterations } => {
            handle_benchmark(file, iterations, cli.verbose).await;
        }
        Commands::Transform { input, operation, output } => {
            handle_transform(input, operation, output, cli.verbose).await;
        }
        Commands::Compare { file1, file2 } => {
            handle_compare(file1, file2, cli.verbose).await;
        }
        Commands::Extract { input, output } => {
            handle_extract(input, output, cli.verbose).await;
        }
        Commands::Stream { input, chunk_size } => {
            handle_stream(input, chunk_size, cli.verbose).await;
        }
    }

    if cli.verbose {
        println!("\n{}", format!("Total execution time: {:?}", start_time.elapsed()).bright_blue());
    }
}

async fn handle_parse(input: PathBuf, output: Option<PathBuf>, validate: bool, pretty: bool, verbose: bool) {
    if verbose {
        println!("{}", format!("🔍 Parsing JSON file: {:?}", input).bright_yellow());
    }

    let contents = fs::read_to_string(&input).expect("Failed to read input file");
    let mut contents_mut = contents.clone();
    
    let parser = HighPerformanceJsonParser::new(16384);
    
    match parser.parse_json(&mut contents_mut) {
        Ok(result) => {
            println!("{}", "✓ JSON parsed successfully".bright_green());
            
            if validate {
                println!("{}", "✓ JSON validation passed".bright_green());
            }
            
            if verbose {
                println!("{}", format!("Parsing time: {} ns", result.metrics.parse_time_ns).bright_blue());
                println!("{}", format!("Objects: {}, Arrays: {}, Strings: {}", 
                    result.metrics.object_count, 
                    result.metrics.array_count, 
                    result.metrics.string_count
                ).bright_blue());
            }
            
            if let Some(output_path) = output {
                let output_json = if pretty {
                    simd_json::to_string_pretty(&result.parsed).expect("Failed to serialize")
                } else {
                    simd_json::to_string(&result.parsed).expect("Failed to serialize")
                };
                
                fs::write(&output_path, output_json).expect("Failed to write output file");
                println!("{}", format!("📁 Output written to: {:?}", output_path).bright_green());
            }
        }
        Err(e) => {
            println!("{}", format!("✗ JSON parsing failed: {}", e).bright_red());
            std::process::exit(1);
        }
    }
}

async fn handle_benchmark(file: PathBuf, iterations: u32, verbose: bool) {
    if verbose {
        println!("{}", format!("📊 Benchmarking JSON file: {:?}", file).bright_yellow());
        println!("{}", format!("Iterations: {}", iterations).bright_blue());
    }

    let contents = fs::read_to_string(&file).expect("Failed to read benchmark file");
    let processor = JsonProcessor::new(32768);

    let pb = ProgressBar::new(iterations as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut total_parse_time = std::time::Duration::new(0, 0);
    let mut total_serialize_time = std::time::Duration::new(0, 0);

    for _ in 0..iterations {
        let mut contents_mut = contents.clone();
        
        // Benchmark parsing
        let parse_start = Instant::now();
        let parsed = processor.parse_json(&mut contents_mut).expect("Parse failed");
        total_parse_time += parse_start.elapsed();
        
        // Benchmark serialization
        let serialize_start = Instant::now();
        let _serialized = processor.serialize_json(&parsed).expect("Serialize failed");
        total_serialize_time += serialize_start.elapsed();
        
        pb.inc(1);
    }

    pb.finish_with_message("Benchmark complete");

    let avg_parse_time = total_parse_time.as_nanos() as f64 / iterations as f64;
    let avg_serialize_time = total_serialize_time.as_nanos() as f64 / iterations as f64;

    println!("\n{}", "Benchmark Results".bright_green().bold());
    println!("{}", format!("Average parse time: {:.2} ns", avg_parse_time).bright_blue());
    println!("{}", format!("Average serialize time: {:.2} ns", avg_serialize_time).bright_blue());
    println!("{}", format!("Total iterations: {}", iterations).bright_blue());
}

async fn handle_transform(input: PathBuf, operation: String, output: PathBuf, verbose: bool) {
    if verbose {
        println!("{}", format!("🔄 Transforming JSON with operation: {}", operation).bright_yellow());
    }

    let contents = fs::read_to_string(&input).expect("Failed to read input file");
    let mut contents_mut = contents.clone();
    
    let parser = HighPerformanceJsonParser::new(16384);
    let mut result = parser.parse_json(&mut contents_mut).expect("Failed to parse JSON");

    match operation.as_str() {
        "flatten" => {
            // Simple flatten operation - move nested keys to top level
            if let Some(obj) = result.parsed.as_object() {
                let mut flattened = simd_json::Object::new();
                Self::flatten_object(&obj, &mut flattened, "");
                result.parsed = simd_json::OwnedValue::Object(flattened);
            }
        }
        "sort_keys" => {
            // Sort all object keys recursively
            result.parsed = Self::sort_json_keys(&result.parsed);
        }
        "remove_nulls" => {
            // Remove all null values
            result.parsed = Self::remove_null_values(&result.parsed);
        }
        _ => {
            println!("{}", format!("✗ Unknown operation: {}", operation).bright_red());
            std::process::exit(1);
        }
    }

    let output_json = simd_json::to_string_pretty(&result.parsed).expect("Failed to serialize");
    fs::write(&output, output_json).expect("Failed to write output file");
    
    println!("{}", format!("✓ Transform complete: {:?}", output).bright_green());
}

async fn handle_compare(file1: PathBuf, file2: PathBuf, verbose: bool) {
    if verbose {
        println!("{}", format!("🔍 Comparing JSON files:").bright_yellow());
        println!("  File 1: {:?}", file1);
        println!("  File 2: {:?}", file2);
    }

    let contents1 = fs::read_to_string(&file1).expect("Failed to read file 1");
    let contents2 = fs::read_to_string(&file2).expect("Failed to read file 2");
    
    let mut contents1_mut = contents1.clone();
    let mut contents2_mut = contents2.clone();
    
    let parser = HighPerformanceJsonParser::new(16384);
    
    let comparison = parser.compare_json_structures(&mut contents1_mut, &mut contents2_mut).expect("Failed to compare");
    
    println!("\n{}", "Comparison Results".bright_green().bold());
    for (key, value) in comparison {
        let status = if value { "✓".bright_green() } else { "✗".bright_red() };
        println!("{} {}: {}", status, key, value);
    }
}

async fn handle_extract(input: PathBuf, output: PathBuf, verbose: bool) {
    if verbose {
        println!("{}", format!("🔌 Extracting JSON from: {:?}", input).bright_yellow());
    }

    let contents = fs::read_to_string(&input).expect("Failed to read input file");
    let parser = HighPerformanceJsonParser::new(16384);
    
    let extracted = parser.extract_json_from_text(&contents);
    
    if extracted.is_empty() {
        println!("{}", "No JSON found in text".bright_yellow());
        return;
    }

    let results: Vec<_> = extracted.into_iter()
        .filter_map(|result| result.ok())
        .map(|result| result.parsed)
        .collect();

    let output_json = simd_json::to_string_pretty(&json!({ "extracted_json": results }))
        .expect("Failed to serialize");
    
    fs::write(&output, output_json).expect("Failed to write output file");
    
    println!("{}", format!("✓ Extracted {} JSON objects to: {:?}", results.len(), output).bright_green());
}

async fn handle_stream(input: PathBuf, chunk_size: Option<usize>, verbose: bool) {
    if verbose {
        println!("{}", format!("📂 Streaming JSON processing: {:?}", input).bright_yellow());
    }

    let file = std::fs::File::open(&input).expect("Failed to open file");
    let parser = HighPerformanceJsonParser::new(chunk_size.unwrap_or(8192));
    
    match parser.stream_parse_large_json(file) {
        Ok(results) => {
            println!("{}", format!("✓ Stream processed {} JSON objects", results.len()).bright_green());
            
            if verbose {
                let total_time: u64 = results.iter().map(|r| r.metrics.parse_time_ns).sum();
                let avg_time = total_time / results.len().max(1) as u64;
                println!("{}", format!("Average parse time: {} ns", avg_time).bright_blue());
            }
        }
        Err(e) => {
            println!("{}", format!("✗ Stream processing failed: {}", e).bright_red());
        }
    }
}

// Helper methods for transformations
impl Cli {
    fn flatten_object(obj: &simd_json::Object, flattened: &mut simd_json::Object, prefix: &str) {
        for (key, value) in obj {
            let new_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };
            
            match value {
                simd_json::OwnedValue::Object(nested_obj) => {
                    Self::flatten_object(nested_obj, flattened, &new_key);
                }
                _ => {
                    flattened.insert(new_key, value.clone());
                }
            }
        }
    }
    
    fn sort_json_keys(value: &simd_json::OwnedValue) -> simd_json::OwnedValue {
        match value {
            simd_json::OwnedValue::Object(obj) => {
                let mut sorted_obj = simd_json::Object::new();
                let mut keys: Vec<_> = obj.keys().collect();
                keys.sort();
                
                for key in keys {
                    if let Some(val) = obj.get(key) {
                        sorted_obj.insert(key.clone(), Self::sort_json_keys(val));
                    }
                }
                simd_json::OwnedValue::Object(sorted_obj)
            }
            simd_json::OwnedValue::Array(arr) => {
                let sorted_arr: Vec<_> = arr.iter().map(Self::sort_json_keys).collect();
                simd_json::OwnedValue::Array(sorted_arr)
            }
            _ => value.clone(),
        }
    }
    
    fn remove_null_values(value: &simd_json::OwnedValue) -> simd_json::OwnedValue {
        match value {
            simd_json::OwnedValue::Object(obj) => {
                let mut cleaned_obj = simd_json::Object::new();
                for (key, val) in obj {
                    if !val.is_null() {
                        cleaned_obj.insert(key.clone(), Self::remove_null_values(val));
                    }
                }
                simd_json::OwnedValue::Object(cleaned_obj)
            }
            simd_json::OwnedValue::Array(arr) => {
                let cleaned_arr: Vec<_> = arr.iter()
                    .filter(|v| !v.is_null())
                    .map(Self::remove_null_values)
                    .collect();
                simd_json::OwnedValue::Array(cleaned_arr)
            }
            _ => value.clone(),
        }
    }
}