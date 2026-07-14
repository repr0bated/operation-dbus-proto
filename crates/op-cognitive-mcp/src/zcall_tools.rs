//! LLM-callable wrappers around `op-inspector`'s `ZcallParser` adapter.
//!
//! Answers "how does an LLM use the zcall introspection surface": these
//! tools let an external LLM client (via cognitive-mcp, the universal MCP
//! gateway) discover the exact plugin/method/argument shape *before*
//! constructing a real `zcall <plugin> <method> --arguments '{...}'` call,
//! instead of guessing argument names/types. Mirrors `gcloud_tools.rs`
//! (`op-tools/src/builtin/gcloud_tools.rs`) — same 4-tool shape
//! (introspect_full / list / get_one / search), same caching pattern.

use anyhow::Result;
use async_trait::async_trait;
use op_inspector::{ZcallParser, ZcallSchema};
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;

struct ZcallCache {
    schema: Option<ZcallSchema>,
}

/// Register all 4 zcall introspection tools into the MCP tool registry.
pub async fn register_zcall_tools(registry: &ToolRegistry) -> Result<usize> {
    let parser = Arc::new(ZcallParser::new());
    let cache = Arc::new(RwLock::new(ZcallCache { schema: None }));

    let tools: Vec<BoxedTool> = vec![
        Arc::new(ZcallIntrospectTool::new(parser.clone(), cache.clone())),
        Arc::new(ZcallListPluginsTool::new(parser.clone())),
        Arc::new(ZcallGetMethodTool::new(parser.clone())),
        Arc::new(ZcallSearchTool::new(parser, cache)),
    ];

    let count = tools.len();
    for tool in tools {
        registry.register(tool).await?;
    }

    tracing::info!(registered = count, "Registered zcall introspection tools");
    Ok(count)
}

async fn get_cached_schema(
    parser: &Arc<ZcallParser>,
    cache: &Arc<RwLock<ZcallCache>>,
) -> Result<ZcallSchema> {
    {
        let read = cache.read().await;
        if let Some(schema) = &read.schema {
            return Ok(schema.clone());
        }
    }
    let schema = parser.introspect_full().await?;
    let mut write = cache.write().await;
    write.schema = Some(schema.clone());
    Ok(schema)
}

// ---------------------------------------------------------------------------
// zcall_introspect — full plugin/method sweep
// ---------------------------------------------------------------------------

struct ZcallIntrospectTool {
    parser: Arc<ZcallParser>,
    cache: Arc<RwLock<ZcallCache>>,
}

impl ZcallIntrospectTool {
    fn new(parser: Arc<ZcallParser>, cache: Arc<RwLock<ZcallCache>>) -> Self {
        Self { parser, cache }
    }
}

#[async_trait]
impl Tool for ZcallIntrospectTool {
    fn name(&self) -> &str {
        "zcall_introspect"
    }

    fn description(&self) -> &str {
        "Introspect the ENTIRE zcall plugin/method surface (every plugin, every method, every \
         typed args/returns schema). Heavy (one subprocess per method) but cached -- use \
         zcall_search or zcall_get_method for a single lookup instead when possible."
    }

    fn category(&self) -> &str {
        "zcall"
    }

    fn namespace(&self) -> &str {
        "op-inspector"
    }

    fn tags(&self) -> Vec<String> {
        vec!["zcall".into(), "introspection".into(), "opdbus".into()]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "refresh": {
                    "type": "boolean",
                    "description": "Force re-introspection even if cached (default: false)",
                    "default": false
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let refresh = input["refresh"].as_bool().unwrap_or(false);
        if refresh {
            self.cache.write().await.schema = None;
        }
        let schema = get_cached_schema(&self.parser, &self.cache).await?;
        Ok(json!({
            "schema_version": schema.schema_version,
            "plugin_count": schema.plugin_count,
            "method_count": schema.method_count,
            "plugins": simd_json::serde::to_owned_value(&schema.plugins)?
        }))
    }
}

// ---------------------------------------------------------------------------
// zcall_list_plugins — just the plugin names (cheap, no per-method calls)
// ---------------------------------------------------------------------------

struct ZcallListPluginsTool {
    parser: Arc<ZcallParser>,
}

impl ZcallListPluginsTool {
    fn new(parser: Arc<ZcallParser>) -> Self {
        Self { parser }
    }
}

#[async_trait]
impl Tool for ZcallListPluginsTool {
    fn name(&self) -> &str {
        "zcall_list_plugins"
    }

