//! Error types for OP-DBUS
//!
//! Provides a comprehensive error type for all OP-DBUS operations.

use thiserror::Error;

/// Main error type for OP-DBUS operations
#[derive(Error, Debug)]
pub enum OpDbusError {
    #[error("DBus error: {0}")]
    Dbus(#[from] zbus::Error),

    #[error("DBus FDO error: {0}")]
    DbusFdo(#[from] zbus::fdo::Error),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Introspection error: {0}")]
    Introspection(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Tool already registered: {0}")]
    ToolAlreadyRegistered(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("External service error: {0}")]
    ExternalServiceError(String),

    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("State store error: {0}")]
    StateStore(#[from] op_state_store::StateStoreError),

    #[error("State error: {0}")]
    StateError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("General error: {0}")]
    General(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid work stack: {0}")]
    InvalidWorkStack(String),
}

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, OpDbusError>;

impl OpDbusError {
    /// Create a connection error
    pub fn connection(msg: impl Into<String>) -> Self {
        OpDbusError::Connection(msg.into())
    }

    /// Create an introspection error
    pub fn introspection(msg: impl Into<String>) -> Self {
        OpDbusError::Introspection(msg.into())
    }

    /// Create a tool execution error
    pub fn tool_execution(msg: impl Into<String>) -> Self {
        OpDbusError::ToolExecution(msg.into())
    }

    /// Create a plugin error
    pub fn plugin(msg: impl Into<String>) -> Self {
        OpDbusError::Plugin(msg.into())
    }

    /// Create an agent error
    pub fn agent(msg: impl Into<String>) -> Self {
        OpDbusError::Agent(msg.into())
    }

    /// Create a not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        OpDbusError::NotFound(msg.into())
    }

    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        OpDbusError::ConfigError(msg.into())
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        OpDbusError::Internal(msg.into())
    }
}

// Note: tokio::io::Error is actually std::io::Error, so we don't need a separate impl

impl From<anyhow::Error> for OpDbusError {
    fn from(err: anyhow::Error) -> Self {
        OpDbusError::General(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = OpDbusError::connection("Failed to connect");
        assert_eq!(err.to_string(), "Connection error: Failed to connect");
    }

    #[test]
    fn test_error_constructors() {
        let _ = OpDbusError::tool_execution("Tool failed");
        let _ = OpDbusError::not_found("Resource missing");
        let _ = OpDbusError::config("Invalid config");
    }
}
