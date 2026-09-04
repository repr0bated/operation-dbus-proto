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
  bin/
    projection_server.rs
  access_control.rs
  data_models.rs
  dbus_reader.rs
  dbus_server.rs
  event_materializer.rs
  grpc_reader.rs
  interfaces.rs
  json_stream.rs
  lib.rs
  ovsdb_mirror.rs
  plugin_reader.rs
  procfs_reader.rs
  projection_engine.rs
  projection_store.rs
  schema_engine.rs
  schema_validator.rs
  sled_reader.rs
Cargo.toml
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/bin/projection_server.rs">
//! Projection Server: Main entry point for the projection system.
//!
//! This binary wires together all components of the projection system:
//! SchemaEngine, ProjectionEngine, EventMaterializer, and SourceReaders.

use anyhow::{Context, Result};
use op_projection::*;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, warn, Level};

// Builtin schemas from op-state-store — the absolute base for all projections.
use op_state_store;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Operation-DBus Projection Server");

    // 1. Initialize Schema Engine
    let mut schema_engine = SchemaEngine::new();

    // Register some initial schemas (in production, load from files)
    let memory_schema = PluginSchema {
        name: "system.memory".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![
            FieldSchema {
                name: "total_kb".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Total system memory in KB".to_string()),
                constraints: vec![Constraint::MinValue(0)],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "free_kb".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Free system memory in KB".to_string()),
                constraints: vec![Constraint::MinValue(0)],
                example: None,
                read_only: true,
            },
        ],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };

    schema_engine.register_schema(memory_schema)?;

    let cpu_schema = PluginSchema {
        name: "system.cpu".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![
            FieldSchema {
                name: "cores".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Number of CPU cores".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "model".to_string(),
                field_type: FieldType::String,
                required: true,
                description: Some("CPU model name".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
        ],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(cpu_schema)?;

    let network_schema = PluginSchema {
        name: "system.network".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![FieldSchema {
            name: "interfaces".to_string(),
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: true,
            description: Some("List of network interfaces".to_string()),
            constraints: vec![],
            example: None,
            read_only: true,
        }],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(network_schema)?;

    let sled_schema = PluginSchema {
        name: "identity.sled".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![
            FieldSchema {
                name: "mutation_index".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: Some("Current mutation index".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "hashed_footprint".to_string(),
                field_type: FieldType::String,
                required: true,
                description: Some("Blake3 hashed footprint".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
            FieldSchema {
                name: "wireguard_pubkey".to_string(),
                field_type: FieldType::String,
                required: true,
                description: Some("WireGuard public key".to_string()),
                constraints: vec![],
                example: None,
                read_only: true,
            },
        ],
        category: Some("identity".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(sled_schema)?;

    let process_schema = PluginSchema {
        name: "system.process".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![FieldSchema {
            name: "name".to_string(),
            field_type: FieldType::String,
            required: true,
            description: Some("Process name".to_string()),
            constraints: vec![],
            example: None,
            read_only: true,
        }],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(process_schema)?;

    let filesystems_schema = PluginSchema {
        name: "system.filesystems".to_string(),
        version: "1.0.0".to_string(),
        fields: vec![FieldSchema {
            name: "types".to_string(),
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: true,
            description: Some("Filesystem types listed by /proc/filesystems".to_string()),
            constraints: vec![],
            example: None,
            read_only: true,
        }],
        category: Some("system".to_string()),
        examples: None,
        secret_paths: vec![],
        pii_paths: vec![],
    };
    schema_engine.register_schema(filesystems_schema)?;

    let plugin_reader = match SystemPluginReader::new().await {
        Ok(reader) => reader,
        Err(error) => {
            warn!(
                error = %error,
                "Failed to initialize plugin projection reader; continuing with non-plugin sources"
            );
            SystemPluginReader::empty()
        }
    };

    for schema in plugin_reader.projection_schemas() {
        register_schema_if_missing(&mut schema_engine, schema)?;
    }

    // Register all builtin schemas from op-state-store so the shm catalog
    // is the single source of truth for UI, snowball, everything.
    // These include web_ui, mcp, wireguard, incus, openflow, etc.
    for runtime_schema in op_state_store::builtin_plugin_schemas() {
        let schema = convert_schema(&runtime_schema);
        register_schema_if_missing(&mut schema_engine, schema)?;
    }

    info!("Registered initial schemas ({} total)", schema_engine.list_schemas().len());

    // 2. Initialize Projection Store and Engine
    let store = ProjectionStore::new();
    let validator = SchemaValidator::new(schema_engine.clone());
    let engine = Arc::new(Mutex::new(ProjectionSystemEngine::new(
        store.clone(),
        validator,
    )));

    // 3. Initialize Source Readers
    let procfs_reader = SystemProcfsReader::new();
    let sled_reader = IdentitySledReader::new();
    let _dbus_reader = SystemDbusReader::new();
    let _grpc_reader = SystemGrpcReader::new();

    info!("Initialized source readers");

    // 4. Initialize JSON-stream Server
    let mut stream_server = ProjectionStreamServer::new();
    stream_server.start(8082)?;
    let mut dbus_server = ProjectionDbusServer::new()
        .await
        .context("failed to start projection D-Bus server")?;

    // 5. Initial Scan and Projection
    {
        let mut initial_entities = Vec::new();

        info!("Performing initial scan...");

        if procfs_reader.is_available() {
            initial_entities.extend(procfs_reader.read_all()?);
        }

        if sled_reader.is_available() {
            if let Ok(entities) = sled_reader.read_all() {
                initial_entities.extend(entities);
            }
        }

        if plugin_reader.is_available() {
            match plugin_reader.read_all_async().await {
                Ok(entities) => initial_entities.extend(entities),
                Err(error) => warn!(error = %error, "Failed to read plugin projection entities"),
            }
        }

        for entity in initial_entities {
            let projection = {
                let mut engine_lock = engine.lock();
                engine_lock.create_projection(entity)?
            };
            dbus_server.upsert(&projection).await?;
            stream_server.broadcast(&ProjectionUpdate {
                update_type: UpdateType::Created,
                projection,
                timestamp: chrono::Utc::now(),
            });
        }

        info!("Initial scan complete");
    }

    // 6. Initialize Access Controller
    let mut access_controller = ProjectionAccessController::new();
    access_controller.add_policy(AccessPolicy {
        id: "allow-all-read".to_string(),
        resource_pattern: ".*".to_string(),
        required_permissions: vec![],
        action: "read".to_string(),
        redact_sensitive: false,
    });

    info!("Projection Server is ready");

    // 7. Keep-alive loop (in production, this would be the event loop)
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        let mut refresh_entities = Vec::new();

        // Periodic refresh from procfs
        if let Ok(entities) = procfs_reader.read_all() {
            refresh_entities.extend(entities);
        }

        // Periodic refresh from Sled
        if let Ok(entities) = sled_reader.read_all() {
            refresh_entities.extend(entities);
        }

        if plugin_reader.is_available() {
            match plugin_reader.read_all_async().await {
                Ok(entities) => refresh_entities.extend(entities),
                Err(error) => warn!(
                    error = %error,
                    "Failed to refresh plugin projection entities"
                ),
            }
        }

        for entity in refresh_entities {
            let update = {
                let mut engine_lock = engine.lock();
                engine_lock.create_projection(entity)?
            };
            dbus_server.upsert(&update).await?;

            stream_server.broadcast(&ProjectionUpdate {
                update_type: UpdateType::Updated,
                projection: update,
                timestamp: chrono::Utc::now(),
            });
        }

        info!("Periodic refresh complete");
    }
}

fn register_schema_if_missing(
    schema_engine: &mut SchemaEngine,
    schema: PluginSchema,
) -> Result<()> {
    if schema_engine.has_valid_schema(&schema.name) {
        return Ok(());
    }

    let schema_name = schema.name.clone();
    schema_engine
        .register_schema(schema)
        .with_context(|| format!("failed to register projection schema '{}'", schema_name))?;
    Ok(())
}
</file>

<file path="src/access_control.rs">
//! Access Controller: Access control and redaction.
//!
//! This module implements the `AccessController` trait, enforcing access
//! control policies and redacting sensitive data (secrets, PII).

use crate::data_models::*;
use crate::interfaces::AccessController;
use anyhow::Result;
use parking_lot::RwLock;
use regex::Regex;
use std::sync::Arc;
use tracing::{debug, warn};

/// Controller that enforces security policies on projections.
#[derive(Debug, Clone)]
pub struct ProjectionAccessController {
    /// Active access policies
    policies: Arc<RwLock<Vec<AccessPolicy>>>,
    /// Audit trail for decisions
    audit_trail: Arc<RwLock<Vec<AccessControlAudit>>>,
}

impl ProjectionAccessController {
    /// Creates a new ProjectionAccessController
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(Vec::new())),
            audit_trail: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for ProjectionAccessController {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessController for ProjectionAccessController {
    fn enforce_policy(&self, projection: &Projection, requester: &Requester) -> Result<Projection> {
        let mut result = projection.clone();

        // Validate access
        self.validate_permissions(requester, "read", &projection.id)?;

        // Check if redaction is needed
        let policies = self.policies.read();
        for policy in policies.iter() {
            let re = Regex::new(&policy.resource_pattern)?;
            if re.is_match(&projection.id) && policy.redact_sensitive {
                result.data = self.redact_sensitive(&result.data, requester);
            }
        }

        Ok(result)
    }

    fn validate_permissions(
        &self,
        requester: &Requester,
        action: &str,
        resource: &str,
    ) -> Result<()> {
        let policies = self.policies.read();
        let mut allowed = false;

        for policy in policies.iter() {
            if policy.action == action {
                let re = Regex::new(&policy.resource_pattern)?;
                if re.is_match(resource) {
                    // Check permissions
                    if policy.required_permissions.is_empty() {
                        allowed = true;
                        break;
                    }

                    for req_perm in &policy.required_permissions {
                        if requester.permissions.contains(req_perm) {
                            allowed = true;
                            break;
                        }
                    }
                }
            }
        }

        // Log decision
        self.log_decision(requester, action, resource, allowed);

        if allowed {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Permission denied: {} on {}",
                action,
                resource
            ))
        }
    }

    fn redact_sensitive(
        &self,
        data: &simd_json::OwnedValue,
        _requester: &Requester,
    ) -> simd_json::OwnedValue {
        // In production, use JSON paths from schema to redact
        data.clone()
    }

    fn log_decision(&self, requester: &Requester, action: &str, resource: &str, allowed: bool) {
        let audit = AccessControlAudit {
            timestamp: chrono::Utc::now(),
            requester_id: requester.id.clone(),
            action: action.to_string(),
            resource: resource.to_string(),
            allowed,
            reason: if allowed {
                "Policy match".to_string()
            } else {
                "No policy match".to_string()
            },
        };

        self.audit_trail.write().push(audit);

        if !allowed {
            warn!(
                requester_id = requester.id,
                action = action,
                resource = resource,
                "Access denied"
            );
        } else {
            debug!(
                requester_id = requester.id,
                action = action,
                resource = resource,
                "Access granted"
            );
        }
    }

    fn is_accessible(&self, _data: &simd_json::OwnedValue, _requester: &Requester) -> bool {
        // Simplified check
        true
    }

    fn add_policy(&mut self, policy: AccessPolicy) {
        let mut policies = self.policies.write();
        policies.push(policy);
    }

    fn get_policies(&self) -> Vec<AccessPolicy> {
        self.policies.read().clone()
    }

    fn get_audit_trail(&self) -> Vec<AccessControlAudit> {
        self.audit_trail.read().clone()
    }
}
</file>

<file path="src/data_models.rs">
//! Core data structures for the Projection System.
//!
//! This module defines all shared data models used throughout the projection system,
//! including schemas, projections, and their various specialized types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// The authoritative JSON schema that defines the structure and validation rules
/// for all plugin-provided objects. If no valid schema exists, the entity does not
/// exist on the system.
///
/// This is the single source of truth for all projections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginSchema {
    /// Unique name of the schema
    pub name: String,
    /// Version string (e.g., "1.0.0")
    pub version: String,
    /// Fields defined by this schema
    pub fields: Vec<FieldSchema>,
    /// Optional category for grouping schemas
    pub category: Option<String>,
    /// Example data instances
    pub examples: Option<Vec<Value>>,
    /// Paths to fields containing secrets (for redaction)
    pub secret_paths: Vec<String>,
    /// Paths to fields containing PII (for redaction)
    pub pii_paths: Vec<String>,
}

/// A field definition within a PluginSchema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldSchema {
    /// Field name
    pub name: String,
    /// Field type
    #[serde(rename = "type")]
    pub field_type: FieldType,
    /// Whether the field is required
    pub required: bool,
    /// Human-readable description
    pub description: Option<String>,
    /// Validation constraints
    pub constraints: Vec<Constraint>,
    /// Example value
    pub example: Option<Value>,
    /// Whether the field is read-only
    pub read_only: bool,
}

/// Supported field types in PluginSchema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FieldType {
    /// String type
    String,
    /// Integer type
    Integer,
    /// Number (floating-point) type
    Number,
    /// Boolean type
    Boolean,
    /// Nested object type
    Object,
    /// Array of items
    Array(Box<FieldType>),
    /// Enumerated string values
    Enum(Vec<String>),
    /// Any type (unconstrained)
    Any,
}

/// Validation constraints for fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Constraint {
    /// Minimum string length
    MinLength(usize),
    /// Maximum string length
    MaxLength(usize),
    /// Minimum numeric value
    MinValue(i64),
    /// Maximum numeric value
    MaxValue(i64),
    /// Regular expression pattern
    Pattern(String),
    /// Enumerated allowed values
    Enum(Vec<String>),
}

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
</file>

<file path="src/dbus_reader.rs">
//! D-Bus Reader: Reading from D-Bus.
//!
//! This module implements the `DbusReader` trait, scanning D-Bus objects
//! and projecting properties into raw entities using zbus.

use crate::interfaces::{DbusReader, RawEntity, SourceReader};
use anyhow::Result;
use simd_json::json;
use std::collections::HashMap;
use tracing::{debug, warn};
use zbus::fdo::{DBusProxy, IntrospectableProxy};
use zbus::Connection;

/// Reader that extracts state from the D-Bus system bus.
#[derive(Debug)]
pub struct SystemDbusReader {
    /// Source identifier
    source: String,
}

impl SystemDbusReader {
    /// Creates a new SystemDbusReader
    pub fn new() -> Self {
        Self {
            source: "dbus".to_string(),
        }
    }

    /// Helper to introspect a D-Bus path
    async fn introspect(
        &self,
        conn: &Connection,
        service: &str,
        path: &str,
    ) -> Result<Vec<RawEntity>> {
        let proxy = IntrospectableProxy::builder(conn)
            .destination(service)?
            .path(path)?
            .build()
            .await?;

        let xml = proxy.introspect().await?;
        let mut entities = Vec::new();

        // Very basic XML parsing for children
        // In production, use a proper XML parser
        let mut children = Vec::new();
        for line in xml.lines() {
            if line.contains("<node name=\"") {
                if let Some(name) = line
                    .split("name=\"")
                    .nth(1)
                    .and_then(|s| s.split('\"').next())
                {
                    if !name.is_empty() {
                        children.push(name.to_string());
                    }
                }
            }
        }

        for child in children {
            let child_path = if path == "/" {
                format!("/{}", child)
            } else {
                format!("{}/{}", path, child)
            };

            entities.push(RawEntity {
                entity_type: "dbus.object".to_string(),
                entity_id: format!("{}:{}", service, child_path),
                data: json!({
                    "service": service,
                    "path": child_path,
                })
                .into(),
                source: self.source.clone(),
            });
        }

        Ok(entities)
    }
}

impl Default for SystemDbusReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for SystemDbusReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        // This is a bit tricky because it's async
        // For now, return a placeholder or use a block_on (not recommended)
        Ok(Vec::new())
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        Ok(RawEntity {
            entity_type: "dbus.object".to_string(),
            entity_id: entity_id.to_string(),
            data: json!({ "properties": {} }).into(),
            source: self.source.clone(),
        })
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        true
    }
}

impl DbusReader for SystemDbusReader {
    fn read_dbus_objects(&self) -> Result<Vec<RawEntity>> {
        Ok(Vec::new())
    }

    fn read_dbus_properties(&self, path: &str) -> Result<RawEntity> {
        Ok(RawEntity {
            entity_type: "dbus.object".to_string(),
            entity_id: path.to_string(),
            data: json!({ "properties": {} }).into(),
            source: self.source.clone(),
        })
    }

    fn watch_signals(&self, _handler: Box<dyn Fn(Vec<RawEntity>) + Send + Sync>) {
        debug!("Watching D-Bus signals");
    }
}
</file>

<file path="src/dbus_server.rs">
//! D-Bus object server for projections.
//!
//! Serves every Projection as a D-Bus object under org.opdbus.projection at
//! /org/opdbus/<category>/<id>, e.g. /org/opdbus/system/memory or
//! /org/opdbus/system/process/1234.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection};

use crate::data_models::Projection;

/// A single projected object on the D-Bus object server.
pub struct ProjectedObject {
    pub entity_type: String,
    pub entity_id: String,
    /// JSON-serialized projection data
    pub data_json: Arc<RwLock<String>>,
    pub state: Arc<RwLock<String>>,
}

#[interface(name = "org.opdbus.projection.v1.Object")]
impl ProjectedObject {
    /// The schema/entity type for this object (e.g. "system.memory")
    #[zbus(property)]
    async fn entity_type(&self) -> &str {
        &self.entity_type
    }

    /// The unique entity ID within its type
    #[zbus(property)]
    async fn entity_id(&self) -> &str {
        &self.entity_id
    }

    /// Current projection state: Valid, Quarantined, Degraded, etc.
    #[zbus(property)]
    async fn state(&self) -> String {
        self.state.read().await.clone()
    }

    /// Full projection data as a JSON string
    #[zbus(property)]
    async fn data(&self) -> String {
        self.data_json.read().await.clone()
    }

    /// Signal emitted when this object's data changes
    #[zbus(signal)]
    async fn updated(emitter: &SignalEmitter<'_>, data_json: &str) -> zbus::Result<()>;
}

/// Derives the D-Bus object path from a projection's entity_type and entity_id.
///
/// entity_type "system.memory"    → /org/opdbus/system/memory
/// entity_type "system.process"   → /org/opdbus/system/process/<entity_id>
/// entity_type "identity.sled"    → /org/opdbus/identity/sled
/// entity_type "ovsdb_bridge"     → /org/opdbus/ovsdb/bridge/<entity_id>
pub fn projection_path(entity_type: &str, entity_id: &str) -> String {
    // Replace dots and underscores in type with slashes for the path prefix
    let type_path = entity_type
        .replace('.', "/")
        .replace('_', "/")
        .to_lowercase();

    // For singleton objects (memory, cpu, filesystems, network) the entity_id
    // is typically the same as the type — omit it to avoid redundancy.
    let is_singleton = entity_id == entity_type
        || entity_id.is_empty()
        || entity_id == "memory"
        || entity_id == "cpu"
        || entity_id == "filesystems"
        || entity_id == "network"
        || entity_id == "sled";

    if is_singleton {
        format!("/org/opdbus/{}", type_path)
    } else {
        // Sanitize entity_id for use in a path segment
        let safe_id: String = entity_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("/org/opdbus/{}/{}", type_path, safe_id)
    }
}

/// Manages the set of D-Bus objects served for all projections.
pub struct ProjectionDbusServer {
    conn: Connection,
    /// path → data/state handles so we can update in place
    objects: HashMap<String, (Arc<RwLock<String>>, Arc<RwLock<String>>)>,
}

impl ProjectionDbusServer {
    pub async fn new() -> Result<Self> {
        let conn = match std::env::var("OP_DBUS_PROJECTION_BUS")
            .unwrap_or_else(|_| "system".to_string())
            .as_str()
        {
            "session" => {
                Builder::session()?
                    .name("org.opdbus.projection")?
                    .build()
                    .await?
            }
            _ => {
                Builder::system()?
                    .name("org.opdbus.projection")?
                    .build()
                    .await?
            }
        };

        info!("D-Bus connection established for org.opdbus.projection");

        Ok(Self {
            conn,
            objects: HashMap::new(),
        })
    }

    pub async fn new_session() -> Result<Self> {
        let conn = Builder::session()?
            .name("org.opdbus.projection")?
            .build()
            .await?;

        info!("D-Bus session bus connection established for org.opdbus.projection");

        Ok(Self {
            conn,
            objects: HashMap::new(),
        })
    }

    /// Register a projection as a D-Bus object (or update it if already registered).
    pub async fn upsert(&mut self, projection: &Projection) -> Result<()> {
        let path = projection_path(&projection.entity_type, &projection.entity_id);
        let data_json = simd_json::to_string(&projection.data).unwrap_or_else(|_| "{}".to_string());
        let state_str = format!("{:?}", projection.state);

        if let Some((data_handle, state_handle)) = self.objects.get(&path) {
            // Update existing object in place
            *data_handle.write().await = data_json.clone();
            *state_handle.write().await = state_str;

            // Emit the updated signal
            let iface_ref = self
                .conn
                .object_server()
                .interface::<_, ProjectedObject>(path.as_str())
                .await?;
            ProjectedObject::updated(iface_ref.signal_emitter(), &data_json).await?;

            debug!(path, "updated D-Bus projection object");
        } else {
            // Register new object
            let data_arc = Arc::new(RwLock::new(data_json));
            let state_arc = Arc::new(RwLock::new(state_str));

            let obj = ProjectedObject {
                entity_type: projection.entity_type.clone(),
                entity_id: projection.entity_id.clone(),
                data_json: data_arc.clone(),
                state: state_arc.clone(),
            };

            self.conn.object_server().at(path.as_str(), obj).await?;

            self.objects.insert(path.clone(), (data_arc, state_arc));
            info!(path, entity_type = %projection.entity_type, "registered D-Bus projection object");
        }

        Ok(())
    }

    /// Remove a projection's D-Bus object.
    pub async fn remove(&mut self, entity_type: &str, entity_id: &str) -> Result<()> {
        let path = projection_path(entity_type, entity_id);
        if self.objects.remove(&path).is_some() {
            self.conn
                .object_server()
                .remove::<ProjectedObject, _>(path.as_str())
                .await?;
            info!(path, "removed D-Bus projection object");
        }
        Ok(())
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
}
</file>

<file path="src/event_materializer.rs">
//! Event Materializer: Event-driven projection updates.
//!
//! This module implements the `EventMaterializer` trait, which handles
//! consuming events from the event bus and materializing projections
//! with 50ms processing guarantees.

use crate::data_models::*;
use crate::interfaces::{EventMaterializer, ProjectionEngine, RawEntity};
use anyhow::Result;
use chrono::Utc;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Materializer that transforms events into projection updates.
#[derive(Debug)]
pub struct ProjectionMaterializer {
    /// Reference to the projection engine
    engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>,
    /// Number of events processed
    events_processed: u64,
    /// Number of events quarantined
    events_quarantined: u64,
    /// Latency of the last processing batch
    last_latency: chrono::Duration,
}

impl ProjectionMaterializer {
    /// Creates a new ProjectionMaterializer with given engine
    pub fn new(engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>) -> Self {
        Self {
            engine,
            events_processed: 0,
            events_quarantined: 0,
            last_latency: chrono::Duration::zero(),
        }
    }
}

impl EventMaterializer for ProjectionMaterializer {
    fn materialize(&mut self, event: &ProjectionEvent) -> Result<Vec<ProjectionUpdate>> {
        let start_time = Utc::now();
        let mut updates = Vec::new();

        debug!(
            event_id = event.id,
            event_type = ?event.event_type,
            "Materializing event"
        );

        match event.event_type {
            EventType::Created | EventType::Updated => {
                let entity = RawEntity {
                    entity_type: event.entity_type.clone(),
                    entity_id: event.entity_id.clone(),
                    data: event.data.clone(),
                    source: event.source.clone(),
                };

                let result = {
                    let mut engine = self.engine.lock();
                    engine.create_projection(entity)
                };

                match result {
                    Ok(projection) => {
                        updates.push(ProjectionUpdate {
                            update_type: if event.event_type == EventType::Created {
                                UpdateType::Created
                            } else {
                                UpdateType::Updated
                            },
                            projection,
                            timestamp: Utc::now(),
                        });
                        self.events_processed += 1;
                    }
                    Err(e) => {
                        self.quarantine_event(event, &format!("Failed to project: {}", e));
                    }
                }
            }
            EventType::Deleted => {
                let projection_id = format!("{}:{}", event.entity_type, event.entity_id);

                let maybe_projection = {
                    let mut engine = self.engine.lock();
                    // Get projection before deletion for the update
                    if let Some(projection) = engine.get_projection(&projection_id) {
                        if let Err(e) = engine.delete_projection(&projection_id) {
                            warn!(projection_id = projection_id, error = %e, "Failed to delete projection");
                            None
                        } else {
                            Some(projection)
                        }
                    } else {
                        None
                    }
                };

                if let Some(projection) = maybe_projection {
                    updates.push(ProjectionUpdate {
                        update_type: UpdateType::Deleted,
                        projection,
                        timestamp: Utc::now(),
                    });
                    self.events_processed += 1;
                }
            }
            _ => {
                debug!(event_type = ?event.event_type, "Ignoring non-materializable event");
            }
        }

        self.last_latency = Utc::now().signed_duration_since(start_time);

        // Ensure 50ms guarantee (log warning if exceeded)
        if self.last_latency > chrono::Duration::milliseconds(50) {
            warn!(
                latency_ms = self.last_latency.num_milliseconds(),
                "Materialization latency exceeded 50ms guarantee"
            );
        }

        Ok(updates)
    }

    fn get_processing_latency(&self) -> chrono::Duration {
        self.last_latency
    }

    fn has_ordering_guarantees(&self) -> bool {
        // This implementation processes events sequentially via the lock
        true
    }

    fn quarantine_event(&mut self, event: &ProjectionEvent, reason: &str) {
        self.events_quarantined += 1;
        error!(event_id = event.id, reason = reason, "Event quarantined");
    }

    fn get_events_processed(&self) -> u64 {
        self.events_processed
    }

    fn get_events_quarantined(&self) -> u64 {
        self.events_quarantined
    }
}
</file>

<file path="src/grpc_reader.rs">
//! gRPC Reader: Reading from gRPC services.
//!
//! This module implements the `GrpcReader` trait, discovering gRPC services
//! and projecting methods and message types into raw entities.

use crate::interfaces::{GrpcReader, RawEntity, SourceReader};
use anyhow::Result;
use simd_json::json;
use tracing::debug;

/// Reader that extracts state from gRPC services.
#[derive(Debug)]
pub struct SystemGrpcReader {
    /// Source identifier
    source: String,
}

impl SystemGrpcReader {
    /// Creates a new SystemGrpcReader
    pub fn new() -> Self {
        Self {
            source: "grpc".to_string(),
        }
    }
}

impl Default for SystemGrpcReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for SystemGrpcReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        self.read_services()
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        debug!(entity_id = entity_id, "Reading gRPC entity");
        Ok(RawEntity {
            entity_type: "grpc.service".to_string(),
            entity_id: entity_id.to_string(),
            data: json!({ "methods": [] }).into(),
            source: self.source.clone(),
        })
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        true
    }
}

impl GrpcReader for SystemGrpcReader {
    fn read_services(&self) -> Result<Vec<RawEntity>> {
        debug!("Discovering gRPC services");
        Ok(Vec::new())
    }

    fn read_methods(&self, service: &str) -> Result<Vec<RawEntity>> {
        debug!(service = service, "Reading gRPC methods");
        Ok(Vec::new())
    }

    fn read_messages(&self, service: &str) -> Result<Vec<RawEntity>> {
        debug!(service = service, "Reading gRPC message types");
        Ok(Vec::new())
    }

    fn read_endpoints(&self) -> Result<Vec<RawEntity>> {
        debug!("Reading gRPC endpoints");
        Ok(Vec::new())
    }
}
</file>

<file path="src/interfaces.rs">
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
    fn enforce_policy(&self, projection: &Projection, requester: &Requester) -> Result<Projection>;

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
    fn log_decision(&self, requester: &Requester, action: &str, resource: &str, allowed: bool);

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
    fn get_at_time(
        &self,
        entity_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Option<&HistoricalProjection>;

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
</file>

<file path="src/json_stream.rs">
//! JSON-stream Server: Real-time UI delivery.
//!
//! This module implements the `JsonStreamServer` trait, providing SSE/WebSocket
//! server functionality for streaming projections to the UI using Axum.

use crate::data_models::*;
use crate::interfaces::{JsonStreamServer, JsonStreamStatus};
use anyhow::Result;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use dashmap::DashMap;
use futures::stream::{self, Stream};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{info, warn};

/// Internal state for the Axum server
struct ServerState {
    tx: broadcast::Sender<ProjectionUpdate>,
    client_count: Arc<std::sync::atomic::AtomicUsize>,
    total_clients: Arc<std::sync::atomic::AtomicU64>,
    snapshot: Arc<DashMap<String, Projection>>,
}

/// Server that streams projection updates to connected clients.
#[derive(Debug)]
pub struct ProjectionStreamServer {
    /// Port to listen on
    port: u16,
    /// Whether the server is running
    running: bool,
    /// Channel for broadcasting updates
    tx: broadcast::Sender<ProjectionUpdate>,
    /// Number of connected clients
    client_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Total clients served
    total_clients: Arc<std::sync::atomic::AtomicU64>,
    /// Total messages sent
    messages_sent: Arc<std::sync::atomic::AtomicU64>,
    /// Latest known projection set for batch-on-connect delivery
    snapshot: Arc<DashMap<String, Projection>>,
}

impl ProjectionStreamServer {
    /// Creates a new ProjectionStreamServer
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            port: 0,
            running: false,
            tx,
            client_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            total_clients: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            messages_sent: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            snapshot: Arc::new(DashMap::new()),
        }
    }
}

