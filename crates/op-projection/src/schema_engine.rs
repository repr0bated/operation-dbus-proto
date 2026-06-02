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
/// This is the single source of truth for UI, blockchain, and all components.
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
    /// This is the single source of truth: UI, blockchain, gRPC reflection,
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
        // This is the single source of truth for UI, blockchain, everything.
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
