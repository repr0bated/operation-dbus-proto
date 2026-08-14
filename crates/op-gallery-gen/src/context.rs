//! Context assembler for gallery generation.
//!
//! Gathers baseline context for the inference loop:
//! - Plugin schemas from sealed blobs
//! - Static documentation (access-instructions, catalog, grammar)
//! - Operator guidance from Antigravity chat

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Assembled context for a single generation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationContext {
    /// Baseline: all plugin schemas from blobs
    pub schemas: Vec<SchemaPayload>,

    /// Static docs: access-instructions.md content
    pub access_instructions: String,

    /// Static docs: json-render-catalog.md content  
    pub catalog_docs: String,

    /// Static docs: spec-grammar.md content
    pub grammar_docs: String,

    /// Universal prompt (vague, goal-oriented)
    pub universal_prompt: String,

    /// Operator guidance from Antigravity chat
    pub operator_guidance: Option<String>,

    /// MCP toggle enabled
    pub mcp_enabled: bool,

    /// Qdrant toggle enabled
    pub qdrant_enabled: bool,

    /// Catalog hash at context assembly time (for freshness checks).
    pub catalog_hash: String,
}

/// Schema payload from a plugin blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaPayload {
    /// Plugin name
    pub name: String,

    /// Plugin version
    pub version: String,

    /// Human-readable description
    pub description: Option<String>,

    /// OSCAL category (software, network, system, security, compliance, observability)
    pub category: Option<String>,

    /// State fields with types and constraints
    pub fields: HashMap<String, FieldSchema>,

    /// Available methods
    pub methods: HashMap<String, MethodSchema>,

    /// OSCAL subids for traceability
    pub subids: HashMap<String, String>,

    /// Authoritative UI routes declared by this plugin's `ui_surfaces` field.
    #[serde(default)]
    pub ui_surfaces: UiSurfaceProjection,

    /// Raw schema JSON (for tool responses)
    pub raw_json: serde_json::Value,
}

/// A plugin-owned UI projection. Empty routes mean gallery generation should
/// retain its schema-derived fallback instead of inventing authoritative paths.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UiSurfaceProjection {
    pub subids: Vec<String>,
    pub routes: Vec<UiSurfaceRoute>,
    pub value_source: Option<String>,
}

impl UiSurfaceProjection {
    pub fn is_authoritative(&self) -> bool {
        !self.subids.is_empty() && !self.routes.is_empty()
    }
}

/// Normalized route plus the original plugin-owned route object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiSurfaceRoute {
    pub path: String,
    pub name: Option<String>,
    pub schema: Option<String>,
    pub raw: serde_json::Value,
}

/// Field schema from a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    /// Field type (string, number, boolean, object, array)
    pub field_type: FieldType,

    /// Human-readable description
    pub description: Option<String>,

    /// Default value if not specified
    pub default: Option<serde_json::Value>,

    /// Example value supplied by the plugin schema.
    pub example: Option<serde_json::Value>,

    /// Whether field is required
    pub required: bool,

    /// Whether field is read-only
    pub read_only: bool,

    /// Validation constraints
    pub constraints: Option<Constraints>,

    /// OSCAL subid
    pub subid: Option<String>,
}

/// Field type variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Number,
    Integer,
    Boolean,
    Array {
        items: Box<FieldType>,
    },
    Object {
        properties: HashMap<String, FieldSchema>,
    },
    Enum {
        values: Vec<String>,
    },
    OneOf {
        variants: Vec<FieldType>,
    },
}

/// Validation constraints for a field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub pattern: Option<String>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

/// Method schema from a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodSchema {
    /// Human-readable description
    pub description: Option<String>,

    /// Input argument schema
    pub args: Option<serde_json::Value>,

    /// Return value schema
    pub returns: Option<serde_json::Value>,

    /// Side effect classification
    pub side_effect: SideEffect,

    /// Whether method is idempotent
    pub idempotent: bool,

    /// Required capability to invoke
    pub required_capability: Option<String>,

    /// OSCAL subid
    pub subid: Option<String>,
}