impl Default for ProjectionStreamServer {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonStreamServer for ProjectionStreamServer {
    fn start(&mut self, port: u16) -> Result<()> {
        self.port = port;
        self.running = true;

        let state = Arc::new(ServerState {
            tx: self.tx.clone(),
            client_count: self.client_count.clone(),
            total_clients: self.total_clients.clone(),
            snapshot: self.snapshot.clone(),
        });

        let app = Router::new()
            .route("/events", get(sse_handler))
            .with_state(state);

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

        info!(port = port, "Starting JSON-stream SSE server");

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    warn!(error = %e, "Failed to bind JSON-stream server");
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app).await {
                warn!(error = %e, "JSON-stream server error");
            }
        });

        info!(port = port, "JSON-stream server started in background");
        Ok(())
    }

    fn broadcast(&self, update: &ProjectionUpdate) {
        match update.update_type {
            UpdateType::Deleted => {
                self.snapshot.remove(&update.projection.id);
            }
            _ => {
                self.snapshot
                    .insert(update.projection.id.clone(), update.projection.clone());
            }
        }

        if !self.running {
            return;
        }

        if let Err(_e) = self.tx.send(update.clone()) {
            // This is expected if no clients are connected
        } else {
            self.messages_sent
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn handle_client(&self, client_id: &str) -> Result<()> {
        info!(client_id = client_id, "New client connected to JSON-stream");
        self.client_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.total_clients
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn disconnect_client(&self, client_id: &str) {
        info!(
            client_id = client_id,
            "Client disconnected from JSON-stream"
        );
        self.client_count
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn client_count(&self) -> usize {
        self.client_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn status(&self) -> JsonStreamStatus {
        JsonStreamStatus {
            running: self.running,
            port: self.port,
            client_count: self.client_count(),
            total_clients: self.total_clients.load(std::sync::atomic::Ordering::SeqCst),
            messages_sent: self.messages_sent.load(std::sync::atomic::Ordering::SeqCst),
        }
    }
}

/// SSE handler for Axum
async fn sse_handler(
    State(state): State<Arc<ServerState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    state
        .client_count
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    state
        .total_clients
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let rx = state.tx.subscribe();
    let mut snapshot = state
        .snapshot
        .iter()
        .map(|projection| projection.value().clone())
        .collect::<Vec<_>>();
    snapshot.sort_by(|left, right| left.id.cmp(&right.id));

    let initial = stream::iter(snapshot.into_iter().map(|projection| {
        let data = serde_json::to_string(&ProjectionUpdate {
            update_type: UpdateType::Created,
            projection,
            timestamp: chrono::Utc::now(),
        })
        .unwrap_or_default();
        Ok(Event::default().event("projection_update").data(data))
    }));

    let live = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(update) => {
            let data = serde_json::to_string(&update).unwrap_or_default();
            Some(Ok(Event::default().event("projection_update").data(data)))
        }
        Err(_) => None,
    });

    // Add keepalive
    let keepalive = stream::repeat_with(|| Ok(Event::default().comment("keepalive")))
        .throttle(std::time::Duration::from_secs(30));

    let combined = stream::select(initial.chain(live), keepalive);

    Sse::new(combined).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}
</file>

<file path="src/lib.rs">
//! Projection System: Schema-validated state transformation engine
//!
//! The Projection System transforms live authoritative state from diverse sources
//! (D-Bus, gRPC, procfs, plugin-defined objects) into schema-validated derived
//! projections for dashboards, orchestration context, audit/compliance, topology,
//! AI context, and historical replay.
//!
//! # Core Principles
//!
//! - **Schema-as-Code Authority**: `PluginSchema` is the single source of truth
//! - **1:1 Direct Read (Zero-Copy)**: Reads from The Sled (`/dev/shm`)
//! - **Zero-Btrfs Overhead**: Identity extraction uses in-memory environment variables
//! - **The Accountability Loop**: `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID`
//!
//! # Modules
//!
//! - `data_models`: Core data structures for projections
//! - `interfaces`: Trait interfaces for projection components
//! - `schema_engine`: Schema registry and validation
//! - `schema_validator`: Entity validation services
//! - `projection_engine`: State transformation engine
//! - `event_materializer`: Event-driven projection updates
//! - `json_stream`: Real-time UI delivery
//! - `access_control`: Access control and redaction

pub mod access_control;
pub mod data_models;
pub mod dbus_reader;
pub mod dbus_server;
pub mod event_materializer;
pub mod grpc_reader;
pub mod interfaces;
pub mod json_stream;
pub mod ovsdb_mirror;
pub mod plugin_reader;
pub mod procfs_reader;
pub mod projection_engine;
pub mod projection_store;
pub mod schema_engine;
pub mod schema_validator;

// Re-export core types
pub mod sled_reader;

pub use access_control::ProjectionAccessController;
pub use data_models::*;
pub use dbus_reader::SystemDbusReader;
pub use dbus_server::ProjectionDbusServer;
pub use event_materializer::ProjectionMaterializer;
pub use grpc_reader::SystemGrpcReader;
pub use interfaces::*;
pub use json_stream::ProjectionStreamServer;
pub use ovsdb_mirror::OvsdbMirrorProjectionImpl;
pub use plugin_reader::{convert_schema, SystemPluginReader};
pub use procfs_reader::SystemProcfsReader;
pub use projection_engine::ProjectionSystemEngine;
pub use projection_store::ProjectionStore;
pub use schema_engine::SchemaEngine;
pub use schema_validator::SchemaValidator;
pub use sled_reader::IdentitySledReader;
</file>

<file path="src/ovsdb_mirror.rs">
//! OVSDB Mirror: Specialized projection for Open vSwitch.
//!
//! This module implements a dedicated projection for OVSDB tables,
//! allowing 1:1 mirroring of OVSDB state into the projection system.

use crate::data_models::*;
use crate::interfaces::{OvsdbMirrorProjection, ProjectionEngine, RawEntity};
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{debug, warn};

/// Specialized projection for Open vSwitch OVSDB mirroring.
#[derive(Debug, Clone)]
pub struct OvsdbMirrorProjectionImpl {
    /// Reference to the projection engine
    engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>,
}

impl OvsdbMirrorProjectionImpl {
    /// Creates a new OvsdbMirrorProjection
    pub fn new(engine: Arc<Mutex<dyn ProjectionEngine + Send + Sync>>) -> Self {
        Self { engine }
    }
}

impl OvsdbMirrorProjection for OvsdbMirrorProjectionImpl {
    fn mirror_table(&mut self, table_name: &str, entities: Vec<RawEntity>) -> Result<()> {
        debug!(
            table_name = table_name,
            count = entities.len(),
            "Mirroring OVSDB table"
        );

        let mut engine = self.engine.lock();
        for entity in entities {
            engine.create_projection(entity)?;
        }

        Ok(())
    }

    fn update_entry(&mut self, entity: RawEntity) -> Result<Projection> {
        let mut engine = self.engine.lock();
        engine.create_projection(entity)
    }

    fn handle_disconnect(&mut self) {
        warn!("OVSDB disconnected; degrading projections");
        let mut engine = self.engine.lock();
        for p in engine.get_all_projections() {
            if p.entity_type.starts_with("ovsdb.") {
                engine.degrade_projection(&p.id, "OVSDB connection lost", Vec::new());
            }
        }
    }
}
</file>

<file path="src/plugin_reader.rs">
//! Plugin Reader: Reading from plugins.
//!
//! This module implements the `PluginReader` trait by loading the default
//! runtime plugins, querying their live state, and emitting both top-level
//! plugin state entities and nested object projections.

use crate::data_models::{Constraint, FieldSchema, FieldType, PluginSchema};
use crate::interfaces::{PluginLifecycleEvent, PluginReader, RawEntity, SourceReader};
use anyhow::{Context, Result};
use op_plugins::DefaultPluginRegistry;
use op_state::StatePlugin;
use op_state_store::{
    builtin_plugin_schema, Constraint as RuntimeConstraint, FieldSchema as RuntimeFieldSchema,
    FieldType as RuntimeFieldType, PluginSchema as RuntimePluginSchema, MemoryStore, StateStore,
};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::future::Future;
use std::sync::Arc;
use tracing::{debug, info, warn};

struct LoadedPlugin {
    name: String,
    schema: Option<RuntimePluginSchema>,
    plugin: Arc<dyn StatePlugin>,
}

/// Reader that extracts live state from runtime plugins.
pub struct SystemPluginReader {
    /// Source identifier
    source: String,
    /// Loaded runtime plugins and their resolved schemas
    plugins: Vec<LoadedPlugin>,
}

impl std::fmt::Debug for SystemPluginReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let plugin_names: Vec<&str> = self
            .plugins
            .iter()
            .map(|plugin| plugin.name.as_str())
            .collect();
        f.debug_struct("SystemPluginReader")
            .field("source", &self.source)
            .field("plugins", &plugin_names)
            .finish()
    }
}

