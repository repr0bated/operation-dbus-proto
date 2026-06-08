//! Core data structures for the Projection System.
//!
//! This module defines all shared data models used throughout the projection system,
//! including schemas, projections, and their various specialized types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

// ── Canonical PluginSchema (single source of truth) ────────────────────────
// "The plugin is the schema." Projection does NOT define its own schema type —
// plugins are all that is projected, so the canonical `PluginSchema` (and its
// `FieldSchema` / `FieldType` / `Constraint` / `ReadOnlyCondition`) is imported
// from the authority surface `op_plugins`. Redaction (`secret_paths`/`pii_paths`)
// and ordered `Vec` fields are out of D-Bus projection scope and intentionally
// gone; the canonical type uses a `HashMap<String, FieldSchema>` and is validated
// via `PluginSchema::validate(...)`.
pub use op_plugins::{Constraint, FieldSchema, FieldType, PluginSchema, ReadOnlyCondition};

/// Validation result with errors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationResult {
    /// Whether validation passed
    pub valid: bool,
    /// List of validation errors
    pub errors: Vec<ValidationError>,
}

/// A single validation error.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationError {
    /// JSON path to the invalid field
    pub path: String,
    /// Human-readable error message
    pub message: String,
    /// Error code for programmatic handling
    pub code: String,
}

/// The state of a projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProjectionState {
    /// Projection passes all schema validations
    Valid,
    /// Projection fails schema validation (preserved for debugging)
    Quarantined,
    /// Projection has missing dependencies (partially valid)
    Degraded,
}

/// Base projection structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Projection {
    /// Unique identifier for this projection
    pub id: String,
    /// Entity type (from schema name)
    pub entity_type: String,
    /// Entity ID (from source data)
    pub entity_id: String,
    /// Current validation state
    pub state: ProjectionState,
    /// Schema version used for validation
    pub schema_version: String,
    /// Projected data (JSON)
    pub data: Value,
    /// Validation errors (if any)
    pub validation_errors: Vec<ValidationError>,
    /// Reason for quarantine (if quarantined)
    pub quarantine_reason: Option<String>,
    /// Reason for degradation (if degraded)
    pub degradation_reason: Option<String>,
    /// IDs of affected dependent projections
    pub affected_dependencies: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Dashboard projection with freshness and source metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardProjection {
    /// Base projection
    pub projection: Projection,
    /// Data freshness duration
    pub freshness: chrono::Duration,
    /// Source identifier
    pub source: String,
    /// List of validation issues
    pub validation_issues: Vec<String>,
}

/// Orchestration projection with relationships and health.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestrationProjection {
    /// Base projection
    pub projection: Projection,
    /// Relationships to other entities
    pub relationships: Vec<Relationship>,
    /// Health status
    pub health_status: HealthStatus,
    /// Composite health of dependencies
    pub dependency_health: Option<CompositeHealth>,
}

/// Relationship between entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    /// Target entity ID
    pub target: String,
    /// Type of relationship
    pub relationship_type: RelationshipType,
    /// Optional metadata
    pub metadata: Option<Value>,
}

/// Types of relationships between entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationshipType {
    /// Parent-child hierarchy
    ParentChild,
    /// Equal-level peer relationship
    PeerPeer,
    /// Dependency relationship
    DependencyDependee,
    /// Type relationship (instance-of)
    IsA,
    /// Composition relationship
    PartOf,
    /// Usage relationship
    Uses,
    /// Dependency relationship
    DependsOn,
}

/// Health status of an entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    /// Healthy
    Healthy,
    /// Warning state
    Warning,
    /// Critical state
    Critical,
    /// Unknown state
    Unknown,
}

/// Composite health of multiple dependencies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositeHealth {
    /// Overall health status
    pub status: HealthStatus,
    /// Individual dependency health
    pub dependencies: HashMap<String, HealthStatus>,
}

/// Audit trail entry for state changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditProjection {
    /// Timestamp of the change
    pub timestamp: DateTime<Utc>,
    /// Actor who made the change
    pub actor: String,
    /// Entity ID that changed
    pub entity_id: String,
    /// Previous state (if available)
    pub old_state: Option<Value>,
    /// New state after change
    pub new_state: Value,
    /// Type of change
    pub change_type: ChangeType,
    /// Footprint hash (The Strike/Etch)
    pub footprint: String,
    /// Trace ID for correlation
    pub trace_id: String,
}

