//! Plugin Schema Registry
//!
//! Provides schema definitions for all state plugins, enabling:
//! - Validation of plugin state against schemas
//! - Schema versioning and migration
//! - Documentation of plugin state structure
//! - Auto-generation of state templates
//! - JSON Schema export with propertyDependencies support
//! - Flexible dialect support (2026 and future versions)

use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Default JSON Schema dialect (can be overridden per-schema)
pub const DEFAULT_SCHEMA_DIALECT: &str = "https://json-schema.org/v1/2026";

/// Known dialect identifiers
pub mod dialects {
    pub const DRAFT_07: &str = "http://json-schema.org/draft-07/schema#";
    pub const V2026: &str = "https://json-schema.org/v1/2026";
}

/// Path to the json-schema-spec repository relative to workspace root
const SCHEMA_SPEC_PATH: &str = "json-schema-spec";

/// Schema field type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array(Box<FieldType>),
    Object(HashMap<String, FieldSchema>),
    Enum(Vec<String>),
    Any,
}

/// Schema for a single field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldSchema {
    /// Field type
    pub field_type: FieldType,
    /// Whether the field is required
    #[serde(default)]
    pub required: bool,
    /// Description of the field
    #[serde(default)]
    pub description: String,
    /// Default value if not provided
    #[serde(default)]
    pub default: Option<Value>,
    /// Example value for documentation
    #[serde(default)]
    pub example: Option<Value>,
    /// Validation constraints
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    /// Unconditional readOnly - field cannot be modified
    #[serde(default)]
    pub read_only: bool,
    /// Conditional readOnly via propertyDependencies
    #[serde(default)]
    pub read_only_when: Option<ReadOnlyCondition>,
}

/// Condition for conditional readOnly (via propertyDependencies)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadOnlyCondition {
    /// The property to check (e.g., "status", "running")
    pub property: String,
    /// The value that triggers readOnly (e.g., "locked", "true")
    pub value: String,
}

/// Validation constraint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Constraint {
    /// Minimum value (for numbers) or length (for strings/arrays)
    Min { value: f64 },
    /// Maximum value (for numbers) or length (for strings/arrays)
    Max { value: f64 },
    /// Regex pattern (for strings)
    Pattern { regex: String },
    /// Value must be one of these
    OneOf { values: Vec<Value> },
    /// Reference to another field that must exist
    RequiresField { field: String },
    /// Custom validation function name
    Custom { validator: String },
}

/// Plugin schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSchema {
    /// Plugin name
    pub name: String,
    /// Schema version
    pub version: String,
    /// Description
    pub description: String,
    /// Fields in the plugin state
    pub fields: HashMap<String, FieldSchema>,
    /// Dependencies on other plugins
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Example state for documentation
    #[serde(default)]
    pub example: Option<Value>,
    /// Paths that are always readOnly (e.g., ["/id", "/metadata"])
    #[serde(default)]
    pub immutable_paths: Vec<String>,
    /// Schema tags (e.g., ["immutable"] for fully immutable schemas)
    #[serde(default)]
    pub tags: Vec<String>,
    /// JSON Schema dialect to use (defaults to DEFAULT_SCHEMA_DIALECT)
    #[serde(default = "default_dialect")]
    pub dialect: String,
}

fn default_dialect() -> String {
    DEFAULT_SCHEMA_DIALECT.to_string()
}

impl PluginSchema {
    /// Create a new plugin schema builder
    pub fn builder(name: &str) -> PluginSchemaBuilder {
        PluginSchemaBuilder::new(name)
    }

    /// Validate a state value against this schema
    pub fn validate(&self, state: &Value) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check required fields
        for (field_name, field_schema) in &self.fields {
            if field_schema.required {
                if state.get(field_name).is_none() {
                    errors.push(format!("Missing required field: {}", field_name));
                }
            }
        }

        // Validate present fields
        if let Some(obj) = state.as_object() {
            for (field_name, field_value) in obj {
                if let Some(field_schema) = self.fields.get(field_name) {
                    if let Err(e) = validate_field(field_name, field_value, field_schema) {
                        errors.push(e);
                    }
                } else {
                    warnings.push(format!("Unknown field: {}", field_name));
                }
            }
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Generate a template state with default values
    pub fn generate_template(&self) -> Value {
        let mut template = simd_json::value::owned::Object::new();

        for (field_name, field_schema) in &self.fields {
            let value = if let Some(default) = &field_schema.default {
                default.clone()
            } else if let Some(example) = &field_schema.example {
                example.clone()
            } else {
                default_for_type(&field_schema.field_type)
            };
            template.insert(field_name.clone(), value);
        }

        Value::Object(Box::new(template))
    }

    /// Convert to JSON Schema 2026 format (default)
    ///
    /// Includes support for:
    /// - `readOnly` on individual fields
    /// - `propertyDependencies` for conditional immutability
    /// - Schema-level immutability via tags
    pub fn to_json_schema(&self) -> Value {
        let is_fully_immutable = self.tags.contains(&"immutable".to_string());
        let mut properties = simd_json::value::owned::Object::new();
        let mut required = Vec::new();
        let mut property_dependencies: HashMap<String, HashMap<String, Vec<String>>> =
            HashMap::new();

        for (field_name, field_schema) in &self.fields {
            let mut field_json = field_type_to_json_schema_2026(&field_schema.field_type);

            // Add description if present
            if !field_schema.description.is_empty() {
                if let Some(obj) = field_json.as_object_mut() {
                    obj.insert("description".to_string(), json!(field_schema.description));
                }
            }

            // Add readOnly if field is unconditionally immutable, in immutable_paths, or schema is fully immutable
            let path = format!("/{}", field_name);
            if field_schema.read_only || self.immutable_paths.contains(&path) || is_fully_immutable
            {
                if let Some(obj) = field_json.as_object_mut() {
                    obj.insert("readOnly".to_string(), json!(true));
                }
            }

            // Collect propertyDependencies for conditional readOnly
            if let Some(condition) = &field_schema.read_only_when {
                property_dependencies
                    .entry(condition.property.clone())
                    .or_default()
                    .entry(condition.value.clone())
                    .or_default()
                    .push(field_name.clone());
            }

            properties.insert(field_name.clone(), field_json);
            if field_schema.required {
                required.push(Value::String(field_name.clone()));
            }
        }

        let mut schema = json!({
            "$schema": &self.dialect,
            "title": self.name,
            "description": self.description,
            "type": "object",
            "properties": properties,
            "required": required
        });

        // Add propertyDependencies if any conditional readOnly fields exist
        if !property_dependencies.is_empty() {
            let mut deps_json = simd_json::value::owned::Object::new();
            for (prop, value_map) in property_dependencies {
                let mut values_json = simd_json::value::owned::Object::new();
                for (value, fields) in value_map {
                    let mut props = simd_json::value::owned::Object::new();
                    for field in fields {
                        props.insert(field, json!({"readOnly": true}));
                    }
                    values_json.insert(
                        value,
                        json!({
                            "properties": props
                        }),
                    );
                }
                deps_json.insert(prop, Value::Object(Box::new(values_json)));
            }
            if let Some(obj) = schema.as_object_mut() {
                obj.insert(
                    "propertyDependencies".to_string(),
                    Value::Object(Box::new(deps_json)),
                );
            }
        }

        schema
    }

    /// Convert to JSON Schema draft-07 format (deprecated, for backward compatibility)
    #[deprecated(since = "2.0.0", note = "Use to_json_schema() for JSON Schema 2026")]
    pub fn to_json_schema_draft07(&self) -> Value {
        let mut properties = simd_json::value::owned::Object::new();
        let mut required = Vec::new();

        for (field_name, field_schema) in &self.fields {
            properties.insert(
                field_name.clone(),
                field_type_to_json_schema(&field_schema.field_type),
            );
            if field_schema.required {
                required.push(Value::String(field_name.clone()));
            }
        }

        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": self.name,
            "description": self.description,
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    /// Convert to the legacy contract-style schema while using this registry
    /// entry as the only source of truth for the tunable section.
    pub fn to_contract_json_schema(&self) -> Value {
        self.to_contract_json_schema_as(&self.name)
    }

    /// Convert to the legacy contract-style schema using an alternate public
    /// plugin name for compatibility aliases such as `systemd`.
    pub fn to_contract_json_schema_as(&self, public_name: &str) -> Value {
        let mut include_paths: Vec<String> = self
            .fields
            .keys()
            .map(|field| format!("/tunable/{field}"))
            .collect();
        include_paths.sort();

        let mut secret_paths: Vec<String> = self
            .fields
            .keys()
            .filter(|field| is_secret_field_name(field))
            .map(|field| format!("/tunable/{field}"))
            .collect();
        secret_paths.sort();

        let mut pii_paths: Vec<String> = self
            .fields
            .keys()
            .filter(|field| is_pii_field_name(field))
            .map(|field| format!("/tunable/{field}"))
            .collect();
        pii_paths.sort();

        let sensitivity = if secret_paths.is_empty() {
            "internal"
        } else {
            "secret"
        };

        json!({
            "$schema": DEFAULT_SCHEMA_DIALECT,
            "$id": format!("https://op-dbus.local/schemas/plugins/{public_name}.contract.json"),
            "title": format!("{public_name} contract schema"),
            "description": self.description,
            "type": "object",
            "required": [
                "schema_version",
                "plugin",
                "object_type",
                "object_id",
                "stub",
                "immutable",
                "tunable",
                "observed",
                "meta",
                "semantic_index",
                "privacy_index"
            ],
            "properties": {
                "schema_version": {
                    "type": "string",
                    "const": self.version
                },
                "plugin": {
                    "type": "string",
                    "const": public_name
                },
                "object_type": {
                    "type": "string",
                    "const": format!("{}_object", public_name.replace('-', "_"))
                },
                "object_id": {
                    "type": "string",
                    "minLength": 1
                },
                "stub": {
                    "type": "object",
                    "required": ["system_id", "source", "source_ref", "discovered_at"],
                    "properties": {
                        "system_id": { "type": "string", "minLength": 1 },
                        "source": { "type": "string", "minLength": 1 },
                        "source_ref": { "type": "string", "minLength": 1 },
                        "discovered_at": { "type": "string", "format": "date-time" }
                    },
                    "additionalProperties": false
                },
                "immutable": {
                    "type": "object",
                    "required": ["created_at", "created_by_plugin", "identity_keys", "provider"],
                    "properties": {
                        "created_at": { "type": "string", "format": "date-time" },
                        "created_by_plugin": { "type": "string", "const": public_name },
                        "identity_keys": {
                            "type": "array",
                            "items": { "type": "string" },
                            "minItems": 1,
                            "default": ["object_id"]
                        },
                        "provider": { "type": "string", "default": "op-dbus" }
                    },
                    "additionalProperties": false
                },
                "tunable": self.to_json_schema(),
                "observed": {
                    "type": "object",
                    "required": ["last_observed_at"],
                    "properties": {
                        "last_observed_at": { "type": "string", "format": "date-time" },
                        "status": { "type": "string" },
                        "drift_detected": { "type": "boolean", "default": false },
                        "metrics": { "type": "object" }
                    },
                    "additionalProperties": true
                },
                "meta": {
                    "type": "object",
                    "required": [
                        "dependencies",
                        "include_in_recovery",
                        "recovery_priority",
                        "sensitivity",
                        "tags",
                        "enabled"
                    ],
                    "properties": {
                        "dependencies": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": self.dependencies
                        },
                        "include_in_recovery": { "type": "boolean", "default": true },
                        "recovery_priority": { "type": "integer", "minimum": 0, "default": 50 },
                        "sensitivity": {
                            "type": "string",
                            "enum": ["public", "internal", "secret"],
                            "default": sensitivity
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": self.tags
                        },
                        "enabled": { "type": "boolean", "default": true }
                    },
                    "additionalProperties": false
                },
                "semantic_index": {
                    "type": "object",
                    "required": ["include_paths", "exclude_paths", "chunking", "redaction"],
                    "properties": {
                        "include_paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": include_paths
                        },
                        "exclude_paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "default": ["/stub/discovered_at"]
                        },
                        "chunking": {
                            "type": "object",
                            "required": ["strategy", "max_tokens"],
                            "properties": {
                                "strategy": { "type": "string", "enum": ["json-path-group"], "default": "json-path-group" },
                                "max_tokens": { "type": "integer", "minimum": 64, "default": 512 }
                            },
                            "additionalProperties": false
                        },
                        "redaction": {
                            "type": "object",
                            "required": ["enabled"],
                            "properties": {
                                "enabled": { "type": "boolean", "default": true }
                            },
                            "additionalProperties": false
                        }
                    },
                    "additionalProperties": false
                },
                "privacy_index": {
                    "type": "object",
                    "required": ["redaction"],
                    "properties": {
                        "redaction": {
                            "type": "object",
                            "required": [
                                "rules",
                                "default_action",
                                "secret_paths",
                                "pii_paths",
                                "hash_salt_ref",
                                "reversible"
                            ],
                            "properties": {
                                "rules": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "required": ["path", "action"],
                                        "properties": {
                                            "path": { "type": "string" },
                                            "action": { "type": "string", "enum": ["drop", "mask", "hash"] },
                                            "reason": { "type": "string" }
                                        },
                                        "additionalProperties": false
                                    },
                                    "default": []
                                },
                                "default_action": {
                                    "type": "string",
                                    "enum": ["drop", "mask", "hash"],
                                    "default": "mask"
                                },
                                "secret_paths": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "default": secret_paths
                                },
                                "pii_paths": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "default": pii_paths
                                },
                                "hash_salt_ref": {
                                    "type": "string",
                                    "default": "vault://op-dbus/privacy/hash-salt"
                                },
                                "reversible": {
                                    "type": "boolean",
                                    "default": false
                                }
                            },
                            "additionalProperties": false
                        }
                    },
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        })
    }
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Builder for creating plugin schemas
pub struct PluginSchemaBuilder {
    name: String,
    version: String,
    description: String,
    fields: HashMap<String, FieldSchema>,
    dependencies: Vec<String>,
    example: Option<Value>,
    immutable_paths: Vec<String>,
    tags: Vec<String>,
    dialect: String,
}