impl SystemPluginReader {
    /// Creates an empty reader when plugin bootstrap is unavailable.
    pub fn empty() -> Self {
        Self {
            source: "plugin".to_string(),
            plugins: Vec::new(),
        }
    }

    /// Creates a new SystemPluginReader backed by the default runtime plugins.
    /// Uses MemoryStore — no SQLite, zero drift. Current state = desired state.
    pub async fn new() -> Result<Self> {
        let state_store: Arc<dyn StateStore> = Arc::new(MemoryStore::new());

        let registry = DefaultPluginRegistry::new(state_store);
        let plugins = registry.load_default_plugins().await?;
        let plugins = plugins
            .into_iter()
            .map(|plugin| {
                let name = plugin.name().to_string();
                let schema = plugin.schema().or_else(|| builtin_plugin_schema(&name));

                if schema.is_none() {
                    warn!(
                        plugin_id = %name,
                        "Plugin has no PluginSchema; top-level state will be projected without plugin-specific validation"
                    );
                }

                LoadedPlugin { name, schema, plugin }
            })
            .collect::<Vec<_>>();

        info!(
            plugin_count = plugins.len(),
            "Initialized plugin projection reader"
        );

        Ok(Self {
            source: "plugin".to_string(),
            plugins,
        })
    }

    /// The schema used to validate nested plugin object projections.
    pub fn nested_object_projection_schema() -> PluginSchema {
        PluginSchema {
            name: "plugin.object".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![
                FieldSchema {
                    name: "plugin_id".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("Owning plugin identifier".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
                FieldSchema {
                    name: "parent_id".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("Parent projection entity ID".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
                FieldSchema {
                    name: "object_path".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("JSON pointer-like path to the nested object".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
                FieldSchema {
                    name: "value".to_string(),
                    field_type: FieldType::Any,
                    required: true,
                    description: Some("Nested object value mirrored from plugin state".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
            ],
            category: Some("plugin".to_string()),
            examples: None,
            secret_paths: Vec::new(),
            pii_paths: Vec::new(),
        }
    }

    /// Returns all schemas required for plugin state projection.
    pub fn projection_schemas(&self) -> Vec<PluginSchema> {
        let mut schemas = self
            .plugins
            .iter()
            .filter_map(|plugin| plugin.schema.as_ref().map(convert_schema))
            .collect::<Vec<_>>();
        schemas.push(Self::nested_object_projection_schema());
        schemas
    }

    /// Reads all plugin-backed projection entities asynchronously.
    pub async fn read_all_async(&self) -> Result<Vec<RawEntity>> {
        let mut entities = Vec::new();

        for plugin in &self.plugins {
            entities.extend(self.read_loaded_plugin(plugin).await?);
        }

        debug!(
            entity_count = entities.len(),
            "Read plugin projection entities"
        );
        Ok(entities)
    }

    /// Reads all projection entities for a single plugin asynchronously.
    pub async fn read_plugin_objects_async(&self, plugin_id: &str) -> Result<Vec<RawEntity>> {
        let plugin = self
            .plugins
            .iter()
            .find(|plugin| plugin.name == plugin_id)
            .with_context(|| format!("unknown plugin '{}'", plugin_id))?;

        self.read_loaded_plugin(plugin).await
    }

    /// Reads nested object projections for a single plugin asynchronously.
    pub async fn read_nested_objects_async(
        &self,
        plugin_id: &str,
        parent_id: &str,
    ) -> Result<Vec<RawEntity>> {
        let entities = self.read_plugin_objects_async(plugin_id).await?;

        Ok(entities
            .into_iter()
            .filter(|entity| {
                entity.entity_type == "plugin.object"
                    && entity
                        .data
                        .get("parent_id")
                        .and_then(|value| value.as_str())
                        == Some(parent_id)
            })
            .collect())
    }

    async fn read_loaded_plugin(&self, plugin: &LoadedPlugin) -> Result<Vec<RawEntity>> {
        let state = match plugin.plugin.query_current_state().await {
            Ok(state) => state,
            Err(error) => {
                warn!(
                    plugin_id = %plugin.name,
                    error = %error,
                    "Skipping plugin projection because state query failed"
                );
                return Ok(Vec::new());
            }
        };

        let entity_type = plugin
            .schema
            .as_ref()
            .map(|schema| schema.name.clone())
            .unwrap_or_else(|| plugin.name.clone());
        let mut entities = vec![RawEntity {
            entity_type,
            entity_id: plugin.name.clone(),
            data: state.clone(),
            source: self.source.clone(),
        }];

        entities.extend(Self::collect_nested_entities(
            &plugin.name,
            &state,
            &self.source,
        ));

        Ok(entities)
    }

    fn collect_nested_entities(plugin_id: &str, state: &Value, source: &str) -> Vec<RawEntity> {
        let mut entities = Vec::new();
        Self::collect_nested_entities_recursive(
            &mut entities,
            plugin_id,
            plugin_id,
            "",
            state,
            source,
        );
        entities
    }

    fn collect_nested_entities_recursive(
        entities: &mut Vec<RawEntity>,
        plugin_id: &str,
        parent_id: &str,
        path: &str,
        value: &Value,
        source: &str,
    ) {
        match value {
            Value::Object(map) => {
                if !path.is_empty() {
                    entities.push(RawEntity {
                        entity_type: "plugin.object".to_string(),
                        entity_id: Self::nested_entity_id(plugin_id, path),
                        data: json!({
                            "plugin_id": plugin_id,
                            "parent_id": parent_id,
                            "object_path": path,
                            "value": value.clone(),
                        })
                        .into(),
                        source: source.to_string(),
                    });
                }

                let current_id = if path.is_empty() {
                    plugin_id.to_string()
                } else {
                    Self::nested_entity_id(plugin_id, path)
                };

                for (key, child) in map.iter() {
                    if child.is_object() || child.is_array() {
                        let child_path = format!("{}/{}", path, key);
                        Self::collect_nested_entities_recursive(
                            entities,
                            plugin_id,
                            &current_id,
                            &child_path,
                            child,
                            source,
                        );
                    }
                }
            }
            Value::Array(array) => {
                if !path.is_empty() {
                    entities.push(RawEntity {
                        entity_type: "plugin.object".to_string(),
                        entity_id: Self::nested_entity_id(plugin_id, path),
                        data: json!({
                            "plugin_id": plugin_id,
                            "parent_id": parent_id,
                            "object_path": path,
                            "value": value.clone(),
                        })
                        .into(),
                        source: source.to_string(),
                    });
                }

                let current_id = if path.is_empty() {
                    plugin_id.to_string()
                } else {
                    Self::nested_entity_id(plugin_id, path)
                };

                for (index, child) in array.iter().enumerate() {
                    if child.is_object() || child.is_array() {
                        let child_path = format!("{}/{}", path, index);
                        Self::collect_nested_entities_recursive(
                            entities,
                            plugin_id,
                            &current_id,
                            &child_path,
                            child,
                            source,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn nested_entity_id(plugin_id: &str, path: &str) -> String {
        format!("{}:{}", plugin_id, path)
    }

    fn block_on<F, T>(&self, future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
            Err(_) => {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("failed to build a tokio runtime for plugin projection")?;
                runtime.block_on(future)
            }
        }
    }
}

impl Default for SystemPluginReader {
    fn default() -> Self {
        Self::empty()
    }
}

impl SourceReader for SystemPluginReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        self.block_on(self.read_all_async())
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        let entities = self.block_on(self.read_all_async())?;
        entities
            .into_iter()
            .find(|entity| entity.entity_id == entity_id)
            .with_context(|| format!("unknown plugin entity '{}'", entity_id))
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        !self.plugins.is_empty()
    }
}

impl PluginReader for SystemPluginReader {
    fn read_plugin_objects(&self, plugin_id: &str) -> Result<Vec<RawEntity>> {
        debug!(plugin_id = plugin_id, "Reading plugin objects");
        self.block_on(self.read_plugin_objects_async(plugin_id))
    }

    fn read_nested_objects(&self, plugin_id: &str, parent_id: &str) -> Result<Vec<RawEntity>> {
        debug!(
            plugin_id = plugin_id,
            parent_id = parent_id,
            "Reading nested plugin objects"
        );
        self.block_on(self.read_nested_objects_async(plugin_id, parent_id))
    }

    fn handle_lifecycle(&self, plugin_id: &str, event: PluginLifecycleEvent) {
        info!(
            plugin_id = plugin_id,
            event = ?event,
            "Plugin lifecycle event"
        );
    }
}

/// Convert an `op_state_store::PluginSchema` into an `op_projection::PluginSchema`.
pub fn convert_schema(schema: &RuntimePluginSchema) -> PluginSchema {
    PluginSchema {
        name: schema.name.clone(),
        version: schema.version.clone(),
        fields: schema
            .fields
            .iter()
            .map(|(name, field)| convert_field(name, field))
            .collect(),
        category: Some(schema.category.clone()),
        examples: schema.example.clone().map(|example| vec![example]),
        secret_paths: Vec::new(),
        pii_paths: Vec::new(),
    }
}

fn convert_field(name: &str, field: &RuntimeFieldSchema) -> FieldSchema {
    FieldSchema {
        name: name.to_string(),
        field_type: convert_field_type(&field.field_type),
        required: field.required,
        description: Some(field.description.clone()).filter(|description| !description.is_empty()),
        constraints: field
            .constraints
            .iter()
            .filter_map(convert_constraint)
            .collect(),
        example: field.example.clone(),
        read_only: field.read_only,
    }
}

fn convert_field_type(field_type: &RuntimeFieldType) -> FieldType {
    match field_type {
        RuntimeFieldType::String => FieldType::String,
        RuntimeFieldType::Integer => FieldType::Integer,
        RuntimeFieldType::Float => FieldType::Number,
        RuntimeFieldType::Boolean => FieldType::Boolean,
        RuntimeFieldType::Array(inner) => FieldType::Array(Box::new(convert_field_type(inner))),
        RuntimeFieldType::Object(_) => FieldType::Object,
        RuntimeFieldType::Enum(values) => FieldType::Enum(values.clone()),
        RuntimeFieldType::Any => FieldType::Any,
    }
}

fn convert_constraint(constraint: &RuntimeConstraint) -> Option<Constraint> {
    match constraint {
        RuntimeConstraint::Min { value } => Some(Constraint::MinValue(*value as i64)),
        RuntimeConstraint::Max { value } => Some(Constraint::MaxValue(*value as i64)),
        RuntimeConstraint::Pattern { regex } => Some(Constraint::Pattern(regex.clone())),
        RuntimeConstraint::OneOf { values } => {
            let values = values
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect::<Vec<_>>();

            if values.is_empty() {
                None
            } else {
                Some(Constraint::Enum(values))
            }
        }
        RuntimeConstraint::RequiresField { .. } | RuntimeConstraint::Custom { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;

    #[test]
    fn should_project_nested_plugin_objects() {
        let state = json!({
            "interfaces": [
                {
                    "name": "wg0",
                    "peers": [
                        { "name": "peer-a" }
                    ]
                }
            ],
            "metadata": {
                "enabled": true
            }
        });

        let entities = SystemPluginReader::collect_nested_entities("wireguard", &state, "plugin");
        let entity_ids = entities
            .iter()
            .map(|entity| entity.entity_id.clone())
            .collect::<Vec<_>>();

        assert!(entity_ids.contains(&"wireguard:/interfaces".to_string()));
        assert!(entity_ids.contains(&"wireguard:/interfaces/0".to_string()));
        assert!(entity_ids.contains(&"wireguard:/interfaces/0/peers".to_string()));
        assert!(entity_ids.contains(&"wireguard:/metadata".to_string()));

        let peers = entities
            .iter()
            .find(|entity| entity.entity_id == "wireguard:/interfaces/0/peers")
            .expect("peers projection");
        assert_eq!(peers.data["parent_id"], "wireguard:/interfaces/0");
        assert_eq!(peers.data["plugin_id"], "wireguard");
    }

    #[test]
    fn should_convert_runtime_schema_to_projection_schema() {
        let schema = RuntimePluginSchema::builder("net")
            .version("1.2.3")
            .category("network")
            .description("Network schema")
            .field(
                "interfaces",
                RuntimeFieldSchema {
                    field_type: RuntimeFieldType::Array(Box::new(RuntimeFieldType::String)),
                    required: true,
                    description: "Interface names".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: true,
                    read_only_when: None,
                },
            )
            .build();

        let converted = convert_schema(&schema);
        assert_eq!(converted.name, "net");
        assert_eq!(converted.version, "1.2.3");
        assert_eq!(converted.fields.len(), 1);
        assert_eq!(converted.fields[0].name, "interfaces");
        assert_eq!(
            converted.fields[0].field_type,
            FieldType::Array(Box::new(FieldType::String))
        );
        assert!(converted.fields[0].read_only);
    }
}
</file>

<file path="src/procfs_reader.rs">
//! Procfs Reader: Reading from /proc.
//!
//! This module implements the `ProcfsReader` trait, scanning the `/proc`
//! directory and projecting standard entries into raw entities.

use crate::interfaces::{ProcfsReader, RawEntity, SourceReader};
use anyhow::Result;
use simd_json::json;
use std::fs;
use tracing::{debug, warn};

/// Reader that extracts state from the /proc filesystem.
#[derive(Debug, Clone)]
pub struct SystemProcfsReader {
    /// Source identifier
    source: String,
}

impl SystemProcfsReader {
    /// Creates a new SystemProcfsReader
    pub fn new() -> Self {
        Self {
            source: "procfs".to_string(),
        }
    }

    /// Helper to read a value from a proc file
    fn read_proc_value(&self, path: &str) -> Option<String> {
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }
}

impl Default for SystemProcfsReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for SystemProcfsReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        let mut entities = Vec::new();

        entities.extend(self.read_processes()?);

        if let Ok(memory) = self.read_memory() {
            entities.push(memory);
        }

        if let Ok(cpu) = self.read_cpu() {
            entities.push(cpu);
        }

        if let Ok(filesystems) = self.read_filesystems() {
            entities.push(filesystems);
        }

        if let Ok(network) = self.read_network() {
            entities.push(network);
        }

        Ok(entities)
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        match entity_id {
            "memory" => self.read_memory(),
            "cpu" => self.read_cpu(),
            "filesystems" => self.read_filesystems(),
            "network" => self.read_network(),
            _ => Err(anyhow::anyhow!("Unknown procfs entity: {}", entity_id)),
        }
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/proc").exists()
    }
}

impl ProcfsReader for SystemProcfsReader {
    fn read_processes(&self) -> Result<Vec<RawEntity>> {
        let mut processes = Vec::new();

        // Iterate over /proc/[0-9]*
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.chars().all(|c| c.is_ascii_digit()) {
                    let pid = name_str.to_string();
                    let comm_path = format!("/proc/{}/comm", pid);

                    if let Some(comm) = self.read_proc_value(&comm_path) {
                        processes.push(RawEntity {
                            entity_type: "system.process".to_string(),
                            entity_id: pid,
                            data: json!({ "name": comm }).into(),
                            source: self.source.clone(),
                        });
                    }
                }

                // Limit to 10 processes for now to avoid overwhelming
                if processes.len() >= 10 {
                    break;
                }
            }
        }

        Ok(processes)
    }

    fn read_memory(&self) -> Result<RawEntity> {
        debug!("Reading memory info from /proc/meminfo");

        let mut total_kb = 0;
        let mut free_kb = 0;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    total_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                } else if line.starts_with("MemFree:") {
                    free_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                }
            }
        }

        Ok(RawEntity {
            entity_type: "system.memory".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "total_kb": total_kb, "free_kb": free_kb }).into(),
            source: self.source.clone(),
        })
    }

    fn read_cpu(&self) -> Result<RawEntity> {
        debug!("Reading CPU info from /proc/cpuinfo");

        let mut cores = 0;
        let mut model = "unknown".to_string();

        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if line.starts_with("processor") {
                    cores += 1;
                } else if line.starts_with("model name") && model == "unknown" {
                    model = line
                        .split(':')
                        .nth(1)
                        .unwrap_or("unknown")
                        .trim()
                        .to_string();
                }
            }
        }

        Ok(RawEntity {
            entity_type: "system.cpu".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "cores": cores, "model": model }).into(),
            source: self.source.clone(),
        })
    }

