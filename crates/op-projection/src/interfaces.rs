//! Trait interfaces for the Projection System.
//!
//! This module defines all the core trait interfaces that components must implement.

use crate::data_models::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// Schema registry trait for managing PluginSchema definitions.
///
/// This is the authoritative source for all schema definitions.
/// All projections must have a valid schema to exist on the system.
pub trait SchemaRegistry {
    /// Register a new schema version
    ///
    /// Returns the schema version number on success.
    fn register_schema(&mut self, schema: PluginSchema) -> Result<u64>;

    /// Validate a schema against the registry
    fn validate_schema(&self, schema: &PluginSchema) -> Result<ValidationResult>;

    /// Get the latest version of a schema by name
    fn get_schema(&self, name: &str) -> Option<&PluginSchema>;

    /// Get all versions of a schema
    fn get_schema_versions(&self, name: &str) -> Vec<&PluginSchema>;

    /// Check if an entity type has a valid schema
    fn has_valid_schema(&self, entity_type: &str) -> bool;

    /// Quarantine a schema and all associated entities
    fn quarantine_schema(&mut self, name: &str, reason: &str);

    /// Get all registered schema names
    fn list_schemas(&self) -> Vec<String>;

    /// Get schema version by name and version string
    fn get_schema_by_version(&self, name: &str, version: &str) -> Option<&PluginSchema>;
}

/// Schema validator trait for validating entities against schemas.
pub trait SchemaValidator {
    /// Validate an entity against its schema
    fn validate_entity(&self, entity: &RawEntity) -> Result<ValidationResult>;

    /// Get validation errors for an entity
    fn get_validation_errors(&self, entity: &RawEntity) -> Vec<ValidationError>;

    /// Validate a single field against its schema
    fn validate_field(&self, field: &FieldSchema, value: &Value) -> Result<ValidationResult>;

    /// Validate constraints on a value
    fn validate_constraints(
        &self,
        constraints: &[Constraint],
        value: &Value,
    ) -> Result<ValidationResult>;

    /// Get the schema for an entity type
    fn get_schema_for_entity(&self, entity_type: &str) -> Option<&PluginSchema>;
}

/// Raw entity structure for validation.
#[derive(Debug, Clone)]
pub struct RawEntity {
    /// Entity type (schema name)
    pub entity_type: String,
    /// Entity ID
    pub entity_id: String,
    /// Entity data
    pub data: Value,
    /// Source identifier
    pub source: String,
}

/// Projection engine trait for state transformation.
///
/// This is the core engine that transforms authoritative state into
/// schema-validated projections.
pub trait ProjectionEngine: std::fmt::Debug {
    /// Create a projection for an entity
    fn create_projection(&mut self, entity: RawEntity) -> Result<Projection>;

    /// Update an existing projection
    fn update_projection(&mut self, projection_id: &str, entity: RawEntity) -> Result<Projection>;

    /// Get a projection by ID
    fn get_projection(&self, projection_id: &str) -> Option<Projection>;

    /// Get all projections for an entity type
    fn get_projections_by_type(&self, entity_type: &str) -> Vec<Projection>;

    /// Get projections by validation state
    fn get_projections_by_state(&self, state: ProjectionState) -> Vec<Projection>;

    /// Mark a projection as quarantined
    fn quarantine_projection(&mut self, projection_id: &str, reason: &str);

    /// Mark a projection as degraded
    fn degrade_projection(
        &mut self,
        projection_id: &str,
        reason: &str,
        affected_dependencies: Vec<String>,
    );

    /// Re-validate all projections from a schema version
    fn revalidate_projections(&mut self, schema_name: &str, old_version: &str);

    /// Get all projections
    fn get_all_projections(&self) -> Vec<Projection>;

    /// Delete a projection
    fn delete_projection(&mut self, projection_id: &str) -> Result<()>;

    /// Get projections by source
    fn get_projections_by_source(&self, source: &str) -> Vec<Projection>;
}

/// Event materializer trait for event-driven projection updates.
///
/// This trait handles consuming events from the event bus and materializing
/// projections with 50ms processing guarantees.
pub trait EventMaterializer {
    /// Consume an event and materialize projections
    fn materialize(&mut self, event: &ProjectionEvent) -> Result<Vec<ProjectionUpdate>>;

    /// Get processing latency for the last batch
    fn get_processing_latency(&self) -> chrono::Duration;

    /// Check if event ordering is guaranteed
    fn has_ordering_guarantees(&self) -> bool;

    /// Handle unprocessable events
    fn quarantine_event(&mut self, event: &ProjectionEvent, reason: &str);

    /// Get the number of events processed
    fn get_events_processed(&self) -> u64;

    /// Get the number of events quarantined
    fn get_events_quarantined(&self) -> u64;
}

/// JSON-stream server trait for real-time UI delivery.
///
/// This trait provides SSE/WebSocket server functionality for streaming
/// projections to the UI.
pub trait JsonStreamServer {
    /// Start the JSON-stream server on the given port
    fn start(&mut self, port: u16) -> Result<()>;

    /// Broadcast updates to all connected clients
    fn broadcast(&self, update: &ProjectionUpdate);

    /// Send current state as batch, then stream updates
    fn handle_client(&self, client_id: &str) -> Result<()>;

    /// Stop streaming to a disconnected client
    fn disconnect_client(&self, client_id: &str);

    /// Get number of connected clients
    fn client_count(&self) -> usize;

    /// Get server status
    fn status(&self) -> JsonStreamStatus;
}

/// JSON-stream server status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonStreamStatus {
    /// Server is running
    pub running: bool,
    /// Port being served
    pub port: u16,
    /// Number of connected clients
    pub client_count: usize,
    /// Total clients served
    pub total_clients: u64,
    /// Messages sent
    pub messages_sent: u64,
}

