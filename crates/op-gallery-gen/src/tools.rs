//! Tool execution layer for inference loop.
//!
//! Provides tools for:
//! - Plugin schema queries (always available)
//! - MCP cross-blob discovery (when enabled)
//! - Qdrant semantic search (when enabled)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::context::GenerationContext;

/// Registry of available tools.
pub struct ToolRegistry {
    /// Optional Qdrant client for real semantic search.
    qdrant: Option<crate::qdrant::GalleryQdrantClient>,
}

/// OpenAI-compatible tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call within a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Tool execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub result: serde_json::Value,
}

impl ToolRegistry {
    /// Create a new tool registry without Qdrant.
    pub fn new() -> Self {
        Self { qdrant: None }
    }

    /// Create a tool registry with Qdrant integration.
    pub fn with_qdrant(qdrant: crate::qdrant::GalleryQdrantClient) -> Self {
        Self {
            qdrant: Some(qdrant),
        }
    }

    /// Execute a tool call.
    pub async fn execute(&self, call: &ToolCall, ctx: &GenerationContext) -> ToolResult {
        let args: HashMap<String, serde_json::Value> =
            serde_json::from_str(&call.function.arguments).unwrap_or_default();

        match call.function.name.as_str() {
            "list_plugins" => self.list_plugins(ctx),
            "get_plugin_schema" => self.get_plugin_schema(ctx, &args),
            "search_fields" => self.search_fields(ctx, &args),
            "search_methods" => self.search_methods(ctx, &args),
            "search_subids" => self.search_subids(ctx, &args),
            "find_related" => self.find_related(ctx, &args),
            "semantic_search" => self.semantic_search(ctx, &args).await,
            _ => ToolResult {
                success: false,
                result: serde_json::json!({
                    "error": format!("Unknown tool: {}", call.function.name)
                }),
            },
        }
    }

