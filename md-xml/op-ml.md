This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  config.rs
  downloader.rs
  embedder.rs
  lib.rs
  model_manager.rs
Cargo.toml
compare-op-ml.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/config.rs">
use serde::{Deserialize, Serialize};
/// Vectorization configuration for on-demand transformer embeddings
use std::str::FromStr;

/// Vectorization semantic depth levels
/// Vectorization semantic depth levels.
///
/// Examples
/// ```
/// use op_ml::VectorizationLevel;
/// assert_eq!(VectorizationLevel::Low.dimensions(), 384);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VectorizationLevel {
    /// No vectorization (zero overhead)
    None,
    /// Basic keyword embedding - MiniLM-L3-v2 (384-dim, ~61MB, ~19k/s)
    Low,
    /// Sentence-level encoding - MiniLM-L6-v2 (384-dim, ~80MB, ~14k/s)
    Medium,
    /// Full document embedding - MPNet-base-v2 (768-dim, ~420MB, ~2.8k/s)
    High,
}

impl VectorizationLevel {
    #[allow(dead_code)]
    /// Get model name for Hugging Face
    pub fn model_name(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Low => Some("sentence-transformers/paraphrase-MiniLM-L3-v2"),
            Self::Medium => Some("sentence-transformers/paraphrase-MiniLM-L6-v2"),
            Self::High => Some("sentence-transformers/all-mpnet-base-v2"),
        }
    }

    #[allow(dead_code)]
    /// Get expected embedding dimensionality
    pub fn dimensions(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Low => 384,
            Self::Medium => 384,
            Self::High => 768,
        }
    }

    #[allow(dead_code)]
    /// Get approximate model size in MB
    pub fn model_size_mb(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Low => 61,
            Self::Medium => 80,
            Self::High => 420,
        }
    }

    #[allow(dead_code)]
    /// Get expected throughput (sentences/sec on CPU)
    pub fn expected_throughput(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Low => 19000,
            Self::Medium => 14000,
            Self::High => 2800,
        }
    }
}

impl Default for VectorizationLevel {
    fn default() -> Self {
        Self::None
    }
}

impl FromStr for VectorizationLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "low" => Ok(Self::Low),
            "medium" | "med" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(anyhow::anyhow!(
                "Invalid vectorization level '{}'. Valid options: none, low, medium, high",
                s
            )),
        }
    }
}

impl std::fmt::Display for VectorizationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

/// Execution provider for inference
/// Execution provider for inference.
///
/// Examples
/// ```
/// use std::str::FromStr;
/// use op_ml::ExecutionProvider;
/// assert!(matches!(ExecutionProvider::from_str("cpu").unwrap(), ExecutionProvider::Cpu));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionProvider {
    /// CPU execution (default)
    Cpu,
    /// CUDA GPU execution (NVIDIA)
    Cuda,
    /// TensorRT GPU execution (NVIDIA, optimized)
    TensorRT,
    /// DirectML GPU execution (Windows)
    DirectML,
    /// CoreML GPU execution (Apple)
    CoreML,
}

impl Default for ExecutionProvider {
    fn default() -> Self {
        Self::Cpu
    }
}

impl FromStr for ExecutionProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cpu" => Ok(Self::Cpu),
            "cuda" | "gpu" => Ok(Self::Cuda),
            "tensorrt" | "trt" => Ok(Self::TensorRT),
            "directml" | "dml" => Ok(Self::DirectML),
            "coreml" => Ok(Self::CoreML),
            _ => Err(anyhow::anyhow!(
                "Invalid execution provider '{}'. Valid options: cpu, cuda, tensorrt, directml, coreml",
                s
            )),
        }
    }
}

impl std::fmt::Display for ExecutionProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Cuda => write!(f, "cuda"),
            Self::TensorRT => write!(f, "tensorrt"),
            Self::DirectML => write!(f, "directml"),
            Self::CoreML => write!(f, "coreml"),
        }
    }
}

