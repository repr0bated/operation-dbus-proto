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
    // Future: Qdrant client when enabled
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
    /// Create a new tool registry.
    pub fn new() -> Self {
        Self {}
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
                    "category": s.category
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

        match ctx.schemas.iter().find(|s| s.name == plugin_name) {
            Some(schema) => ToolResult {
                success: true,
                result: schema.raw_json.clone(),
            },
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

        // TODO: Implement actual Qdrant integration
        // For now, do simple keyword matching as fallback
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for schema in &ctx.schemas {
            let mut score = 0.0;

            // Check name match
            if schema.name.to_lowercase().contains(&query_lower) {
                score += 0.5;
            }

            // Check description match
            if let Some(desc) = &schema.description {
                if desc.to_lowercase().contains(&query_lower) {
                    score += 0.3;
                }
            }

            // Check field names
            for field_name in schema.fields.keys() {
                if field_name.to_lowercase().contains(&query_lower) {
                    score += 0.1;
                }
            }

            if score > 0.0 {
                results.push(serde_json::json!({
                    "plugin": schema.name,
                    "score": score,
                    "description": schema.description
                }));
            }
        }

        results.sort_by(|a, b| {
            let score_a = a.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            let score_b = b.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(limit);

        ToolResult {
            success: true,
            result: serde_json::json!({
                "query": query,
                "results": results,
                "note": "Using keyword matching (Qdrant not yet integrated)"
            }),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
