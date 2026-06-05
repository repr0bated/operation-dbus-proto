//! Schema Loader - Runtime Schema Loading and Validation
//!
//! Provides functionality to load plugin schemas from JSON files at runtime,
//! validate them against the meta-schema, and cache them for performance.
//!
//! ## Architecture
//!
//! ```text
//! schemas/plugin/*.json
//!         ↓
//! SchemaLoader::load()
//!         ↓
//! JSON parsing + validation against meta-schema
//!         ↓
//! PluginSchema (runtime model)
//!         ↓
//! Cached for subsequent lookups
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use op_plugins::schema_loader::SchemaLoader;
//!
//! let loader = SchemaLoader::new("schemas/plugin");
//! let schema = loader.load_schema("incus").await?;
//! ```

use anyhow::{Context, Result};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema, ReadOnlyCondition};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info, warn};

/// Schema loader for runtime JSON schema loading
pub struct SchemaLoader {
    /// Base directory for schema files
    schema_dir: PathBuf,
    /// In-memory cache of loaded schemas
    cache: tokio::sync::RwLock<HashMap<String, PluginSchema>>,
    /// Whether to enforce strict validation
    strict: bool,
}

impl SchemaLoader {
    /// Create a new schema loader
    pub fn new(schema_dir: impl Into<PathBuf>) -> Self {
        Self {
            schema_dir: schema_dir.into(),
            cache: tokio::sync::RwLock::new(HashMap::new()),
            strict: true,
        }
    }

    /// Create a new schema loader with non-strict mode (for migration)
    pub fn new_permissive(schema_dir: impl Into<PathBuf>) -> Self {
        Self {
            schema_dir: schema_dir.into(),
            cache: tokio::sync::RwLock::new(HashMap::new()),
            strict: false,
        }
    }

    /// Load a schema by plugin name
    ///
    /// First checks the cache, then loads from disk if not cached.
    /// Returns None if schema file doesn't exist.
    pub async fn load_schema(&self, plugin_name: &str) -> Result<Option<PluginSchema>> {
        let sanitized = sanitize_plugin_name(plugin_name);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(schema) = cache.get(&sanitized) {
                debug!(plugin = %sanitized, "Schema loaded from cache");
                return Ok(Some(schema.clone()));
            }
        }

        // Construct file path
        let schema_path = self.schema_dir.join(format!("{}.json", sanitized));

        // Check if file exists
        if !schema_path.exists() {
            if self.strict {
                warn!(
                    plugin = %sanitized,
                    path = %schema_path.display(),
                    "Schema file not found in strict mode"
                );
            } else {
                debug!(
                    plugin = %sanitized,
                    path = %schema_path.display(),
                    "Schema file not found (permissive mode)"
                );
            }
            return Ok(None);
        }

        // Load and parse schema
        let schema = self.load_from_file(&schema_path).await?;

        // Validate against meta-schema
        if self.strict {
            self.validate_schema(&schema).await?;
        }

        // Cache the schema
        {
            let mut cache = self.cache.write().await;
            cache.insert(sanitized.clone(), schema.clone());
        }