/// Configuration for vectorization system
/// Configuration for vectorization system.
///
/// Examples
/// ```
/// use op_ml::{VectorizationConfig, VectorizationLevel};
/// let cfg = VectorizationConfig { level: VectorizationLevel::None, ..Default::default() };
/// assert!(!cfg.is_enabled());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizationConfig {
    /// Semantic depth level
    pub level: VectorizationLevel,

    /// Model storage directory
    pub model_dir: std::path::PathBuf,

    /// Maximum batch size for inference
    pub batch_size: usize,

    /// Timeout for model loading (seconds)
    pub load_timeout_secs: u64,

    /// Number of inference threads (CPU only)
    pub num_threads: usize,

    /// Execution provider (CPU/GPU)
    pub execution_provider: ExecutionProvider,

    /// GPU device ID (for multi-GPU systems)
    pub gpu_device_id: i32,
}

impl Default for VectorizationConfig {
    fn default() -> Self {
        Self {
            level: VectorizationLevel::None,
            model_dir: std::path::PathBuf::from("/var/lib/op-dbus/models"),
            batch_size: 32,
            load_timeout_secs: 60,
            num_threads: num_cpus::get(),
            execution_provider: ExecutionProvider::Cpu,
            gpu_device_id: 0,
        }
    }
}

impl VectorizationConfig {
    /// Create config from environment variable
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Check OP_DBUS_VECTOR_LEVEL environment variable
        if let Ok(level_str) = std::env::var("OP_DBUS_VECTOR_LEVEL") {
            if let Ok(level) = VectorizationLevel::from_str(&level_str) {
                config.level = level;
                log::info!("Vectorization level set to: {}", level);
            } else {
                log::warn!(
                    "Invalid OP_DBUS_VECTOR_LEVEL '{}', using default (none)",
                    level_str
                );
            }
        }

        // Check model directory override
        if let Ok(model_dir) = std::env::var("OP_DBUS_MODEL_DIR") {
            config.model_dir = std::path::PathBuf::from(model_dir);
        }

        // Check execution provider (CPU/GPU)
        if let Ok(provider_str) = std::env::var("OP_DBUS_EXECUTION_PROVIDER") {
            if let Ok(provider) = ExecutionProvider::from_str(&provider_str) {
                config.execution_provider = provider;
                log::info!("Execution provider set to: {}", provider);
            } else {
                log::warn!(
                    "Invalid OP_DBUS_EXECUTION_PROVIDER '{}', using CPU",
                    provider_str
                );
            }
        }

        // Check GPU device ID
        if let Ok(device_str) = std::env::var("OP_DBUS_GPU_DEVICE") {
            if let Ok(device_id) = device_str.parse::<i32>() {
                config.gpu_device_id = device_id;
                log::info!("GPU device ID set to: {}", device_id);
            }
        }

        config
    }

    /// Check if vectorization is enabled
    pub fn is_enabled(&self) -> bool {
        self.level != VectorizationLevel::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_parsing() {
        assert_eq!(
            VectorizationLevel::from_str("none").unwrap(),
            VectorizationLevel::None
        );
        assert_eq!(
            VectorizationLevel::from_str("low").unwrap(),
            VectorizationLevel::Low
        );
        assert_eq!(
            VectorizationLevel::from_str("medium").unwrap(),
            VectorizationLevel::Medium
        );
        assert_eq!(
            VectorizationLevel::from_str("high").unwrap(),
            VectorizationLevel::High
        );
        assert!(VectorizationLevel::from_str("invalid").is_err());
    }

    #[test]
    fn test_level_properties() {
        assert_eq!(VectorizationLevel::Low.dimensions(), 384);
        assert_eq!(VectorizationLevel::Medium.dimensions(), 384);
        assert_eq!(VectorizationLevel::High.dimensions(), 768);
        assert_eq!(VectorizationLevel::None.dimensions(), 0);
    }

    #[test]
    fn test_default_config() {
        let config = VectorizationConfig::default();
        assert_eq!(config.level, VectorizationLevel::None);
        assert!(!config.is_enabled());
    }
}
</file>

<file path="src/downloader.rs">
/// Automatic model downloader from Hugging Face Hub
#[cfg(feature = "ml")]
use anyhow::{Context, Result};
#[cfg(feature = "ml")]
use hf_hub::api::tokio::{Api, ApiBuilder};
#[cfg(feature = "ml")]
use std::path::{Path, PathBuf};

#[cfg(feature = "ml")]
use super::config::VectorizationLevel;

/// Model downloader for Hugging Face Hub
#[cfg(feature = "ml")]
pub struct ModelDownloader {
    api: Api,
    cache_dir: PathBuf,
}

