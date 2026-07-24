//! MCP tool exposing the full sealed-blob plugin catalog as one payload.
//!
//! Per-plugin tools (`read_plugin_schema_shm(id)`) only ever show a model one
//! blob at a time, so it can never reason about relationships *across*
//! plugins. This tool dumps every active plugin's schema in a single
//! response so the calling model can find cross-blob connections itself —
//! e.g. for UI generation that combines fields/state from unrelated plugins
//! in ways a fixed per-plugin template never would.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_blob_catalog_tool(registry: &ToolRegistry) -> Result<()> {
    registry
        .register(Arc::new(BlobCatalogTool) as BoxedTool)
        .await
}

struct BlobCatalogTool;

#[async_trait]
impl Tool for BlobCatalogTool {
    fn name(&self) -> &str {
        "blob_catalog"
    }

    fn description(&self) -> &str {
        "Return every active plugin's sealed schema (from the SHM blob catalog) in one \
         payload, keyed by plugin id. Unlike per-plugin schema lookups, this gives a model \
         the full catalog at once so it can reason about connections across plugins.\n\
         Use mode='summary' for a lightweight overview (id, name, description, category, method/signal counts). \
         Use mode='full' for complete schemas (may exceed output limits for large catalogs)."
    }

    fn category(&self) -> &str {
        "schema"
    }

    fn namespace(&self) -> &str {
        "plugins"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "schema".to_string(),
            "blob".to_string(),
            "catalog".to_string(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["summary", "full"],
                    "default": "summary",
                    "description": "Output mode: 'summary' returns lightweight metadata only (~50KB), 'full' returns complete schemas (~2MB, may truncate)"
                },
                "category": {
                    "type": "string",
                    "description": "Optional filter: only return plugins matching this category"
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let mode = input.get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("summary");
        let category_filter = input.get("category")
            .and_then(|v| v.as_str());

        let ids = op_blob::catalog::read_manifest_plugin_ids_shm()
            .context("failed to read plugin blob manifest from SHM catalog")?;

        let mut plugins = simd_json::owned::Object::new();
        let mut missing = Vec::new();

        for id in &ids {
            if let Some(schema) = op_blob::catalog::read_plugin_schema_shm(id) {
                // Apply category filter if specified
                if let Some(cat) = category_filter {
                    let schema_value = simd_json::to_value(schema.clone())
                        .with_context(|| format!("failed to serialize schema for '{id}'"))?;
                    if let Some(plugin_category) = schema_value.get("category") {
                        if plugin_category.as_str().unwrap_or("") != cat {
                            continue; // Skip plugins not matching category
                        }
                    }
                }

                if mode == "summary" {
                    // Summary mode: return lightweight metadata only
                    let schema_value = simd_json::to_value(schema.clone())
                        .with_context(|| format!("failed to serialize schema for '{id}'"))?;

                    let mut summary = simd_json::owned::Object::new();
                    summary.insert("name".to_string(),
                        schema_value.get("name").cloned().unwrap_or(Value::Null));
                    summary.insert("description".to_string(),
                        schema_value.get("description").cloned().unwrap_or(Value::Null));
                    summary.insert("category".to_string(),
                        schema_value.get("category").cloned().unwrap_or(Value::Null));
                    summary.insert("version".to_string(),
                        schema_value.get("version").cloned().unwrap_or(Value::Null));

                    // Count methods and signals
                    let method_count = schema_value.get("methods")
                        .and_then(|m| m.as_object())
                        .map(|m| m.len())
                        .unwrap_or(0);
                    let signal_count = schema_value.get("signals")
                        .and_then(|s| s.as_array())
                        .map(|s| s.len())
                        .unwrap_or(0);

                    summary.insert("method_count".to_string(), json!(method_count));
                    summary.insert("signal_count".to_string(), json!(signal_count));

                    plugins.insert(id.clone(), Value::Object(Box::new(summary)));
                } else {
                    // Full mode: return complete schema
                    let value = simd_json::serde::to_owned_value(schema)
                        .with_context(|| format!("failed to serialize schema for '{id}'"))?;
                    plugins.insert(id.clone(), value);
                }
            } else {
                missing.push(id.clone());
            }
        }

        Ok(json!({
            "plugin_count": plugins.len(),
            "mode": mode,
            "plugins": Value::Object(Box::new(plugins)),
            "missing_schemas": missing,
        }))
    }
}
