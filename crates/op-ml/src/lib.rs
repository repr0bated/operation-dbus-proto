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