/// Types of state changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    /// Entity was created
    Created,
    /// Entity was updated
    Updated,
    /// Entity was deleted
    Deleted,
    /// Entity state changed (e.g., valid -> quarantined)
    StateChanged,
    /// Entity was quarantined
    Quarantined,
    /// Entity was degraded
    Degraded,
}

/// Topology projection with nodes and links.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopologyProjection {
    /// Nodes in the topology
    pub nodes: Vec<Node>,
    /// Links between nodes
    pub links: Vec<Link>,
}

/// A node in the topology.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    /// Unique node ID
    pub id: String,
    /// Entity type
    pub entity_type: String,
    /// Entity ID
    pub entity_id: String,
    /// Validation state
    pub state: ProjectionState,
    /// Node metadata
    pub metadata: Value,
}

/// A link between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Link {
    /// Source node ID
    pub source: String,
    /// Target node ID
    pub target: String,
    /// Type of relationship
    pub relationship: RelationshipType,
    /// Latency in milliseconds (optional)
    pub latency_ms: Option<f64>,
    /// Bandwidth in Mbps (optional)
    pub bandwidth_mbps: Option<f64>,
    /// Reliability (0.0 to 1.0, optional)
    pub reliability: Option<f64>,
}

/// AI context projection with semantic relationships.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIContextProjection {
    /// Base projection
    pub projection: Projection,
    /// Semantic relationships to other entities
    pub semantic_relationships: Vec<SemanticRelationship>,
}

/// A semantic relationship between entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticRelationship {
    /// Source entity ID
    pub source: String,
    /// Target entity ID
    pub target: String,
    /// Type of semantic relationship
    pub relationship_type: RelationshipType,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Whether this relationship is tentative
    pub is_tentative: bool,
}

/// Historical projection with versioning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoricalProjection {
    /// Base projection
    pub projection: Projection,
    /// Version number
    pub version: u64,
    /// Timestamp of this version
    pub timestamp: DateTime<Utc>,
    /// Whether this version was quarantined
    pub is_quarantined: bool,
}

/// Event structure for event-fed materialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectionEvent {
    /// Event ID
    pub id: String,
    /// Event type
    pub event_type: EventType,
    /// Entity type
    pub entity_type: String,
    /// Entity ID
    pub entity_id: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event data
    pub data: Value,
    /// Source identifier
    pub source: String,
}

/// Types of projection events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    /// Entity created
    Created,
    /// Entity updated
    Updated,
    /// Entity deleted
    Deleted,
    /// Entity state changed
    StateChanged,
    /// Schema registered
    SchemaRegistered,
    /// Schema updated
    SchemaUpdated,
    /// Schema quarantined
    SchemaQuarantined,
}

/// Projection update structure for delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectionUpdate {
    /// Update type
    pub update_type: UpdateType,
    /// Projection that was updated
    pub projection: Projection,
    /// Timestamp of update
    pub timestamp: DateTime<Utc>,
}

/// Types of projection updates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateType {
    /// New projection created
    Created,
    /// Existing projection updated
    Updated,
    /// Projection deleted
    Deleted,
    /// Projection state changed
    StateChanged,
}

/// Requester information for access control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Requester {
    /// Requester ID
    pub id: String,
    /// Requester type (user, service, etc.)
    pub requester_type: String,
    /// Permissions list
    pub permissions: Vec<String>,
    /// Metadata
    pub metadata: Option<Value>,
}

/// Access control policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessPolicy {
    /// Policy ID
    pub id: String,
    /// Resource pattern (regex)
    pub resource_pattern: String,
    /// Required permissions
    pub required_permissions: Vec<String>,
    /// Action allowed
    pub action: String,
    /// Whether to redact sensitive data
    pub redact_sensitive: bool,
}

/// Audit trail entry for access control decisions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessControlAudit {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Requester ID
    pub requester_id: String,
    /// Action performed
    pub action: String,
    /// Resource accessed
    pub resource: String,
    /// Whether access was allowed
    pub allowed: bool,
    /// Reason for decision
    pub reason: String,
}
