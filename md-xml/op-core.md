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
  connection.rs
  error.rs
  execution.rs
  lib.rs
  lib.rs.patch
  message.rs
  security.rs
  self_identity.rs
  state_publisher.rs
  types.rs
Cargo.toml
compare-op-core.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/config.rs">
//! Environment Configuration Loader
//!
//! Loads environment variables from the canonical location: `/etc/op-dbus/environment`
//! This ensures all op-dbus components share the same configuration.
//!
//! ## Usage
//!
//! Call `load_environment()` early in main() before accessing any config:
//!
//! ```rust
//! use op_core::config::load_environment;
//!
//! fn main() {
//!     load_environment();
//!     // Now all env vars from /etc/op-dbus/environment are available
//! }
//! ```

use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

/// Default path for the environment file
pub const DEFAULT_ENV_FILE: &str = "/etc/op-dbus/environment";

/// Alternative paths to check (in order of priority)
pub const ENV_FILE_PATHS: &[&str] = &["/etc/op-dbus/environment", "/etc/op-dbus.env", ".env"];

/// Load environment variables from the canonical configuration file.
///
/// This function:
/// 1. Checks `/etc/op-dbus/environment` first (system-wide)
/// 2. Falls back to `.env` in current directory (development)
/// 3. Does NOT override existing environment variables
///
/// Returns the path that was loaded, or None if no file was found.
pub fn load_environment() -> Option<String> {
    // Check if a custom path is specified
    if let Ok(custom_path) = std::env::var("OP_ENV_FILE") {
        if let Some(path) = try_load_env_file(&custom_path) {
            return Some(path);
        }
    }

    // Try each path in order
    for path in ENV_FILE_PATHS {
        if let Some(loaded_path) = try_load_env_file(path) {
            return Some(loaded_path);
        }
    }

    debug!("No environment file found, using existing environment");
    None
}

/// Try to load an environment file from the given path.
fn try_load_env_file(path: &str) -> Option<String> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return None;
    }

    match fs::read_to_string(path_obj) {
        Ok(content) => {
            let mut loaded_count = 0;
            let mut skipped_count = 0;

            for line in content.lines() {
                let line = line.trim();

                // Skip comments and empty lines
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                // Parse KEY=VALUE
                if let Some((key, value)) = parse_env_line(line) {
                    // Don't override existing environment variables
                    if std::env::var(&key).is_err() {
                        std::env::set_var(&key, &value);
                        loaded_count += 1;
                        debug!(
                            "Loaded: {}={}",
                            key,
                            if key.contains("KEY")
                                || key.contains("TOKEN")
                                || key.contains("SECRET")
                            {
                                "***"
                            } else {
                                &value
                            }
                        );
                    } else {
                        skipped_count += 1;
                        debug!("Skipped (already set): {}", key);
                    }
                }
            }

            info!(
                "Loaded {} environment variables from {} ({} skipped - already set)",
                loaded_count, path, skipped_count
            );

            Some(path.to_string())
        }
        Err(e) => {
            warn!("Failed to read environment file {}: {}", path, e);
            None
        }
    }
}

/// Parse a single environment line into key-value pair.
fn parse_env_line(line: &str) -> Option<(String, String)> {
    // Handle: KEY=VALUE, KEY="VALUE", KEY='VALUE'
    let mut parts = line.splitn(2, '=');
    let key = parts.next()?.trim();
    let value = parts.next()?.trim();

    if key.is_empty() {
        return None;
    }

    // Remove surrounding quotes
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value);

    Some((key.to_string(), value.to_string()))
}

/// Get a configuration value with a default.
pub fn get_config(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Get an optional configuration value.
pub fn get_config_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// Get a boolean configuration value.
pub fn get_config_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"))
        .unwrap_or(default)
}

/// Get an integer configuration value.
pub fn get_config_int(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_line_simple() {
        let (k, v) = parse_env_line("FOO=bar").unwrap();
        assert_eq!(k, "FOO");
        assert_eq!(v, "bar");
    }

    #[test]
    fn test_parse_env_line_quoted() {
        let (k, v) = parse_env_line("FOO=\"bar baz\"").unwrap();
        assert_eq!(k, "FOO");
        assert_eq!(v, "bar baz");
    }

    #[test]
    fn test_parse_env_line_single_quoted() {
        let (k, v) = parse_env_line("FOO='bar'").unwrap();
        assert_eq!(k, "FOO");
        assert_eq!(v, "bar");
    }

    #[test]
    fn test_parse_env_line_empty() {
        assert!(parse_env_line("").is_none());
        assert!(parse_env_line("=value").is_none());
    }
}
</file>

<file path="src/connection.rs">
//! DBus connection management

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::Connection;

use crate::error::{Error, Result};
use crate::types::BusType;

/// Configuration for DBus connections
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Whether to auto-reconnect on connection loss
    pub auto_reconnect: bool,
    /// Connection timeout in milliseconds
    pub timeout_ms: u64,
    /// Maximum retry attempts for connection
    pub max_retries: u32,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            timeout_ms: 30000,
            max_retries: 3,
        }
    }
}

/// DBus connection manager
///
/// Manages connections to both system and session buses with
/// automatic reconnection support.
pub struct DbusConnection {
    system: Arc<RwLock<Option<Connection>>>,
    session: Arc<RwLock<Option<Connection>>>,
    config: ConnectionConfig,
}

