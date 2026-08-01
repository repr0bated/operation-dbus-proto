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
  execution_context.rs
  execution_tracker.rs
  lib.rs
  metrics.rs
  record.rs
  telemetry.rs
Cargo.toml
compare-op-execution-tracker.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/execution_context.rs">
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Execution context for tracking tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique execution ID
    pub execution_id: String,

    /// Trace ID for distributed tracing
    pub trace_id: String,

    /// Parent execution ID (if this is a sub-execution)
    pub parent_id: Option<String>,

    /// Tool name being executed
    pub tool_name: String,

    /// Execution status
    pub status: ExecutionStatus,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Execution metadata
    pub metadata: simd_json::OwnedValue,
}

/// Execution status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    /// Execution has been requested
    Requested,

    /// Execution has been dispatched to executor
    Dispatched,

    /// Execution is currently running
    Running,

    /// Execution completed successfully
    Completed,

    /// Execution failed
    Failed,

    /// Execution was cancelled
    Cancelled,
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionStatus::Requested => write!(f, "Requested"),
            ExecutionStatus::Dispatched => write!(f, "Dispatched"),
            ExecutionStatus::Running => write!(f, "Running"),
            ExecutionStatus::Completed => write!(f, "Completed"),
            ExecutionStatus::Failed => write!(f, "Failed"),
            ExecutionStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Execution result for tracked tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub result: Option<simd_json::OwnedValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
    pub finished_at: chrono::DateTime<chrono::Utc>,
}

impl ExecutionContext {
    /// Create new execution context
    pub fn new(tool_name: &str) -> Self {
        let now = Utc::now();
        Self {
            execution_id: Uuid::new_v4().to_string(),
            trace_id: Uuid::new_v4().to_string(),
            parent_id: None,
            tool_name: tool_name.to_string(),
            status: ExecutionStatus::Requested,
            created_at: now,
            updated_at: now,
            metadata: simd_json::json!({}),
        }
    }

    /// Create child execution context
    pub fn new_child(parent: &ExecutionContext, tool_name: &str) -> Self {
        let now = Utc::now();
        Self {
            execution_id: Uuid::new_v4().to_string(),
            trace_id: parent.trace_id.clone(), // Share trace ID for distributed tracing
            parent_id: Some(parent.execution_id.clone()),
            tool_name: tool_name.to_string(),
            status: ExecutionStatus::Requested,
            created_at: now,
            updated_at: now,
            metadata: simd_json::json!({}),
        }
    }

    /// Update status
    pub fn update_status(&mut self, new_status: ExecutionStatus) {
        self.status = new_status;
        self.updated_at = Utc::now();
    }

    /// Set metadata
    pub fn set_metadata(&mut self, metadata: simd_json::OwnedValue) {
        self.metadata = metadata;
        self.updated_at = Utc::now();
    }
}
</file>

<file path="src/execution_tracker.rs">
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use simd_json::ValueBuilder;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

use crate::record::{ExecutionRecord, ExecutionStatus};

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

/// Event emitted when execution state changes
#[derive(Clone, Debug)]
pub enum ExecutionEvent {
    Started(Box<ExecutionRecord>),
    Completed(String, bool),                // execution_id, success
    StatusUpdated(String, ExecutionStatus), // execution_id, new_status
}

/// Execution tracker for monitoring tool executions
#[derive(Clone)]
pub struct ExecutionTracker {
    /// History of executions (ring buffer)
    records: Arc<RwLock<Vec<ExecutionRecord>>>,
    /// Maximum history size
    max_history: usize,
    /// Statistics
    stats: Arc<RwLock<ExecutionStats>>,
    /// Event broadcaster
    event_sender: broadcast::Sender<ExecutionEvent>,
}