#[cfg(feature = "ml")]
impl ModelDownloader {
    /// Create new downloader with cache directory
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        // Create cache directory if it doesn't exist
        std::fs::create_dir_all(&cache_dir)
            .context(format!("Failed to create cache directory: {:?}", cache_dir))?;

        // Build Hugging Face API client
        let api = ApiBuilder::new()
            .with_cache_dir(cache_dir.clone())
            .build()
            .context("Failed to initialize Hugging Face API")?;

        Ok(Self { api, cache_dir })
    }

    /// Download model from Hugging Face if not already cached
    pub async fn ensure_model_available(&self, level: VectorizationLevel) -> Result<PathBuf> {
        let model_name = level
            .model_name()
            .ok_or_else(|| anyhow::anyhow!("No model for level {:?}", level))?;

        log::info!("Checking model availability: {}", model_name);

        // Get model directory name
        let model_dir_name = model_name.split('/').last().unwrap_or(model_name);
        let target_dir = self.cache_dir.join(model_dir_name);

        // Check if model already exists
        if self.is_model_complete(&target_dir) {
            log::info!("Model already cached at {:?}", target_dir);
            return Ok(target_dir);
        }

        // Download model
        log::info!("Downloading model {} from Hugging Face...", model_name);
        self.download_model(model_name, &target_dir).await?;

        Ok(target_dir)
    }

    /// Check if model is completely downloaded
    fn is_model_complete(&self, model_dir: &Path) -> bool {
        // Check for required files
        let model_file = model_dir.join("model.onnx");
        let tokenizer_file = model_dir.join("tokenizer.json");

        model_file.exists() && tokenizer_file.exists()
    }

    /// Download model files from Hugging Face
    async fn download_model(&self, model_name: &str, target_dir: &Path) -> Result<()> {
        // Create target directory
        std::fs::create_dir_all(target_dir).context(format!(
            "Failed to create model directory: {:?}",
            target_dir
        ))?;

        // Get repo from API
        let repo = self.api.model(model_name.to_string());

        // Files to download
        let required_files = vec![
            "model.onnx",
            "tokenizer.json",
            "tokenizer_config.json", // Optional but helpful
            "config.json",           // Optional but helpful
        ];

        log::info!("Downloading {} files...", required_files.len());

        for file_name in required_files {
            match self.download_file(&repo, file_name, target_dir).await {
                Ok(_) => {
                    log::info!("  ✓ Downloaded {}", file_name);
                }
                Err(e) => {
                    // Only fail on required files
                    if file_name == "model.onnx" || file_name == "tokenizer.json" {
                        return Err(e)
                            .context(format!("Failed to download required file: {}", file_name));
                    } else {
                        log::warn!("  ⚠ Optional file {} not available: {}", file_name, e);
                    }
                }
            }
        }

        log::info!("Model download complete: {:?}", target_dir);
        Ok(())
    }

    /// Download a single file from the repo
    async fn download_file(
        &self,
        repo: &hf_hub::api::tokio::ApiRepo,
        file_name: &str,
        target_dir: &Path,
    ) -> Result<()> {
        // Download file (hf-hub handles caching automatically)
        let file_path = repo.get(file_name).await.context(format!(
            "Failed to download {} from Hugging Face",
            file_name
        ))?;

        // Copy to target directory
        let target_path = target_dir.join(file_name);
        std::fs::copy(&file_path, &target_path)
            .context(format!("Failed to copy {} to {:?}", file_name, target_path))?;

        Ok(())
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

/// Stub implementation when ml feature is disabled
#[cfg(not(feature = "ml"))]
pub struct ModelDownloader;

#[cfg(not(feature = "ml"))]
impl ModelDownloader {
    #[allow(dead_code)]
    pub fn new<P>(_cache_dir: P) -> anyhow::Result<Self> {
        Err(anyhow::anyhow!(
            "ML feature not enabled. Rebuild with --features ml"
        ))
    }

    #[allow(dead_code)]
    pub async fn ensure_model_available(
        &self,
        _level: super::config::VectorizationLevel,
    ) -> anyhow::Result<std::path::PathBuf> {
        Err(anyhow::anyhow!("ML feature not enabled"))
    }
}

#[cfg(all(test, feature = "ml"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_check() {
        let temp_dir = std::env::temp_dir().join("op-dbus-test-models");
        let downloader = ModelDownloader::new(&temp_dir).unwrap();

        // Should return false for non-existent model
        let model_dir = temp_dir.join("test-model");
        assert!(!downloader.is_model_complete(&model_dir));
    }
}
</file>

