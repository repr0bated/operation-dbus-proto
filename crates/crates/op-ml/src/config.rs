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
