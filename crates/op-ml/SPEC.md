# op-ml - Specification

## Overview
**Crate**: `op-ml`  
**Location**: `crates/op-ml`  
**Description**: ML/Embedding support: model management, text embedder, vector storage

## Purpose

The `op-ml` crate provides machine learning capabilities for the operation-dbus system, with a focus on text embeddings and semantic search. It offers a production-ready, lazy-loading ML infrastructure that supports multiple execution backends (CPU, CUDA, TensorRT, DirectML, CoreML).

Key capabilities:
- **Text Embeddings**: Convert text to high-dimensional vectors for semantic similarity
- **Model Management**: Automatic model downloading and caching
- **Multi-Backend Support**: CPU, NVIDIA GPU, Apple Neural Engine, Windows DirectML
- **Lazy Loading**: Models loaded on-demand to minimize startup overhead
- **Configurable Quality**: Fast, balanced, and best quality embedding models

This crate enables:
- Semantic search across D-Bus interfaces and documentation
- Intelligent plugin discovery based on natural language queries
- Context-aware agent routing
- Vector-based similarity matching

## Architecture

### Lazy Loading Design
Models are loaded on first use to avoid startup penalties:
1. Application starts with minimal overhead
2. First embedding request triggers model download/load
3. Subsequent requests use cached model
4. Global singleton ensures single model instance

### Execution Providers
Supports multiple hardware acceleration backends:
- **CPU**: Multi-threaded inference with configurable thread count
- **CUDA**: NVIDIA GPU acceleration
- **TensorRT**: Optimized NVIDIA inference
- **DirectML**: Windows GPU acceleration
- **CoreML**: Apple Neural Engine and GPU

### Model Tiers
Three quality levels balancing speed vs accuracy:

| Level | Model | Dimensions | Speed | Use Case |
|-------|-------|------------|-------|----------|
| Fast | MiniLM-L6 | 384 | Fastest | Real-time queries |
| Balanced | MiniLM-L12 | 384 | Medium | General purpose |
| Best | BGE-Base | 768 | Slower | High accuracy needs |

## Key Components

### ModelManager
Central component for model lifecycle management.

```rust
pub struct ModelManager {
    config: VectorizationConfig,
    embedder: OnceCell<TextEmbedder>,
}
```

**Key Methods**:
```rust
// Create new manager with config
ModelManager::new(config)

// Get global singleton instance
ModelManager::global()

// Check if ML is enabled
manager.is_enabled()

// Embed single text
manager.embed(text) -> Result<Vec<f32>>

// Embed batch of texts
manager.embed_batch(texts) -> Result<Vec<Vec<f32>>>
```

**Singleton Pattern**:
```rust
static MODEL_MANAGER: OnceCell<Arc<ModelManager>> = OnceCell::new();
```

### TextEmbedder
ONNX Runtime-based text embedding engine.

```rust
pub struct TextEmbedder {
    session: Session,           // ONNX Runtime session
    tokenizer: Tokenizer,       // HuggingFace tokenizer
    level: VectorizationLevel,  // Quality level
}
```

**Key Methods**:
```rust
// Load model from directory
TextEmbedder::load(model_dir, config)

// Embed text to vector
embedder.embed(text) -> Result<Vec<f32>>

// Embed batch
embedder.embed_batch(texts) -> Result<Vec<Vec<f32>>>
```

### ModelDownloader
Automatic model downloading and caching.

```rust
pub struct ModelDownloader {
    cache_dir: PathBuf,
    client: reqwest::Client,
}
```

**Key Methods**:
```rust
// Create downloader with cache directory
ModelDownloader::new(cache_dir)

// Download model if not cached
downloader.ensure_model(level) -> Result<PathBuf>

// Check if model is cached
downloader.is_cached(level) -> bool

// Get model directory path
downloader.model_path(level) -> PathBuf
```

**Download Sources**:
- HuggingFace model hub
- Local mirror support
- Checksum verification with SHA256

### VectorizationConfig
Configuration for embedding behavior.