impl DbusConnection {
    /// Create a new DBus connection manager
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            system: Arc::new(RwLock::new(None)),
            session: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ConnectionConfig::default())
    }

    /// Connect to the system bus
    pub async fn connect_system(&self) -> Result<Connection> {
        let mut conn = self.system.write().await;

        if let Some(ref existing) = *conn {
            // Check if connection is still valid by attempting to get the unique name
            if existing.unique_name().is_some() {
                debug!("Reusing existing system bus connection");
                return Ok(existing.clone());
            }
            warn!("System bus connection lost, reconnecting...");
        }

        let new_conn = self.try_connect(BusType::System).await?;
        *conn = Some(new_conn.clone());
        info!("Connected to system bus");
        Ok(new_conn)
    }

    /// Connect to the session bus
    pub async fn connect_session(&self) -> Result<Connection> {
        let mut conn = self.session.write().await;

        if let Some(ref existing) = *conn {
            // Check if connection is still valid by attempting to get the unique name
            if existing.unique_name().is_some() {
                debug!("Reusing existing session bus connection");
                return Ok(existing.clone());
            }
            warn!("Session bus connection lost, reconnecting...");
        }

        let new_conn = self.try_connect(BusType::Session).await?;
        *conn = Some(new_conn.clone());
        info!("Connected to session bus");
        Ok(new_conn)
    }

    /// Get connection for specified bus type
    pub async fn get(&self, bus_type: BusType) -> Result<Connection> {
        match bus_type {
            BusType::System => self.connect_system().await,
            BusType::Session => self.connect_session().await,
        }
    }

    /// Check if system bus is connected
    pub async fn is_system_connected(&self) -> bool {
        let conn = self.system.read().await;
        conn.as_ref().is_some_and(|c| c.unique_name().is_some())
    }

    /// Check if session bus is connected
    pub async fn is_session_connected(&self) -> bool {
        let conn = self.session.read().await;
        conn.as_ref().is_some_and(|c| c.unique_name().is_some())
    }

    /// Disconnect from all buses
    pub async fn disconnect(&self) {
        let mut system = self.system.write().await;
        let mut session = self.session.write().await;

        *system = None;
        *session = None;
        info!("Disconnected from all DBus connections");
    }

    /// Try to establish connection with retries
    async fn try_connect(&self, bus_type: BusType) -> Result<Connection> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            debug!("Connection attempt {} for {:?} bus", attempt, bus_type);

            let result = match bus_type {
                BusType::System => Connection::system().await,
                BusType::Session => Connection::session().await,
            };

            match result {
                Ok(conn) => return Ok(conn),
                Err(e) => {
                    warn!("Connection attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        // Exponential backoff
                        let delay = std::time::Duration::from_millis(100 * 2u64.pow(attempt - 1));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(Error::Connection(format!(
            "Failed to connect to {:?} bus after {} attempts: {:?}",
            bus_type, self.config.max_retries, last_error
        )))
    }
}

impl Default for DbusConnection {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl Clone for DbusConnection {
    fn clone(&self) -> Self {
        Self {
            system: Arc::clone(&self.system),
            session: Arc::clone(&self.session),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_config_default() {
        let config = ConnectionConfig::default();
        assert!(config.auto_reconnect);
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.max_retries, 3);
    }
}
</file>

<file path="src/error.rs">
//! Error types for op-dbus-v2

use thiserror::Error;

/// Main error type for op-dbus operations
#[derive(Error, Debug)]
pub enum Error {
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

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] simd_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a connection error
    pub fn connection(msg: impl Into<String>) -> Self {
        Error::Connection(msg.into())
    }

    /// Create an introspection error
    pub fn introspection(msg: impl Into<String>) -> Self {
        Error::Introspection(msg.into())
    }

    /// Create a tool execution error
    pub fn tool_execution(msg: impl Into<String>) -> Self {
        Error::ToolExecution(msg.into())
    }

    /// Create a plugin error
    pub fn plugin(msg: impl Into<String>) -> Self {
        Error::Plugin(msg.into())
    }

    /// Create an agent error
    pub fn agent(msg: impl Into<String>) -> Self {
        Error::Agent(msg.into())
    }

    /// Create a not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        Error::NotFound(msg.into())
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Error::Internal(msg.into())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Internal(err.to_string())
    }
}
</file>

<file path="src/execution.rs">
//! Execution Tracking for Tool and Agent Operations
//!
//! Provides accountability and audit trail for all tool executions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Request received, not yet started
    Pending,
    /// Currently executing
    Running,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Cancelled by user or system
    Cancelled,
    /// Timed out
    Timeout,
}

/// Record of a single tool/agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique execution ID
    pub id: String,
    /// Trace ID for correlation across systems
    pub trace_id: String,
    /// Tool or agent name
    pub tool_name: String,
    /// Input arguments (sanitized)
    pub input_summary: Option<simd_json::OwnedValue>,
    /// Execution status
    pub status: ExecutionStatus,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// End time (if completed)
    pub ended_at: Option<DateTime<Utc>>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Output summary (truncated if large)
    pub output_summary: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Whether execution was successful
    pub success: bool,
    /// User/session that initiated execution
    pub initiated_by: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ExecutionRecord {
    /// Create a new execution record
    pub fn new(tool_name: &str, trace_id: Option<String>) -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            id: id.clone(),
            trace_id: trace_id.unwrap_or_else(|| id.clone()),
            tool_name: tool_name.to_string(),
            input_summary: None,
            status: ExecutionStatus::Pending,
            started_at: Utc::now(),
            ended_at: None,
            duration_ms: None,
            output_summary: None,
            error: None,
            success: false,
            initiated_by: None,
            metadata: HashMap::new(),
        }
    }

    /// Mark as running
    pub fn start(&mut self) {
        self.status = ExecutionStatus::Running;
        self.started_at = Utc::now();
    }

    /// Mark as completed successfully
    pub fn complete(&mut self, output: Option<String>) {
        let now = Utc::now();
        self.ended_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
        self.status = ExecutionStatus::Completed;
        self.success = true;
        self.output_summary = output.map(|s| truncate_string(&s, 1000));
    }

    /// Mark as failed
    pub fn fail(&mut self, error: String) {
        let now = Utc::now();
        self.ended_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
        self.status = ExecutionStatus::Failed;
        self.success = false;
        self.error = Some(error);
    }

    /// Mark as timed out
    pub fn timeout(&mut self) {
        let now = Utc::now();
        self.ended_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
        self.status = ExecutionStatus::Timeout;
        self.success = false;
        self.error = Some("Execution timed out".to_string());
    }
}

