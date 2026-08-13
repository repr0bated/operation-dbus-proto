#![recursion_limit = "512"]

//! OP State Store - schema, in-memory state, and event-chain primitives.
//!
//! Features:
//! - In-memory StateStore implementation for ephemeral runtime wiring
//! - Prometheus metrics
//! - Plugin schema catalog with JSON Schema 2026 support
//! - Disaster recovery export/import
//! - OpenTelemetry tracing integration
//! - Blockchain-style event chain for compliance and reproducibility
//! - Schema-aware canonical hashing with Merkle batching

pub mod disaster_recovery;
pub mod error;
pub mod event_chain;
pub mod execution_job;
pub mod memory_store;
pub mod metrics;
pub mod plugin_schema;
pub mod schema_validator;
pub mod state_store;
pub mod subid_ui;

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
pub use memory_store::MemoryStore;
pub use plugin_schema::{
    builtin_plugin_schema, builtin_plugin_schemas, dialects, Constraint, FieldSchema, FieldType,
    MethodDecl, PluginCapabilities, PluginSchema, ReadOnlyCondition, SchemaCatalog,
    SchemaLoadError, SchemaRegistry, SideEffect, SignalDecl,
    ValidationResult as SchemaValidationResult, DEFAULT_SCHEMA_DIALECT,
};
pub use schema_validator::{
    canonicalize_json, SchemaValidator, ValidationError, ValidationReport, ValidatorError,
};
pub use state_store::StateStore;
pub use subid_ui::{
    element_key_from_subid, project_schema_ui, role_population, subid_category, ui_role_from_subid,
    UiFieldShape, UiRole, UiSubidProjection, SUBID_CATEGORIES,
};

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