/// Method side effect classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    Read,
    Mutation,
}

impl GenerationContext {
    /// Create a new generation context with baseline data.
    pub fn new(schemas: Vec<SchemaPayload>, catalog_hash: String) -> Self {
        Self {
            schemas,
            access_instructions: include_str!("../../../docs/gallery-gen/access-instructions.md")
                .to_string(),
            catalog_docs: include_str!("../../../docs/gallery-gen/json-render-catalog.md")
                .to_string(),
            grammar_docs: include_str!("../../../docs/gallery-gen/spec-grammar.md").to_string(),
            universal_prompt:
                "Make this dataset as accessible to as many people, industries, causes as possible."
                    .to_string(),
            operator_guidance: None,
            mcp_enabled: false,
            qdrant_enabled: false,
            catalog_hash,
        }
    }

    /// Build the system message for the inference model.
    ///
    /// Intentionally does **not** frame this as "building a UI" or a gallery.
    /// The model receives: (1) how to read the dataset via tools / access docs,
    /// (2) the json-render vocabulary + grammar as the only output format.
    /// The goal lives in the user message (`universal_prompt`).
    pub fn build_system_message(&self) -> String {
        let mut parts = vec![
            "# Dataset".to_string(),
            "".to_string(),
            "You are given a dataset. Discover it only through the provided tools \
             (`list_plugins`, `get_plugin_schema`, `search_fields`, and any enabled \
             discovery tools). Do not invent fields, methods, or values."
                .to_string(),
            "".to_string(),
            "Produce one JSON document that expresses the dataset using the vocabulary \
             and grammar below. Stay inside that vocabulary; do not invent component types."
                .to_string(),
            "".to_string(),
            "## Dataset (from sealed plugin schemas)".to_string(),
            "Use tools to refine or search; the payloads below are already loaded.".to_string(),
            "".to_string(),
        ];

        for schema in &self.schemas {
            parts.push(format!("### {}", schema.name));
            parts.push(
                serde_json::to_string_pretty(&schema.raw_json).unwrap_or_else(|_| "{}".to_string()),
            );
            parts.push("".to_string());
        }

        parts.push("## Vocabulary".to_string());
        parts.push(self.catalog_docs.clone());
        parts.push("".to_string());
        parts.push("## Grammar".to_string());
        parts.push(self.grammar_docs.clone());
        parts.push("".to_string());
        parts.push("## How to read the dataset".to_string());
        parts.push(self.access_instructions.clone());

        parts.join("\n")
    }

    /// Build the user message with universal prompt and operator guidance.
    pub fn build_user_message(&self) -> String {
        let mut msg = self.universal_prompt.clone();

        if let Some(guidance) = &self.operator_guidance {
            msg.push_str("\n\n## Operator Guidance\n");
            msg.push_str(guidance);
        }

        msg
    }
}

impl SchemaPayload {
    /// Parse a schema payload from raw plugin schema JSON.
    pub fn from_raw(name: String, raw: serde_json::Value) -> Result<Self> {
        let version = raw
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0")
            .to_string();

        let description = raw
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());

        let category = raw
            .get("x-oscal-category")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        // Parse fields (simplified for now - full parsing would be recursive)
        let fields = parse_fields(raw.get("fields"))?;

        // Parse methods
        let methods = parse_methods(raw.get("methods"))?;

        // Extract subids
        let subids = raw
            .get("subids")
            .and_then(|s| s.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let ui_surfaces = extract_ui_surfaces(&subids, fields.get("ui_surfaces"));

        Ok(Self {
            name,
            version,
            description,
            category,
            fields,
            methods,
            subids,
            ui_surfaces,
            raw_json: raw,
        })
    }
}

