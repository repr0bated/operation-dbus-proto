use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateStoreError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] simd_json::Error),
    #[error("Job not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StateStoreError>;