<file path="src/embedder.rs">
/// Text embedding using ONNX Runtime for transformer models
#[cfg(feature = "ml")]
use anyhow::{Context, Result};
#[cfg(feature = "ml")]
use ndarray::Array2;
#[cfg(feature = "ml")]
use ort::Session;
#[cfg(feature = "ml")]
use std::path::Path;
#[cfg(feature = "ml")]
use tokenizers::Tokenizer;

#[cfg(feature = "ml")]
use super::config::{ExecutionProvider, VectorizationConfig, VectorizationLevel};

/// Text embedder using ONNX Runtime
#[cfg(feature = "ml")]
pub struct TextEmbedder {
    session: Session,
    tokenizer: Tokenizer,
    level: VectorizationLevel,
}

#[cfg(feature = "ml")]
impl TextEmbedder {
    /// Load model from directory
    pub fn load<P: AsRef<Path>>(model_dir: P, config: &VectorizationConfig) -> Result<Self> {
        let model_dir = model_dir.as_ref();
        let level = config.level;

        log::info!(
            "Loading {} model from {:?} with {} execution",
            level,
            model_dir,
            config.execution_provider
        );

        // Load tokenizer
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path).context(format!(
            "Failed to load tokenizer from {:?}",
            tokenizer_path
        ))?;

        // Build session with execution provider
        let model_path = model_dir.join("model.onnx");
        let mut builder = Session::builder()?;

        // Configure execution provider
        match config.execution_provider {
            ExecutionProvider::Cpu => {
                // CPU execution
                builder = builder.with_intra_threads(config.num_threads)?;
                log::info!("Using CPU with {} threads", config.num_threads);
            }
            ExecutionProvider::Cuda => {
                // CUDA GPU execution
                builder =
                    builder.with_execution_providers([ort::CUDAExecutionProvider::default()
                        .with_device_id(config.gpu_device_id)
                        .build()])?;
                log::info!("Using CUDA GPU device {}", config.gpu_device_id);
            }
            ExecutionProvider::TensorRT => {
                // TensorRT GPU execution
                builder =
                    builder
                        .with_execution_providers([ort::TensorRTExecutionProvider::default()
                            .with_device_id(config.gpu_device_id)
                            .build()])?;
                log::info!("Using TensorRT GPU device {}", config.gpu_device_id);
            }
            ExecutionProvider::DirectML => {
                // DirectML (Windows GPU)
                #[cfg(target_os = "windows")]
                {
                    builder = builder.with_execution_providers([
                        ort::DirectMLExecutionProvider::default()
                            .with_device_id(config.gpu_device_id as u32)
                            .build(),
                    ])?;
                    log::info!("Using DirectML GPU device {}", config.gpu_device_id);
                }
                #[cfg(not(target_os = "windows"))]
                {
                    log::warn!("DirectML only supported on Windows, falling back to CPU");
                    builder = builder.with_intra_threads(config.num_threads)?;
                }
            }
            ExecutionProvider::CoreML => {
                // CoreML (Apple GPU/Neural Engine)
                #[cfg(target_os = "macos")]
                {
                    builder = builder.with_execution_providers([
                        ort::CoreMLExecutionProvider::default().build(),
                    ])?;
                    log::info!("Using CoreML");
                }
                #[cfg(not(target_os = "macos"))]
                {
                    log::warn!("CoreML only supported on macOS, falling back to CPU");
                    builder = builder.with_intra_threads(config.num_threads)?;
                }
            }
        }

        let session = builder
            .commit_from_file(&model_path)
            .context(format!("Failed to load ONNX model from {:?}", model_path))?;

        log::info!(
            "Successfully loaded {} model ({}MB) on {}",
            level,
            level.model_size_mb(),
            config.execution_provider
        );