fn extract_ui_surfaces(
    subids: &HashMap<String, String>,
    field: Option<&FieldSchema>,
) -> UiSurfaceProjection {
    let mut projection = UiSurfaceProjection {
        subids: subids
            .values()
            .filter(|subid| subid.ends_with(".ui-surfaces@v1"))
            .cloned()
            .collect(),
        ..Default::default()
    };
    projection.subids.sort();
    projection.subids.dedup();

    let (value, source) = match field {
        Some(field) if field.default.is_some() => (field.default.as_ref(), Some("default")),
        Some(field) if field.example.is_some() => (field.example.as_ref(), Some("example")),
        _ => (None, None),
    };
    projection.value_source = source.map(str::to_string);
    projection.routes = value.map(extract_ui_surface_routes).unwrap_or_default();
    projection
}

fn extract_ui_surface_routes(value: &serde_json::Value) -> Vec<UiSurfaceRoute> {
    let candidates = value
        .as_array()
        .cloned()
        .or_else(|| value.get("routes").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();
    candidates
        .into_iter()
        .filter_map(|raw| {
            let path = raw
                .get("path")
                .or_else(|| raw.get("route"))
                .or_else(|| raw.get("href"))
                .and_then(|v| v.as_str())?
                .to_string();
            Some(UiSurfaceRoute {
                path,
                name: raw.get("name").and_then(|v| v.as_str()).map(str::to_string),
                schema: raw
                    .get("schema")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                raw,
            })
        })
        .collect()
}

fn parse_fields(fields_val: Option<&serde_json::Value>) -> Result<HashMap<String, FieldSchema>> {
    let mut fields = HashMap::new();

    if let Some(obj) = fields_val.and_then(|v| v.as_object()) {
        for (name, field_val) in obj {
            if let Ok(field) = parse_field(field_val) {
                fields.insert(name.clone(), field);
            }
        }
    }

    Ok(fields)
}

fn parse_field(val: &serde_json::Value) -> Result<FieldSchema> {
    let field_type = parse_field_type(val.get("field_type"))?;

    let description = val
        .get("description")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    let default = val.get("default").cloned();
    let example = val.get("example").cloned();
    let required = val
        .get("required")
        .and_then(|r| r.as_bool())
        .unwrap_or(false);
    let read_only = val
        .get("read_only")
        .and_then(|r| r.as_bool())
        .unwrap_or(false);

    let constraints = val.get("constraints").map(|c| parse_constraints(c));

    let subid = val
        .get("subid")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    Ok(FieldSchema {
        field_type,
        description,
        default,
        example,
        required,
        read_only,
        constraints,
        subid,
    })
}

fn parse_field_type(val: Option<&serde_json::Value>) -> Result<FieldType> {
    match val {
        Some(serde_json::Value::String(s)) => {
            match s.as_str() {
                "string" => Ok(FieldType::String),
                "number" => Ok(FieldType::Number),
                "integer" => Ok(FieldType::Integer),
                "boolean" => Ok(FieldType::Boolean),
                _ => Ok(FieldType::String), // Default fallback
            }
        }
        Some(serde_json::Value::Object(obj)) => {
            if let Some(items) = obj.get("array") {
                let item_type = parse_field_type(Some(items))?;
                Ok(FieldType::Array {
                    items: Box::new(item_type),
                })
            } else if let Some(props) = obj.get("object") {
                let properties = parse_fields(Some(props))?;
                Ok(FieldType::Object { properties })
            } else if let Some(vals) = obj.get("enumValues") {
                let values = vals
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(FieldType::Enum { values })
            } else {
                Ok(FieldType::String)
            }
        }
        _ => Ok(FieldType::String),
    }
}

fn parse_constraints(val: &serde_json::Value) -> Constraints {
    Constraints {
        min: val.get("min").and_then(|v| v.as_f64()),
        max: val.get("max").and_then(|v| v.as_f64()),
        pattern: val
            .get("pattern")
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        min_length: val
            .get("minLength")
            .and_then(|v| v.as_u64().map(|n| n as usize)),
        max_length: val
            .get("maxLength")
            .and_then(|v| v.as_u64().map(|n| n as usize)),
    }
}

fn parse_methods(methods_val: Option<&serde_json::Value>) -> Result<HashMap<String, MethodSchema>> {
    let mut methods = HashMap::new();

    if let Some(obj) = methods_val.and_then(|v| v.as_object()) {
        for (name, method_val) in obj {
            if let Ok(method) = parse_method(method_val) {
                methods.insert(name.clone(), method);
            }
        }
    }

    Ok(methods)
}

fn parse_method(val: &serde_json::Value) -> Result<MethodSchema> {
    let description = val
        .get("description")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    let args = val.get("args").cloned();
    let returns = val.get("returns").cloned();

    let side_effect = val
        .get("side_effect")
        .and_then(|s| s.as_str())
        .map(|s| match s {
            "mutation" => SideEffect::Mutation,
            _ => SideEffect::Read,
        })
        .unwrap_or(SideEffect::Read);

    let idempotent = val
        .get("idempotent")
        .and_then(|i| i.as_bool())
        .unwrap_or(false);

    let required_capability = val
        .get("required_capability")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    let subid = val
        .get("subid")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    Ok(MethodSchema {
        description,
        args,
        returns,
        side_effect,
        idempotent,
        required_capability,
        subid,
    })
}

// ── Blob Catalog Loading ─────────────────────────────────────────────────────

/// Result of loading the sealed blob catalog from SHM.
#[derive(Debug, Clone)]
pub struct CatalogLoadResult {
    /// Parsed schemas for all active plugins.
    pub schemas: Vec<SchemaPayload>,
    /// The catalog_hash at load time (blake3 leaf-fold of sorted plugin hashes).
    pub catalog_hash: String,
    /// The manifest generation counter (monotonic).
    pub generation: u64,
    /// Plugin IDs that failed to load (non-fatal — logged and skipped).
    pub failed_plugins: Vec<String>,
}

/// Load all plugin schemas from the default SHM blob catalog.
///
/// Reads `.manifest.json` to get plugin IDs and catalog_hash, then loads each
/// plugin's sealed blob via `op_blob::catalog::read_plugin_schema_shm`.
///
/// This is the entry point for REQ-2.1 (baseline data tier) and REQ-2.2
/// (fresh assembly from live catalog — no stale caches).
pub fn load_from_shm() -> Result<CatalogLoadResult> {
    load_from_dir(Path::new(op_blob::catalog::DEFAULT_SHM_DIR))
}

/// Infer a conventional base owner for compatibility-only companions.
fn compatibility_base_id(plugin_id: &str) -> Option<&str> {
    ["_chat", "_compat", "_bridge", "-chat", "-compat", "-bridge"]
        .iter()
        .find_map(|suffix| plugin_id.strip_suffix(suffix))
        .filter(|base| !base.is_empty())
}

/// Resolve a compatibility companion only when its base owner is available.
pub(crate) fn resolve_ui_plugin_id<'a>(
    plugin_id: &'a str,
    schemas: &'a [SchemaPayload],
) -> &'a str {
    if schemas
        .iter()
        .any(|schema| schema.name == plugin_id && schema.ui_surfaces.is_authoritative())
    {
        return plugin_id;
    }
    compatibility_base_id(plugin_id)
        .filter(|base| {
            schemas
                .iter()
                .any(|schema| schema.name == *base && schema.ui_surfaces.is_authoritative())
        })
        .unwrap_or(plugin_id)
}

