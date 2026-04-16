use chrono::{DateTime, Utc};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub service_name: String,
    pub base_object: simd_json::OwnedValue,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub id: String,
    pub plugin_name: String,
    pub definition: simd_json::OwnedValue,
    pub discovered_from: Option<String>,
    pub discovered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Plugin document stored in the schema library.
///
/// Catalog rule:
/// - The plugin defines the schema.
/// - That same schema provides the footprint and JSON render contract.
/// - This document is reusable schema material for builders/projection layers.
///
/// The document stays intentionally small. We do not create separate runtime
/// "schema", "footprint", or "render" stores here because that would
/// reintroduce drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCatalogDocument {
    /// Plugin-owned schema copied into the schema library.
    pub schema: PluginSchema,
    /// Origin marker for diagnostics; runtime plugin registration should use
    /// `"plugin"` rather than inventing a second schema source.
    #[serde(default = "default_schema_document_source")]
    pub source: String,
}

fn default_schema_document_source() -> String {
    "plugin".to_string()
}
