#![recursion_limit = "512"]

//! OP State Store - Execution State Tracking and Job Ledger
//!
//! Provides persistent storage for execution jobs with state transitions:
//! REQUESTED → DISPATCHED → RUNNING → COMPLETED/FAILED
//!
//! Features:
//! - SQLite persistent storage
//! - Redis real-time stream
//! - Prometheus metrics
//! - Plugin schema registry with JSON Schema 2026 support
//! - Disaster recovery export/import
//! - OpenTelemetry tracing integration
//! - Blockchain-style event chain for compliance and reproducibility
//! - Schema-aware canonical hashing with Merkle batching

pub mod disaster_recovery;
pub mod error;
pub mod event_chain;
pub mod execution_job;
pub mod metrics;
pub mod plugin_schema;
pub mod redis_stream;
pub mod schema_validator;
pub mod sqlite_store;
pub mod state_store;

pub use disaster_recovery::{
    get_global_dependencies, get_plugin_dependencies, DisasterRecoveryExport, HostInfo,
    PluginStateExport, RestoreResult, SystemDependency,
};
pub use error::StateStoreError;
pub use event_chain::{
    ActionOrigin, ChainConfig, ChainEvent, ChainVerificationResult, Decision, DenyReason,
    EventBatch, EventChain, MerkleProof, OperationType, StateSnapshot, TagImmutabilityProof,
};
pub use execution_job::{ExecutionJob, ExecutionResult, ExecutionStatus};
pub use plugin_schema::{
    dialects, Constraint, FieldSchema, FieldType, PluginSchema, ReadOnlyCondition, SchemaLoadError,
    SchemaRegistry, ValidationResult as SchemaValidationResult, DEFAULT_SCHEMA_DIALECT,
};
pub use redis_stream::RedisStream;
pub use schema_validator::{
    canonicalize_json, SchemaValidator, ValidationError, ValidationReport, ValidatorError,
};
pub use sqlite_store::SqliteStore;
pub use state_store::StateStore;

use serde::{Deserialize, Serialize};

/// A stored object for export/import
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredObject {
    pub id: String,
    pub object_type: String,
    pub namespace: String,
    pub data: simd_json::OwnedValue,
}

/// Export data structure for disaster recovery
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CanonicalDbExport {
    pub objects: Vec<StoredObject>,
    pub executions: Vec<simd_json::OwnedValue>,
    pub blockchain: Vec<simd_json::OwnedValue>,
}