    /// List all available plugins.
    fn list_plugins(&self, ctx: &GenerationContext) -> ToolResult {
        let plugins: Vec<serde_json::Value> = ctx
            .schemas
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "version": s.version,
                    "description": s.description,
                    "category": s.category,
                    "authoritative_ui_surfaces": s.ui_surfaces,
                    "ui_surface_fallback": !s.ui_surfaces.is_authoritative()
                })
            })
            .collect();

        ToolResult {
            success: true,
            result: serde_json::json!({
                "plugins": plugins,
                "count": plugins.len()
            }),
        }
    }

    /// Get the full schema for a specific plugin.
    fn get_plugin_schema(
        &self,
        ctx: &GenerationContext,
        args: &HashMap<String, serde_json::Value>,
    ) -> ToolResult {
        let plugin_name = match args.get("plugin_name").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => {
                return ToolResult {
                    success: false,
                    result: serde_json::json!({ "error": "Missing plugin_name parameter" }),
                }
            }
        };

        let plugin_name = crate::context::resolve_ui_plugin_id(plugin_name, &ctx.schemas);
        match ctx.schemas.iter().find(|s| s.name == plugin_name) {
            Some(schema) => {
                let mut result = schema.raw_json.clone();
                if let Some(object) = result.as_object_mut() {
                    object.insert(
                        "authoritative_ui_surfaces".to_string(),
                        serde_json::to_value(&schema.ui_surfaces)
                            .unwrap_or_else(|_| serde_json::json!({})),
                    );
                    object.insert(
                        "ui_surface_fallback".to_string(),
                        serde_json::Value::Bool(!schema.ui_surfaces.is_authoritative()),
                    );
                }
                ToolResult {
                    success: true,
                    result,
                }
            }
            None => ToolResult {
                success: false,
                result: serde_json::json!({
                    "error": format!("Plugin not found: {}", plugin_name)
                }),
            },
        }
    }

    /// Search for fields across all plugins by name pattern.
    fn search_fields(
        &self,
        ctx: &GenerationContext,
        args: &HashMap<String, serde_json::Value>,
    ) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p.to_lowercase(),
            None => {
                return ToolResult {
                    success: false,
                    result: serde_json::json!({ "error": "Missing pattern parameter" }),
                }
            }
        };

        let mut results = Vec::new();

        for schema in &ctx.schemas {
            for (field_name, field) in &schema.fields {
                if field_name.to_lowercase().contains(&pattern) {
                    results.push(serde_json::json!({
                        "plugin": schema.name,
                        "field": field_name,
                        "type": format!("{:?}", field.field_type),
                        "description": field.description
                    }));
                }
            }
        }

        ToolResult {
            success: true,
            result: serde_json::json!({
                "matches": results,
                "count": results.len()
            }),
        }
    }

    /// Search for methods across all plugins (MCP tool).
    fn search_methods(
        &self,
        ctx: &GenerationContext,
        args: &HashMap<String, serde_json::Value>,
    ) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p.to_lowercase(),
            None => {
                return ToolResult {
                    success: false,
                    result: serde_json::json!({ "error": "Missing pattern parameter" }),
                }
            }
        };

        let mut results = Vec::new();

        for schema in &ctx.schemas {
            for (method_name, method) in &schema.methods {
                if method_name.to_lowercase().contains(&pattern) {
                    results.push(serde_json::json!({
                        "plugin": schema.name,
                        "method": method_name,
                        "description": method.description,
                        "side_effect": format!("{:?}", method.side_effect)
                    }));
                }
            }
        }

        ToolResult {
            success: true,
            result: serde_json::json!({
                "matches": results,
                "count": results.len()
            }),
        }
    }

    /// Search for OSCAL subids (MCP tool).
    fn search_subids(
        &self,
        ctx: &GenerationContext,
        args: &HashMap<String, serde_json::Value>,
    ) -> ToolResult {
        let category = args.get("category").and_then(|v| v.as_str());
        let pattern = args.get("pattern").and_then(|v| v.as_str());

        let mut results = Vec::new();

        for schema in &ctx.schemas {
            for (subid, description) in &schema.subids {
                let matches_category =
                    category.map_or(true, |c| subid.starts_with(&format!("{}.", c)));
                let matches_pattern =
                    pattern.map_or(true, |p| subid.to_lowercase().contains(&p.to_lowercase()));

                if matches_category && matches_pattern {
                    results.push(serde_json::json!({
                        "plugin": schema.name,
                        "subid": subid,
                        "description": description
                    }));
                }
            }
        }

        ToolResult {
            success: true,
            result: serde_json::json!({
                "matches": results,
                "count": results.len()
            }),
        }
    }

    /// Find plugins related to a given plugin (MCP tool).
    fn find_related(
        &self,
        ctx: &GenerationContext,
        args: &HashMap<String, serde_json::Value>,
    ) -> ToolResult {
        let plugin_name = match args.get("plugin_name").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => {
                return ToolResult {
                    success: false,
                    result: serde_json::json!({ "error": "Missing plugin_name parameter" }),
                }
            }
        };

        let target_schema = match ctx.schemas.iter().find(|s| s.name == plugin_name) {
            Some(s) => s,
            None => {
                return ToolResult {
                    success: false,
                    result: serde_json::json!({ "error": format!("Plugin not found: {}", plugin_name) }),
                }
            }
        };

        let target_fields: std::collections::HashSet<_> =
            target_schema.fields.keys().cloned().collect();
        let target_methods: std::collections::HashSet<_> =
            target_schema.methods.keys().cloned().collect();

        let mut related = Vec::new();

        for schema in &ctx.schemas {
            if schema.name == plugin_name {
                continue;
            }

            let shared_fields: Vec<_> = schema
                .fields
                .keys()
                .filter(|f| target_fields.contains(*f))
                .cloned()
                .collect();

            let shared_methods: Vec<_> = schema
                .methods
                .keys()
                .filter(|m| target_methods.contains(*m))
                .cloned()
                .collect();

            if !shared_fields.is_empty() || !shared_methods.is_empty() {
                related.push(serde_json::json!({
                    "plugin": schema.name,
                    "shared_fields": shared_fields,
                    "shared_methods": shared_methods
                }));
            }
        }

        ToolResult {
            success: true,
            result: serde_json::json!({
                "target": plugin_name,
                "related": related,
                "count": related.len()
            }),
        }
    }

    /// Semantic search using Qdrant (Qdrant tool).
    async fn semantic_search(
        &self,
        ctx: &GenerationContext,
        args: &HashMap<String, serde_json::Value>,
    ) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return ToolResult {
                    success: false,
                    result: serde_json::json!({ "error": "Missing query parameter" }),
                }
            }
        };

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        // Use real Qdrant if available, otherwise fall back to keyword matching
        if let Some(qdrant) = &self.qdrant {
            match qdrant.search(query, limit, None).await {
                Ok(results) => {
                    let results_json: Vec<serde_json::Value> = results
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "plugin_id": r.plugin_id,
                                "fragment": r.fragment,
                                "domain_tag": r.domain_tag,
                                "score": r.score,
                            })
                        })
                        .collect();

                    return ToolResult {
                        success: true,
                        result: serde_json::json!({
                            "query": query,
                            "results": results_json,
                            "count": results_json.len(),
                            "source": "qdrant"
                        }),
                    };
                }
                Err(e) => {
                    tracing::warn!("Qdrant search failed, falling back to keywords: {}", e);
                    // Fall through to keyword matching below
                }
            }
        }

        // Keyword matching fallback
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for schema in &ctx.schemas {
            let mut score = 0.0_f64;

            if schema.name.to_lowercase().contains(&query_lower) {
                score += 0.5;
            }
            if let Some(desc) = &schema.description {
                if desc.to_lowercase().contains(&query_lower) {
                    score += 0.3;
                }
            }
            for field_name in schema.fields.keys() {
                if field_name.to_lowercase().contains(&query_lower) {
                    score += 0.1;
                }
            }

            if score > 0.0 {
                results.push(serde_json::json!({
                    "plugin_id": schema.name,
                    "score": score,
                    "domain_tag": schema.category,
                    "fragment": schema.description,
                }));
            }
        }

        results.sort_by(|a, b| {
            let sa = a.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            let sb = b.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        ToolResult {
            success: true,
            result: serde_json::json!({
                "query": query,
                "results": results,
                "count": results.len(),
                "source": "keyword_fallback"
            }),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{SchemaPayload, UiSurfaceProjection, UiSurfaceRoute};

    fn context() -> GenerationContext {
        let schema = SchemaPayload {
            name: "demo".into(),
            version: "1.0.0".into(),
            description: None,
            category: None,
            fields: HashMap::new(),
            methods: HashMap::new(),
            subids: HashMap::new(),
            ui_surfaces: UiSurfaceProjection {
                subids: vec!["exp.service.demo.ui-surfaces@v1".into()],
                routes: vec![UiSurfaceRoute {
                    path: "/demo".into(),
                    name: Some("Demo".into()),
                    schema: Some("demo".into()),
                    raw: serde_json::json!({"path": "/demo"}),
                }],
                value_source: Some("default".into()),
            },
            raw_json: serde_json::json!({"name": "demo"}),
        };
        GenerationContext::new(vec![schema], String::new())
    }

    #[test]
    fn plugin_tools_expose_authoritative_routes() {
        let registry = ToolRegistry::new();
        let ctx = context();
        let listed = registry.list_plugins(&ctx);
        assert_eq!(
            listed.result["plugins"][0]["authoritative_ui_surfaces"]["routes"][0]["path"],
            "/demo"
        );
        assert_eq!(listed.result["plugins"][0]["ui_surface_fallback"], false);

        let args = HashMap::from([("plugin_name".into(), serde_json::json!("demo"))]);
        let schema = registry.get_plugin_schema(&ctx, &args);
        assert_eq!(
            schema.result["authoritative_ui_surfaces"]["routes"][0]["path"],
            "/demo"
        );
        assert_eq!(schema.result["ui_surface_fallback"], false);
    }
}