        info!(plugin = %sanitized, "Schema loaded and cached");
        Ok(Some(schema))
    }

    /// Load schema from a specific file path
    async fn load_from_file(&self, path: &Path) -> Result<PluginSchema> {
        let content = fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read schema file: {}", path.display()))?;

        let mut content_bytes = content.into_bytes();
        let json_value: Value = simd_json::from_slice(&mut content_bytes)
            .with_context(|| format!("Failed to parse JSON in schema file: {}", path.display()))?;

        let schema = parse_plugin_schema(&json_value)
            .with_context(|| format!("Failed to parse schema from: {}", path.display()))?;

        Ok(schema)
    }

    /// Validate a schema against the meta-schema
    async fn validate_schema(&self, schema: &PluginSchema) -> Result<()> {
        // Basic validation - ensure required fields are present
        if schema.name.is_empty() {
            return Err(anyhow::anyhow!("Schema missing required field: name"));
        }

        if schema.version.is_empty() {
            return Err(anyhow::anyhow!("Schema missing required field: version"));
        }

        if schema.description.is_empty() {
            warn!(schema_name = %schema.name, "Schema missing description");
        }

        // Validate field types
        for (field_name, field) in &schema.fields {
            validate_field_schema(field_name, field)?;
        }

        debug!(schema_name = %schema.name, "Schema validation passed");
        Ok(())
    }

    /// Check if a schema exists (without loading)
    pub async fn schema_exists(&self, plugin_name: &str) -> bool {
        let sanitized = sanitize_plugin_name(plugin_name);
        let schema_path = self.schema_dir.join(format!("{}.json", sanitized));
        schema_path.exists()
    }

    /// List all available schemas in the directory
    pub async fn list_available_schemas(&self) -> Result<Vec<String>> {
        let mut schemas = Vec::new();

        let mut entries = fs::read_dir(&self.schema_dir).await.with_context(|| {
            format!(
                "Failed to read schema directory: {}",
                self.schema_dir.display()
            )
        })?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    schemas.push(stem.to_string());
                }
            }
        }

        Ok(schemas)
    }

    /// Clear the in-memory cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("Schema cache cleared");
    }

    /// Reload a schema from disk (bypass cache)
    pub async fn reload_schema(&self, plugin_name: &str) -> Result<Option<PluginSchema>> {
        let sanitized = sanitize_plugin_name(plugin_name);

        // Remove from cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&sanitized);
        }

        // Load fresh
        self.load_schema(&sanitized).await
    }

    /// Get the schema directory path
    pub fn schema_dir(&self) -> &Path {
        &self.schema_dir
    }

    /// Require a schema to exist (strict mode)
    ///
    /// Returns Err if schema doesn't exist in strict mode
    pub async fn require_schema(&self, plugin_name: &str) -> Result<PluginSchema> {
        match self.load_schema(plugin_name).await? {
            Some(schema) => Ok(schema),
            None => {
                if self.strict {
                    Err(anyhow::anyhow!(
                        "Required schema not found for plugin: {}. \
                         Schema file should be at: {}/{}.json",
                        plugin_name,
                        self.schema_dir.display(),
                        sanitize_plugin_name(plugin_name)
                    ))
                } else {
                    // In permissive mode, return a default schema
                    warn!(
                        plugin = %plugin_name,
                        "Using default schema in permissive mode"
                    );
                    Ok(create_default_schema(plugin_name))
                }
            }
        }
    }
}

/// Parse a PluginSchema from JSON value
fn parse_plugin_schema(value: &Value) -> Result<PluginSchema> {
    use simd_json::prelude::*;

    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Schema root must be an object"))?;

    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Schema missing 'name' field"))?
        .to_string();

    let version = obj
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("1.0.0")
        .to_string();

    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let category = obj
        .get("plugin_type")
        .and_then(|v| v.as_str())
        .or_else(|| obj.get("category").and_then(|v| v.as_str()))
        .unwrap_or("custom")
        .to_string();

    let mut builder = PluginSchema::builder(&name)
        .version(&version)
        .category(&category)
        .description(&description);

    // Parse dependencies
    if let Some(deps) = obj.get("dependencies").and_then(|v| v.as_array()) {
        for dep in deps {
            if let Some(dep_name) = dep.as_str() {
                builder = builder.dependency(dep_name);
            } else if let Some(dep_obj) = dep.as_object() {
                if let Some(dep_name) = dep_obj.get("name").and_then(|v| v.as_str()) {
                    builder = builder.dependency(dep_name);
                }
            }
        }
    }

    // Parse fields
    if let Some(fields) = obj.get("fields").and_then(|v| v.as_object()) {
        for (field_name, field_value) in fields.iter() {
            let field = parse_field_schema(field_value)?;
            builder = builder.field(&field_name.to_string(), field);
        }
    }

    // Parse schema object as fields (alternative format)
    if let Some(schema_obj) = obj.get("schema").and_then(|v| v.as_object()) {
        for (field_name, field_value) in schema_obj.iter() {
            let field = parse_field_schema(field_value)?;
            builder = builder.field(&field_name.to_string(), field);
        }
    }

    // Parse properties as fields (JSON Schema format)
    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        let required: Vec<String> = obj
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        for (prop_name, prop_value) in props.iter() {
            let mut field = parse_field_schema(prop_value)?;
            field.required = required.contains(prop_name);
            builder = builder.field(&prop_name.to_string(), field);
        }
    }

    // Set example if present
    if let Some(example) = obj.get("example") {
        builder = builder.example(example.clone());
    }

    Ok(builder.build())
}

