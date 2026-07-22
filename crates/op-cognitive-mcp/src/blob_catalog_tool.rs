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
         the full catalog at once so it can reason about connections across plugins."
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
        json!({"type": "object", "additionalProperties": false})
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let ids = op_blob::catalog::read_manifest_plugin_ids_shm()
            .context("failed to read plugin blob manifest from SHM catalog")?;

        let mut plugins = simd_json::owned::Object::new();
        let mut missing = Vec::new();
        for id in &ids {
            match op_blob::catalog::read_plugin_schema_shm(id) {
                Some(schema) => {
                    let value = simd_json::serde::to_owned_value(schema)
                        .with_context(|| format!("failed to serialize schema for '{id}'"))?;
                    plugins.insert(id.clone(), value);
                }
                None => missing.push(id.clone()),
            }
        }

        Ok(json!({
            "plugin_count": plugins.len(),
            "plugins": Value::Object(Box::new(plugins)),
            "missing_schemas": missing,
        }))
    }
}