impl ExecutionTracker {
    /// Create new execution tracker
    pub fn new(max_history: usize) -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self {
            records: Arc::new(RwLock::new(Vec::with_capacity(max_history))),
            max_history,
            stats: Arc::new(RwLock::new(ExecutionStats::default())),
            event_sender: tx,
        }
    }

    /// Subscribe to execution events
    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.event_sender.subscribe()
    }

    /// Start tracking a new execution (Legacy compatibility API)
    pub async fn start_execution(
        &self,
        tool_name: &str,
        input: Option<Value>,
        initiated_by: Option<String>,
    ) -> ExecutionRecord {
        let mut record = ExecutionRecord::new(tool_name, None);
        record.input = input.unwrap_or(Value::null());
        record.initiated_by = initiated_by;
        record.start();

        let mut records = self.records.write().await;
        records.push(record.clone());

        // Trim if over limit
        if records.len() > self.max_history {
            records.remove(0);
        }

        // Notify subscribers
        let _ = self
            .event_sender
            .send(ExecutionEvent::Started(Box::new(record.clone())));

        info!(execution_id = %record.id, tool = %tool_name, "Execution started");

        record
    }

    /// Complete an execution (Legacy compatibility API)
    pub async fn complete_execution(&self, id: &str, output: Option<String>) {
        let mut records = self.records.write().await;
        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            record.complete(output);

            // Update stats
            let mut stats = self.stats.write().await;
            stats.total_executions += 1;
            stats.successful_executions += 1;
            if record.timing.duration_ms > 0 {
                stats.total_duration_ms += record.timing.duration_ms;
            }
            *stats
                .executions_by_tool
                .entry(record.tool_name.clone())
                .or_insert(0) += 1;

            // Notify subscribers
            let _ = self
                .event_sender
                .send(ExecutionEvent::Completed(id.to_string(), true));

            info!(
                execution_id = %id,
                tool = %record.tool_name,
                duration_ms = record.timing.duration_ms,
                "Execution completed successfully"
            );
        }
    }

    /// Fail an execution (Legacy compatibility API)
    pub async fn fail_execution(&self, id: &str, error: String) {
        let mut records = self.records.write().await;
        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            record.fail(error.clone());

            // Update stats
            let mut stats = self.stats.write().await;
            stats.total_executions += 1;
            stats.failed_executions += 1;
            if record.timing.duration_ms > 0 {
                stats.total_duration_ms += record.timing.duration_ms;
            }
            *stats
                .executions_by_tool
                .entry(record.tool_name.clone())
                .or_insert(0) += 1;
            *stats
                .failures_by_tool
                .entry(record.tool_name.clone())
                .or_insert(0) += 1;

            // Notify subscribers
            let _ = self
                .event_sender
                .send(ExecutionEvent::Completed(id.to_string(), false));

            warn!(
                execution_id = %id,
                tool = %record.tool_name,
                error = %error,
                "Execution failed"
            );
        }
    }

    /// Get execution record by ID
    pub async fn get_execution(&self, id: &str) -> Option<ExecutionRecord> {
        let records = self.records.read().await;
        let result = records.iter().find(|r| r.id == id).cloned();
        result
    }

    /// List active executions
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

    /// List recent completed executions
    pub async fn get_recent(&self, limit: usize) -> Vec<ExecutionRecord> {
        let records = self.records.read().await;
        records
            .as_slice()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Alias for get_recent (compatibility)
    pub async fn list_recent_completed(&self, limit: usize) -> Vec<ExecutionRecord> {
        self.get_recent(limit).await
    }

    /// Get statistics snapshot
    pub async fn get_stats(&self) -> ExecutionStats {
        self.stats.read().await.clone()
    }
}

impl Default for ExecutionTracker {
    fn default() -> Self {
        Self::new(1000)
    }
}
</file>

<file path="src/lib.rs">
//! OP Execution Tracker - Lightweight Execution Monitoring Layer
//!
//! Complements existing state management by providing:
//! - Execution acknowledgment protocol
//! - Real-time execution tracking
//! - Integration with existing workflow/orchestration states
//! - Observability without duplicating state management

pub mod execution_context;

pub mod execution_tracker;

pub mod metrics;

pub mod telemetry;

pub mod record;

pub use execution_context::{ExecutionContext, ExecutionResult};

pub use execution_tracker::{ExecutionEvent, ExecutionStats, ExecutionTracker};

pub use metrics::ExecutionMetrics;

pub use telemetry::ExecutionTelemetry;

pub use record::ExecutionStatus as RecordExecutionStatus;
pub use record::{hash_execution, ExecutionRecord, ExecutionRecordBuilder, ExecutionTiming};
</file>

