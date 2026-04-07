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