/// Access controller trait for access control and redaction.
///
/// This trait enforces access control policies and redacts sensitive data.
pub trait AccessController {
    /// Enforce access control policies on projections
    fn enforce_policy(
        &self,
        projection: &Projection,
        requester: &Requester,
    ) -> Result<Projection>;

    /// Validate requester permissions
    fn validate_permissions(
        &self,
        requester: &Requester,
        action: &str,
        resource: &str,
    ) -> Result<()>;

    /// Redact sensitive data
    fn redact_sensitive(&self, data: &Value, requester: &Requester) -> Value;

    /// Log access control decisions
    fn log_decision(
        &self,
        requester: &Requester,
        action: &str,
        resource: &str,
        allowed: bool,
    );

    /// Check if data is accessible
    fn is_accessible(&self, data: &Value, requester: &Requester) -> bool;

    /// Add an access policy
    fn add_policy(&mut self, policy: AccessPolicy);

    /// Get all policies
    fn get_policies(&self) -> Vec<AccessPolicy>;

    /// Get audit trail
    fn get_audit_trail(&self) -> Vec<AccessControlAudit>;
}

/// Audit trail trait for immutable state change records.
pub trait AuditTrail {
    /// Create an audit entry
    fn create_entry(&mut self, entry: AuditProjection) -> Result<()>;

    /// Get audit entries for an entity
    fn get_entity_audit(&self, entity_id: &str) -> Vec<&AuditProjection>;

    /// Get audit entries for a time range
    fn get_time_range_audit(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&AuditProjection>;

    /// Get audit entries by change type
    fn get_audit_by_change_type(&self, change_type: ChangeType) -> Vec<&AuditProjection>;

    /// Get all audit entries
    fn get_all_audit(&self) -> Vec<&AuditProjection>;
}

/// Historical projection trait for temporal queries.
pub trait HistoricalStore {
    /// Store a historical projection
    fn store_historical(&mut self, projection: &Projection) -> Result<()>;

    /// Get historical projection at a specific timestamp
    fn get_at_time(&self, entity_id: &str, timestamp: DateTime<Utc>) -> Option<&HistoricalProjection>;

    /// Get historical projections in a time range
    fn get_in_range(
        &self,
        entity_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&HistoricalProjection>;

    /// Get change set for an entity
    fn get_change_set(&self, entity_id: &str) -> Vec<&HistoricalProjection>;

    /// Get all historical projections
    fn get_all_historical(&self) -> Vec<&HistoricalProjection>;
}

/// Source reader trait for reading from various sources.
pub trait SourceReader {
    /// Read all entities from the source
    fn read_all(&self) -> Result<Vec<RawEntity>>;

    /// Read a specific entity
    fn read_entity(&self, entity_id: &str) -> Result<RawEntity>;

    /// Get source identifier
    fn source_id(&self) -> &str;

    /// Check if source is available
    fn is_available(&self) -> bool;
}

/// Procfs reader trait for reading from /proc.
pub trait ProcfsReader: SourceReader {
    /// Read process information
    fn read_processes(&self) -> Result<Vec<RawEntity>>;

    /// Read memory information
    fn read_memory(&self) -> Result<RawEntity>;

    /// Read CPU information
    fn read_cpu(&self) -> Result<RawEntity>;

    /// Read filesystem information
    fn read_filesystems(&self) -> Result<RawEntity>;

    /// Read network information
    fn read_network(&self) -> Result<RawEntity>;
}

/// D-Bus reader trait for reading from D-Bus.
pub trait DbusReader: SourceReader {
    /// Read D-Bus objects
    fn read_dbus_objects(&self) -> Result<Vec<RawEntity>>;

    /// Read D-Bus properties
    fn read_dbus_properties(&self, path: &str) -> Result<RawEntity>;

    /// Watch D-Bus signals
    fn watch_signals(&self, handler: Box<dyn Fn(Vec<RawEntity>) + Send + Sync>);
}

/// gRPC reader trait for reading from gRPC services.
pub trait GrpcReader: SourceReader {
    /// Read gRPC services
    fn read_services(&self) -> Result<Vec<RawEntity>>;

    /// Read gRPC methods
    fn read_methods(&self, service: &str) -> Result<Vec<RawEntity>>;

    /// Read gRPC message types
    fn read_messages(&self, service: &str) -> Result<Vec<RawEntity>>;

    /// Read gRPC endpoints
    fn read_endpoints(&self) -> Result<Vec<RawEntity>>;
}

/// Plugin reader trait for reading from plugins.
pub trait PluginReader: SourceReader {
    /// Read plugin objects
    fn read_plugin_objects(&self, plugin_id: &str) -> Result<Vec<RawEntity>>;

    /// Read nested plugin objects
    fn read_nested_objects(&self, plugin_id: &str, parent_id: &str) -> Result<Vec<RawEntity>>;

    /// Handle plugin lifecycle events
    fn handle_lifecycle(&self, plugin_id: &str, event: PluginLifecycleEvent);
}

/// OVSDB mirror projection trait.
pub trait OvsdbMirrorProjection {
    /// Mirror an OVSDB table
    fn mirror_table(&mut self, table_name: &str, entities: Vec<RawEntity>) -> Result<()>;

    /// Update an OVSDB entry
    fn update_entry(&mut self, entity: RawEntity) -> Result<Projection>;

    /// Handle OVSDB disconnect
    fn handle_disconnect(&mut self);
}

/// Plugin lifecycle event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginLifecycleEvent {
    /// Plugin activated
    Activated,
    /// Plugin deactivated
    Deactivated,
    /// Plugin updated
    Updated,
    /// Plugin removed
    Removed,
}