    fn read_filesystems(&self) -> Result<RawEntity> {
        debug!("Reading filesystems from /proc/filesystems");

        let mut fs_types = Vec::new();
        if let Ok(content) = fs::read_to_string("/proc/filesystems") {
            for line in content.lines() {
                fs_types.push(line.trim().to_string());
            }
        }

        Ok(RawEntity {
            entity_type: "system.filesystems".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "types": fs_types }).into(),
            source: self.source.clone(),
        })
    }

    fn read_network(&self) -> Result<RawEntity> {
        debug!("Reading network info from /proc/net/dev");

        let mut interfaces = Vec::new();
        if let Ok(content) = fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                if let Some(iface) = line.split(':').next() {
                    interfaces.push(iface.trim().to_string());
                }
            }
        }

        Ok(RawEntity {
            entity_type: "system.network".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "interfaces": interfaces }).into(),
            source: self.source.clone(),
        })
    }
}
</file>

<file path="src/projection_engine.rs">
//! Projection Engine: Authoritative state transformation engine.
//!
//! This module implements the `ProjectionEngine` trait, providing the core
//! logic for transforming authoritative state into schema-validated projections.

use crate::data_models::*;
use crate::interfaces::{ProjectionEngine, RawEntity};
use crate::projection_store::ProjectionStore;
use crate::schema_engine::SchemaValidator;
use anyhow::Result;
use chrono::Utc;
use tracing::{debug, info, warn};