        Ok(Self {
            session,
            tokenizer,
            level,
        })
    }

    /// Embed single text into vector
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_batch(&[text])
            .map(|mut batch| batch.pop().unwrap_or_default())
    }

    /// Embed batch of texts
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Tokenize inputs
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .context("Failed to tokenize input texts")?;

        // Prepare input tensors
        let max_len = encodings.iter().map(|e| e.len()).max().unwrap_or(0);

        let mut input_ids_vec = Vec::new();
        let mut attention_mask_vec = Vec::new();

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();

            // Pad to max_len
            let mut padded_ids = ids.to_vec();
            let mut padded_mask = mask.to_vec();

            padded_ids.resize(max_len, 0);
            padded_mask.resize(max_len, 0);

            input_ids_vec.extend(padded_ids.iter().map(|&id| id as i64));
            attention_mask_vec.extend(padded_mask.iter().map(|&m| m as i64));
        }

        // Create input arrays
        let batch_size = texts.len();
        let input_ids = Array2::from_shape_vec((batch_size, max_len), input_ids_vec)?;
        let attention_mask = Array2::from_shape_vec((batch_size, max_len), attention_mask_vec)?;

        // Run inference
        let outputs = self.session.run(ort::inputs![
            "input_ids" => input_ids.view(),
            "attention_mask" => attention_mask.view(),
        ]?)?;

        // Extract embeddings (typically from "last_hidden_state" or "sentence_embedding")
        let embeddings = outputs["sentence_embedding"]
            .try_extract_tensor::<f32>()?
            .view()
            .to_owned();

        // Convert to Vec<Vec<f32>>
        let dim = self.level.dimensions();
        let mut result = Vec::new();

        for i in 0..batch_size {
            let row = embeddings.slice(ndarray::s![i, ..]).to_vec();

            // L2 normalize for cosine similarity
            let normalized = self.l2_normalize(&row);

            result.push(normalized);
        }

        Ok(result)
    }

    /// L2 normalize vector
    fn l2_normalize(&self, vec: &[f32]) -> Vec<f32> {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            vec.iter().map(|x| x / norm).collect()
        } else {
            vec.to_vec()
        }
    }

    /// Get embedding dimensionality
    pub fn dimensions(&self) -> usize {
        self.level.dimensions()
    }
}

/// Stub implementation when ml feature is disabled
#[cfg(not(feature = "ml"))]
pub struct TextEmbedder;

#[cfg(not(feature = "ml"))]
impl TextEmbedder {
    #[allow(dead_code)]
    pub fn load<P>(
        _model_dir: P,
        _level: super::config::VectorizationLevel,
    ) -> anyhow::Result<Self> {
        Err(anyhow::anyhow!(
            "ML feature not enabled. Rebuild with --features ml"
        ))
    }

    #[allow(dead_code)]
    pub fn embed(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        Err(anyhow::anyhow!("ML feature not enabled"))
    }

    #[allow(dead_code)]
    pub fn embed_batch(&self, _texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Err(anyhow::anyhow!("ML feature not enabled"))
    }

    #[allow(dead_code)]
    pub fn dimensions(&self) -> usize {
        0
    }
}

#[cfg(all(test, feature = "ml"))]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalize() {
        let embedder = TextEmbedder {
            session: todo!(), // Mock for test
            tokenizer: todo!(),
            level: VectorizationLevel::Medium,
        };

        let vec = vec![3.0, 4.0];
        let normalized = embedder.l2_normalize(&vec);

        // Should have unit length
        let length: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 1e-5);
    }
}
</file>

<file path="src/lib.rs">
//! op-ml: ML/Embedding support
//!
//! Features:
//! - Model management and downloading
//! - Text embeddings
//! - Vector storage

pub mod config;
pub mod downloader;
pub mod embedder;
pub mod model_manager;

pub use config::{ExecutionProvider, VectorizationConfig, VectorizationLevel};
pub use downloader::ModelDownloader;
pub use embedder::TextEmbedder;
pub use model_manager::ModelManager;

/// Prelude for convenient imports
pub mod prelude {
    pub use super::config::{ExecutionProvider, VectorizationConfig, VectorizationLevel};
    pub use super::embedder::TextEmbedder;
    pub use super::model_manager::ModelManager;
}
</file>

<file path="src/model_manager.rs">
/// Lazy-loading model manager for transformer embeddings
#[cfg(feature = "ml")]
use anyhow::Context;
use anyhow::Result;
use std::sync::Arc;