<file path="src/metrics.rs">
use prometheus::{Histogram, HistogramOpts, IntCounter, IntGauge, Registry};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Execution metrics collector
#[derive(Clone)]
pub struct ExecutionMetrics {
    /// Total executions started
    executions_started: IntCounter,

    /// Currently active executions
    active_executions: IntGauge,

    /// Executions completed successfully
    executions_succeeded: IntCounter,

    /// Executions failed
    executions_failed: IntCounter,

    /// Execution duration histogram
    execution_duration: Histogram,

    /// Status transitions
    status_transitions: IntCounter,

    /// Registry for custom metrics
    registry: Arc<RwLock<Registry>>,
}

impl ExecutionMetrics {
    /// Create new metrics collector
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let executions_started = IntCounter::new(
            "mcp_executions_started_total",
            "Total number of executions started",
        )?;
        registry.register(Box::new(executions_started.clone()))?;

        let active_executions = IntGauge::new(
            "mcp_active_executions",
            "Number of currently active executions",
        )?;
        registry.register(Box::new(active_executions.clone()))?;

        let executions_succeeded = IntCounter::new(
            "mcp_executions_succeeded_total",
            "Total number of successfully completed executions",
        )?;
        registry.register(Box::new(executions_succeeded.clone()))?;

        let executions_failed = IntCounter::new(
            "mcp_executions_failed_total",
            "Total number of failed executions",
        )?;
        registry.register(Box::new(executions_failed.clone()))?;

        let execution_duration = Histogram::with_opts(
            HistogramOpts::new(
                "mcp_execution_duration_seconds",
                "Execution duration in seconds",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
        )?;
        registry.register(Box::new(execution_duration.clone()))?;

        let status_transitions = IntCounter::new(
            "mcp_status_transitions_total",
            "Total number of execution status transitions",
        )?;
        registry.register(Box::new(status_transitions.clone()))?;

        Ok(Self {
            executions_started,
            active_executions,
            executions_succeeded,
            executions_failed,
            execution_duration,
            status_transitions,
            registry: Arc::new(RwLock::new(registry)),
        })
    }

    pub fn execution_started(&self, _tool_name: &str) {
        self.executions_started.inc();
        self.active_executions.inc();
    }

    pub fn status_updated(&self, _tool_name: &str, _status: &str) {
        // Keep this lightweight to avoid high-cardinality labels.
        self.status_transitions.inc();
    }

    pub fn execution_succeeded(&self, _tool_name: &str, duration_ms: u64) {
        self.executions_succeeded.inc();
        self.execution_duration.observe(duration_ms as f64 / 1000.0);
        self.active_executions.dec();
    }

    pub fn execution_failed(&self, _tool_name: &str) {
        self.executions_failed.inc();
        self.active_executions.dec();
    }

    /// Get metrics registry for scraping
    pub async fn get_registry(&self) -> Registry {
        self.registry.read().await.clone()
    }

    /// Get metrics as JSON (simplified version)
    pub async fn get_metrics_json(&self) -> Result<simd_json::OwnedValue, simd_json::Error> {
        let registry = self.get_registry().await;
        let metric_families = registry.gather();

        // Simplified JSON format - just return metric names and basic info
        let mut metrics = Vec::new();

        for family in metric_families {
            metrics.push(simd_json::json!({
                "name": family.get_name(),
                "help": family.get_help(),
                "metric_count": family.get_metric().len(),
            }));
        }

        Ok(simd_json::json!({
            "metrics": metrics
        }))
    }
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self::new().expect("Failed to create default metrics")
    }
}
</file>

<file path="src/record.rs">
//! Execution Tracking Record
//!
//! Provides accountability and audit trail for all tool executions.
//! - ExecutionTiming for precise timing capture
//! - Builder pattern for ExecutionRecord creation
//! - Hash-based execution fingerprinting

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use simd_json::OwnedValue as Value;
use simd_json::ValueBuilder;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

// ============================================================================
// EXECUTION STATUS
// ============================================================================

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Request received, not yet started
    #[default]
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

// ============================================================================
// EXECUTION TIMING
// ============================================================================