/// Load all plugin schemas from a specific blob catalog directory.
///
/// Useful for testing against a non-default directory.
pub fn load_from_dir(dir: &Path) -> Result<CatalogLoadResult> {
    // 1. Read manifest to get plugin IDs and catalog_hash
    let manifest_path = dir.join(op_blob::catalog::MANIFEST_FILENAME);
    let manifest_bytes = std::fs::read(&manifest_path).with_context(|| {
        format!(
            "Failed to read catalog manifest at {}",
            manifest_path.display()
        )
    })?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).context("Failed to parse catalog manifest JSON")?;

    let catalog_hash = manifest
        .get("catalog_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let generation = manifest
        .get("generation")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let plugin_ids: Vec<String> = manifest
        .get("plugins")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    tracing::info!(
        "Loading blob catalog: {} plugins, catalog_hash={}, generation={}",
        plugin_ids.len(),
        &catalog_hash[..catalog_hash.len().min(12)],
        generation
    );

    // 2. Load each plugin schema from its sealed blob
    let mut schemas = Vec::with_capacity(plugin_ids.len());
    let mut failed_plugins = Vec::new();

    for plugin_id in &plugin_ids {
        match op_blob::catalog::read_plugin_state_store_schema(dir, plugin_id) {
            Some(plugin_schema) => match convert_plugin_schema(plugin_id, &plugin_schema) {
                Ok(payload) => schemas.push(payload),
                Err(e) => {
                    tracing::warn!("Failed to convert schema for plugin '{}': {}", plugin_id, e);
                    failed_plugins.push(plugin_id.clone());
                }
            },
            None => {
                tracing::warn!("Failed to read blob for plugin '{}'", plugin_id);
                failed_plugins.push(plugin_id.clone());
            }
        }
    }

    let authoritative_owners = schemas
        .iter()
        .filter(|schema| schema.ui_surfaces.is_authoritative())
        .map(|schema| schema.name.clone())
        .collect::<std::collections::HashSet<_>>();
    schemas.retain(|schema| {
        schema.ui_surfaces.is_authoritative()
            || compatibility_base_id(&schema.name)
                .is_none_or(|base| !authoritative_owners.contains(base))
    });

    tracing::info!(
        "Loaded {} plugin schemas ({} failed)",
        schemas.len(),
        failed_plugins.len()
    );

    Ok(CatalogLoadResult {
        schemas,
        catalog_hash,
        generation,
        failed_plugins,
    })
}