/// The core engine for state transformation.
///
/// This engine manages the lifecycle of projections, ensuring all data is
/// validated against the authoritative schema registry.
#[derive(Debug, Clone)]
pub struct ProjectionSystemEngine {
    /// In-memory projection store
    store: ProjectionStore,
    /// Schema validator for data validation
    validator: SchemaValidator,
}

impl ProjectionSystemEngine {
    /// Creates a new ProjectionSystemEngine with given store and validator
    pub fn new(store: ProjectionStore, validator: SchemaValidator) -> Self {
        Self { store, validator }
    }

    /// Internal helper to create a projection from raw entity
    fn build_projection(&self, entity: RawEntity) -> Projection {
        let timestamp = Utc::now();
        let entity_type = entity.entity_type.clone();

        // Validate entity against schema
        let validation_result = self.validator.validate_entity(&entity);

        // Default projection
        let mut projection = Projection {
            id: format!("{}:{}", entity.entity_type, entity.entity_id),
            entity_type: entity.entity_type,
            entity_id: entity.entity_id,
            state: ProjectionState::Valid,
            schema_version: "0.0.0".to_string(),
            data: entity.data,
            validation_errors: Vec::new(),
            quarantine_reason: None,
            degradation_reason: None,
            affected_dependencies: Vec::new(),
            created_at: timestamp,
            updated_at: timestamp,
        };

        match validation_result {
            Ok(result) => {
                projection.validation_errors = result.errors;
                if !result.valid {
                    projection.state = ProjectionState::Quarantined;
                    projection.quarantine_reason = Some("Schema validation failed".to_string());
                }

                // Set schema version from registry
                if let Some(schema) = self.validator.get_schema_for_entity(&entity_type) {
                    projection.schema_version = schema.version.clone();
                }
            }
            Err(e) => {
                projection.state = ProjectionState::Quarantined;
                projection.quarantine_reason = Some(format!("Validation error: {}", e));
                warn!(
                    entity_type = entity_type,
                    entity_id = projection.entity_id,
                    error = %e,
                    "Failed to validate entity"
                );
            }
        }

        projection
    }
}