/// High-precision execution timing
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ExecutionTiming {
    /// Wall-clock start time
    pub started_at: DateTime<Utc>,
    /// Wall-clock end time
    pub ended_at: Option<DateTime<Utc>>,
    /// Monotonic nanoseconds (for ordering)
    pub monotonic_ns: u128,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Duration in nanoseconds (compatibility alias)
    pub duration_ns: u128,
    /// Wall-clock nanoseconds (compatibility alias)
    pub wallclock_ns: u128,
}

impl ExecutionTiming {
    /// Capture the start of an execution
    pub fn capture_start() -> (Instant, Self) {
        let now = Instant::now();
        let monotonic = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let timing = Self {
            started_at: Utc::now(),
            ended_at: None,
            monotonic_ns: monotonic,
            duration_ms: 0,
            duration_ns: 0,
            wallclock_ns: monotonic,
        };
        (now, timing)
    }

    /// Complete the timing with duration
    pub fn complete(mut self, start: Instant) -> Self {
        let elapsed = start.elapsed();
        self.ended_at = Some(Utc::now());
        self.duration_ms = elapsed.as_millis() as u64;
        self.duration_ns = elapsed.as_nanos();
        self.wallclock_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        self
    }
}

// ============================================================================
// EXECUTION RECORD
// ============================================================================

/// Record of a single tool/agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique execution ID
    pub id: String,
    /// Trace ID for correlation across systems
    pub trace_id: String,
    /// Tool or agent name
    pub tool_name: String,
    /// Input arguments
    pub input: Value,
    /// Output value
    pub output: Value,
    /// Execution status
    pub status: ExecutionStatus,
    /// Execution timing
    pub timing: ExecutionTiming,
    /// Policy ID governing this execution
    pub policy_id: String,
    /// Plugin core hash for determinism
    pub plugin_core_hash: String,
    /// Tunable hash for determinism
    pub tunable_hash: String,
    /// Previous execution hash for chaining
    pub prev_hash: String,
    /// This execution's hash
    pub exec_hash: String,
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
    /// Create a new execution record with default values
    pub fn new(tool_name: &str, trace_id: Option<String>) -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            id: id.clone(),
            trace_id: trace_id.unwrap_or_else(|| id.clone()),
            tool_name: tool_name.to_string(),
            input: Value::null(),
            output: Value::null(),
            status: ExecutionStatus::Pending,
            timing: ExecutionTiming::default(),
            policy_id: String::new(),
            plugin_core_hash: String::new(),
            tunable_hash: String::new(),
            prev_hash: String::new(),
            exec_hash: String::new(),
            output_summary: None,
            error: None,
            success: false,
            initiated_by: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a builder for ExecutionRecord
    pub fn builder(tool_name: &str) -> ExecutionRecordBuilder {
        ExecutionRecordBuilder::new(tool_name)
    }

    /// Mark as running
    pub fn start(&mut self) {
        self.status = ExecutionStatus::Running;
        self.timing.started_at = Utc::now();
    }

    /// Mark as completed successfully
    pub fn complete(&mut self, output: Option<String>) {
        let now = Utc::now();
        self.timing.ended_at = Some(now);
        self.timing.duration_ms = (now - self.timing.started_at).num_milliseconds().max(0) as u64;
        self.status = ExecutionStatus::Completed;
        self.success = true;
        self.output_summary = output.map(|s| truncate_string(&s, 1000));
    }

    /// Mark as failed
    pub fn fail(&mut self, error: String) {
        let now = Utc::now();
        self.timing.ended_at = Some(now);
        self.timing.duration_ms = (now - self.timing.started_at).num_milliseconds().max(0) as u64;
        self.status = ExecutionStatus::Failed;
        self.success = false;
        self.error = Some(error);
    }

    /// Mark as timed out
    pub fn timeout(&mut self) {
        let now = Utc::now();
        self.timing.ended_at = Some(now);
        self.timing.duration_ms = (now - self.timing.started_at).num_milliseconds().max(0) as u64;
        self.status = ExecutionStatus::Timeout;
        self.success = false;
        self.error = Some("Execution timed out".to_string());
    }

    /// Mark as cancelled
    pub fn cancel(&mut self) {
        let now = Utc::now();
        self.timing.ended_at = Some(now);
        self.timing.duration_ms = (now - self.timing.started_at).num_milliseconds().max(0) as u64;
        self.status = ExecutionStatus::Cancelled;
        self.success = false;
        self.error = Some("Execution cancelled".to_string());
    }

    // ================= COMPATIBILITY ACCESSORS =================

    /// Alias for id (compatibility)
    pub fn execution_id(&self) -> &str {
        &self.id
    }

    /// Alias for tool_name (compatibility)
    pub fn tool(&self) -> &str {
        &self.tool_name
    }

    /// Alias for exec_hash (compatibility)
    pub fn hash(&self) -> &str {
        &self.exec_hash
    }

    /// Verify hash integrity
    pub fn verify_integrity(&self) -> bool {
        let computed = hash_execution(&self.tool_name, &self.input, &self.output, &self.prev_hash);
        computed == self.exec_hash
    }
}