/// Truncate string to max length
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}... (truncated)", &s[..max_len])
    }
}

/// Execution tracker - maintains history of all executions
#[derive(Clone)]
pub struct ExecutionTracker {
    /// Recent executions (ring buffer)
    records: Arc<RwLock<Vec<ExecutionRecord>>>,
    /// Maximum records to keep
    max_records: usize,
    /// Statistics
    stats: Arc<RwLock<ExecutionStats>>,
}

/// Execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub total_duration_ms: u64,
    pub executions_by_tool: HashMap<String, u64>,
    pub failures_by_tool: HashMap<String, u64>,
}

impl ExecutionStats {
    pub fn average_duration_ms(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_executions as f64
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.successful_executions as f64 / self.total_executions as f64 * 100.0
        }
    }
}

impl ExecutionTracker {
    /// Create a new tracker
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::with_capacity(max_records))),
            max_records,
            stats: Arc::new(RwLock::new(ExecutionStats::default())),
        }
    }

    /// Start tracking a new execution
    pub async fn start_execution(
        &self,
        tool_name: &str,
        input: Option<simd_json::OwnedValue>,
        initiated_by: Option<String>,
    ) -> ExecutionRecord {
        let mut record = ExecutionRecord::new(tool_name, None);
        record.input_summary = input;
        record.initiated_by = initiated_by;
        record.start();

        let mut records = self.records.write().await;
        records.push(record.clone());

        // Trim if over limit
        if records.len() > self.max_records {
            records.remove(0);
        }

        tracing::info!(
            execution_id = %record.id,
            tool = %tool_name,
            "Execution started"
        );

        record
    }

    /// Complete an execution
    pub async fn complete_execution(&self, id: &str, output: Option<String>) {
        let mut records = self.records.write().await;
        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            record.complete(output);

            // Update stats
            let mut stats = self.stats.write().await;
            stats.total_executions += 1;
            stats.successful_executions += 1;
            if let Some(duration) = record.duration_ms {
                stats.total_duration_ms += duration;
            }
            *stats
                .executions_by_tool
                .entry(record.tool_name.clone())
                .or_insert(0) += 1;

            tracing::info!(
                execution_id = %id,
                tool = %record.tool_name,
                duration_ms = ?record.duration_ms,
                "Execution completed successfully"
            );
        }
    }

    /// Fail an execution
    pub async fn fail_execution(&self, id: &str, error: String) {
        let mut records = self.records.write().await;
        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            record.fail(error.clone());

            // Update stats
            let mut stats = self.stats.write().await;
            stats.total_executions += 1;
            stats.failed_executions += 1;
            if let Some(duration) = record.duration_ms {
                stats.total_duration_ms += duration;
            }
            *stats
                .executions_by_tool
                .entry(record.tool_name.clone())
                .or_insert(0) += 1;
            *stats
                .failures_by_tool
                .entry(record.tool_name.clone())
                .or_insert(0) += 1;

            tracing::error!(
                execution_id = %id,
                tool = %record.tool_name,
                error = %error,
                "Execution failed"
            );
        }
    }

    /// Get recent executions
    pub async fn get_recent(&self, limit: usize) -> Vec<ExecutionRecord> {
        let records = self.records.read().await;
        records.iter().rev().take(limit).cloned().collect()
    }

    /// Get execution by ID
    pub async fn get_execution(&self, id: &str) -> Option<ExecutionRecord> {
        let records = self.records.read().await;
        records.iter().find(|r| r.id == id).cloned()
    }

    /// Get executions for a specific tool
    pub async fn get_by_tool(&self, tool_name: &str, limit: usize) -> Vec<ExecutionRecord> {
        let records = self.records.read().await;
        records
            .iter()
            .filter(|r| r.tool_name == tool_name)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get statistics
    pub async fn get_stats(&self) -> ExecutionStats {
        self.stats.read().await.clone()
    }

    /// Get all pending/running executions
    pub async fn get_active(&self) -> Vec<ExecutionRecord> {
        let records = self.records.read().await;
        records
            .iter()
            .filter(|r| {
                r.status == ExecutionStatus::Running || r.status == ExecutionStatus::Pending
            })
            .cloned()
            .collect()
    }
}

impl Default for ExecutionTracker {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execution_tracking() {
        let tracker = ExecutionTracker::new(100);

        let record = tracker
            .start_execution("test_tool", Some(simd_json::json!({"arg": "value"})), None)
            .await;

        assert_eq!(record.status, ExecutionStatus::Running);
        assert_eq!(record.tool_name, "test_tool");

        tracker
            .complete_execution(&record.id, Some("success output".to_string()))
            .await;

        let completed = tracker.get_execution(&record.id).await.unwrap();
        assert_eq!(completed.status, ExecutionStatus::Completed);
        assert!(completed.success);
        assert!(completed.duration_ms.is_some());
    }

    #[tokio::test]
    async fn test_execution_failure() {
        let tracker = ExecutionTracker::new(100);

        let record = tracker.start_execution("failing_tool", None, None).await;

        tracker
            .fail_execution(&record.id, "Something went wrong".to_string())
            .await;

        let failed = tracker.get_execution(&record.id).await.unwrap();
        assert_eq!(failed.status, ExecutionStatus::Failed);
        assert!(!failed.success);
        assert_eq!(failed.error, Some("Something went wrong".to_string()));
    }