    fn description(&self) -> &str {
        "List every zcall plugin name (cheap -- one subprocess call, no per-method schema fetch)."
    }

    fn category(&self) -> &str {
        "zcall"
    }

    fn namespace(&self) -> &str {
        "op-inspector"
    }

    fn tags(&self) -> Vec<String> {
        vec!["zcall".into(), "list".into(), "opdbus".into()]
    }

    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let plugins = self.parser.list_plugins().await?;
        Ok(json!({ "plugins": plugins, "count": plugins.len() }))
    }
}

// ---------------------------------------------------------------------------
// zcall_get_method — one method's exact typed schema
// ---------------------------------------------------------------------------

struct ZcallGetMethodTool {
    parser: Arc<ZcallParser>,
}

impl ZcallGetMethodTool {
    fn new(parser: Arc<ZcallParser>) -> Self {
        Self { parser }
    }
}

#[async_trait]
impl Tool for ZcallGetMethodTool {
    fn name(&self) -> &str {
        "zcall_get_method"
    }

    fn description(&self) -> &str {
        "Get one zcall method's exact typed args/returns JSON Schema, required capability, and \
         side effect (read/mutation). Call this BEFORE constructing a real `zcall <plugin> \
         <method> --arguments '{...}'` invocation so the argument shape is correct on the first try."
    }

    fn category(&self) -> &str {
        "zcall"
    }

    fn namespace(&self) -> &str {
        "op-inspector"
    }

    fn tags(&self) -> Vec<String> {
        vec!["zcall".into(), "schema".into(), "opdbus".into()]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "plugin": { "type": "string", "description": "Plugin name (e.g. 'qdrant')" },
                "method": { "type": "string", "description": "Method name (e.g. 'list_collections')" }
            },
            "required": ["plugin", "method"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let plugin = input["plugin"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing plugin"))?;
        let method = input["method"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing method"))?;

        let m = self.parser.get_method(plugin, method).await?;
        Ok(json!({
            "name": m.name,
            "side_effect": m.side_effect,
            "required_capability": m.required_capability,
            "subid": m.subid,
            "idempotent": m.idempotent,
            "args": simd_json::serde::to_owned_value(&m.args)?,
            "returns": simd_json::serde::to_owned_value(&m.returns)?,
            "grpc_path": m.grpc_path,
        }))
    }
}

// ---------------------------------------------------------------------------
// zcall_search — find plugins/methods by substring, across the full sweep
// ---------------------------------------------------------------------------

struct ZcallSearchTool {
    parser: Arc<ZcallParser>,
    cache: Arc<RwLock<ZcallCache>>,
}

impl ZcallSearchTool {
    fn new(parser: Arc<ZcallParser>, cache: Arc<RwLock<ZcallCache>>) -> Self {
        Self { parser, cache }
    }
}

#[async_trait]
impl Tool for ZcallSearchTool {
    fn name(&self) -> &str {
        "zcall_search"
    }

    fn description(&self) -> &str {
        "Search plugin and method names (and subid/capability strings) for a substring. Use this \
         first when you don't know the exact plugin/method name -- e.g. searching 'qdrant' finds \
         the qdrant plugin and every one of its methods."
    }

    fn category(&self) -> &str {
        "zcall"
    }

    fn namespace(&self) -> &str {
        "op-inspector"
    }

    fn tags(&self) -> Vec<String> {
        vec!["zcall".into(), "search".into(), "opdbus".into()]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Substring to search for (case-insensitive)" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing query"))?
            .to_lowercase();

        let schema = get_cached_schema(&self.parser, &self.cache).await?;

        let mut matches = Vec::new();
        for (plugin_name, plugin) in &schema.plugins {
            let plugin_hit = plugin_name.to_lowercase().contains(&query);
            for (method_name, method) in &plugin.methods {
                let hit = plugin_hit
                    || method_name.to_lowercase().contains(&query)
                    || method.subid.to_lowercase().contains(&query)
                    || method.required_capability.to_lowercase().contains(&query);
                if hit {
                    matches.push(json!({
                        "plugin": plugin_name,
                        "method": method_name,
                        "side_effect": method.side_effect,
                        "subid": method.subid,
                    }));
                }
            }
        }

        Ok(json!({ "query": query, "matches": matches, "count": matches.len() }))
    }
}