/// Parse a FieldSchema from JSON value
fn parse_field_schema(value: &Value) -> Result<FieldSchema> {
    use simd_json::prelude::*;

    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Field schema must be an object"))?;

    let field_type = obj
        .get("type")
        .or_else(|| obj.get("field_type"))
        .and_then(|v| v.as_str())
        .map(parse_field_type)
        .unwrap_or(FieldType::Any);

    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let required = obj
        .get("required")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let default = obj.get("default").cloned();

    let example = obj.get("example").cloned();

    let read_only = obj
        .get("readOnly")
        .or_else(|| obj.get("read_only"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Parse constraints
    let constraints = parse_constraints(value)?;

    Ok(FieldSchema {
        field_type,
        required,
        description,
        default,
        example,
        constraints,
        read_only,
        read_only_when: None,
    })
}

/// Parse field type from string
fn parse_field_type(type_str: &str) -> FieldType {
    match type_str {
        "string" => FieldType::String,
        "integer" | "int" => FieldType::Integer,
        "number" | "float" => FieldType::Float,
        "boolean" | "bool" => FieldType::Boolean,
        "array" => FieldType::Array(Box::new(FieldType::Any)),
        "object" => FieldType::Object(HashMap::new()),
        "any" => FieldType::Any,
        _ => FieldType::Any,
    }
}

/// Parse constraints from field object
fn parse_constraints(value: &Value) -> Result<Vec<Constraint>> {
    use simd_json::prelude::ValueAsScalar;
    use simd_json::prelude::*;

    let mut constraints = Vec::new();

    // Minimum constraint (numeric or string/array length)
    if let Some(min) = value
        .get("minimum")
        .or_else(|| value.get("minLength"))
        .and_then(|v| v.as_f64())
    {
        constraints.push(Constraint::Min { value: min });
    }

    // Maximum constraint (numeric or string/array length)
    if let Some(max) = value
        .get("maximum")
        .or_else(|| value.get("maxLength"))
        .and_then(|v| v.as_f64())
    {
        constraints.push(Constraint::Max { value: max });
    }

    // Pattern constraint (regex)
    if let Some(pattern) = value.get("pattern").and_then(|v| v.as_str()) {
        constraints.push(Constraint::Pattern {
            regex: pattern.to_string(),
        });
    }

    // OneOf constraint from enum values
    if let Some(enum_values) = value.get("enum").and_then(|v| v.as_array()) {
        let values: Vec<Value> = enum_values.iter().cloned().collect();
        if !values.is_empty() {
            constraints.push(Constraint::OneOf { values });
        }
    }

    Ok(constraints)
}

/// Validate a field schema
fn validate_field_schema(field_name: &str, field: &FieldSchema) -> Result<()> {
    // Check for empty descriptions in required fields
    if field.required && field.description.is_empty() {
        warn!(
            field = %field_name,
            "Required field missing description"
        );
    }

    // Validate constraints make sense for the type
    match &field.field_type {
        FieldType::Integer | FieldType::Float => {
            // Numeric constraints are valid
        }
        FieldType::String => {
            // String constraints are valid
        }
        _ => {
            // Other types shouldn't have numeric constraints
            for constraint in &field.constraints {
                match constraint {
                    Constraint::Min { .. } | Constraint::Max { .. } => {
                        warn!(
                            field = %field_name,
                            "Numeric constraint on non-numeric field type"
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

/// Sanitize plugin name for file system
fn sanitize_plugin_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "")
        .replace('-', "_")
}

/// Create a default schema for a plugin (fallback)
fn create_default_schema(plugin_name: &str) -> PluginSchema {
    PluginSchema::builder(plugin_name)
        .version("1.0.0")
        .category("custom")
        .description(&format!("Default schema for {}", plugin_name))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_plugin_name() {
        assert_eq!(sanitize_plugin_name("My-Plugin"), "my_plugin");
        assert_eq!(sanitize_plugin_name("  net  "), "net");
        assert_eq!(sanitize_plugin_name("plugin123"), "plugin123");
    }

    #[test]
    fn test_parse_field_type() {
        assert!(matches!(parse_field_type("string"), FieldType::String));
        assert!(matches!(parse_field_type("integer"), FieldType::Integer));
        assert!(matches!(parse_field_type("boolean"), FieldType::Boolean));
        assert!(matches!(parse_field_type("unknown"), FieldType::Any));
    }
}