    #[tokio::test]
    async fn test_stats() {
        let tracker = ExecutionTracker::new(100);

        // Successful execution
        let r1 = tracker.start_execution("tool1", None, None).await;
        tracker.complete_execution(&r1.id, None).await;

        // Failed execution
        let r2 = tracker.start_execution("tool2", None, None).await;
        tracker.fail_execution(&r2.id, "error".to_string()).await;

        let stats = tracker.get_stats().await;
        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.successful_executions, 1);
        assert_eq!(stats.failed_executions, 1);
        assert!((stats.success_rate() - 50.0).abs() < 0.01);
    }
}
</file>

<file path="src/lib.rs">
//! Core types and utilities for op-dbus-v2
//!
//! # Modules
//!
//! - `config`: Configuration management
//! - `error`: Error types and Result alias
//! - `security`: Security types (IP access, permissions)
//! - `self_identity`: Self repository identification
//! - `types`: Common types used across op-dbus-v2

pub mod config;
pub mod error;
pub mod execution;
pub mod security;
pub mod self_identity;
pub mod state_publisher;
pub mod types;

// Re-exports
pub use error::{Error, Result};
pub use execution::{ExecutionRecord, ExecutionStats, ExecutionStatus, ExecutionTracker};
pub use security::{AccessZone, NetworkConfig, SecurityLevel};
pub use self_identity::{get_self_repo_path, SelfRepositoryInfo};
pub use types::*;
</file>

<file path="src/lib.rs.patch">
// Add to crates/op-core/src/lib.rs:

pub mod self_identity;
pub use self_identity::{get_self_repo_path, is_self_repo_configured, SelfRepositoryInfo};
</file>

<file path="src/message.rs">
//! Internal message types for actor communication

use crate::types::*;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

/// Message envelope for actor mailbox
#[derive(Debug)]
pub struct Message {
    pub id: String,
    pub kind: MessageKind,
    pub reply_to: Option<oneshot::Sender<Response>>,
}

impl Message {
    pub fn new(kind: MessageKind) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            reply_to: None,
        }
    }

    pub fn with_reply(kind: MessageKind, reply_to: oneshot::Sender<Response>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            reply_to: Some(reply_to),
        }
    }
}

/// Message types for the actor system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum MessageKind {
    // Chat messages
    Chat(ChatRequest),
    ChatStream(ChatStreamRequest),

    // Tool operations
    ListTools,
    ExecuteTool(ToolRequest),

    // Agent operations
    ListAgents,
    StartAgent(String),
    StopAgent(String),
    AgentStatus(String),

    // Introspection
    Introspect(IntrospectRequest),
    ListServices(BusType),

    // DBus operations
    DbusCall(DbusCallRequest),
    DbusGetProperty(DbusPropertyRequest),
    DbusSetProperty(DbusPropertySetRequest),

    // System
    Health,
    Shutdown,

    // Plugin operations
    ListPlugins,
    LoadPlugin(String),
    UnloadPlugin(String),
}

/// Chat request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// Streaming chat request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Introspection request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectRequest {
    pub bus_type: BusType,
    pub service: String,
    #[serde(default)]
    pub path: Option<String>,
}

/// DBus method call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbusCallRequest {
    pub bus_type: BusType,
    pub destination: String,
    pub path: String,
    pub interface: String,
    pub method: String,
    #[serde(default)]
    pub args: Vec<simd_json::OwnedValue>,
}

/// DBus property get request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbusPropertyRequest {
    pub bus_type: BusType,
    pub destination: String,
    pub path: String,
    pub interface: String,
    pub property: String,
}

/// DBus property set request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbusPropertySetRequest {
    pub bus_type: BusType,
    pub destination: String,
    pub path: String,
    pub interface: String,
    pub property: String,
    pub value: simd_json::OwnedValue,
    pub signature: String,
}

/// Response from actor operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Response {
    Success(simd_json::OwnedValue),
    Error { code: String, message: String },

    // Specific responses
    Tools(Vec<ToolDefinition>),
    ToolResult(ToolResult),

    Agents(Vec<AgentDefinition>),
    AgentStatus(AgentStatus),

    Services(Vec<ServiceInfo>),
    Introspection(ObjectInfo),

    Chat(ChatMessage),

    Health(HealthStatus),

    Plugins(Vec<PluginInfo>),

    Ack,
}

impl Response {
    pub fn success(value: impl Serialize) -> Self {
        Response::Success(simd_json::serde::to_owned_value(value).unwrap_or(simd_json::OwnedValue::Null))
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Response::Error {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn is_success(&self) -> bool {
        !matches!(self, Response::Error { .. })
    }
}

/// Plugin information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub tools: Vec<String>,
}
</file>

<file path="src/security.rs">
//! Core security types and access control logic
//!
//! Provides IP-based access zones and security levels used across the system.

use serde::{Deserialize, Serialize};

/// Security level for resources/tools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SecurityLevel {
    /// Safe, read-only operations - any IP
    #[default]
    Public,
    /// Normal operations - any IP
    Standard,
    /// System modifications - localhost or private network
    Elevated,
    /// Dangerous commands - localhost only
    Restricted,
}

impl SecurityLevel {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "public" => Self::Public,
            "standard" => Self::Standard,
            "elevated" => Self::Elevated,
            "restricted" => Self::Restricted,
            _ => Self::Standard,
        }
    }
}

/// IP-based access control zones
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccessZone {
    /// 127.0.0.1, ::1 - full access to everything
    Localhost,
    /// Trusted VPN/mesh networks (Netmaker, Tailscale, etc.) - full access
    TrustedMesh,
    /// 192.168.x.x, 10.x.x.x, 172.16-31.x.x - elevated access
    PrivateNetwork,
    /// Public IPs - restricted to safe tools only
    #[default]
    Public,
}