impl ProjectionEngine for ProjectionSystemEngine {
    fn create_projection(&mut self, entity: RawEntity) -> Result<Projection> {
        let projection = self.build_projection(entity);
        let projection_clone = projection.clone();
        self.store.upsert(projection);

        debug!(
            id = projection_clone.id,
            state = ?projection_clone.state,
            "Projection created"
        );

        Ok(projection_clone)
    }

    fn update_projection(&mut self, _projection_id: &str, entity: RawEntity) -> Result<Projection> {
        // In this implementation, ID is derived from entity_type and entity_id
        // So update is same as create (upsert)
        let projection = self.build_projection(entity);
        let projection_clone = projection.clone();
        self.store.upsert(projection);

        debug!(
            id = projection_clone.id,
            state = ?projection_clone.state,
            "Projection updated"
        );

        Ok(projection_clone)
    }

    fn get_projection(&self, projection_id: &str) -> Option<Projection> {
        self.store.get(projection_id)
    }

    fn get_projections_by_type(&self, entity_type: &str) -> Vec<Projection> {
        self.store.get_by_type(entity_type)
    }

    fn get_projections_by_state(&self, state: ProjectionState) -> Vec<Projection> {
        self.store.get_by_state(state)
    }

    fn quarantine_projection(&mut self, projection_id: &str, reason: &str) {
        if let Some(mut projection) = self.store.get(projection_id) {
            projection.state = ProjectionState::Quarantined;
            projection.quarantine_reason = Some(reason.to_string());
            projection.updated_at = Utc::now();
            self.store.upsert(projection);

            info!(
                projection_id = projection_id,
                reason = reason,
                "Projection quarantined"
            );
        }
    }

    fn degrade_projection(
        &mut self,
        projection_id: &str,
        reason: &str,
        affected_dependencies: Vec<String>,
    ) {
        if let Some(mut projection) = self.store.get(projection_id) {
            projection.state = ProjectionState::Degraded;
            projection.degradation_reason = Some(reason.to_string());
            projection.affected_dependencies = affected_dependencies;
            projection.updated_at = Utc::now();
            self.store.upsert(projection);

            info!(
                projection_id = projection_id,
                reason = reason,
                "Projection degraded"
            );
        }
    }

    fn revalidate_projections(&mut self, schema_name: &str, _old_version: &str) {
        let projections = self.store.get_by_type(schema_name);
        for mut projection in projections {
            // Re-validate against current schema
            let entity = RawEntity {
                entity_type: projection.entity_type.clone(),
                entity_id: projection.entity_id.clone(),
                data: projection.data.clone(),
                source: "revalidation".to_string(),
            };

            let updated = self.build_projection(entity);
            projection.state = updated.state;
            projection.validation_errors = updated.validation_errors;
            projection.quarantine_reason = updated.quarantine_reason;
            projection.schema_version = updated.schema_version;
            projection.updated_at = Utc::now();

            self.store.upsert(projection);
        }
    }

    fn get_all_projections(&self) -> Vec<Projection> {
        self.store.get_all()
    }

    fn delete_projection(&mut self, projection_id: &str) -> Result<()> {
        self.store.delete(projection_id)
    }

    fn get_projections_by_source(&self, source: &str) -> Vec<Projection> {
        // Filter projections by source if we had source field,
        // for now just return empty or placeholder filter
        self.store
            .get_all()
            .into_iter()
            .filter(|p| p.id.contains(source))
            .collect()
    }
}
</file>

<file path="src/projection_store.rs">
//! Projection Store: In-memory projection storage with persistence.
//!
//! This module implements the authoritative store for all schema-validated
//! projections. It uses DashMap for high-concurrency access and supports
//! time-indexed versioning for historical replay.

use crate::data_models::*;
use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// In-memory authoritative store for all projections.
///
/// This store is the source of truth for the current state of all projections.
/// It provides zero-copy read access and maintains historical versions.
#[derive(Debug, Clone, Default)]
pub struct ProjectionStore {
    /// Current projections by ID
    projections: Arc<DashMap<String, Projection>>,
    /// Historical projections by ID and version
    history: Arc<DashMap<String, Vec<HistoricalProjection>>>,
}

impl ProjectionStore {
    /// Creates a new empty ProjectionStore
    pub fn new() -> Self {
        Self {
            projections: Arc::new(DashMap::new()),
            history: Arc::new(DashMap::new()),
        }
    }

    /// Insert or update a projection
    pub fn upsert(&self, projection: Projection) {
        let id = projection.id.clone();

        // Update history before replacing current
        if let Some(old) = self.projections.get(&id) {
            let historical = HistoricalProjection {
                projection: old.clone(),
                version: self.history.get(&id).map(|h| h.len() as u64).unwrap_or(0) + 1,
                timestamp: chrono::Utc::now(),
                is_quarantined: old.state == ProjectionState::Quarantined,
            };

            self.history
                .entry(id.clone())
                .or_insert_with(Vec::new)
                .push(historical);
        }

        self.projections.insert(id, projection);
    }

    /// Get a projection by ID
    pub fn get(&self, id: &str) -> Option<Projection> {
        self.projections.get(id).map(|p| p.clone())
    }

    /// Get all projections for an entity type
    pub fn get_by_type(&self, entity_type: &str) -> Vec<Projection> {
        self.projections
            .iter()
            .filter(|p| p.entity_type == entity_type)
            .map(|p| p.value().clone())
            .collect()
    }

    /// Get all projections for a state
    pub fn get_by_state(&self, state: ProjectionState) -> Vec<Projection> {
        self.projections
            .iter()
            .filter(|p| p.state == state)
            .map(|p| p.value().clone())
            .collect()
    }

    /// Get all projections
    pub fn get_all(&self) -> Vec<Projection> {
        self.projections.iter().map(|p| p.value().clone()).collect()
    }

    /// Delete a projection
    pub fn delete(&self, id: &str) -> Result<()> {
        if self.projections.remove(id).is_some() {
            debug!(projection_id = id, "Projection deleted from store");
            Ok(())
        } else {
            Err(anyhow::anyhow!("Projection '{}' not found", id))
        }
    }

    /// Get historical versions for a projection
    pub fn get_history(&self, id: &str) -> Vec<HistoricalProjection> {
        self.history.get(id).map(|h| h.clone()).unwrap_or_default()
    }

    /// Clear all projections and history
    pub fn clear(&self) {
        self.projections.clear();
        self.history.clear();
        info!("Projection store cleared");
    }
}
</file>

<file path="src/schema_engine.rs">
//! Schema Engine: Schema registry and validation services.
//!
//! This module implements the authoritative schema registry that validates
//! and manages all PluginSchema definitions. It enforces the Schema-as-Code
//! Authority principle: PluginSchema is the single source of truth for all
//! projections.

use crate::data_models::*;
use crate::interfaces::{RawEntity, SchemaRegistry};
use anyhow::Result;
use regex::Regex;
use simd_json::prelude::*;
use simd_json::{OwnedValue as Value, StaticNode};
use std::collections::HashMap;
use std::io::Write;
use tracing::{debug, error, info, warn};

/// Shared-memory path for the canonical PluginSchema catalog.
/// This is the single source of truth for UI, snowball, and all components.
const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

/// Schema version identifier
pub type SchemaVersion = u64;

/// The authoritative schema registry that validates and manages all PluginSchema definitions.
///
/// This is the single source of truth for all projections. All projections must have
/// a valid schema to exist on the system.
///
/// # Core Principles
///
/// - **Schema-as-Code Authority**: PluginSchema is the single source of truth
/// - **Versioned Registry**: All schemas are versioned with immutable history
/// - **Validation**: Schemas are validated against the registry before enabling projection
/// - **Audit Trail**: All schema changes are recorded with footprints
#[derive(Debug, Clone)]
pub struct SchemaEngine {
    /// Registry of schemas by name
    schemas: HashMap<String, Vec<PluginSchema>>,
    /// Quarantined schemas with reasons
    quarantined: HashMap<String, String>,
    /// Version counter for new schema registrations
    version_counter: HashMap<String, SchemaVersion>,
    /// Audit trail for schema changes
    audit_trail: Vec<SchemaAuditEntry>,
}

/// Audit entry for schema changes
#[derive(Debug, Clone)]
pub struct SchemaAuditEntry {
    /// Timestamp of the change
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Actor who made the change
    pub actor: String,
    /// Schema name
    pub schema_name: String,
    /// Change type
    pub change_type: SchemaChangeType,
    /// Reason for the change
    pub reason: String,
    /// Footprint hash (The Strike/Etch)
    pub footprint: String,
    /// Trace ID for correlation
    pub trace_id: String,
}

/// Types of schema changes
#[derive(Debug, Clone)]
pub enum SchemaChangeType {
    /// Schema registered
    Registered,
    /// Schema updated
    Updated,
    /// Schema quarantined
    Quarantined,
    /// Schema revalidated
    Revalidated,
}

impl Default for SchemaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaEngine {
    /// Creates a new SchemaEngine instance.
    /// If tmpfs is available, writes an empty schema catalog to enforce
    /// the Absolute Base rule: without a valid schema, no entity exists.
    pub fn new() -> Self {
        let engine = Self {
            schemas: HashMap::new(),
            quarantined: HashMap::new(),
            version_counter: HashMap::new(),
            audit_trail: Vec::new(),
        };
        // Write initial (empty) schema catalog to shared memory so consumers
        // can immediately detect whether the projection system is alive.
        let _ = engine.write_schemas_to_shm();
        engine
    }

    /// Creates a new SchemaEngine with the given actor and trace ID
    pub fn with_context(_actor: &str, _trace_id: &str) -> Self {
        Self::new()
    }

    /// Generates a footprint hash for audit trail using Blake3 (Strike/Etch).
    fn generate_footprint(&self, schema_name: &str, version: SchemaVersion) -> String {
        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let data = format!("{}:{}:{}", schema_name, version, timestamp);
        let hash = blake3::hash(data.as_bytes());
        hash.to_hex().to_string()
    }