```rust
pub struct VectorizationConfig {
    pub level: VectorizationLevel,
    pub execution_provider: ExecutionProvider,
    pub num_threads: usize,
    pub gpu_device_id: i32,
}
```

**Environment Variables**:
```bash
VECTORIZATION_LEVEL=fast|balanced|best|off
VECTORIZATION_PROVIDER=cpu|cuda|tensorrt|directml|coreml
VECTORIZATION_THREADS=4
VECTORIZATION_GPU_DEVICE=0
```

**Defaults**:
```rust
VectorizationConfig {
    level: VectorizationLevel::Fast,
    execution_provider: ExecutionProvider::Cpu,
    num_threads: num_cpus::get(),
    gpu_device_id: 0,
}
```

### VectorizationLevel
Quality/speed trade-off levels.

```rust
pub enum VectorizationLevel {
    Off,       // Disabled
    Fast,      // MiniLM-L6 (384d)
    Balanced,  // MiniLM-L12 (384d)
    Best,      // BGE-Base (768d)
}
```

### ExecutionProvider
Hardware acceleration backend.

```rust
pub enum ExecutionProvider {
    Cpu,       // Multi-threaded CPU
    Cuda,      // NVIDIA CUDA
    TensorRT,  // NVIDIA TensorRT
    DirectML,  // Windows DirectML
    CoreML,    // Apple Neural Engine
}
```

## Module Structure

### Core Modules
- **model_manager**: Lazy-loading model lifecycle management
- **embedder**: ONNX Runtime text embedding
- **downloader**: Model downloading and caching
- **config**: Configuration types and environment parsing

## Dependencies

### ML Dependencies (feature = "ml")
- **ort**: ONNX Runtime bindings for inference
- **tokenizers**: HuggingFace tokenizers for text preprocessing
- **ndarray**: N-dimensional arrays for tensor operations
- **once_cell**: Lazy static initialization

### Core Dependencies
- **tokio**: Async runtime for downloads
- **reqwest**: HTTP client for model downloads
- **serde**: Configuration serialization
- **simd-json**: High-performance JSON

### Utilities
- **sha2**: SHA256 checksums for model verification
- **num_cpus**: CPU count detection for threading
- **anyhow/thiserror**: Error handling
- **tracing/log**: Logging

## Usage

### Basic Embedding

```rust
use op_ml::ModelManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get global model manager
    let manager = ModelManager::global();
    
    // Embed text (lazy loads model on first call)
    let embedding = manager.embed("Hello, world!")?;
    
    println!("Embedding dimension: {}", embedding.len());
    
    Ok(())
}
```

### Configuration

```rust
use op_ml::{VectorizationConfig, VectorizationLevel, ExecutionProvider};

// Create custom config
let config = VectorizationConfig {
    level: VectorizationLevel::Best,
    execution_provider: ExecutionProvider::Cuda,
    num_threads: 8,
    gpu_device_id: 0,
};

// Create manager with config
let manager = ModelManager::new(config);
```

### Environment-Based Configuration

```bash
# Set quality level
export VECTORIZATION_LEVEL=best

# Use CUDA GPU
export VECTORIZATION_PROVIDER=cuda
export VECTORIZATION_GPU_DEVICE=0

# Run application
./my-app
```

```rust
// Load config from environment
let config = VectorizationConfig::from_env();
let manager = ModelManager::new(config);
```

### Batch Embedding

```rust
// Embed multiple texts efficiently
let texts = vec![
    "First document",
    "Second document",
    "Third document",
];

let embeddings = manager.embed_batch(&texts)?;

for (i, embedding) in embeddings.iter().enumerate() {
    println!("Document {}: {} dimensions", i, embedding.len());
}
```

### Semantic Similarity

```rust
// Compute cosine similarity between embeddings
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}

// Compare documents
let doc1 = manager.embed("Machine learning is fascinating")?;
let doc2 = manager.embed("AI and ML are interesting topics")?;
let doc3 = manager.embed("I like pizza")?;

let sim_1_2 = cosine_similarity(&doc1, &doc2);
let sim_1_3 = cosine_similarity(&doc1, &doc3);

println!("Similarity 1-2: {:.3}", sim_1_2); // High
println!("Similarity 1-3: {:.3}", sim_1_3); // Low
```