impl AccessZone {
    /// Detect access zone from IP address string
    pub fn from_ip(ip: &str) -> Self {
        Self::from_ip_with_config(ip, &NetworkConfig::default())
    }

    /// Detect access zone with custom network configuration
    pub fn from_ip_with_config(ip: &str, config: &NetworkConfig) -> Self {
        let ip = ip.trim();

        // 1. Localhost - always full access
        if Self::is_localhost(ip) {
            return Self::Localhost;
        }

        // 2. Check custom trusted networks first (from config or env)
        if config.is_trusted(ip) {
            return Self::TrustedMesh;
        }

        // 3. Known VPN/mesh networks - auto-detect common ranges
        if Self::is_mesh_network(ip) {
            return Self::TrustedMesh;
        }

        // 4. Standard private networks (RFC 1918)
        if Self::is_private_network(ip) {
            return Self::PrivateNetwork;
        }

        Self::Public
    }

    fn is_localhost(ip: &str) -> bool {
        ip == "127.0.0.1" || ip == "::1" || ip == "localhost" || ip.starts_with("127.")
    }

    /// Check for known VPN/mesh network ranges
    fn is_mesh_network(ip: &str) -> bool {
        // Netmaker default ranges (commonly 10.x.x.x but checking specific patterns)
        // Netmaker often uses: 10.101.0.0/16, 10.102.0.0/16, etc.
        if ip.starts_with("10.101.") || ip.starts_with("10.102.") || ip.starts_with("10.103.") {
            return true;
        }

        // Tailscale CGNAT range: 100.64.0.0/10 (100.64.x.x - 100.127.x.x)
        if let Some(first) = ip.split('.').next() {
            if first == "100" {
                if let Some(second) = ip.split('.').nth(1) {
                    if let Ok(n) = second.parse::<u8>() {
                        if (64..=127).contains(&n) {
                            return true;
                        }
                    }
                }
            }
        }

        // ZeroTier default range: often 10.147.x.x, 10.244.x.x
        if ip.starts_with("10.147.") || ip.starts_with("10.244.") {
            return true;
        }

        // WireGuard common ranges: often 10.0.0.x, 10.200.x.x, 10.66.66.x
        if ip.starts_with("10.0.0.") || ip.starts_with("10.200.") || ip.starts_with("10.66.66.") {
            return true;
        }

        // Nebula default: often 10.42.x.x
        if ip.starts_with("10.42.") {
            return true;
        }

        // IPv6 ULA for mesh (fd00::/8)
        if ip.starts_with("fd") {
            return true;
        }

        false
    }

    fn is_private_network(ip: &str) -> bool {
        // RFC 1918 private ranges
        if ip.starts_with("192.168.") || ip.starts_with("10.") {
            return true;
        }

        // 172.16.0.0 - 172.31.255.255
        if let Some(rest) = ip.strip_prefix("172.") {
            if let Some(second_octet) = rest.split('.').next() {
                if let Ok(n) = second_octet.parse::<u8>() {
                    if (16..=31).contains(&n) {
                        return true;
                    }
                }
            }
        }

        // IPv6 link-local
        if ip.starts_with("fe80") {
            return true;
        }

        false
    }

    /// Check if this zone can access a security level
    pub fn can_access(&self, level: SecurityLevel) -> bool {
        match (self, level) {
            // Localhost can access everything
            (Self::Localhost, _) => true,

            // Trusted mesh (Netmaker, Tailscale, etc.) - full access like localhost
            (Self::TrustedMesh, _) => true,

            // Private network: public, standard, elevated (not restricted)
            (Self::PrivateNetwork, SecurityLevel::Public) => true,
            (Self::PrivateNetwork, SecurityLevel::Standard) => true,
            (Self::PrivateNetwork, SecurityLevel::Elevated) => true,
            (Self::PrivateNetwork, SecurityLevel::Restricted) => false,

            // Public: only public and standard
            (Self::Public, SecurityLevel::Public) => true,
            (Self::Public, SecurityLevel::Standard) => true,
            (Self::Public, _) => false,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Localhost => "localhost (full access)",
            Self::TrustedMesh => "trusted mesh/VPN (full access)",
            Self::PrivateNetwork => "private network (elevated access)",
            Self::Public => "public network (limited access)",
        }
    }
}

/// Network configuration for trusted ranges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Custom trusted CIDR ranges (e.g., "10.50.0.0/16")
    #[serde(default)]
    pub trusted_cidrs: Vec<String>,

    /// Custom trusted IP prefixes (e.g., "10.50.")
    #[serde(default)]
    pub trusted_prefixes: Vec<String>,

    /// Exact trusted IPs
    #[serde(default)]
    pub trusted_ips: Vec<String>,

    /// Auto-detect Netmaker networks
    #[serde(default = "default_true")]
    pub auto_netmaker: bool,

    /// Auto-detect Tailscale networks  
    #[serde(default = "default_true")]
    pub auto_tailscale: bool,

    /// Auto-detect ZeroTier networks
    #[serde(default = "default_true")]
    pub auto_zerotier: bool,

    /// Auto-detect WireGuard common ranges
    #[serde(default = "default_true")]
    pub auto_wireguard: bool,
}

fn default_true() -> bool {
    true
}

impl Default for NetworkConfig {
    fn default() -> Self {
        // Also check environment variable for additional trusted networks
        let env_trusted: Vec<String> = std::env::var("OP_TRUSTED_NETWORKS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        Self {
            trusted_cidrs: env_trusted,
            trusted_prefixes: vec![],
            trusted_ips: vec![],
            auto_netmaker: true,
            auto_tailscale: true,
            auto_zerotier: true,
            auto_wireguard: true,
        }
    }
}

