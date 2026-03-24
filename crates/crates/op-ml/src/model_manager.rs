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