    /// Write the entire schema catalog to shared memory as JSON.
    /// This is the single source of truth: UI, snowball, gRPC reflection,
    /// and all downstream consumers read from this file.
    pub fn write_schemas_to_shm(&self) -> Result<String> {
        let catalog: std::collections::HashMap<String, Vec<&PluginSchema>> = self
            .schemas
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().collect()))
            .collect();

        let json_bytes = serde_json::to_vec_pretty(&catalog)
            .map_err(|e| anyhow::anyhow!("Failed to serialize schema catalog: {}", e))?;

        let mut file = std::fs::File::create(SHM_SCHEMA_PATH)
            .map_err(|e| anyhow::anyhow!("Cannot write schema SHM: {}", e))?;
        file.write_all(&json_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to write schema SHM: {}", e))?;
        file.sync_all()
            .map_err(|e| anyhow::anyhow!("Failed to sync schema SHM: {}", e))?;

        let hash = blake3::hash(&json_bytes);
        let hex = hash.to_hex().to_string();
        info!(path = SHM_SCHEMA_PATH, footprint = %hex, "Schema catalog written to shared memory");
        Ok(hex)
    }

    /// Read the Blake3 footprint of the current schema catalog on disk.
    pub fn read_schema_footprint(&self) -> Result<String> {
        let bytes = std::fs::read(SHM_SCHEMA_PATH)
            .map_err(|e| anyhow::anyhow!("Cannot read schema SHM: {}", e))?;
        let hash = blake3::hash(&bytes);
        Ok(hash.to_hex().to_string())
    }

    /// Generates a trace ID for audit trail
    fn generate_trace_id(&self) -> String {
        // In production, use a proper UUID or distributed tracing ID
        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        format!("trace-{}", timestamp)
    }

    /// Records an audit entry for schema changes
    fn record_audit(&mut self, schema_name: &str, change_type: SchemaChangeType, reason: &str) {
        let footprint = self.generate_footprint(schema_name, 0);
        let trace_id = self.generate_trace_id();
        let change_type_clone = change_type.clone();

        let entry = SchemaAuditEntry {
            timestamp: chrono::Utc::now(),
            actor: "system".to_string(),
            schema_name: schema_name.to_string(),
            change_type,
            reason: reason.to_string(),
            footprint,
            trace_id,
        };

        self.audit_trail.push(entry);
        debug!(
            schema_name = schema_name,
            change_type = ?change_type_clone,
            "Schema audit entry recorded"
        );
    }
}

impl SchemaRegistry for SchemaEngine {
    /// Register a new schema version
    ///
    /// Returns the schema version number on success.
    fn register_schema(&mut self, schema: PluginSchema) -> Result<u64> {
        let schema_name = schema.name.clone();
        let schema_version = schema.version.clone();

        // Check if schema is quarantined
        if let Some(reason) = self.quarantined.get(&schema_name) {
            return Err(anyhow::anyhow!(
                "Cannot register schema '{}': it is quarantined: {}",
                schema_name,
                reason
            ));
        }

        // Get or initialize version counter
        let version = self.version_counter.get(&schema_name).copied().unwrap_or(0) + 1;

        // Update version counter
        self.version_counter.insert(schema_name.clone(), version);

        // Add schema to registry
        self.schemas
            .entry(schema_name.clone())
            .or_insert_with(Vec::new)
            .push(schema.clone());

        // Record audit entry
        self.record_audit(
            &schema_name,
            SchemaChangeType::Registered,
            &format!("Registered schema version {}", schema_version),
        );

        // Write the updated canonical catalog to shared memory.
        // This is the single source of truth for UI, snowball, everything.
        if let Err(e) = self.write_schemas_to_shm() {
            warn!(error = %e, "Failed to sync schema catalog to shared memory");
        }

        info!(
            schema_name = schema_name,
            version = version,
            "Schema registered successfully"
        );

        Ok(version)
    }

    /// Validate a schema against the registry
    fn validate_schema(&self, schema: &PluginSchema) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let schema_name = &schema.name;

        // Check if schema name is empty
        if schema_name.is_empty() {
            errors.push(ValidationError {
                path: "name".to_string(),
                message: "Schema name cannot be empty".to_string(),
                code: "SCHEMA_NAME_EMPTY".to_string(),
            });
        }

        // Check if schema version is empty
        if schema.version.is_empty() {
            errors.push(ValidationError {
                path: "version".to_string(),
                message: "Schema version cannot be empty".to_string(),
                code: "SCHEMA_VERSION_EMPTY".to_string(),
            });
        }

        // Validate fields
        for (index, field) in schema.fields.iter().enumerate() {
            let field_path = format!("fields[{}].name", index);

            // Check field name
            if field.name.is_empty() {
                errors.push(ValidationError {
                    path: field_path.clone(),
                    message: "Field name cannot be empty".to_string(),
                    code: "FIELD_NAME_EMPTY".to_string(),
                });
            }

            // Validate field type is valid
            match &field.field_type {
                FieldType::Array(inner_type) => {
                    // Array type must have a valid inner type
                    match inner_type.as_ref() {
                        FieldType::String
                        | FieldType::Integer
                        | FieldType::Number
                        | FieldType::Boolean
                        | FieldType::Object
                        | FieldType::Enum(_)
                        | FieldType::Any => {}
                        _ => {
                            errors.push(ValidationError {
                                path: format!("fields[{}].type", index),
                                message: "Array field type must have a valid inner type"
                                    .to_string(),
                                code: "INVALID_ARRAY_TYPE".to_string(),
                            });
                        }
                    }
                }
                _ => {
                    // Valid field type
                }
            }
        }

        let valid = errors.is_empty();

        Ok(ValidationResult { valid, errors })
    }

    /// Get the latest version of a schema by name
    fn get_schema(&self, name: &str) -> Option<&PluginSchema> {
        self.schemas.get(name).and_then(|versions| versions.last())
    }

    /// Get all versions of a schema
    fn get_schema_versions(&self, name: &str) -> Vec<&PluginSchema> {
        self.schemas
            .get(name)
            .map(|versions| versions.iter().collect())
            .unwrap_or_default()
    }

    /// Check if an entity type has a valid schema
    fn has_valid_schema(&self, entity_type: &str) -> bool {
        // Check if schema exists and is not quarantined
        if self.quarantined.contains_key(entity_type) {
            return false;
        }

        self.schemas.contains_key(entity_type)
    }

    /// Quarantine a schema and all associated entities
    fn quarantine_schema(&mut self, name: &str, reason: &str) {
        // Check if already quarantined
        if self.quarantined.contains_key(name) {
            warn!(schema_name = name, "Schema already quarantined");
            return;
        }

        // Mark schema as quarantined
        self.quarantined
            .insert(name.to_string(), reason.to_string());

        // Record audit entry
        self.record_audit(name, SchemaChangeType::Quarantined, reason);

        // Sync updated catalog (quarantine changes validity) to shared memory
        if let Err(e) = self.write_schemas_to_shm() {
            warn!(error = %e, "Failed to sync schema catalog after quarantine");
        }

        error!(schema_name = name, reason = reason, "Schema quarantined");
    }

    /// Get all registered schema names
    fn list_schemas(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }

    /// Get schema version by name and version string
    fn get_schema_by_version(&self, name: &str, version: &str) -> Option<&PluginSchema> {
        self.schemas
            .get(name)
            .and_then(|versions| versions.iter().find(|s| s.version == version))
    }
}

/// Schema validator implementation
///
/// This validator checks entities against their schemas and provides
/// detailed validation error reporting.
#[derive(Debug, Clone)]
pub struct SchemaValidator {
    /// Reference to the schema registry
    registry: SchemaEngine,
}

impl SchemaValidator {
    /// Creates a new SchemaValidator with the given registry
    pub fn new(registry: SchemaEngine) -> Self {
        Self { registry }
    }

    /// Validates a single field against its schema
    pub fn validate_field(&self, field: &FieldSchema, value: &Value) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Check required fields
        if field.required && value.is_null() {
            errors.push(ValidationError {
                path: field.name.clone(),
                message: format!("Required field '{}' is missing", field.name),
                code: "FIELD_REQUIRED".to_string(),
            });
            return Ok(ValidationResult {
                valid: false,
                errors,
            });
        }

        // Skip validation for null values if not required
        if value.is_null() {
            return Ok(ValidationResult {
                valid: true,
                errors,
            });
        }

        // Validate field type
        match (&field.field_type, value) {
            (FieldType::String, Value::String(_)) => {
                // String type matches
            }
            (
                FieldType::Integer,
                Value::Static(StaticNode::I64(_)) | Value::Static(StaticNode::U64(_)),
            ) => {
                // Integer type matches
            }
            (FieldType::Number, Value::Static(StaticNode::F64(_))) => {
                // Number type matches
            }
            (FieldType::Boolean, Value::Static(StaticNode::Bool(_))) => {
                // Boolean type matches
            }
            (FieldType::Object, Value::Object(_)) => {
                // Object type matches
            }
            (FieldType::Array(_), Value::Array(_)) => {
                // Array type matches
            }
            (FieldType::Enum(values), Value::String(s)) => {
                if !values.contains(&s.to_string()) {
                    errors.push(ValidationError {
                        path: field.name.clone(),
                        message: format!(
                            "Field value '{}' is not in allowed values: {:?}",
                            s, values
                        ),
                        code: "FIELD_VALUE_NOT_IN_ENUM".to_string(),
                    });
                }
            }
            (FieldType::Any, _) => {
                // Any type accepts any value
            }
            (field_type, value) => {
                errors.push(ValidationError {
                    path: field.name.clone(),
                    message: format!(
                        "Field type mismatch: expected {:?}, got {:?}",
                        field_type, value
                    ),
                    code: "FIELD_TYPE_MISMATCH".to_string(),
                });
            }
        }

        // Validate constraints
        let constraint_result = self.validate_constraints(&field.constraints, value);
        if let Err(e) = constraint_result {
            errors.push(ValidationError {
                path: field.name.clone(),
                message: e.to_string(),
                code: "CONSTRAINT_VALIDATION_FAILED".to_string(),
            });
        }

        let valid = errors.is_empty();