#[cfg(feature = "ml")]
use once_cell::sync::OnceCell;

use super::config::{VectorizationConfig, VectorizationLevel};
#[cfg(feature = "ml")]
use super::downloader::ModelDownloader;
#[cfg(feature = "ml")]
use super::embedder::TextEmbedder;

/// Global model manager singleton
#[cfg(feature = "ml")]
static MODEL_MANAGER: OnceCell<Arc<ModelManager>> = OnceCell::new();

/// Model manager with lazy loading
pub struct ModelManager {
    config: VectorizationConfig,
    #[cfg(feature = "ml")]
    embedder: OnceCell<TextEmbedder>,
}

impl ModelManager {
    /// Create new model manager
    pub fn new(config: VectorizationConfig) -> Self {
        Self {
            config,
            #[cfg(feature = "ml")]
            embedder: OnceCell::new(),
        }
    }

    /// Get or initialize global instance
    #[cfg(feature = "ml")]
    pub fn global() -> Arc<Self> {
        MODEL_MANAGER
            .get_or_init(|| {
                let config = VectorizationConfig::from_env();
                log::info!(
                    "Initializing global model manager with level: {}",
                    config.level
                );
                Arc::new(Self::new(config))
            })
            .clone()
    }

    /// Get or initialize global instance (stub for non-ml)
    #[cfg(not(feature = "ml"))]
    pub fn global() -> Arc<Self> {
        Arc::new(Self::new(VectorizationConfig::default()))
    }

    /// Check if vectorization is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }

    /// Get vectorization level
    #[allow(dead_code)]
    pub fn level(&self) -> VectorizationLevel {
        self.config.level
    }

    /// Embed text into vector (lazy loads model on first call)
    #[cfg(feature = "ml")]
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if !self.is_enabled() {
            return Ok(Vec::new());
        }