// ============================================================================
// EXECUTION RECORD BUILDER
// ============================================================================

/// Builder pattern for creating execution records
pub struct ExecutionRecordBuilder {
    tool_name: String,
    input: Value,
    output: Value,
    policy_id: String,
    plugin_core_hash: String,
    tunable_hash: String,
    timing: ExecutionTiming,
    prev_hash: String,
    initiated_by: Option<String>,
    metadata: HashMap<String, String>,
}

impl ExecutionRecordBuilder {
    pub fn new(tool_name: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            input: Value::null(),
            output: Value::null(),
            policy_id: "default".to_string(),
            plugin_core_hash: String::new(),
            tunable_hash: String::new(),
            timing: ExecutionTiming::default(),
            prev_hash: String::new(),
            initiated_by: None,
            metadata: HashMap::new(),
        }
    }

    pub fn input(mut self, input: Value) -> Self {
        self.input = input;
        self
    }

    pub fn output(mut self, output: Value) -> Self {
        self.output = output;
        self
    }

    pub fn policy_id(mut self, policy_id: &str) -> Self {
        self.policy_id = policy_id.to_string();
        self
    }

    pub fn plugin_core_hash(mut self, hash: &str) -> Self {
        self.plugin_core_hash = hash.to_string();
        self
    }

    pub fn tunable_hash(mut self, hash: &str) -> Self {
        self.tunable_hash = hash.to_string();
        self
    }

    pub fn timing(mut self, timing: ExecutionTiming) -> Self {
        self.timing = timing;
        self
    }

    pub fn prev_hash(mut self, hash: &str) -> Self {
        self.prev_hash = hash.to_string();
        self
    }

    pub fn initiated_by(mut self, user: &str) -> Self {
        self.initiated_by = Some(user.to_string());
        self
    }

    pub fn metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn build(self) -> ExecutionRecord {
        let id = Uuid::new_v4().to_string();
        let exec_hash = hash_execution(&self.tool_name, &self.input, &self.output, &self.prev_hash);

        ExecutionRecord {
            id: id.clone(),
            trace_id: id,
            tool_name: self.tool_name,
            input: self.input,
            output: self.output.clone(),
            status: ExecutionStatus::Completed,
            timing: self.timing,
            policy_id: self.policy_id,
            plugin_core_hash: self.plugin_core_hash,
            tunable_hash: self.tunable_hash,
            prev_hash: self.prev_hash,
            exec_hash,
            output_summary: Some(truncate_string(
                &simd_json::to_string(&self.output).unwrap_or_default(),
                1000,
            )),
            error: None,
            success: true,
            initiated_by: self.initiated_by,
            metadata: self.metadata,
        }
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Truncate string to max length
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}... (truncated)", &s[..max_len])
    }
}

/// Compute execution hash for deterministic fingerprinting
pub fn hash_execution(tool_name: &str, input: &Value, output: &Value, prev_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(tool_name.as_bytes());
    hasher.update(simd_json::to_vec(input).unwrap_or_default());
    hasher.update(simd_json::to_vec(output).unwrap_or_default());
    hasher.update(prev_hash.as_bytes());
    hex::encode(hasher.finalize())
}
</file>

<file path="src/telemetry.rs">
use tracing::{info, instrument};

use crate::execution_context::{ExecutionContext, ExecutionResult};

/// Execution telemetry for distributed tracing
/// Simplified to use tracing instead of OpenTelemetry directly
pub struct ExecutionTelemetry {
    /// Service name for tracing
    service_name: String,
}