        Ok(ValidationResult { valid, errors })
    }

    /// Validates constraints on a value
    pub fn validate_constraints(&self, constraints: &[Constraint], value: &Value) -> Result<()> {
        for constraint in constraints {
            match (constraint, value) {
                (Constraint::MinLength(min), Value::String(s)) => {
                    if s.len() < *min {
                        return Err(anyhow::anyhow!(
                            "String length {} is less than minimum {}",
                            s.len(),
                            min
                        ));
                    }
                }
                (Constraint::MaxLength(max), Value::String(s)) => {
                    if s.len() > *max {
                        return Err(anyhow::anyhow!(
                            "String length {} exceeds maximum {}",
                            s.len(),
                            max
                        ));
                    }
                }
                (Constraint::MinValue(min), Value::Static(StaticNode::I64(v))) => {
                    if *v < *min {
                        return Err(anyhow::anyhow!("Value {} is less than minimum {}", *v, min));
                    }
                }
                (Constraint::MaxValue(max), Value::Static(StaticNode::I64(v))) => {
                    if *v > *max {
                        return Err(anyhow::anyhow!("Value {} exceeds maximum {}", *v, max));
                    }
                }
                (Constraint::Pattern(pattern), Value::String(s)) => {
                    let regex = Regex::new(pattern)
                        .map_err(|_| anyhow::anyhow!("Invalid regex pattern: '{}'", pattern))?;
                    if !regex.is_match(s) {
                        return Err(anyhow::anyhow!(
                            "String '{}' does not match pattern '{}'",
                            s,
                            pattern
                        ));
                    }
                }
                (Constraint::Enum(values), Value::String(s)) => {
                    if !values.contains(s) {
                        return Err(anyhow::anyhow!(
                            "Value '{}' is not in allowed values: {:?}",
                            s,
                            values
                        ));
                    }
                }
                _ => {
                    // Constraint doesn't apply to this value type
                }
            }
        }

        Ok(())
    }

    /// Validates an entity against its schema
    pub fn validate_entity(&self, entity: &RawEntity) -> Result<ValidationResult> {
        // Get schema for entity type
        let schema = self
            .registry
            .get_schema(&entity.entity_type)
            .ok_or_else(|| {
                anyhow::anyhow!("No schema found for entity type '{}'", entity.entity_type)
            })?;

        // Validate entity against schema
        self.validate_entity_with_schema(entity, schema)
    }

    /// Validates an entity against a specific schema
    pub fn validate_entity_with_schema(
        &self,
        entity: &RawEntity,
        schema: &PluginSchema,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Validate each field in the schema
        for field in &schema.fields {
            // Get field value from entity data
            let field_value = self.get_field_value(&entity.data, &field.name);

            // Validate field
            let field_result = self.validate_field(field, &field_value);

            if let Ok(result) = field_result {
                if !result.valid {
                    errors.extend(result.errors);
                }
            } else {
                errors.push(ValidationError {
                    path: field.name.clone(),
                    message: "Failed to validate field".to_string(),
                    code: "FIELD_VALIDATION_ERROR".to_string(),
                });
            }
        }

        let valid = errors.is_empty();

        Ok(ValidationResult { valid, errors })
    }

    /// Gets a field value from entity data using simple property access
    fn get_field_value(&self, data: &Value, field_name: &str) -> Value {
        match data {
            Value::Object(obj) => obj
                .get(field_name)
                .cloned()
                .unwrap_or(Value::Static(StaticNode::Null)),
            _ => Value::Static(StaticNode::Null),
        }
    }

    /// Gets validation errors for an entity
    pub fn get_validation_errors(&self, entity: &RawEntity) -> Vec<ValidationError> {
        match self.validate_entity(entity) {
            Ok(result) => result.errors,
            Err(e) => vec![ValidationError {
                path: "entity".to_string(),
                message: e.to_string(),
                code: "VALIDATION_ERROR".to_string(),
            }],
        }
    }

    /// Gets the schema for an entity type
    pub fn get_schema_for_entity(&self, entity_type: &str) -> Option<&PluginSchema> {
        self.registry.get_schema(entity_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_models::{FieldType, PluginSchema};
    use crate::interfaces::SchemaRegistry;
    use simd_json::OwnedValue as Value;

    #[test]
    fn test_schema_engine_creation() {
        let engine = SchemaEngine::new();
        assert!(engine.list_schemas().is_empty());
    }

    #[test]
    fn test_register_schema() {
        let mut engine = SchemaEngine::new();

        let schema = PluginSchema {
            name: "test_entity".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![],
            category: None,
            examples: None,
            secret_paths: vec![],
            pii_paths: vec![],
        };

        let version = engine.register_schema(schema).unwrap();
        assert_eq!(version, 1);
        assert!(engine.has_valid_schema("test_entity"));
    }

    #[test]
    fn test_register_multiple_versions() {
        let mut engine = SchemaEngine::new();

        let schema1 = PluginSchema {
            name: "test_entity".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![],
            category: None,
            examples: None,
            secret_paths: vec![],
            pii_paths: vec![],
        };

        let schema2 = PluginSchema {
            name: "test_entity".to_string(),
            version: "2.0.0".to_string(),
            fields: vec![],
            category: None,
            examples: None,
            secret_paths: vec![],
            pii_paths: vec![],
        };

        let version1 = engine.register_schema(schema1).unwrap();
        let version2 = engine.register_schema(schema2).unwrap();

        assert_eq!(version1, 1);
        assert_eq!(version2, 2);

        let versions = engine.get_schema_versions("test_entity");
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_get_schema() {
        let mut engine = SchemaEngine::new();

        let schema = PluginSchema {
            name: "test_entity".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![],
            category: None,
            examples: None,
            secret_paths: vec![],
            pii_paths: vec![],
        };

        engine.register_schema(schema).unwrap();

        let retrieved = engine.get_schema("test_entity");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().version, "1.0.0");
    }

    #[test]
    fn test_quarantine_schema() {
        let mut engine = SchemaEngine::new();

        let schema = PluginSchema {
            name: "test_entity".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![],
            category: None,
            examples: None,
            secret_paths: vec![],
            pii_paths: vec![],
        };

        engine.register_schema(schema).unwrap();
        engine.quarantine_schema("test_entity", "Invalid schema");

        assert!(!engine.has_valid_schema("test_entity"));
    }

    #[test]
    fn test_validate_schema() {
        let engine = SchemaEngine::new();

        let schema = PluginSchema {
            name: "test_entity".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![],
            category: None,
            examples: None,
            secret_paths: vec![],
            pii_paths: vec![],
        };

        let result = engine.validate_schema(&schema).unwrap();
        assert!(result.valid);
    }

    #[test]
    fn test_validate_schema_empty_name() {
        let engine = SchemaEngine::new();

        let schema = PluginSchema {
            name: "".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![],
            category: None,
            examples: None,
            secret_paths: vec![],
            pii_paths: vec![],
        };

        let result = engine.validate_schema(&schema).unwrap();
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.code == "SCHEMA_NAME_EMPTY"));
    }

    #[test]
    fn test_validate_field_string() {
        let engine = SchemaEngine::new();
        let validator = SchemaValidator::new(engine);

        let field = FieldSchema {
            name: "test_field".to_string(),
            field_type: FieldType::String,
            required: true,
            description: None,
            constraints: vec![],
            example: None,
            read_only: false,
        };

        let result = validator
            .validate_field(&field, &Value::String("test".to_string().into()))
            .unwrap();
        assert!(result.valid);
    }

    #[test]
    fn test_validate_field_type_mismatch() {
        let engine = SchemaEngine::new();
        let validator = SchemaValidator::new(engine);

        let field = FieldSchema {
            name: "test_field".to_string(),
            field_type: FieldType::String,
            required: true,
            description: None,
            constraints: vec![],
            example: None,
            read_only: false,
        };

        let result = validator
            .validate_field(&field, &Value::Static(StaticNode::I64(123)))
            .unwrap();
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == "FIELD_TYPE_MISMATCH"));
    }

    #[test]
    fn test_validate_constraints_min_length() {
        let engine = SchemaEngine::new();
        let validator = SchemaValidator::new(engine);

        let constraints = vec![Constraint::MinLength(5)];
        let result =
            validator.validate_constraints(&constraints, &Value::String("test".to_string().into()));

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_constraints_max_length() {
        let engine = SchemaEngine::new();
        let validator = SchemaValidator::new(engine);

        let constraints = vec![Constraint::MaxLength(3)];
        let result =
            validator.validate_constraints(&constraints, &Value::String("test".to_string().into()));

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_constraints_pattern() {
        let engine = SchemaEngine::new();
        let validator = SchemaValidator::new(engine);

        let constraints = vec![Constraint::Pattern("^test".to_string())];
        let result = validator
            .validate_constraints(&constraints, &Value::String("other".to_string().into()));

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_constraints_enum() {
        let engine = SchemaEngine::new();
        let validator = SchemaValidator::new(engine);

        let constraints = vec![Constraint::Enum(vec!["a".to_string(), "b".to_string()])];
        let result =
            validator.validate_constraints(&constraints, &Value::String("c".to_string().into()));

        assert!(result.is_err());
    }
}
</file>

<file path="src/schema_validator.rs">
//! Schema Validator: Entity validation services.
//!
//! This module provides detailed validation services for entities against
//! their schemas, including field-level validation and constraint checking.

// Re-export SchemaValidator from schema_engine
pub use crate::schema_engine::SchemaValidator;
</file>

<file path="src/sled_reader.rs">
//! Sled Reader: Zero-copy reads from The Sled (/dev/shm).
//!
//! This module implements a reader for the `IdentitySled` struct
//! located in shared memory, following the 3tched Architecture principles.

use crate::interfaces::{RawEntity, SourceReader};
use anyhow::Result;
use op_identity::{read_sled, IdentitySled};
use simd_json::json;
use tracing::{debug, warn};

/// Reader that extracts state from the Identity Sled in shared memory.
#[derive(Debug, Clone)]
pub struct IdentitySledReader {
    /// Source identifier
    source: String,
}

impl IdentitySledReader {
    /// Creates a new IdentitySledReader
    pub fn new() -> Self {
        Self {
            source: "identity-sled".to_string(),
        }
    }
}

impl Default for IdentitySledReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for IdentitySledReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        let mut entities = Vec::new();

        match self.read_sled_entity() {
            Ok(entity) => entities.push(entity),
            Err(e) => debug!("Sled not available: {}", e),
        }

        Ok(entities)
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        if entity_id == "current" {
            self.read_sled_entity()
        } else {
            Err(anyhow::anyhow!("Unknown sled entity: {}", entity_id))
        }
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        std::path::Path::new(op_identity::SHM_SLED_PATH).exists()
    }
}

impl IdentitySledReader {
    fn read_sled_entity(&self) -> Result<RawEntity> {
        let (ptr, _mmap) =
            read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
        let sled = unsafe { &*ptr };

        let footprint = hex::encode(sled.hashed_footprint);
        let pubkey = hex::encode(sled.wireguard_pubkey);

        Ok(RawEntity {
            entity_type: "identity.sled".to_string(),
            entity_id: "current".to_string(),
            data: json!({
                "mutation_index": sled.mutation_index,
                "hashed_footprint": footprint,
                "wireguard_pubkey": pubkey,
            })
            .into(),
            source: self.source.clone(),
        })
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-projection"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Projection System: Schema-validated state transformation engine"

[dependencies]
# Core
op-core = { path = "../op-core" }
op-state = { path = "../op-state" }
op-state-store = { path = "../op-state-store" }
op-plugins = { path = "../op-plugins" }
op-dbus-mirror = { path = "../op-dbus-mirror" }
op-grpc-bridge = { path = "../op-grpc-bridge" }
op-snowball = { path = "../op-snowball" }
op-identity = { path = "../op-identity" }

# Async runtime
tokio = { workspace = true, features = ["full", "sync"] }
tokio-stream = { workspace = true }
futures = { workspace = true }

# Web
axum = { workspace = true, features = ["ws", "macros", "tokio"] }
tower = { workspace = true }
tower-http = { workspace = true, features = ["cors", "fs", "trace"] }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
simd-json = { workspace = true }

# Utils
regex = { workspace = true }
hex = { workspace = true }

# gRPC
tonic = { workspace = true }
prost = { workspace = true }

# D-Bus
zbus = { workspace = true }

# Time
chrono = { workspace = true, features = ["serde"] }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Tracing
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

# Data structures
dashmap = "5.0"
parking_lot = "0.12"

# Hashing
sha2 = "0.10"
blake3 = { workspace = true }

[dev-dependencies]
tokio-test = "0.4"
mockall = "0.11"
</file>

</files>