/// Convert an `op_state_store::PluginSchema` into the gallery-gen `SchemaPayload`.
///
/// This bridges the typed state-store schema into the serializable format
/// used in the inference context.
fn convert_plugin_schema(
    plugin_id: &str,
    schema: &op_state_store::PluginSchema,
) -> Result<SchemaPayload> {
    // Convert fields
    let fields: HashMap<String, FieldSchema> = schema
        .fields
        .iter()
        .map(|(name, f)| (name.clone(), convert_field_schema(f)))
        .collect();

    // Convert methods
    let methods: HashMap<String, MethodSchema> = schema
        .methods
        .iter()
        .map(|(name, m)| (name.clone(), convert_method_decl(m)))
        .collect();

    // Extract subids
    let subids = schema.subids.clone();

    let ui_surfaces = extract_ui_surfaces(&subids, fields.get("ui_surfaces"));

    // Build raw JSON for tool responses (the full schema as serde_json::Value)
    let raw_json = serde_json::to_value(schema).unwrap_or_else(
        |_| serde_json::json!({ "name": plugin_id, "error": "serialization failed" }),
    );

    Ok(SchemaPayload {
        name: schema.name.clone(),
        version: schema.version.clone(),
        description: if schema.description.is_empty() {
            None
        } else {
            Some(schema.description.clone())
        },
        category: Some(schema.category.clone()),
        fields,
        methods,
        subids,
        ui_surfaces,
        raw_json,
    })
}

/// Convert a state-store `FieldSchema` into the gallery-gen `FieldSchema`.
fn convert_field_schema(f: &op_state_store::FieldSchema) -> FieldSchema {
    let constraints = if f.constraints.is_empty() {
        None
    } else {
        Some(convert_constraints(&f.constraints))
    };

    FieldSchema {
        field_type: convert_field_type(&f.field_type),
        description: if f.description.is_empty() {
            None
        } else {
            Some(f.description.clone())
        },
        default: f.default.as_ref().map(|v| owned_value_to_serde(v)),
        example: f.example.as_ref().map(|v| owned_value_to_serde(v)),
        required: f.required,
        read_only: f.read_only,
        constraints,
        subid: None, // subids are at the schema level, not per-field here
    }
}