### Model Download

```rust
use op_ml::{ModelDownloader, VectorizationLevel};

// Create downloader
let downloader = ModelDownloader::new("/var/cache/op-ml");

// Ensure model is downloaded
let model_path = downloader.ensure_model(VectorizationLevel::Fast).await?;

println!("Model cached at: {:?}", model_path);
```

## Feature Flags

### `ml` Feature
The ML functionality is behind a feature flag to make it optional:

```toml
[dependencies]
op-ml = { version = "0.1", features = ["ml"] }
```

**Without `ml` feature**:
- Minimal dependencies
- Stub implementations return empty vectors
- No ONNX Runtime dependency
- Suitable for environments without ML requirements

**With `ml` feature**:
- Full ML capabilities
- ONNX Runtime and tokenizers included
- Larger binary size
- Requires ONNX Runtime system libraries

## Performance Considerations

### Model Loading
- **First Call**: 100-500ms (model load + inference)
- **Subsequent Calls**: 1-10ms (inference only)
- **Batch Processing**: More efficient than individual calls

### Memory Usage
- **Fast Model**: ~100MB RAM
- **Balanced Model**: ~150MB RAM
- **Best Model**: ~300MB RAM

### Throughput
| Backend | Embeddings/sec | Latency |
|---------|----------------|---------|
| CPU (8 threads) | 100-200 | 5-10ms |
| CUDA | 500-1000 | 1-2ms |
| TensorRT | 1000-2000 | 0.5-1ms |

### Optimization Tips
- Use batch embedding for multiple texts
- Choose appropriate quality level for use case
- Enable GPU acceleration when available
- Reuse ModelManager instance (singleton pattern)

## Integration Points

### Semantic Search
```rust
// Search D-Bus interfaces by natural language
let query_embedding = manager.embed("network configuration")?;

// Compare with interface descriptions
for interface in interfaces {
    let desc_embedding = manager.embed(&interface.description)?;
    let similarity = cosine_similarity(&query_embedding, &desc_embedding);
    
    if similarity > 0.7 {
        println!("Found relevant interface: {}", interface.name);
    }
}
```

### Agent Routing
```rust
// Route user query to appropriate agent
let query = "How do I configure the firewall?";
let query_embedding = manager.embed(query)?;

let mut best_agent = None;
let mut best_score = 0.0;

for agent in agents {
    let agent_embedding = manager.embed(&agent.description)?;
    let score = cosine_similarity(&query_embedding, &agent_embedding);
    
    if score > best_score {
        best_score = score;
        best_agent = Some(agent);
    }
}
```

## Error Handling

### Common Errors
- **Model Not Found**: Model not downloaded or cache corrupted
- **ONNX Runtime Error**: Inference failure or invalid input
- **Tokenization Error**: Text encoding issues
- **GPU Not Available**: Requested GPU backend not available

### Recovery Strategies
- Automatic fallback to CPU if GPU unavailable
- Model re-download on checksum mismatch
- Graceful degradation when ML disabled

## Testing

### Unit Tests
- Configuration parsing
- Model path resolution
- Embedding dimension validation

### Integration Tests
- End-to-end embedding pipeline
- Model download and caching
- Multi-backend execution

### Benchmarks
- Embedding throughput
- Batch vs individual performance
- Backend comparison

## Future Enhancements

- **Vector Database Integration**: Native vector storage
- **Quantization**: INT8/FP16 models for faster inference
- **Model Fine-tuning**: Domain-specific model training
- **Multilingual Support**: Non-English embedding models
- **Streaming Embeddings**: Process large documents in chunks
- **Caching**: LRU cache for frequently embedded texts
- **Distributed Inference**: Load balancing across GPUs
- **Model Versioning**: Support multiple model versions

## Related Crates

- **op-agents**: Agent routing using embeddings
- **op-introspection**: Semantic D-Bus interface search
- **op-chat**: Context-aware conversation using embeddings
- **op-plugins**: Plugin discovery by semantic matching

---
*Production-ready ML embeddings with multi-backend support*
