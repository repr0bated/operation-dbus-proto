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