impl NetworkConfig {
    /// Create new network config
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a trusted CIDR range (e.g., "10.50.0.0/16")
    pub fn trust_cidr(mut self, cidr: &str) -> Self {
        self.trusted_cidrs.push(cidr.to_string());
        self
    }

    /// Add a trusted prefix (e.g., "10.50." for 10.50.x.x)
    pub fn trust_prefix(mut self, prefix: &str) -> Self {
        self.trusted_prefixes.push(prefix.to_string());
        self
    }

    /// Add a trusted IP
    pub fn trust_ip(mut self, ip: &str) -> Self {
        self.trusted_ips.push(ip.to_string());
        self
    }

    /// Add your Netmaker network range
    pub fn trust_netmaker(mut self, cidr: &str) -> Self {
        self.trusted_cidrs.push(cidr.to_string());
        self
    }

    /// Check if an IP is in trusted networks
    pub fn is_trusted(&self, ip: &str) -> bool {
        // Check exact IPs
        if self.trusted_ips.contains(&ip.to_string()) {
            return true;
        }

        // Check prefixes
        for prefix in &self.trusted_prefixes {
            if ip.starts_with(prefix) {
                return true;
            }
        }

        // Check CIDRs (simplified - just checks prefix for now)
        for cidr in &self.trusted_cidrs {
            if let Some(network) = cidr.split('/').next() {
                // Simple prefix match based on CIDR
                let prefix = Self::cidr_to_prefix(network, cidr);
                if ip.starts_with(&prefix) {
                    return true;
                }
            }
        }

        false
    }

    /// Convert CIDR to prefix for simple matching
    fn cidr_to_prefix(network: &str, cidr: &str) -> String {
        let mask: u8 = cidr
            .split('/')
            .nth(1)
            .and_then(|m| m.parse().ok())
            .unwrap_or(24);

        let octets: Vec<&str> = network.split('.').collect();

        match mask {
            0..=8 => octets
                .first()
                .map(|s| format!("{}.", s))
                .unwrap_or_default(),
            9..=16 => {
                if octets.len() >= 2 {
                    format!("{}.{}.", octets[0], octets[1])
                } else {
                    network.to_string()
                }
            }
            17..=24 => {
                if octets.len() >= 3 {
                    format!("{}.{}.{}.", octets[0], octets[1], octets[2])
                } else {
                    network.to_string()
                }
            }
            _ => network.to_string(),
        }
    }
}

/// Quick helper to create trusted network config
pub fn trust_networks(prefixes: &[&str]) -> NetworkConfig {
    let mut config = NetworkConfig::new();
    for prefix in prefixes {
        config = config.trust_prefix(prefix);
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_detection() {
        // Localhost
        assert_eq!(AccessZone::from_ip("127.0.0.1"), AccessZone::Localhost);
        assert_eq!(AccessZone::from_ip("::1"), AccessZone::Localhost);
        assert_eq!(AccessZone::from_ip("localhost"), AccessZone::Localhost);

        // Tailscale (100.64.0.0/10)
        assert_eq!(AccessZone::from_ip("100.64.1.1"), AccessZone::TrustedMesh);
        assert_eq!(AccessZone::from_ip("100.100.50.1"), AccessZone::TrustedMesh);
        assert_eq!(
            AccessZone::from_ip("100.127.255.255"),
            AccessZone::TrustedMesh
        );

        // Netmaker common ranges
        assert_eq!(AccessZone::from_ip("10.101.0.5"), AccessZone::TrustedMesh);
        assert_eq!(AccessZone::from_ip("10.102.1.1"), AccessZone::TrustedMesh);

        // ZeroTier
        assert_eq!(AccessZone::from_ip("10.147.20.1"), AccessZone::TrustedMesh);

        // WireGuard common
        assert_eq!(AccessZone::from_ip("10.0.0.5"), AccessZone::TrustedMesh);
        assert_eq!(AccessZone::from_ip("10.66.66.1"), AccessZone::TrustedMesh);

        // Private networks (non-mesh)
        assert_eq!(
            AccessZone::from_ip("192.168.1.100"),
            AccessZone::PrivateNetwork
        );
        assert_eq!(AccessZone::from_ip("10.1.0.1"), AccessZone::PrivateNetwork); // Generic 10.x
        assert_eq!(
            AccessZone::from_ip("172.16.0.1"),
            AccessZone::PrivateNetwork
        );
        assert_eq!(
            AccessZone::from_ip("172.31.255.255"),
            AccessZone::PrivateNetwork
        );

        // Public
        assert_eq!(AccessZone::from_ip("8.8.8.8"), AccessZone::Public);
        assert_eq!(AccessZone::from_ip("172.15.0.1"), AccessZone::Public); // Not in 172.16-31
        assert_eq!(AccessZone::from_ip("172.32.0.1"), AccessZone::Public);
        assert_eq!(AccessZone::from_ip("100.63.0.1"), AccessZone::Public); // Just below Tailscale range
        assert_eq!(AccessZone::from_ip("100.128.0.1"), AccessZone::Public); // Just above Tailscale range
    }
}
</file>

<file path="src/self_identity.rs">
//! Self-Repository Identity
//!
//! Provides awareness of the chatbot's own source code repository.

use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// Get the self-repository path from environment
pub fn get_self_repo_path() -> Option<PathBuf> {
    std::env::var("OP_SELF_REPO_PATH")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

/// Check if self-repository is configured
pub fn is_self_repo_configured() -> bool {
    get_self_repo_path().is_some()
}

/// Information about the self-repository
#[derive(Debug, Clone)]
pub struct SelfRepositoryInfo {
    pub path: PathBuf,
    pub name: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub has_changes: bool,
    pub has_git: bool,
}

impl SelfRepositoryInfo {
    /// Gather information about the self-repository
    pub fn gather() -> Option<Self> {
        let path = get_self_repo_path()?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let has_git = path.join(".git").exists();

        let (branch, commit, has_changes) = if has_git {
            (
                Self::get_git_branch(&path),
                Self::get_git_commit(&path),
                Self::check_git_changes(&path),
            )
        } else {
            (None, None, false)
        };

        info!(
            "Self-repository: {} at {:?} (branch: {:?}, commit: {:?})",
            name, path, branch, commit
        );

        Some(Self {
            path,
            name,
            branch,
            commit,
            has_changes,
            has_git,
        })
    }

    fn get_git_branch(path: &PathBuf) -> Option<String> {
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(path)
            .output()
            .ok()?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() {
                return Some(branch);
            }
        }
        None
    }

    fn get_git_commit(path: &PathBuf) -> Option<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(path)
            .output()
            .ok()?;

        if output.status.success() {
            let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !commit.is_empty() {
                return Some(commit);
            }
        }
        None
    }

    fn check_git_changes(path: &PathBuf) -> bool {
        Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(path)
            .output()
            .ok()
            .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false)
    }

    /// Generate system prompt context for self-awareness
    pub fn to_system_prompt_context(&self) -> String {
        let git_info = if self.has_git {
            format!(
                "**Branch**: `{}`\n**Commit**: `{}`\n**Uncommitted Changes**: {}",
                self.branch.as_deref().unwrap_or("unknown"),
                self.commit.as_deref().unwrap_or("unknown"),
                if self.has_changes {
                    "Yes ⚠️"
                } else {
                    "No ✓"
                }
            )
        } else {
            "Not a git repository".to_string()
        };

        format!(
            r#"## 🔮 SELF-AWARENESS: YOUR OWN SOURCE CODE

You have access to your own source code. This IS you.

**Repository Path**: `{}`
**Repository Name**: `{}`
{}

### Self-Modification Tools
| Tool | Description |
|------|-------------|
| `self_read_file` | Read your source files |
| `self_write_file` | Modify your source files |
| `self_list_directory` | Explore your codebase |
| `self_search_code` | Search your code |
| `self_git_status` | Check git status |
| `self_git_diff` | View pending changes |
| `self_git_commit` | Commit changes |
| `self_git_log` | View history |
| `self_build` | Build yourself |
| `self_deploy` | Deploy yourself |

**⚠️ Changes to your code affect your own capabilities!**"#,
            self.path.display(),
            self.name,
            git_info
        )
    }
}
</file>