/// Convert a state-store `FieldType` into the gallery-gen `FieldType`.
fn convert_field_type(ft: &op_state_store::FieldType) -> FieldType {
    match ft {
        op_state_store::FieldType::String => FieldType::String,
        op_state_store::FieldType::Integer => FieldType::Integer,
        op_state_store::FieldType::Float => FieldType::Number,
        op_state_store::FieldType::Boolean => FieldType::Boolean,
        op_state_store::FieldType::Array(inner) => FieldType::Array {
            items: Box::new(convert_field_type(inner)),
        },
        op_state_store::FieldType::Object(props) => FieldType::Object {
            properties: props
                .iter()
                .map(|(k, v)| (k.clone(), convert_field_schema(v)))
                .collect(),
        },
        op_state_store::FieldType::Enum(values) => FieldType::Enum {
            values: values.clone(),
        },
        op_state_store::FieldType::OneOf(variants) => FieldType::OneOf {
            variants: variants.iter().map(|v| convert_field_type(v)).collect(),
        },
        op_state_store::FieldType::Any => FieldType::String, // fallback
    }
}

/// Convert state-store constraints into gallery-gen constraints.
fn convert_constraints(constraints: &[op_state_store::Constraint]) -> Constraints {
    let mut result = Constraints {
        min: None,
        max: None,
        pattern: None,
        min_length: None,
        max_length: None,
    };

    for c in constraints {
        match c {
            op_state_store::Constraint::Min { value } => result.min = Some(*value),
            op_state_store::Constraint::Max { value } => result.max = Some(*value),
            op_state_store::Constraint::Pattern { regex } => result.pattern = Some(regex.clone()),
            _ => {} // OneOf, RequiresField, Custom — not relevant for context docs
        }
    }

    result
}

/// Convert a state-store `MethodDecl` into the gallery-gen `MethodSchema`.
fn convert_method_decl(m: &op_state_store::MethodDecl) -> MethodSchema {
    MethodSchema {
        description: Some(m.name.clone()), // Use method name as fallback description
        args: Some(owned_value_to_serde(&m.args)),
        returns: m.returns.as_ref().map(|v| owned_value_to_serde(v)),
        side_effect: match m.side_effect {
            op_state_store::SideEffect::Read => SideEffect::Read,
            op_state_store::SideEffect::Mutation => SideEffect::Mutation,
        },
        idempotent: m.idempotent,
        required_capability: m.required_capability.clone(),
        subid: Some(m.subid.clone()),
    }
}