impl ExecutionTelemetry {
    /// Create new telemetry service
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
        }
    }

    /// Start execution span
    #[instrument(skip(self, context), fields(
        execution_id = %context.execution_id,
        trace_id = %context.trace_id,
        tool_name = %context.tool_name,
        status = %context.status,
        service = %self.service_name
    ))]
    pub fn start_execution_span(&self, context: &ExecutionContext) {
        info!(
            parent_id = ?context.parent_id,
            "Started execution trace span"
        );
    }

    /// End execution span
    #[instrument(skip(self, context, result), fields(
        execution_id = %context.execution_id,
        tool_name = %context.tool_name,
        success = result.success,
        duration_ms = result.duration_ms,
        service = %self.service_name
    ))]
    pub fn end_execution_span(&self, context: &ExecutionContext, result: &ExecutionResult) {
        if result.success {
            info!(
                final_status = %context.status,
                "Execution completed successfully"
            );
        } else {
            info!(
                final_status = %context.status,
                error = ?result.error,
                "Execution failed"
            );
        }
    }

    /// Record execution event
    #[instrument(skip(self, context), fields(
        execution_id = %context.execution_id,
        tool_name = %context.tool_name,
        service = %self.service_name
    ))]
    pub fn record_event(&self, context: &ExecutionContext, event_name: &str, details: &str) {
        info!(
            event = event_name,
            details = details,
            "Recorded execution event"
        );
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-execution-tracker"
version = "0.1.0"
edition = "2021"
description = "MCP Execution Tracking Layer - Lightweight execution monitoring that complements existing state management"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true, features = ["derive"] }
simd-json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
chrono = { workspace = true, features = ["serde"] }
uuid = { workspace = true, features = ["v4", "serde"] }
sha2 = { workspace = true }
hex = "0.4"
prometheus = { workspace = true }
</file>

<file path="compare-op-execution-tracker.md">
# compare-op-execution-tracker

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 6 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 5 |
| Partial artifacts | 0 |
| Spec-listed source files | 6 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- MCP Execution Tracking Layer - Lightweight execution monitoring that complements existing state management

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/telemetry.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/telemetry.rs |
| `src/record.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/record.rs |
| `src/metrics.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/metrics.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/execution_tracker.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/execution_tracker.rs |
| `src/execution_context.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/execution_context.rs |
| `root` | ✅ Present | root source group | src/execution_context.rs, src/execution_tracker.rs, src/lib.rs, src/metrics.rs, src/record.rs, src/telemetry.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| telemetry | ✅ Implemented | src/telemetry.rs | SPEC main module |
| record | ✅ Implemented | src/record.rs | SPEC main module |
| metrics | ✅ Implemented | src/metrics.rs | SPEC main module |
| execution_tracker | ✅ Implemented | src/execution_tracker.rs | SPEC main module |
| execution_context | ✅ Implemented | src/execution_context.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - documented in SPEC
- `sha2` - documented in SPEC
- `hex` - documented in SPEC
- `prometheus` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: execution_context, execution_tracker, metrics, telemetry, record.
</file>

<file path="SPEC.md">
# op-execution-tracker - Specification

## Overview
**Crate**: `op-execution-tracker`  
**Location**: `crates/op-execution-tracker`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-execution-tracker"
version = "0.1.0"
edition = "2021"
description = "MCP Execution Tracking Layer - Lightweight execution monitoring that complements existing state management"
```

### Source Structure
```
op-execution-tracker/src/telemetry.rs
op-execution-tracker/src/record.rs
op-execution-tracker/src/metrics.rs
op-execution-tracker/src/lib.rs
op-execution-tracker/src/execution_tracker.rs
op-execution-tracker/src/execution_context.rs
```

### Key Dependencies
```toml
tokio = { workspace = true }
serde = { workspace = true, features = ["derive"] }
simd-json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
chrono = { workspace = true, features = ["serde"] }
uuid = { workspace = true, features = ["v4", "serde"] }
sha2 = { workspace = true }
hex = "0.4"
prometheus = { workspace = true }
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
       6 Rust source files

### Main Modules
telemetry
record
metrics
execution_tracker
execution_context

## Purpose
MCP Execution Tracking Layer - Lightweight execution monitoring that complements existing state management

## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