<file path="src/state_publisher.rs">
use anyhow::Result;
use async_trait::async_trait;
use simd_json::OwnedValue as Value;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum ChangeType {
    PropertySet,
    Signal,
    Deleted,
}

#[async_trait]
pub trait StatePublisher: Send + Sync + Debug {
    #[allow(clippy::too_many_arguments)]
    async fn publish_change(
        &self,
        plugin_id: String,
        path: String,
        change_type: ChangeType,
        property: Option<String>,
        old_value: Option<Value>,
        new_value: Value,
        tags: Vec<String>,
        source: String,
    ) -> Result<()>;
}
</file>

<file path="src/types.rs">
//! Common types used across op-dbus-v2

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use simd_json::ValueBuilder;
use std::collections::HashMap;
use uuid::Uuid;

/// Bus type for DBus connections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BusType {
    #[default]
    System,
    Session,
}

impl std::fmt::Display for BusType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BusType::System => write!(f, "system"),
            BusType::Session => write!(f, "session"),
        }
    }
}

/// DBus service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub bus_type: BusType,
    pub activatable: bool,
    pub active: bool,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub uid: Option<u32>,
}

/// DBus object path information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub path: String,
    pub interfaces: Vec<InterfaceInfo>,
    #[serde(default)]
    pub children: Vec<String>,
}

/// DBus interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub methods: Vec<MethodInfo>,
    pub signals: Vec<SignalInfo>,
    pub properties: Vec<PropertyInfo>,
}

/// DBus method information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodInfo {
    pub name: String,
    #[serde(default)]
    pub in_args: Vec<ArgInfo>,
    #[serde(default)]
    pub out_args: Vec<ArgInfo>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// DBus signal information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalInfo {
    pub name: String,
    pub args: Vec<ArgInfo>,
}

/// DBus property information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyInfo {
    pub name: String,
    pub signature: String,
    pub access: PropertyAccess,
}

/// DBus method/signal argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgInfo {
    pub name: Option<String>,
    pub signature: String,
    pub direction: ArgDirection,
}

/// Argument direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ArgDirection {
    #[default]
    In,
    Out,
}

/// Property access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropertyAccess {
    Read,
    Write,
    ReadWrite,
}

/// Tool definition (local to avoid cycle)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: OwnedValue,
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub namespace: String,
}

/// Tool execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub id: String,
    pub tool_name: String,
    pub arguments: OwnedValue,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

impl ToolRequest {
    pub fn new(tool_name: impl Into<String>, arguments: OwnedValue) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tool_name: tool_name.into(),
            arguments,
            timeout_ms: None,
        }
    }
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub id: String,
    pub success: bool,
    pub content: OwnedValue,
    #[serde(default)]
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

impl ToolResult {
    pub fn success(id: impl Into<String>, content: OwnedValue, exec_time: u64) -> Self {
        Self {
            id: id.into(),
            success: true,
            content,
            error: None,
            execution_time_ms: exec_time,
        }
    }

    pub fn error(id: impl Into<String>, error: impl Into<String>, exec_time: u64) -> Self {
        Self {
            id: id.into(),
            success: false,
            content: OwnedValue::null(),
            error: Some(error.into()),
            execution_time_ms: exec_time,
        }
    }
}