/// Convert `simd_json::OwnedValue` to `serde_json::Value`.
///
/// The state-store uses `simd_json::OwnedValue` internally, but our inference
/// context needs standard `serde_json::Value` for serialization to the model.
fn owned_value_to_serde(v: &simd_json::OwnedValue) -> serde_json::Value {
    // simd_json OwnedValue serializes to JSON the same as serde_json Value,
    // so round-trip through bytes is the simplest correct conversion.
    let bytes = simd_json::to_vec(v).unwrap_or_else(|_| b"null".to_vec());
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

/// Read only the catalog_hash from the manifest without loading any blobs.
/// Used for freshness checks during a run (NFR-4).
pub fn read_catalog_hash_from_shm() -> Option<String> {
    op_blob::catalog::ActiveReflectionCatalog::read_catalog_hash(Path::new(
        op_blob::catalog::DEFAULT_SHM_DIR,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui_field(
        default: Option<serde_json::Value>,
        example: Option<serde_json::Value>,
    ) -> FieldSchema {
        FieldSchema {
            field_type: FieldType::Array {
                items: Box::new(FieldType::Object {
                    properties: HashMap::new(),
                }),
            },
            description: None,
            default,
            example,
            required: false,
            read_only: true,
            constraints: None,
            subid: None,
        }
    }

    fn payload(name: &str, ui_surfaces: UiSurfaceProjection) -> SchemaPayload {
        SchemaPayload {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: None,
            category: None,
            fields: HashMap::new(),
            methods: HashMap::new(),
            subids: HashMap::new(),
            ui_surfaces,
            raw_json: serde_json::json!({"name": name}),
        }
    }

    #[test]
    fn extracts_authoritative_routes_from_default_and_matching_subid() {
        let subids = HashMap::from([
            (
                "ui_surfaces".into(),
                "exp.service.factory.ui-surfaces@v1".into(),
            ),
            ("other".into(), "obs.service.factory.status@v1".into()),
        ]);
        let field = ui_field(
            Some(serde_json::json!([
                {"path": "/factory", "name": "Factory", "schema": "factory"},
                {"route": "/factory/models", "name": "Models"}
            ])),
            None,
        );
        let projection = extract_ui_surfaces(&subids, Some(&field));
        assert!(projection.is_authoritative());
        assert_eq!(projection.value_source.as_deref(), Some("default"));
        assert_eq!(projection.subids, ["exp.service.factory.ui-surfaces@v1"]);
        assert_eq!(projection.routes[1].path, "/factory/models");
    }

    #[test]
    fn uses_example_then_preserves_fallback_without_routes() {
        let subids = HashMap::from([(
            "ui_surfaces".into(),
            "exp.service.demo.ui-surfaces@v1".into(),
        )]);
        let field = ui_field(
            None,
            Some(serde_json::json!({"routes": [{"href": "/demo"}]})),
        );
        let projection = extract_ui_surfaces(&subids, Some(&field));
        assert_eq!(projection.value_source.as_deref(), Some("example"));
        assert_eq!(projection.routes[0].path, "/demo");

        let fallback = extract_ui_surfaces(&HashMap::new(), None);
        assert!(!fallback.is_authoritative());
        assert!(fallback.routes.is_empty());
    }

    #[test]
    fn system_message_is_dataset_plus_docs_not_ui_roleplay() {
        let projection = UiSurfaceProjection {
            subids: vec!["exp.service.demo.ui-surfaces@v1".into()],
            routes: vec![UiSurfaceRoute {
                path: "/demo".into(),
                name: Some("Demo".into()),
                schema: Some("demo".into()),
                raw: serde_json::json!({"path": "/demo"}),
            }],
            value_source: Some("default".into()),
        };
        let ctx = GenerationContext::new(
            vec![
                payload("demo", projection),
                payload("plain", UiSurfaceProjection::default()),
            ],
            String::new(),
        );
        let system = ctx.build_system_message();
        let user = ctx.build_user_message();

        // Goal lives in the user message — vague accessibility baseline.
        assert!(user.contains("as accessible to as many people"));
        assert!(!user.to_lowercase().contains("ui specification"));

        // System message: dataset + vocabulary/grammar — not "you are a UI builder".
        assert!(system.contains("# Dataset"));
        assert!(system.contains("### demo"));
        assert!(system.contains("### plain"));
        assert!(system.contains("## Vocabulary"));
        assert!(system.contains("## Grammar"));
        assert!(!system.to_lowercase().contains("ui specification generator"));
        assert!(!system.contains("Authoritative UI Routes"));
        assert!(!system.contains("schema-derived fallback"));
    }

    #[test]
    fn compatibility_companion_resolves_only_when_base_exists() {
        let projection = UiSurfaceProjection {
            subids: vec!["exp.service.antigravity.ui-surfaces@v1".into()],
            routes: vec![UiSurfaceRoute {
                path: "/antigravity".into(),
                name: None,
                schema: None,
                raw: serde_json::json!({"path": "/antigravity"}),
            }],
            value_source: Some("default".into()),
        };
        let schemas = vec![payload("antigravity", projection)];
        assert_eq!(
            resolve_ui_plugin_id("antigravity_chat", &schemas),
            "antigravity"
        );
        assert_eq!(
            resolve_ui_plugin_id("standalone_chat", &schemas),
            "standalone_chat"
        );

        let no_routes = vec![payload("antigravity", UiSurfaceProjection::default())];
        assert_eq!(
            resolve_ui_plugin_id("antigravity_chat", &no_routes),
            "antigravity_chat"
        );
    }
}
