// Core trait for pluggable state management
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

use chrono::{DateTime, Utc};

/// Desired state configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    pub state: Value,
    pub timestamp: DateTime<Utc>,
    pub hash: String,
    pub description: Option<String>,
    pub source: StateSource,
}

impl DesiredState {
    pub fn new(state: Value) -> Self {
        let hash = format!(
            "{:x}",
            md5::compute(simd_json::to_string(&state).unwrap_or_default())
        );
        Self {
            state,
            timestamp: Utc::now(),
            hash,
            description: None,
            source: StateSource::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateSource {
    User,
    AutoDiscovered,
    Import(String),
    Plugin(String),
    Default,
}

/// Represents a change to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub operation: ChangeOperation,
    pub path: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub description: String,
    pub hash: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeOperation {
    Create,
    Update,
    Delete,
    NoOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
            suggestions: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub code: String,
}

/// Core trait that all state management plugins must implement
#[async_trait::async_trait]
pub trait StatePlugin: Send + Sync {
    /// Plugin identifier (e.g., "network", "filesystem", "user")
    fn name(&self) -> &str;

    /// Plugin version for compatibility checking
    #[allow(dead_code)]
    fn version(&self) -> &str;

    /// Check if this plugin's dependencies are available on the system
    /// Returns true if the plugin can operate, false if dependencies are missing
    /// Default implementation returns true (plugin always available)
    fn is_available(&self) -> bool {
        true
    }

    /// Get a message explaining why the plugin is unavailable (if it is)
    /// Only called if is_available() returns false
    fn unavailable_reason(&self) -> String {
        format!("Plugin '{}' is not available", self.name())
    }

    /// Query current system state in this domain
    async fn query_current_state(&self) -> Result<Value>;

    /// Calculate difference between current and desired state
    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff>;

    /// Apply the state changes (may be multi-step)
    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult>;

    /// Verify that current state matches desired state
    #[allow(dead_code)]
    async fn verify_state(&self, desired: &Value) -> Result<bool>;

    /// Create a checkpoint for rollback capability
    async fn create_checkpoint(&self) -> Result<Checkpoint>;

    /// Rollback to a previous checkpoint
    #[allow(dead_code)]
    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()>;

    /// Get plugin capabilities and limitations
    #[allow(dead_code)]
    fn capabilities(&self) -> PluginCapabilities;
}

/// Represents the difference between current and desired state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub plugin: String,
    pub actions: Vec<StateAction>,
    pub metadata: DiffMetadata,
}

/// Metadata about the diff calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffMetadata {
    pub timestamp: i64,
    pub current_hash: String,
    pub desired_hash: String,
}

/// Actions to be performed on resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateAction {
    Create { resource: String, config: Value },
    Modify { resource: String, changes: Value },
    Delete { resource: String },
    NoOp { resource: String },
}

/// Result of applying state changes
#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyResult {
    pub success: bool,
    pub changes_applied: Vec<String>,
    pub errors: Vec<String>,
    pub checkpoint: Option<Checkpoint>,
}

/// Checkpoint for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub plugin: String,
    pub timestamp: i64,
    pub state_snapshot: Value,
    pub backend_checkpoint: Option<Value>, // Plugin-specific checkpoint data
}

/// Plugin capabilities flags
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PluginCapabilities {
    pub supports_rollback: bool,
    pub supports_checkpoints: bool,
    pub supports_verification: bool,
    pub atomic_operations: bool,
}