/// Agent definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub tools: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub config: HashMap<String, OwnedValue>,
}

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    #[default]
    Idle,
    Running,
    Paused,
    Error,
    Stopped,
}

/// Chat message for AI interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: ChatRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub metadata: HashMap<String, OwnedValue>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: ChatRole::User,
            content: content.into(),
            timestamp: Utc::now(),
            tool_calls: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: ChatRole::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            tool_calls: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: ChatRole::System,
            content: content.into(),
            timestamp: Utc::now(),
            tool_calls: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// Chat role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Tool call within a chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub tool_name: String,
    pub arguments: OwnedValue,
    #[serde(default)]
    pub result: Option<ToolResult>,
}

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub version: String,
    pub uptime_secs: u64,
    pub components: HashMap<String, ComponentHealth>,
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: ComponentStatus,
    #[serde(default)]
    pub message: Option<String>,
    pub last_check: DateTime<Utc>,
}

/// Component status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ComponentStatus {
    Healthy,
    Degraded,
    Unhealthy,
    #[default]
    Unknown,
}

// ============================================================================
// OBJECT SCHEMA REFERENCE (for D-Bus schema linking)
// ============================================================================

/// Reference to a D-Bus object schema stored in StateStore
///
/// Used to link plugins to their discovered D-Bus interfaces.
/// These schemas are persisted and restored during disaster recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchemaRef {
    /// Object type (e.g., "dbus_interface", "dbus_service")
    pub object_type: String,
    /// Namespace - typically D-Bus service name (e.g., "org.freedesktop.NetworkManager")
    pub namespace: String,
    /// D-Bus object path (e.g., "/org/freedesktop/NetworkManager")
    pub path: String,
    /// Hash of the interface schema for integrity verification
    pub schema_hash: String,
}

impl ObjectSchemaRef {
    pub fn new(
        object_type: impl Into<String>,
        namespace: impl Into<String>,
        path: impl Into<String>,
        schema_hash: impl Into<String>,
    ) -> Self {
        Self {
            object_type: object_type.into(),
            namespace: namespace.into(),
            path: path.into(),
            schema_hash: schema_hash.into(),
        }
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Core types and utilities for op-dbus-v2"

[dependencies]
async-trait = { workspace = true }
serde = { workspace = true }
simd-json = { version = "0.13", features = ["serde", "serde_impl"] }
uuid = { workspace = true }
chrono = { workspace = true }
tokio = { workspace = true, features = ["sync", "time"] }
tracing = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
zbus = { workspace = true }
op-execution-tracker = { path = "../op-execution-tracker" }

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-util"] }
</file>

<file path="compare-op-core.md">
# compare-op-core

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 10 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 7 |
| Partial artifacts | 1 |
| Spec-listed source files | 9 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- Core types and utilities for op-dbus-v2
- Internal crate integrations: op-execution-tracker.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/types.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/types.rs |
| `src/self_identity.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/self_identity.rs |
| `src/security.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/security.rs |
| `src/message.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/message.rs |
| `src/lib.rs` | ⚠️ Partial | Declared in source inventory from spec/design docs | src/lib.rs; partial artifacts: src/lib.rs.patch |
| `src/execution.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/execution.rs |
| `src/error.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/error.rs |
| `src/connection.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/connection.rs |
| `src/config.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/config.rs |
| `root` | ✅ Present | root source group | src/config.rs, src/connection.rs, src/error.rs, src/execution.rs, src/lib.rs, src/message.rs, src/security.rs, src/self_identity.rs, ... (+2 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| types | ✅ Implemented | src/types.rs | SPEC main module |
| self_identity | ✅ Implemented | src/self_identity.rs | SPEC main module |
| security | ✅ Implemented | src/security.rs | SPEC main module |
| message | ✅ Implemented | src/message.rs | SPEC main module |
| execution | ✅ Implemented | src/execution.rs | SPEC main module |
| error | ✅ Implemented | src/error.rs | SPEC main module |
| connection | ✅ Implemented | src/connection.rs | SPEC main module |
| config | ✅ Implemented | src/config.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-execution-tracker` - documented in SPEC

### External Runtime Dependencies
- `async-trait` - not listed in SPEC dependency block
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `uuid` - documented in SPEC
- `chrono` - documented in SPEC
- `tokio` - documented in SPEC
- `tracing` - documented in SPEC
- `thiserror` - documented in SPEC
- `anyhow` - documented in SPEC
- `zbus` - documented in SPEC

### Development and Build Dependencies
- `dev:tokio`

## Notes and Observations

- Local documentation files present: SPEC.md.
- Transitional or partial artifacts detected: src/lib.rs.patch.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: config, error, execution, security, self_identity, state_publisher, types.
- 1 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="SPEC.md">
# op-core - Specification

## Overview
**Crate**: `op-core`  
**Location**: `crates/op-core`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Core types and utilities for op-dbus-v2"
```

### Source Structure
```
op-core/src/types.rs
op-core/src/self_identity.rs
op-core/src/security.rs
op-core/src/message.rs
op-core/src/lib.rs
op-core/src/execution.rs
op-core/src/error.rs
op-core/src/connection.rs
op-core/src/config.rs
```

### Key Dependencies
```toml
serde = { workspace = true }
simd-json = { version = "0.13", features = ["serde", "serde_impl"] }
uuid = { workspace = true }
chrono = { workspace = true }
tokio = { workspace = true, features = ["sync", "time"] }
tracing = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
zbus = { workspace = true }
op-execution-tracker = { path = "../op-execution-tracker" }

tokio = { workspace = true, features = ["full", "test-util"] }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       9 Rust source files

### Main Modules
types
self_identity
security
message
execution
error
connection
config

## Purpose
Core types and utilities for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: 0.1.0
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-execution-tracker

---
*Generated from crate analysis*
</file>

</files>