impl PluginSchemaBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            fields: HashMap::new(),
            dependencies: Vec::new(),
            example: None,
            immutable_paths: Vec::new(),
            tags: Vec::new(),
            dialect: DEFAULT_SCHEMA_DIALECT.to_string(),
        }
    }

    pub fn version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn field(mut self, name: &str, schema: FieldSchema) -> Self {
        self.fields.insert(name.to_string(), schema);
        self
    }

    pub fn string_field(self, name: &str, required: bool, description: &str) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::String,
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn integer_field(self, name: &str, required: bool, description: &str) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Integer,
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn boolean_field(self, name: &str, required: bool, description: &str) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Boolean,
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn array_field(
        self,
        name: &str,
        item_type: FieldType,
        required: bool,
        description: &str,
    ) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Array(Box::new(item_type)),
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn object_field(
        self,
        name: &str,
        fields: HashMap<String, FieldSchema>,
        required: bool,
        description: &str,
    ) -> Self {
        self.field(
            name,
            FieldSchema {
                field_type: FieldType::Object(fields),
                required,
                description: description.to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
    }

    pub fn dependency(mut self, plugin_name: &str) -> Self {
        self.dependencies.push(plugin_name.to_string());
        self
    }

    pub fn example(mut self, example: Value) -> Self {
        self.example = Some(example);
        self
    }

    /// Add a path that should always be readOnly (e.g., "/id")
    pub fn immutable_path(mut self, path: &str) -> Self {
        self.immutable_paths.push(path.to_string());
        self
    }

    /// Add multiple immutable paths at once
    pub fn immutable_paths(mut self, paths: &[&str]) -> Self {
        self.immutable_paths
            .extend(paths.iter().map(|s| s.to_string()));
        self
    }

    /// Add a tag to the schema (e.g., "immutable" for fully immutable)
    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// Mark the entire schema as immutable
    pub fn fully_immutable(self) -> Self {
        self.tag("immutable")
    }

    /// Set the JSON Schema dialect (e.g., dialects::V2026)
    pub fn dialect(mut self, dialect: &str) -> Self {
        self.dialect = dialect.to_string();
        self
    }

    pub fn build(self) -> PluginSchema {
        PluginSchema {
            name: self.name,
            version: self.version,
            description: self.description,
            fields: self.fields,
            dependencies: self.dependencies,
            example: self.example,
            immutable_paths: self.immutable_paths,
            tags: self.tags,
            dialect: self.dialect,
        }
    }
}

/// Registry of all plugin schemas with support for loading from files
pub struct SchemaRegistry {
    schemas: HashMap<String, PluginSchema>,
    meta_schemas: HashMap<String, Value>,
    spec_base_path: Option<PathBuf>,
}

impl SchemaRegistry {
    /// Create a new schema registry with built-in schemas
    pub fn new() -> Self {
        let mut registry = Self {
            schemas: HashMap::new(),
            meta_schemas: HashMap::new(),
            spec_base_path: None,
        };
        registry.register_builtin_schemas();
        registry
    }

    /// Create a registry with a custom spec base path
    pub fn with_spec_path(spec_path: impl AsRef<Path>) -> Self {
        let mut registry = Self {
            schemas: HashMap::new(),
            meta_schemas: HashMap::new(),
            spec_base_path: Some(spec_path.as_ref().to_path_buf()),
        };
        registry.register_builtin_schemas();
        registry
    }

    /// Set the base path for the json-schema-spec repository
    pub fn set_spec_path(&mut self, path: impl AsRef<Path>) {
        self.spec_base_path = Some(path.as_ref().to_path_buf());
    }

    /// Load meta-schema from the spec repository
    pub fn load_meta_schema(&mut self, dialect: &str) -> Result<(), SchemaLoadError> {
        let spec_path = self
            .spec_base_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(SCHEMA_SPEC_PATH));

        // Map dialect URL to file path
        let meta_path = match dialect {
            d if d.contains("2026") => spec_path.join("specs/meta/meta.json"),
            _ => return Err(SchemaLoadError::UnsupportedDialect(dialect.to_string())),
        };

        let mut content = fs::read_to_string(&meta_path)
            .map_err(|e| SchemaLoadError::IoError(meta_path.clone(), e.to_string()))?;

        let schema: Value = unsafe { simd_json::from_str(&mut content) }
            .map_err(|e| SchemaLoadError::ParseError(meta_path.clone(), e.to_string()))?;

        self.meta_schemas.insert(dialect.to_string(), schema);
        Ok(())
    }

    /// Get a loaded meta-schema
    pub fn get_meta_schema(&self, dialect: &str) -> Option<&Value> {
        self.meta_schemas.get(dialect)
    }

    /// Load a plugin schema from a JSON file
    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> Result<String, SchemaLoadError> {
        let path = path.as_ref();
        let mut content = fs::read_to_string(path)
            .map_err(|e| SchemaLoadError::IoError(path.to_path_buf(), e.to_string()))?;

        let schema: PluginSchema = unsafe { simd_json::from_str(&mut content) }
            .map_err(|e| SchemaLoadError::ParseError(path.to_path_buf(), e.to_string()))?;

        let name = schema.name.clone();
        self.register(schema);
        Ok(name)
    }

    /// Load all schema files from a directory
    pub fn load_from_directory(
        &mut self,
        dir: impl AsRef<Path>,
    ) -> Result<Vec<String>, SchemaLoadError> {
        let dir = dir.as_ref();
        let mut loaded = Vec::new();

        let entries = fs::read_dir(dir)
            .map_err(|e| SchemaLoadError::IoError(dir.to_path_buf(), e.to_string()))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| SchemaLoadError::IoError(dir.to_path_buf(), e.to_string()))?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match self.load_from_file(&path) {
                    Ok(name) => loaded.push(name),
                    Err(e) => {
                        tracing::warn!("Failed to load schema from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Export all schemas as JSON Schema documents
    pub fn export_all(&self) -> HashMap<String, Value> {
        self.schemas
            .iter()
            .map(|(name, schema)| (name.clone(), schema.to_json_schema()))
            .collect()
    }

    /// Export all schemas in draft-07 format (for backward compatibility)
    #[allow(deprecated)]
    pub fn export_all_draft07(&self) -> HashMap<String, Value> {
        self.schemas
            .iter()
            .map(|(name, schema)| (name.clone(), schema.to_json_schema_draft07()))
            .collect()
    }

    /// Export all schemas as legacy contract documents keyed by canonical plugin name.
    pub fn export_all_contract(&self) -> HashMap<String, Value> {
        self.schemas
            .iter()
            .map(|(name, schema)| (name.clone(), schema.to_contract_json_schema()))
            .collect()
    }

    /// Register a plugin schema
    pub fn register(&mut self, schema: PluginSchema) {
        self.schemas.insert(schema.name.clone(), schema);
    }

    /// Get a plugin schema by name
    pub fn get(&self, name: &str) -> Option<&PluginSchema> {
        self.schemas.get(Self::canonical_name(name))
    }

    /// Export one schema as a legacy contract document, preserving alias names.
    pub fn export_contract_for(&self, name: &str) -> Option<Value> {
        self.get(name)
            .map(|schema| schema.to_contract_json_schema_as(name))
    }

    /// List all registered schema names
    pub fn list(&self) -> Vec<&str> {
        self.schemas.keys().map(|s| s.as_str()).collect()
    }

    /// Validate state for a plugin
    pub fn validate(&self, plugin_name: &str, state: &Value) -> Option<ValidationResult> {
        self.get(plugin_name).map(|schema| schema.validate(state))
    }

    /// Register all built-in plugin schemas
    fn register_builtin_schemas(&mut self) {
        // LXC Container Schema
        self.register(create_lxc_schema());
        self.register(create_incus_schema());
        self.register(create_incus_wireguard_ingress_schema());
        self.register(create_incus_xray_reality_client_schema());
        self.register(create_incus_xray_reality_server_schema());

        // Network Schema
        self.register(create_net_schema());
        self.register(create_rtnetlink_schema());

        // OpenFlow Schema
        self.register(create_openflow_schema());

        // Dinit Schema
        self.register(create_dinit_schema());

        // Privacy Router Schema
        self.register(create_privacy_router_schema());
        self.register(create_privacy_routes_schema());

        // Netmaker Schema
        self.register(create_netmaker_schema());

        // Additional plugin-specific schemas for full materialization coverage
        self.register(create_adc_schema());
        self.register(create_agent_config_schema());
        self.register(create_config_schema());
        self.register(create_dnsresolver_schema());
        self.register(create_endpoint_schema());
        self.register(create_full_system_schema());
        self.register(create_gcloud_adc_schema());
        self.register(create_hardware_schema());
        self.register(create_keypair_schema());
        self.register(create_keyring_schema());
        self.register(create_login1_schema());
        self.register(create_mcp_schema());
        self.register(create_openflow_obfuscation_schema());
        self.register(create_ovsdb_bridge_schema());
        self.register(create_packagekit_schema());
        self.register(create_pcidecl_schema());
        self.register(create_privacy_schema());
        self.register(create_proxmox_schema());
        self.register(create_proxy_server_schema());
        self.register(create_service_schema());
        self.register(create_sess_decl_schema());
        self.register(create_software_schema());
        self.register(create_users_schema());
        self.register(create_web_ui_schema());
        self.register(create_wireguard_schema());
    }

    fn canonical_name(name: &str) -> &str {
        match name {
            "incus_wireguard_ingress" => "incus-wireguard-ingress",
            "incus_xray_reality_client" => "incus-xray-reality-client",
            "incus_xray_reality_server" => "incus-xray-reality-server",
            "systemd" => "dinit",
            "web-ui" => "web_ui",
            other => other,
        }
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when loading schemas
#[derive(Debug, Clone)]
pub enum SchemaLoadError {
    IoError(PathBuf, String),
    ParseError(PathBuf, String),
    UnsupportedDialect(String),
}

impl std::fmt::Display for SchemaLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(path, msg) => write!(f, "IO error reading {:?}: {}", path, msg),
            Self::ParseError(path, msg) => write!(f, "Parse error in {:?}: {}", path, msg),
            Self::UnsupportedDialect(d) => write!(f, "Unsupported dialect: {}", d),
        }
    }
}

impl std::error::Error for SchemaLoadError {}

// ============================================================================
// Built-in Schema Definitions
// ============================================================================

fn any_field(required: bool, description: &str, default: Option<Value>) -> FieldSchema {
    FieldSchema {
        field_type: FieldType::Any,
        required,
        description: description.to_string(),
        default,
        example: None,
        constraints: Vec::new(),
        read_only: false,
        read_only_when: None,
    }
}

fn is_secret_field_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "secret",
        "private",
        "token",
        "password",
        "credential",
        "license",
        "api_key",
        "key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_pii_field_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["email", "account", "google_id", "google_email", "user_id"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn simple_schema(
    name: &str,
    description: &str,
    dependencies: &[&str],
    fields: Vec<(&str, FieldSchema)>,
) -> PluginSchema {
    let mut builder = PluginSchema::builder(name)
        .version("1.0.0")
        .description(description);
    for dep in dependencies {
        builder = builder.dependency(dep);
    }
    for (field_name, schema) in fields {
        builder = builder.field(field_name, schema);
    }
    builder.build()
}

fn create_adc_schema() -> PluginSchema {
    simple_schema(
        "adc",
        "Application default credentials state",
        &[],
        vec![(
            "configured",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether ADC is configured".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

fn create_agent_config_schema() -> PluginSchema {
    simple_schema(
        "agent_config",
        "Agent configuration and tool assignments",
        &[],
        vec![(
            "agents",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Any)),
                required: true,
                description: "List of agent configurations".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

fn create_config_schema() -> PluginSchema {
    simple_schema(
        "config",
        "Global key/value config store",
        &[],
        vec![(
            "configs",
            any_field(true, "Configuration map", Some(json!({}))),
        )],
    )
}

fn create_dnsresolver_schema() -> PluginSchema {
    simple_schema(
        "dnsresolver",
        "DNS resolver declaration state",
        &["net"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: false,
                    description: "Schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "items",
                FieldSchema {
                    field_type: FieldType::Array(Box::new(FieldType::Any)),
                    required: true,
                    description: "Resolver items".to_string(),
                    default: Some(json!([])),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

fn create_endpoint_schema() -> PluginSchema {
    simple_schema(
        "endpoint",
        "Endpoint configuration",
        &["net"],
        vec![(
            "endpoints",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Declared endpoints".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

fn create_full_system_schema() -> PluginSchema {
    simple_schema(
        "full_system",
        "Full system recovery snapshot",
        &["net", "service", "software", "users", "lxc", "dinit"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Snapshot schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "captured_at",
                FieldSchema {
                    field_type: FieldType::String,
                    required: false,
                    description: "Capture timestamp".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            ("hostname", any_field(true, "Host name", Some(json!("")))),
            (
                "system",
                any_field(false, "System details", Some(json!({}))),
            ),
            (
                "network",
                any_field(false, "Network snapshot", Some(json!({}))),
            ),
            (
                "services",
                any_field(false, "Service snapshot", Some(json!([]))),
            ),
            (
                "packages",
                any_field(false, "Package snapshot", Some(json!([]))),
            ),
            ("users", any_field(false, "User snapshot", Some(json!([])))),
            (
                "storage",
                any_field(false, "Storage snapshot", Some(json!({}))),
            ),
            (
                "containers",
                any_field(false, "Container snapshot", Some(json!({}))),
            ),
            (
                "plugins",
                any_field(false, "Plugin snapshots", Some(json!({}))),
            ),
        ],
    )
}

fn create_gcloud_adc_schema() -> PluginSchema {
    simple_schema(
        "gcloud_adc",
        "Google Cloud ADC state",
        &[],
        vec![
            ("account", any_field(false, "Authenticated account", None)),
            ("project_id", any_field(false, "Project id", None)),
            (
                "authenticated",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Authentication status".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

fn create_hardware_schema() -> PluginSchema {
    simple_schema(
        "hardware",
        "Hardware inventory snapshot",
        &[],
        vec![
            ("cpu", any_field(true, "CPU info", Some(json!({})))),
            ("memory", any_field(true, "Memory info", Some(json!({})))),
            ("disks", any_field(true, "Disk list", Some(json!([])))),
        ],
    )
}

fn create_keypair_schema() -> PluginSchema {
    simple_schema(
        "keypair",
        "Keypair declaration state",
        &[],
        vec![(
            "keypairs",
            any_field(true, "Managed keypairs", Some(json!([]))),
        )],
    )
}

fn create_keyring_schema() -> PluginSchema {
    simple_schema(
        "keyring",
        "Secret service collections state",
        &[],
        vec![
            (
                "collections",
                any_field(true, "Secret collections", Some(json!([]))),
            ),
            (
                "default_collection",
                any_field(false, "Default collection path", None),
            ),
        ],
    )
}

fn create_login1_schema() -> PluginSchema {
    simple_schema(
        "login1",
        "Runtime login sessions",
        &["users"],
        vec![(
            "sessions",
            any_field(true, "Active sessions", Some(json!([]))),
        )],
    )
}

fn create_mcp_schema() -> PluginSchema {
    simple_schema(
        "mcp",
        "MCP server and tool-group configuration",
        &["agent_config"],
        vec![
            (
                "servers",
                any_field(false, "MCP server map", Some(json!({}))),
            ),
            (
                "tool_groups",
                any_field(false, "Tool group config", Some(json!({}))),
            ),
            (
                "compact_mode",
                any_field(false, "Compact mode config", Some(json!({}))),
            ),
        ],
    )
}

fn create_openflow_obfuscation_schema() -> PluginSchema {
    simple_schema(
        "openflow_obfuscation",
        "OpenFlow traffic obfuscation configuration",
        &["openflow", "net"],
        vec![(
            "config",
            any_field(true, "Obfuscation config", Some(json!({}))),
        )],
    )
}

fn create_ovsdb_bridge_schema() -> PluginSchema {
    simple_schema(
        "ovsdb_bridge",
        "OVS bridge declarations",
        &["net"],
        vec![(
            "bridges",
            any_field(true, "Bridge declarations", Some(json!([]))),
        )],
    )
}

fn create_packagekit_schema() -> PluginSchema {
    simple_schema(
        "packagekit",
        "PackageKit package declarations",
        &["software"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: false,
                    description: "Schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "packages",
                any_field(true, "Package declaration map", Some(json!({}))),
            ),
        ],
    )
}

fn create_pcidecl_schema() -> PluginSchema {
    simple_schema(
        "pcidecl",
        "PCI device declaration state",
        &["hardware"],
        vec![
            (
                "version",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: false,
                    description: "Schema version".to_string(),
                    default: Some(json!(1)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 1.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "items",
                any_field(true, "PCI declarations", Some(json!([]))),
            ),
        ],
    )
}

fn create_privacy_schema() -> PluginSchema {
    simple_schema(
        "privacy",
        "Privacy coordination configuration",
        &["wireguard", "proxmox", "privacy_router"],
        vec![("config", any_field(true, "Privacy config", Some(json!({}))))],
    )
}

fn create_proxmox_schema() -> PluginSchema {
    simple_schema(
        "proxmox",
        "Proxmox container declarations",
        &["net"],
        vec![(
            "containers",
            any_field(true, "Container declarations", Some(json!([]))),
        )],
    )
}

fn create_proxy_server_schema() -> PluginSchema {
    simple_schema(
        "proxy_server",
        "Proxy server runtime config",
        &["net"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable proxy".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "port",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Proxy port".to_string(),
                    default: Some(json!(8080)),
                    example: None,
                    constraints: vec![
                        Constraint::Min { value: 1.0 },
                        Constraint::Max { value: 65535.0 },
                    ],
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

fn create_service_schema() -> PluginSchema {
    simple_schema(
        "service",
        "Service definition declarations",
        &["net"],
        vec![("services", any_field(true, "Service map", Some(json!({}))))],
    )
}

fn create_sess_decl_schema() -> PluginSchema {
    simple_schema(
        "sess_decl",
        "Session declaration state",
        &["users"],
        vec![(
            "sessions",
            any_field(true, "Session declarations", Some(json!([]))),
        )],
    )
}

fn create_software_schema() -> PluginSchema {
    simple_schema(
        "software",
        "Software package inventory",
        &[],
        vec![("packages", any_field(true, "Package list", Some(json!([]))))],
    )
}

fn create_users_schema() -> PluginSchema {
    simple_schema(
        "users",
        "User account declarations",
        &[],
        vec![("users", any_field(true, "Users list", Some(json!([]))))],
    )
}

fn create_web_ui_schema() -> PluginSchema {
    simple_schema(
        "web_ui",
        "Web UI tunables",
        &["mcp"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable UI".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cors_origins",
                any_field(false, "Allowed CORS origins", Some(json!([]))),
            ),
            (
                "compression",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable compression".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cache_ttl",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Cache TTL seconds".to_string(),
                    default: Some(json!(86400)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 0.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "theme",
                any_field(true, "Theme name", Some(json!("default"))),
            ),
            (
                "feature_flags",
                any_field(false, "Feature flag map", Some(json!({}))),
            ),
        ],
    )
}

fn create_wireguard_schema() -> PluginSchema {
    simple_schema(
        "wireguard",
        "WireGuard interface state",
        &["net"],
        vec![(
            "interfaces",
            any_field(true, "WireGuard interfaces", Some(json!([]))),
        )],
    )
}

fn create_lxc_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Container VMID".to_string(),
                default: None,
                example: Some(json!("100")),
                constraints: vec![Constraint::Pattern {
                    regex: r"^\d+$".to_string(),
                }],
                read_only: true, // ID is immutable once created
                read_only_when: None,
            },
        );
        fields.insert(
            "veth".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Veth interface name".to_string(),
                default: None,
                example: Some(json!("vi100")),
                constraints: Vec::new(),
                read_only: false,
                // veth becomes readOnly when container is running
                read_only_when: Some(ReadOnlyCondition {
                    property: "running".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "bridge".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "OVS bridge name".to_string(),
                default: Some(json!("ovs-br0")),
                example: Some(json!("ovs-br0")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "running".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether container is running".to_string(),
                default: Some(json!(false)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "properties".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Container properties (hostname, memory, cores, etc.)".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "hostname": "my-container",
                    "memory": 512,
                    "cores": 2,
                    "template": "local:vztmpl/debian-13.tar.zst"
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("lxc")
        .version("2.0.0")
        .description("LXC container management via native Proxmox API")
        .array_field(
            "containers",
            FieldType::Object(container_fields),
            true,
            "List of containers",
        )
        .example(json!({
            "containers": [
                {
                    "id": "100",
                    "veth": "vi100",
                    "bridge": "ovs-br0",
                    "running": true,
                    "properties": {
                        "hostname": "wireguard-gateway",
                        "memory": 512,
                        "cores": 1,
                        "network_type": "bridge"
                    }
                }
            ]
        }))
        .build()
}

fn create_incus_schema() -> PluginSchema {
    let instance_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-123")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "status".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "Running".to_string(),
                    "Stopped".to_string(),
                    "Frozen".to_string(),
                ]),
                required: true,
                description: "Instance status".to_string(),
                default: Some(json!("Stopped")),
                example: Some(json!("Running")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "container".to_string(),
                    "virtual-machine".to_string(),
                ]),
                required: true,
                description: "Instance type".to_string(),
                default: Some(json!("container")),
                example: Some(json!("container")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source image reference".to_string(),
                default: None,
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "storage_pool".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Preferred Incus storage pool for initial creation".to_string(),
                default: None,
                example: Some(json!("registration")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Applied Incus profiles".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "config".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance configuration map".to_string(),
                default: Some(json!({})),
                example: Some(json!({"limits.cpu": "2"})),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance device definitions".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "eth0": {
                        "type": "nic",
                        "nictype": "bridged",
                        "parent": "ovsbr0"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus")
        .version("1.0.0")
        .description("Incus instance management")
        .array_field(
            "instances",
            FieldType::Object(instance_fields),
            true,
            "List of Incus instances",
        )
        .example(json!({
            "instances": [
                {
                    "name": "privacy-user-123",
                    "status": "Running",
                    "type": "container",
                    "image": "images:debian/13",
                    "storage_pool": "registration",
                    "profiles": ["default"],
                    "config": {
                        "limits.cpu": "2"
                    },
                    "devices": {
                        "eth0": {
                            "type": "nic",
                            "nictype": "bridged",
                            "parent": "ovsbr0"
                        }
                    }
                }
            ]
        }))
        .build()
}

fn create_incus_wireguard_ingress_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus image alias for the WireGuard ingress container".to_string(),
                default: Some(json!("images:alpine/edge")),
                example: Some(json!("images:alpine/edge")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Incus profiles applied to the container".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default", "privacy-system"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            any_field(
                false,
                "Incus device overrides such as NICs and disks",
                Some(json!({})),
            ),
        );
        fields
    };

    let peer_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Peer public key".to_string(),
                default: None,
                example: Some(json!("base64publickey")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "allowed_ips".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Allowed IP ranges for the peer".to_string(),
                default: Some(json!(["0.0.0.0/0"])),
                example: Some(json!(["10.0.0.2/32"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "endpoint".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional peer endpoint host:port".to_string(),
                default: None,
                example: Some(json!("vpn.example.com:51820")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "persistent_keepalive".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Persistent keepalive interval in seconds".to_string(),
                default: None,
                example: Some(json!(25)),
                constraints: vec![
                    Constraint::Min { value: 0.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let wireguard_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "interface".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "WireGuard interface name inside the container".to_string(),
                default: Some(json!("wg0")),
                example: Some(json!("wg0")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "listen_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "WireGuard listen port".to_string(),
                default: Some(json!(51820)),
                example: Some(json!(51820)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "private_key_env".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Environment variable name holding the private key".to_string(),
                default: Some(json!("WIREGUARD_PRIVATE_KEY")),
                example: Some(json!("WIREGUARD_PRIVATE_KEY")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "CIDR address assigned to the WireGuard interface".to_string(),
                default: None,
                example: Some(json!("10.0.0.1/24")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "dns".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "DNS resolvers pushed to clients".to_string(),
                default: Some(json!([])),
                example: Some(json!(["1.1.1.1", "1.0.0.1"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "peers".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(peer_fields))),
                required: true,
                description: "WireGuard peers served by the ingress gateway".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "public_key": "base64publickey",
                    "allowed_ips": ["10.0.0.2/32"]
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let capability_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "requires_root".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the container requires root privileges".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "supports_rollback".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the deployment supports rollback".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus-wireguard-ingress")
        .version("1.0.0")
        .description("Incus system container declaration for the WireGuard ingress gateway")
        .field(
            "name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema object name".to_string(),
                default: Some(json!("incus-wireguard-ingress")),
                example: Some(json!("incus-wireguard-ingress")),
                constraints: vec![
                    Constraint::Pattern {
                        regex: "^[a-z0-9_-]+$".to_string(),
                    },
                    Constraint::Max { value: 64.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema version".to_string(),
                default: Some(json!("1.0.0")),
                example: Some(json!("1.0.0")),
                constraints: vec![Constraint::Pattern {
                    regex: "^\\d+\\.\\d+\\.\\d+$".to_string(),
                }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "plugin_type",
            FieldSchema {
                field_type: FieldType::Enum(vec!["network".to_string()]),
                required: true,
                description: "Container schema category".to_string(),
                default: Some(json!("network")),
                example: Some(json!("network")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .object_field(
            "container",
            container_fields,
            true,
            "Incus container image, profiles, and device overrides",
        )
        .object_field(
            "wireguard",
            wireguard_fields,
            true,
            "WireGuard ingress service configuration",
        )
        .object_field(
            "capabilities",
            capability_fields,
            false,
            "Operational capabilities for the container implementation",
        )
        .field(
            "service",
            any_field(false, "Optional service declaration", Some(json!({}))),
        )
        .example(json!({
            "name": "incus-wireguard-ingress",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:alpine/edge",
                "profiles": ["default", "privacy-system"],
                "devices": {}
            },
            "wireguard": {
                "interface": "wg0",
                "listen_port": 51820,
                "private_key_env": "WIREGUARD_PRIVATE_KEY",
                "address": "10.0.0.1/24",
                "dns": ["1.1.1.1", "1.0.0.1"],
                "peers": [{
                    "public_key": "base64publickey",
                    "allowed_ips": ["10.0.0.2/32"]
                }]
            },
            "capabilities": {
                "requires_root": true,
                "supports_rollback": false
            },
            "service": {}
        }))
        .build()
}

fn create_incus_xray_reality_client_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus image alias for the Xray client container".to_string(),
                default: Some(json!("images:debian/13")),
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Incus profiles applied to the container".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default", "privacy-system"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            any_field(
                false,
                "Incus device overrides such as NICs and disks",
                Some(json!({})),
            ),
        );
        fields
    };

    let inbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray inbound tag".to_string(),
                default: None,
                example: Some(json!("socks-in")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "socks".to_string(),
                    "http".to_string(),
                    "dokodemo-door".to_string(),
                ]),
                required: true,
                description: "Inbound protocol".to_string(),
                default: Some(json!("socks")),
                example: Some(json!("socks")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Listener port".to_string(),
                default: Some(json!(1080)),
                example: Some(json!(1080)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "listen".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Listener bind address".to_string(),
                default: Some(json!("127.0.0.1")),
                example: Some(json!("127.0.0.1")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "sniffing".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Enable protocol sniffing".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vnext_user_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "User UUID".to_string(),
                default: None,
                example: Some(json!("00000000-0000-0000-0000-000000000000")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flow".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["xtls-rprx-vision".to_string()]),
                required: true,
                description: "REALITY flow".to_string(),
                default: Some(json!("xtls-rprx-vision")),
                example: Some(json!("xtls-rprx-vision")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "encryption".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["none".to_string()]),
                required: false,
                description: "Encryption mode".to_string(),
                default: Some(json!("none")),
                example: Some(json!("none")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vnext_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Remote Xray server hostname or IP".to_string(),
                default: None,
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Remote Xray server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "users".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(vnext_user_fields))),
                required: true,
                description: "Authorized VLESS users".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "id": "00000000-0000-0000-0000-000000000000",
                    "flow": "xtls-rprx-vision",
                    "encryption": "none"
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let outbound_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "vnext".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(vnext_fields))),
                required: true,
                description: "Remote VLESS upstream definitions".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "address": "vps.example.com",
                    "port": 443,
                    "users": [{
                        "id": "00000000-0000-0000-0000-000000000000",
                        "flow": "xtls-rprx-vision",
                        "encryption": "none"
                    }]
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let reality_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "server_name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "TLS server name to mimic".to_string(),
                default: None,
                example: Some(json!("www.microsoft.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Server public key".to_string(),
                default: None,
                example: Some(json!("base64publickey")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "short_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "REALITY short ID".to_string(),
                default: None,
                example: Some(json!("1234abcd")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "fingerprint".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Client fingerprint".to_string(),
                default: Some(json!("chrome")),
                example: Some(json!("chrome")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let stream_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "network".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["tcp".to_string()]),
                required: true,
                description: "Transport network".to_string(),
                default: Some(json!("tcp")),
                example: Some(json!("tcp")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "security".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["reality".to_string()]),
                required: true,
                description: "Transport security".to_string(),
                default: Some(json!("reality")),
                example: Some(json!("reality")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "reality_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(reality_fields),
                required: true,
                description: "REALITY transport settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "server_name": "www.microsoft.com",
                    "public_key": "base64publickey",
                    "short_id": "1234abcd",
                    "fingerprint": "chrome"
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let outbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray outbound tag".to_string(),
                default: None,
                example: Some(json!("reality-out")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["vless".to_string()]),
                required: true,
                description: "Outbound protocol".to_string(),
                default: Some(json!("vless")),
                example: Some(json!("vless")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(outbound_settings_fields),
                required: true,
                description: "Outbound server settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "vnext": [{
                        "address": "vps.example.com",
                        "port": 443,
                        "users": [{
                            "id": "00000000-0000-0000-0000-000000000000",
                            "flow": "xtls-rprx-vision",
                            "encryption": "none"
                        }]
                    }]
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "stream_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(stream_settings_fields),
                required: true,
                description: "REALITY transport settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "network": "tcp",
                    "security": "reality",
                    "reality_settings": {
                        "server_name": "www.microsoft.com",
                        "public_key": "base64publickey",
                        "short_id": "1234abcd",
                        "fingerprint": "chrome"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "log_level".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "debug".to_string(),
                    "info".to_string(),
                    "warning".to_string(),
                    "error".to_string(),
                    "none".to_string(),
                ]),
                required: false,
                description: "Xray log level".to_string(),
                default: Some(json!("warning")),
                example: Some(json!("warning")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "inbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(inbound_fields))),
                required: true,
                description: "Local proxy listeners".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "tag": "socks-in",
                    "protocol": "socks",
                    "port": 1080,
                    "listen": "127.0.0.1",
                    "sniffing": true
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "outbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(outbound_fields))),
                required: true,
                description: "REALITY egress definitions".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "tag": "reality-out",
                    "protocol": "vless",
                    "settings": {
                        "vnext": [{
                            "address": "vps.example.com",
                            "port": 443,
                            "users": [{
                                "id": "00000000-0000-0000-0000-000000000000",
                                "flow": "xtls-rprx-vision",
                                "encryption": "none"
                            }]
                        }]
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "server_name": "www.microsoft.com",
                            "public_key": "base64publickey",
                            "short_id": "1234abcd",
                            "fingerprint": "chrome"
                        }
                    }
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let capability_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "requires_root".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the container requires root privileges".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "supports_rollback".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the deployment supports rollback".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus-xray-reality-client")
        .version("1.0.0")
        .description("Incus system container declaration for the Xray REALITY outbound client")
        .field(
            "name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema object name".to_string(),
                default: Some(json!("incus-xray-reality-client")),
                example: Some(json!("incus-xray-reality-client")),
                constraints: vec![
                    Constraint::Pattern {
                        regex: "^[a-z0-9_-]+$".to_string(),
                    },
                    Constraint::Max { value: 64.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema version".to_string(),
                default: Some(json!("1.0.0")),
                example: Some(json!("1.0.0")),
                constraints: vec![Constraint::Pattern {
                    regex: "^\\d+\\.\\d+\\.\\d+$".to_string(),
                }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "plugin_type",
            FieldSchema {
                field_type: FieldType::Enum(vec!["network".to_string()]),
                required: true,
                description: "Container schema category".to_string(),
                default: Some(json!("network")),
                example: Some(json!("network")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .object_field(
            "container",
            container_fields,
            true,
            "Incus container image, profiles, and device overrides",
        )
        .object_field(
            "xray",
            xray_fields,
            true,
            "Xray REALITY client configuration",
        )
        .object_field(
            "capabilities",
            capability_fields,
            false,
            "Operational capabilities for the container implementation",
        )
        .field(
            "service",
            any_field(false, "Optional service declaration", Some(json!({}))),
        )
        .example(json!({
            "name": "incus-xray-reality-client",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:debian/13",
                "profiles": ["default", "privacy-system"],
                "devices": {}
            },
            "xray": {
                "log_level": "warning",
                "inbounds": [{
                    "tag": "socks-in",
                    "protocol": "socks",
                    "port": 1080,
                    "listen": "127.0.0.1",
                    "sniffing": true
                }],
                "outbounds": [{
                    "tag": "reality-out",
                    "protocol": "vless",
                    "settings": {
                        "vnext": [{
                            "address": "vps.example.com",
                            "port": 443,
                            "users": [{
                                "id": "00000000-0000-0000-0000-000000000000",
                                "flow": "xtls-rprx-vision",
                                "encryption": "none"
                            }]
                        }]
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "server_name": "www.microsoft.com",
                            "public_key": "base64publickey",
                            "short_id": "1234abcd",
                            "fingerprint": "chrome"
                        }
                    }
                }]
            },
            "capabilities": {
                "requires_root": false,
                "supports_rollback": false
            },
            "service": {}
        }))
        .build()
}

fn create_incus_xray_reality_server_schema() -> PluginSchema {
    let container_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus image alias for the Xray server container".to_string(),
                default: Some(json!("images:debian/13")),
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Incus profiles applied to the container".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default", "privacy-system"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            any_field(
                false,
                "Incus device overrides such as NICs and disks",
                Some(json!({})),
            ),
        );
        fields
    };

    let client_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Authorized user UUID".to_string(),
                default: None,
                example: Some(json!("00000000-0000-0000-0000-000000000000")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flow".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["xtls-rprx-vision".to_string()]),
                required: true,
                description: "REALITY flow".to_string(),
                default: Some(json!("xtls-rprx-vision")),
                example: Some(json!("xtls-rprx-vision")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "email".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional client label".to_string(),
                default: None,
                example: Some(json!("user@example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let inbound_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "clients".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(client_fields))),
                required: true,
                description: "Authorized inbound clients".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "id": "00000000-0000-0000-0000-000000000000",
                    "flow": "xtls-rprx-vision",
                    "email": "user@example.com"
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "decryption".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["none".to_string()]),
                required: false,
                description: "VLESS decryption mode".to_string(),
                default: Some(json!("none")),
                example: Some(json!("none")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let reality_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "show".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Enable REALITY debug output".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "dest".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Fallback destination for non-REALITY traffic".to_string(),
                default: None,
                example: Some(json!("www.microsoft.com:443")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "server_names".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Allowed SNI values".to_string(),
                default: Some(json!([])),
                example: Some(json!(["www.microsoft.com"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "private_key_env".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Environment variable name holding the private key".to_string(),
                default: None,
                example: Some(json!("XRAY_PRIVATE_KEY")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "private_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Inline x25519 private key".to_string(),
                default: None,
                example: Some(json!("base64privatekey")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "short_ids".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Allowed REALITY short IDs".to_string(),
                default: Some(json!([])),
                example: Some(json!(["1234abcd"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let stream_settings_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "network".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["tcp".to_string()]),
                required: true,
                description: "Transport network".to_string(),
                default: Some(json!("tcp")),
                example: Some(json!("tcp")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "security".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["reality".to_string()]),
                required: true,
                description: "Transport security".to_string(),
                default: Some(json!("reality")),
                example: Some(json!("reality")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "reality_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(reality_fields),
                required: true,
                description: "REALITY listener settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "show": false,
                    "dest": "www.microsoft.com:443",
                    "server_names": ["www.microsoft.com"],
                    "private_key_env": "XRAY_PRIVATE_KEY",
                    "short_ids": ["1234abcd"]
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let inbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray inbound tag".to_string(),
                default: None,
                example: Some(json!("reality-in")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["vless".to_string()]),
                required: true,
                description: "Inbound protocol".to_string(),
                default: Some(json!("vless")),
                example: Some(json!("vless")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Listener port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "listen".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Listener bind address".to_string(),
                default: Some(json!("0.0.0.0")),
                example: Some(json!("0.0.0.0")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(inbound_settings_fields),
                required: true,
                description: "Inbound client settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "clients": [{
                        "id": "00000000-0000-0000-0000-000000000000",
                        "flow": "xtls-rprx-vision",
                        "email": "user@example.com"
                    }],
                    "decryption": "none"
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "stream_settings".to_string(),
            FieldSchema {
                field_type: FieldType::Object(stream_settings_fields),
                required: true,
                description: "REALITY transport settings".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "network": "tcp",
                    "security": "reality",
                    "reality_settings": {
                        "show": false,
                        "dest": "www.microsoft.com:443",
                        "server_names": ["www.microsoft.com"],
                        "private_key_env": "XRAY_PRIVATE_KEY",
                        "short_ids": ["1234abcd"]
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let outbound_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "tag".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional Xray outbound tag".to_string(),
                default: None,
                example: Some(json!("direct")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocol".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["freedom".to_string(), "blackhole".to_string()]),
                required: true,
                description: "Outbound protocol".to_string(),
                default: Some(json!("freedom")),
                example: Some(json!("freedom")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "log_level".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "debug".to_string(),
                    "info".to_string(),
                    "warning".to_string(),
                    "error".to_string(),
                    "none".to_string(),
                ]),
                required: false,
                description: "Xray log level".to_string(),
                default: Some(json!("warning")),
                example: Some(json!("warning")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "inbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(inbound_fields))),
                required: true,
                description: "REALITY server listeners".to_string(),
                default: Some(json!([])),
                example: Some(json!([{
                    "tag": "reality-in",
                    "protocol": "vless",
                    "port": 443,
                    "listen": "0.0.0.0",
                    "settings": {
                        "clients": [{
                            "id": "00000000-0000-0000-0000-000000000000",
                            "flow": "xtls-rprx-vision",
                            "email": "user@example.com"
                        }],
                        "decryption": "none"
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "show": false,
                            "dest": "www.microsoft.com:443",
                            "server_names": ["www.microsoft.com"],
                            "private_key_env": "XRAY_PRIVATE_KEY",
                            "short_ids": ["1234abcd"]
                        }
                    }
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "outbounds".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(outbound_fields))),
                required: true,
                description: "Server-side outbounds, typically direct or blackhole".to_string(),
                default: Some(json!([{"protocol": "freedom"}])),
                example: Some(json!([{
                    "tag": "direct",
                    "protocol": "freedom"
                }])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let capability_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "requires_root".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the container requires root privileges".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "supports_rollback".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the deployment supports rollback".to_string(),
                default: Some(json!(false)),
                example: Some(json!(false)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus-xray-reality-server")
        .version("1.0.0")
        .description("Incus system container declaration for the Xray REALITY inbound server")
        .field(
            "name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema object name".to_string(),
                default: Some(json!("incus-xray-reality-server")),
                example: Some(json!("incus-xray-reality-server")),
                constraints: vec![
                    Constraint::Pattern {
                        regex: "^[a-z0-9_-]+$".to_string(),
                    },
                    Constraint::Max { value: 64.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Schema version".to_string(),
                default: Some(json!("1.0.0")),
                example: Some(json!("1.0.0")),
                constraints: vec![Constraint::Pattern {
                    regex: "^\\d+\\.\\d+\\.\\d+$".to_string(),
                }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "plugin_type",
            FieldSchema {
                field_type: FieldType::Enum(vec!["network".to_string()]),
                required: true,
                description: "Container schema category".to_string(),
                default: Some(json!("network")),
                example: Some(json!("network")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .object_field(
            "container",
            container_fields,
            true,
            "Incus container image, profiles, and device overrides",
        )
        .object_field(
            "xray",
            xray_fields,
            true,
            "Xray REALITY server configuration",
        )
        .object_field(
            "capabilities",
            capability_fields,
            false,
            "Operational capabilities for the container implementation",
        )
        .field(
            "service",
            any_field(false, "Optional service declaration", Some(json!({}))),
        )
        .example(json!({
            "name": "incus-xray-reality-server",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:debian/13",
                "profiles": ["default", "privacy-system"],
                "devices": {}
            },
            "xray": {
                "log_level": "warning",
                "inbounds": [{
                    "tag": "reality-in",
                    "protocol": "vless",
                    "port": 443,
                    "listen": "0.0.0.0",
                    "settings": {
                        "clients": [{
                            "id": "00000000-0000-0000-0000-000000000000",
                            "flow": "xtls-rprx-vision",
                            "email": "user@example.com"
                        }],
                        "decryption": "none"
                    },
                    "stream_settings": {
                        "network": "tcp",
                        "security": "reality",
                        "reality_settings": {
                            "show": false,
                            "dest": "www.microsoft.com:443",
                            "server_names": ["www.microsoft.com"],
                            "private_key_env": "XRAY_PRIVATE_KEY",
                            "short_ids": ["1234abcd"]
                        }
                    }
                }],
                "outbounds": [{
                    "tag": "direct",
                    "protocol": "freedom"
                }]
            },
            "capabilities": {
                "requires_root": false,
                "supports_rollback": false
            },
            "service": {}
        }))
        .build()
}

fn create_net_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true, // Interface name is identity
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "ethernet".to_string(),
                    "bridge".to_string(),
                    "veth".to_string(),
                    "vlan".to_string(),
                    "bond".to_string(),
                ]),
                required: true,
                description: "Interface type".to_string(),
                default: Some(json!("ethernet")),
                example: Some(json!("ethernet")),
                constraints: Vec::new(),
                read_only: true, // Type cannot change after creation
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "IP addresses".to_string(),
                default: Some(json!([])),
                example: Some(json!(["192.168.1.100/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("net")
        .version("1.0.0")
        .description("Network interface management via rtnetlink")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "List of network interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "eth0",
                    "type": "ethernet",
                    "state": "up",
                    "addresses": ["192.168.1.100/24"]
                }
            ]
        }))
        .build()
}

fn create_rtnetlink_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Administrative interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Interface IP addresses in CIDR form".to_string(),
                default: Some(json!([])),
                example: Some(json!(["10.0.0.2/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "mac_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional MAC address override".to_string(),
                default: None,
                example: Some(json!("02:00:00:00:00:01")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "default_gateway".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Default gateway for this interface".to_string(),
                default: None,
                example: Some(json!("10.0.0.1")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("rtnetlink")
        .version("1.0.0")
        .description("Native kernel rtnetlink interface management")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "Desired rtnetlink-managed interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "ovsbr0",
                    "state": "up",
                    "addresses": ["10.10.0.1/24"],
                    "default_gateway": "10.10.0.254"
                }
            ]
        }))
        .build()
}

fn create_openflow_schema() -> PluginSchema {
    let bridge_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Bridge name".to_string(),
                default: None,
                example: Some(json!("ovs-br0")),
                constraints: Vec::new(),
                read_only: true, // Bridge name is identity
                read_only_when: None,
            },
        );
        fields.insert(
            "datapath_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Datapath ID".to_string(),
                default: None,
                example: Some(json!("0000000000000001")),
                constraints: Vec::new(),
                read_only: true, // Datapath ID is immutable
                read_only_when: None,
            },
        );
        fields.insert(
            "protocols".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Supported OpenFlow protocols".to_string(),
                default: Some(json!(["OpenFlow13"])),
                example: Some(json!(["OpenFlow10", "OpenFlow13"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flows".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "table".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "OpenFlow table number".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 254.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "priority".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "Flow priority".to_string(),
                            default: Some(json!(100)),
                            example: Some(json!(22000)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 65535.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "match_fields".to_string(),
                        FieldSchema {
                            field_type: FieldType::Any,
                            required: true,
                            description: "OpenFlow match fields".to_string(),
                            default: None,
                            example: Some(
                                json!({"in_port": "ovsbr0-sock", "nw_src": "10.100.0.2"}),
                            ),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "actions".to_string(),
                        FieldSchema {
                            field_type: FieldType::Array(Box::new(FieldType::Any)),
                            required: true,
                            description: "OpenFlow actions".to_string(),
                            default: None,
                            example: Some(json!([{"type": "output", "port": "priv_wg"}])),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "cookie".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Flow cookie for idempotent route ownership".to_string(),
                            default: None,
                            example: Some(json!(5787125521171081216u64)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "idle_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Idle timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "hard_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Hard timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Flows managed for this bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_ports".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "OVS socket port name".to_string(),
                            default: None,
                            example: Some(json!("ovsbr0-sock")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "container_name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: false,
                            description: "Optional legacy container name bound to this port"
                                .to_string(),
                            default: None,
                            example: Some(json!("privacy-user-abc")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "port_type".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "Socket port role".to_string(),
                            default: Some(json!("SharedIngress")),
                            example: Some(json!("SharedIngress")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "ofport".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Resolved OpenFlow port number".to_string(),
                            default: None,
                            example: Some(json!(7)),
                            constraints: Vec::new(),
                            read_only: true,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Managed OVS socket ports for the bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("openflow")
        .version("1.0.0")
        .description("OpenFlow flow table management")
        .dependency("net")
        .dependency("privacy_routes")
        .array_field(
            "bridges",
            FieldType::Object(bridge_fields),
            true,
            "OVS bridges",
        )
        .string_field("controller_endpoint", false, "OpenFlow controller endpoint")
        .boolean_field(
            "auto_discover_containers",
            false,
            "Auto-create flows from discovered legacy container sockets",
        )
        .boolean_field("enable_security_flows", false, "Inject hardening flows before route flows")
        .integer_field("obfuscation_level", false, "Traffic obfuscation level for generated flows")
        .example(json!({
            "bridges": [
                {
                    "name": "ovsbr0",
                    "protocols": ["OpenFlow13"],
                    "socket_ports": [
                        {
                            "name": "ovsbr0-sock",
                            "port_type": "SharedIngress"
                        }
                    ],
                    "flows": [
                        {
                            "table": 0,
                            "priority": 22000,
                            "match_fields": {"in_port": "ovsbr0-sock", "ip": "", "nw_src": "10.100.0.2"},
                            "actions": [{"type": "output", "port": "priv_wg"}],
                            "cookie": 5787125521171081216u64,
                            "idle_timeout": 0,
                            "hard_timeout": 0
                        }
                    ]
                }
            ],
            "auto_discover_containers": false,
            "enable_security_flows": false,
            "obfuscation_level": 0
        }))
        .build()
}

fn create_dinit_schema() -> PluginSchema {
    let unit_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unit name".to_string(),
                default: None,
                example: Some(json!("nginx.service")),
                constraints: Vec::new(),
                read_only: true, // Unit name is identity
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "active".to_string(),
                    "inactive".to_string(),
                    "failed".to_string(),
                ]),
                required: false,
                description: "Desired unit state".to_string(),
                default: Some(json!("active")),
                example: Some(json!("active")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether unit is enabled at boot".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("dinit")
        .version("1.0.0")
        .description("Dinit service management")
        .array_field(
            "units",
            FieldType::Object(unit_fields),
            true,
            "Dinit services",
        )
        .example(json!({
            "units": [
                {
                    "name": "nginx.service",
                    "state": "active",
                    "enabled": true
                }
            ]
        }))
        .build()
}

fn create_privacy_router_schema() -> PluginSchema {
    let wireguard_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable WireGuard tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for WireGuard".to_string(),
                default: Some(json!(100)),
                example: Some(json!(100)),
                constraints: Vec::new(),
                read_only: false,
                // Container ID becomes immutable when enabled
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "listen_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "WireGuard listen port".to_string(),
                default: Some(json!(51820)),
                example: Some(json!(51820)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port name for the WireGuard ingress container"
                    .to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let warp_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable Cloudflare WARP tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "bridge_interface".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host WireGuard interface bridged into OVS for WARP egress"
                    .to_string(),
                default: Some(json!("wgcf")),
                example: Some(json!("wgcf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wgcf_config".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Path to wgcf WireGuard config used to create the host interface"
                    .to_string(),
                default: Some(json!("/etc/wireguard/wgcf.conf")),
                example: Some(json!("/etc/wireguard/wgcf.conf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable system XRay client tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for the local XRay client".to_string(),
                default: Some(json!(101)),
                example: Some(json!(101)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port for the local XRay client".to_string(),
                default: Some(json!("priv_xray")),
                example: Some(json!("priv_xray")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socks_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "SOCKS listener port exposed by the local XRay client".to_string(),
                default: Some(json!(1080)),
                example: Some(json!(1080)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vps_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "xray_server".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "xray_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_router")
        .version("1.1.0")
        .description("System privacy fabric (WireGuard/XRay ingress, WARP bridge, XRay egress)")
        .dependency("incus")
        .dependency("openflow")
        .dependency("privacy_routes")
        .string_field("bridge_name", true, "OVS bridge for privacy network")
        .object_field(
            "wireguard",
            wireguard_fields,
            true,
            "WireGuard tunnel config",
        )
        .object_field("warp", warp_fields, true, "Cloudflare WARP bridge config")
        .object_field(
            "xray",
            xray_fields,
            true,
            "XRay REALITY egress client config",
        )
        .object_field(
            "vps",
            vps_fields,
            true,
            "Remote XRay server endpoint config",
        )
        .example(json!({
            "bridge_name": "ovsbr0",
            "wireguard": {
                "enabled": true,
                "container_id": 100,
                "socket_port": "priv_wg",
                "listen_port": 51820
            },
            "warp": {
                "enabled": true,
                "bridge_interface": "wgcf",
                "wgcf_config": "/etc/wireguard/wgcf.conf"
            },
            "xray": {
                "enabled": true,
                "container_id": 101,
                "socket_port": "priv_xray",
                "socks_port": 1080,
                "vps_address": "vps.example.com",
                "vps_port": 443
            },
            "vps": {
                "xray_server": "vps.example.com",
                "xray_port": 443
            }
        }))
        .build()
}

fn create_privacy_routes_schema() -> PluginSchema {
    let route_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Stable route object identifier".to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "route_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Derived route ID from WireGuard public key and shared secret"
                    .to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "user_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Internal privacy user identifier".to_string(),
                default: None,
                example: Some(json!("550e8400-e29b-41d4-a716-446655440000")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "email".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "User email for audit and publication context".to_string(),
                default: None,
                example: Some(json!("user@example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wireguard_public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "WireGuard public key backing this route identity".to_string(),
                default: None,
                example: Some(json!("P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "assigned_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Assigned WireGuard tunnel address".to_string(),
                default: None,
                example: Some(json!("10.100.0.2/32")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "selector_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Packet-visible selector used for OpenFlow matching".to_string(),
                default: None,
                example: Some(json!("10.100.0.2")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Associated Incus instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-550e8400")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "ingress_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Shared OVS ingress port for route matching".to_string(),
                default: Some(json!("ovsbr0-sock")),
                example: Some(json!("ovsbr0-sock")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "next_hop".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "First logical next hop for this route".to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether this route should be active".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "created_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Creation timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:00:00Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "updated_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Last update timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:05:00Z")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_routes")
        .version("1.0.0")
        .description("Per-user privacy route objects keyed by WireGuard identity")
        .dependency("wireguard")
        .dependency("privacy_router")
        .array_field(
            "routes",
            FieldType::Object(route_fields),
            true,
            "Published privacy route objects",
        )
        .example(json!({
            "routes": [
                {
                    "name": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "route_id": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "user_id": "550e8400-e29b-41d4-a716-446655440000",
                    "email": "user@example.com",
                    "wireguard_public_key": "P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=",
                    "assigned_ip": "10.100.0.2/32",
                    "selector_ip": "10.100.0.2",
                    "container_name": "privacy-user-550e8400",
                    "ingress_port": "ovsbr0-sock",
                    "next_hop": "priv_wg",
                    "enabled": true,
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                }
            ]
        }))
        .build()
}

fn create_netmaker_schema() -> PluginSchema {
    PluginSchema::builder("netmaker")
        .version("1.0.0")
        .description("Netmaker mesh network management")
        .dependency("net")
        .string_field("network_name", true, "Netmaker network name")
        .string_field("interface", false, "WireGuard interface name (e.g., nm0)")
        .string_field("server_url", false, "Netmaker server URL")
        .string_field(
            "enrollment_token",
            false,
            "Enrollment token for joining network",
        )
        .boolean_field("auto_enroll", false, "Auto-enroll containers in mesh")
        .example(json!({
            "network_name": "container-mesh",
            "interface": "nm0",
            "auto_enroll": true
        }))
        .build()
}

// ============================================================================
// Helper Functions
// ============================================================================

fn validate_field(name: &str, value: &Value, schema: &FieldSchema) -> Result<(), String> {
    validate_value_against_type(name, value, &schema.field_type)?;

    // Validate constraints
    for constraint in &schema.constraints {
        match constraint {
            Constraint::Min { value: min } => {
                if let Some(n) = value.as_f64() {
                    if n < *min {
                        return Err(format!("Field '{}' must be >= {}", name, min));
                    }
                }
                if let Some(s) = value.as_str() {
                    if (s.len() as f64) < *min {
                        return Err(format!("Field '{}' length must be >= {}", name, min));
                    }
                }
            }
            Constraint::Max { value: max } => {
                if let Some(n) = value.as_f64() {
                    if n > *max {
                        return Err(format!("Field '{}' must be <= {}", name, max));
                    }
                }
                if let Some(s) = value.as_str() {
                    if (s.len() as f64) > *max {
                        return Err(format!("Field '{}' length must be <= {}", name, max));
                    }
                }
            }
            Constraint::Pattern { regex } => {
                if let Some(s) = value.as_str() {
                    if let Ok(re) = regex::Regex::new(regex) {
                        if !re.is_match(s) {
                            return Err(format!("Field '{}' must match pattern: {}", name, regex));
                        }
                    }
                }
            }
            Constraint::OneOf { values } => {
                if !values.contains(value) {
                    return Err(format!("Field '{}' must be one of: {:?}", name, values));
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_value_against_type(
    name: &str,
    value: &Value,
    field_type: &FieldType,
) -> Result<(), String> {
    match field_type {
        FieldType::String => {
            if !value.is_str() {
                return Err(format!("Field '{}' must be a string", name));
            }
        }
        FieldType::Integer => {
            if !value.is_i64() && !value.is_u64() {
                return Err(format!("Field '{}' must be an integer", name));
            }
        }
        FieldType::Float => {
            if !value.is_f64() && !value.is_i64() {
                return Err(format!("Field '{}' must be a number", name));
            }
        }
        FieldType::Boolean => {
            if !value.is_bool() {
                return Err(format!("Field '{}' must be a boolean", name));
            }
        }
        FieldType::Array(_) => {
            if !value.is_array() {
                return Err(format!("Field '{}' must be an array", name));
            }
            if let Some(items) = value.as_array() {
                if let FieldType::Array(item_type) = field_type {
                    for (index, item) in items.iter().enumerate() {
                        validate_value_against_type(
                            &format!("{}[{}]", name, index),
                            item,
                            item_type,
                        )?;
                    }
                }
            }
        }
        FieldType::Object(fields) => {
            if !value.is_object() {
                return Err(format!("Field '{}' must be an object", name));
            }
            validate_object_fields(name, value, fields)?;
        }
        FieldType::Enum(valid_values) => {
            if let Some(s) = value.as_str() {
                if !valid_values.contains(&s.to_string()) {
                    return Err(format!(
                        "Field '{}' must be one of: {:?}",
                        name, valid_values
                    ));
                }
            } else {
                return Err(format!("Field '{}' must be a string enum value", name));
            }
        }
        FieldType::Any => {}
    }

    Ok(())
}

fn validate_object_fields(
    name: &str,
    value: &Value,
    fields: &HashMap<String, FieldSchema>,
) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err(format!("Field '{}' must be an object", name));
    };

    for (field_name, field_schema) in fields {
        if field_schema.required && obj.get(field_name).is_none() {
            return Err(format!("Missing required field: {}.{}", name, field_name));
        }
    }

    for (field_name, field_value) in obj {
        if let Some(field_schema) = fields.get(field_name) {
            validate_field(
                &format!("{}.{}", name, field_name),
                field_value,
                field_schema,
            )?;
        }
    }

    Ok(())
}

fn default_for_type(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::String => json!(""),
        FieldType::Integer => json!(0),
        FieldType::Float => json!(0.0),
        FieldType::Boolean => json!(false),
        FieldType::Array(_) => json!([]),
        FieldType::Object(_) => json!({}),
        FieldType::Enum(values) => values.first().map(|s| json!(s)).unwrap_or(json!("")),
        FieldType::Any => json!(null),
    }
}

fn field_type_to_json_schema(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::String => json!({"type": "string"}),
        FieldType::Integer => json!({"type": "integer"}),
        FieldType::Float => json!({"type": "number"}),
        FieldType::Boolean => json!({"type": "boolean"}),
        FieldType::Array(item_type) => json!({
            "type": "array",
            "items": field_type_to_json_schema(item_type)
        }),
        FieldType::Object(fields) => {
            let mut properties = simd_json::value::owned::Object::new();
            for (name, schema) in fields {
                properties.insert(name.clone(), field_type_to_json_schema(&schema.field_type));
            }
            json!({
                "type": "object",
                "properties": properties
            })
        }
        FieldType::Enum(values) => json!({
            "type": "string",
            "enum": values
        }),
        FieldType::Any => json!({}),
    }
}

/// Convert field type to JSON Schema 2026 format with full metadata
fn field_type_to_json_schema_2026(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::String => json!({"type": "string"}),
        FieldType::Integer => json!({"type": "integer"}),
        FieldType::Float => json!({"type": "number"}),
        FieldType::Boolean => json!({"type": "boolean"}),
        FieldType::Array(item_type) => json!({
            "type": "array",
            "items": field_type_to_json_schema_2026(item_type)
        }),
        FieldType::Object(fields) => {
            let mut properties = simd_json::value::owned::Object::new();
            let mut required = Vec::new();
            for (name, schema) in fields {
                let mut field_json = field_type_to_json_schema_2026(&schema.field_type);
                if !schema.description.is_empty() {
                    if let Some(obj) = field_json.as_object_mut() {
                        obj.insert("description".to_string(), json!(schema.description));
                    }
                }
                if schema.read_only {
                    if let Some(obj) = field_json.as_object_mut() {
                        obj.insert("readOnly".to_string(), json!(true));
                    }
                }
                properties.insert(name.clone(), field_json);
                if schema.required {
                    required.push(json!(name));
                }
            }
            let mut result = json!({
                "type": "object",
                "properties": properties
            });
            if !required.is_empty() {
                if let Some(obj) = result.as_object_mut() {
                    obj.insert("required".to_string(), json!(required));
                }
            }
            result
        }
        FieldType::Enum(values) => json!({
            "type": "string",
            "enum": values
        }),
        FieldType::Any => json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_registry() {
        let registry = SchemaRegistry::new();
        assert!(registry.get("lxc").is_some());
        assert!(registry.get("incus").is_some());
        assert!(registry.get("incus-wireguard-ingress").is_some());
        assert!(registry.get("incus-xray-reality-client").is_some());
        assert!(registry.get("incus-xray-reality-server").is_some());
        assert!(registry.get("net").is_some());
        assert!(registry.get("openflow").is_some());
        assert!(registry.get("systemd").is_some());
        assert!(registry.get("privacy_routes").is_some());
        assert!(registry.get("privacy_router").is_some());
        assert!(registry.get("netmaker").is_some());
    }

    #[test]
    fn test_schema_registry_aliases() {
        let registry = SchemaRegistry::new();
        assert!(registry.get("incus_wireguard_ingress").is_some());
        assert!(registry.get("incus_xray_reality_client").is_some());
        assert!(registry.get("incus_xray_reality_server").is_some());
    }

    #[test]
    fn test_lxc_validation() {
        let registry = SchemaRegistry::new();
        let schema = registry.get("lxc").unwrap();

        // Valid state
        let valid_state = json!({
            "containers": [
                {
                    "id": "100",
                    "veth": "vi100",
                    "bridge": "ovs-br0",
                    "running": true
                }
            ]
        });
        let result = schema.validate(&valid_state);
        assert!(result.valid, "Errors: {:?}", result.errors);

        // Missing required field
        let invalid_state = json!({});
        let result = schema.validate(&invalid_state);
        assert!(!result.valid);
    }

    #[test]
    fn test_template_generation() {
        let registry = SchemaRegistry::new();
        let schema = registry.get("lxc").unwrap();
        let template = schema.generate_template();
        assert!(template.get("containers").is_some());
    }

    #[test]
    fn test_json_schema_export() {
        let registry = SchemaRegistry::new();
        let schema = registry.get("lxc").unwrap();
        let json_schema = schema.to_json_schema();
        assert_eq!(json_schema["title"], "lxc");
        assert!(json_schema["properties"].is_object());
    }

    #[test]
    fn test_json_schema_2026_dialect() {
        let registry = SchemaRegistry::new();
        let schema = registry.get("lxc").unwrap();
        let json_schema = schema.to_json_schema();

        // Check that 2026 dialect is used
        assert_eq!(json_schema["$schema"], DEFAULT_SCHEMA_DIALECT);
    }

    #[test]
    fn test_json_schema_property_dependencies() {
        // Create a schema with conditional readOnly
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .field(
                "status",
                FieldSchema {
                    field_type: FieldType::String,
                    required: true,
                    description: "Status".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            )
            .field(
                "id",
                FieldSchema {
                    field_type: FieldType::String,
                    required: true,
                    description: "ID".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: Some(ReadOnlyCondition {
                        property: "status".to_string(),
                        value: "locked".to_string(),
                    }),
                },
            )
            .build();

        let json_schema = schema.to_json_schema();

        // Check that propertyDependencies is generated
        assert!(json_schema.get("propertyDependencies").is_some());
        let deps = &json_schema["propertyDependencies"];
        assert!(deps["status"]["locked"]["properties"]["id"]["readOnly"]
            .as_bool()
            .unwrap_or(false));
    }

    #[test]
    fn test_nested_required_fields_for_wireguard_ingress() {
        let registry = SchemaRegistry::new();
        let schema = registry.get("incus-wireguard-ingress").unwrap();

        let invalid_state = json!({
            "name": "incus-wireguard-ingress",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "profiles": ["default"]
            },
            "wireguard": {
                "private_key_env": "WIREGUARD_PRIVATE_KEY",
                "peers": []
            }
        });

        let result = schema.validate(&invalid_state);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("container.image")));
    }

    #[test]
    fn test_nested_required_fields_for_xray_client() {
        let registry = SchemaRegistry::new();
        let schema = registry.get("incus-xray-reality-client").unwrap();

        let invalid_state = json!({
            "name": "incus-xray-reality-client",
            "version": "1.0.0",
            "plugin_type": "network",
            "container": {
                "image": "images:debian/13",
                "profiles": ["default"]
            },
            "xray": {
                "outbounds": []
            }
        });

        let result = schema.validate(&invalid_state);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("xray.inbounds")));
    }

    #[test]
    fn test_contract_schema_sections_for_incus_components() {
        let registry = SchemaRegistry::new();

        for schema_name in [
            "incus-wireguard-ingress",
            "incus-xray-reality-client",
            "incus-xray-reality-server",
        ] {
            let schema = registry.get(schema_name).unwrap();
            let contract = schema.to_contract_json_schema();
            let required = contract["required"].as_array().unwrap();

            assert!(required.iter().any(|value| value == "stub"));
            assert!(required.iter().any(|value| value == "immutable"));
            assert!(required.iter().any(|value| value == "tunable"));
            assert!(contract["properties"]["stub"].is_object());
            assert!(contract["properties"]["immutable"].is_object());
            assert!(contract["properties"]["tunable"].is_object());
        }
    }

    #[test]
    fn test_privacy_router_container_ids_are_integers() {
        let registry = SchemaRegistry::new();
        let schema = registry.get("privacy_router").unwrap();

        let valid_state = json!({
            "bridge_name": "ovsbr0",
            "wireguard": {
                "enabled": true,
                "container_id": 100,
                "socket_port": "priv_wg",
                "listen_port": 51820,
                "resources": {
                    "vcpus": 1,
                    "memory_mb": 512,
                    "disk_gb": 4,
                    "os_template": "images:debian/13",
                    "swap_mb": 0,
                    "unprivileged": true
                }
            },
            "warp": {
                "enabled": true,
                "bridge_interface": "wgcf",
                "wgcf_config": "/etc/wireguard/wgcf.conf"
            },
            "xray": {
                "enabled": true,
                "container_id": 101,
                "socket_port": "priv_xray",
                "socks_port": 1080,
                "vps_address": "vps.example.com",
                "vps_port": 443,
                "resources": {
                    "vcpus": 1,
                    "memory_mb": 512,
                    "disk_gb": 4,
                    "os_template": "images:debian/13",
                    "swap_mb": 0,
                    "unprivileged": true
                }
            },
            "vps": {
                "xray_server": "vps.example.com",
                "xray_port": 443
            },
            "socket_networking": {
                "enabled": true,
                "privacy_sockets": [
                    {
                        "name": "priv_wg",
                        "container_id": 100
                    },
                    {
                        "name": "priv_xray",
                        "container_id": 101
                    }
                ]
            },
            "openflow": {
                "enabled": true,
                "enable_security_flows": true,
                "obfuscation_level": 2,
                "privacy_flows": [],
                "function_routing": []
            },
            "containers": []
        });

        let result = schema.validate(&valid_state);
        assert!(result.valid, "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_json_schema_immutable_paths() {
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .string_field("id", true, "ID field")
            .string_field("name", true, "Name field")
            .immutable_path("/id")
            .build();

        let json_schema = schema.to_json_schema();
        let properties = json_schema["properties"].as_object().unwrap();

        // Check that id has readOnly
        assert!(properties["id"]["readOnly"].as_bool().unwrap_or(false));
        // name should not be readOnly
        assert!(!properties
            .get("name")
            .and_then(|value| value.get("readOnly"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false));
    }

    #[test]
    fn test_json_schema_fully_immutable() {
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .string_field("id", true, "ID field")
            .string_field("name", true, "Name field")
            .fully_immutable()
            .build();

        let json_schema = schema.to_json_schema();

        // All fields should be readOnly
        assert!(json_schema["properties"]["id"]["readOnly"]
            .as_bool()
            .unwrap_or(false));
        assert!(json_schema["properties"]["name"]["readOnly"]
            .as_bool()
            .unwrap_or(false));
    }

    #[test]
    fn test_schema_custom_dialect() {
        let schema = PluginSchema::builder("test")
            .version("1.0.0")
            .description("Test schema")
            .dialect(dialects::DRAFT_07)
            .string_field("name", true, "Name")
            .build();

        let json_schema = schema.to_json_schema();
        assert_eq!(json_schema["$schema"], dialects::DRAFT_07);
    }
}