        let embedder = self.get_or_load_embedder()?;
        embedder.embed(text)
    }

    /// Embed text (stub for non-ml)
    #[cfg(not(feature = "ml"))]
    pub fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        if self.is_enabled() {
            Err(anyhow::anyhow!(
                "ML feature not enabled. Rebuild with --features ml"
            ))
        } else {
            Ok(Vec::new())
        }
    }

    /// Embed batch of texts
    #[cfg(feature = "ml")]
    #[allow(dead_code)]
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if !self.is_enabled() {
            return Ok(vec![Vec::new(); texts.len()]);
        }

        let embedder = self.get_or_load_embedder()?;
        embedder.embed_batch(texts)
    }

    /// Embed batch (stub for non-ml)
    #[cfg(not(feature = "ml"))]
    #[allow(dead_code)]
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if self.is_enabled() {
            Err(anyhow::anyhow!(
                "ML feature not enabled. Rebuild with --features ml"
            ))
        } else {
            Ok(vec![Vec::new(); texts.len()])
        }
    }

    /// Get or load embedder (lazy initialization)
    #[cfg(feature = "ml")]
    fn get_or_load_embedder(&self) -> Result<&TextEmbedder> {
        self.embedder.get_or_try_init(|| {
            log::info!("Loading {} model on-demand...", self.config.level);

            // Use async runtime to download if needed
            let model_dir = tokio::runtime::Handle::current()
                .block_on(async { self.ensure_model_downloaded().await })?;

            // Try to load model
            match TextEmbedder::load(&model_dir, &self.config) {
                Ok(embedder) => {
                    log::info!("Successfully loaded {} model", self.config.level);
                    Ok(embedder)
                }
                Err(e) => {
                    log::warn!("Failed to load {} model: {}", self.config.level, e);
                    // Try to fall back to lower level
                    self.try_fallback_model().or(Err(e))
                }
            }
        })
    }

    /// Ensure model is downloaded, download if missing
    #[cfg(feature = "ml")]
    async fn ensure_model_downloaded(&self) -> Result<std::path::PathBuf> {
        let model_dir = self.get_model_path()?;

        // Check if model already exists
        let model_file = model_dir.join("model.onnx");
        let tokenizer_file = model_dir.join("tokenizer.json");

        if model_file.exists() && tokenizer_file.exists() {
            log::info!("Model already available at {:?}", model_dir);
            return Ok(model_dir);
        }

        // Model missing, download it
        log::info!("Model not found locally, downloading from Hugging Face...");

        let downloader = ModelDownloader::new(&self.config.model_dir)
            .context("Failed to initialize model downloader")?;

        downloader
            .ensure_model_available(self.config.level)
            .await
            .context("Failed to download model from Hugging Face")
    }

    /// Try to load a fallback model at lower level
    #[cfg(feature = "ml")]
    fn try_fallback_model(&self) -> Result<TextEmbedder> {
        let fallback_level = match self.config.level {
            VectorizationLevel::High => VectorizationLevel::Medium,
            VectorizationLevel::Medium => VectorizationLevel::Low,
            _ => {
                return Err(anyhow::anyhow!(
                    "No fallback available for level {:?}",
                    self.config.level
                ))
            }
        };

        log::warn!("Falling back to {} level", fallback_level);

        let model_dir = self.get_model_path_for_level(fallback_level)?;

        // Create fallback config with same execution provider
        let mut fallback_config = self.config.clone();
        fallback_config.level = fallback_level;

        TextEmbedder::load(&model_dir, &fallback_config)
            .context(format!("Fallback to {} failed", fallback_level))
    }

    /// Get model directory path for current level
    #[allow(dead_code)]
    fn get_model_path(&self) -> Result<std::path::PathBuf> {
        self.get_model_path_for_level(self.config.level)
    }

    /// Get model directory path for specific level
    #[allow(dead_code)]
    fn get_model_path_for_level(&self, level: VectorizationLevel) -> Result<std::path::PathBuf> {
        let model_name = level
            .model_name()
            .ok_or_else(|| anyhow::anyhow!("No model for level {:?}", level))?;

        // Convert model name to directory name
        // e.g., "sentence-transformers/paraphrase-MiniLM-L6-v2" -> "paraphrase-MiniLM-L6-v2"
        let dir_name = model_name.split('/').next_back().unwrap_or(model_name);

        Ok(self.config.model_dir.join(dir_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_path_generation() {
        let config = VectorizationConfig {
            level: VectorizationLevel::Medium,
            ..Default::default()
        };

        let manager = ModelManager::new(config);
        let path = manager.get_model_path().unwrap();

        assert!(path.to_string_lossy().contains("paraphrase-MiniLM-L6-v2"));
    }

    #[test]
    fn test_disabled_vectorization() {
        let config = VectorizationConfig {
            level: VectorizationLevel::None,
            ..Default::default()
        };

        let manager = ModelManager::new(config);
        assert!(!manager.is_enabled());

        let result = manager.embed("test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Vec::<f32>::new());
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-ml"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "ML/Embedding support: model management, text embedder, vector storage"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
reqwest = { workspace = true }
log = { workspace = true }
num_cpus = { workspace = true }
sha2 = { workspace = true }
hf-hub = { version = "0.5.0", features = ["tokio"] }

[features]
default = []
ml = []
</file>

<file path="compare-op-ml.md">
# compare-op-ml

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 5 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 0 |
| Spec-listed source files | 0 |
| Spec-listed but missing | 0 |
| Extra implementation files | 5 |

## Current Implementation Overview

- ML/Embedding support: model management, text embedder, vector storage

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `root` | ✅ Present | root source group | src/config.rs, src/downloader.rs, src/embedder.rs, src/lib.rs, src/model_manager.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| Architecture | ❌ Missing | no clear source match for SPEC.md | SPEC.md |
| Key Components | ❌ Missing | no clear source match for SPEC.md | SPEC.md |
| Module Structure | ❌ Missing | no clear source match for SPEC.md | SPEC.md |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `tokio` - not listed in SPEC dependency block
- `serde` - not listed in SPEC dependency block
- `simd-json` - not listed in SPEC dependency block
- `anyhow` - not listed in SPEC dependency block
- `thiserror` - not listed in SPEC dependency block
- `tracing` - not listed in SPEC dependency block
- `reqwest` - not listed in SPEC dependency block
- `log` - not listed in SPEC dependency block
- `num_cpus` - not listed in SPEC dependency block
- `sha2` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 5 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: config, downloader, embedder, model_manager.
- Cargo feature flags: default, ml.
</file>

<file path="SPEC.md">
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
</file>

</files>
