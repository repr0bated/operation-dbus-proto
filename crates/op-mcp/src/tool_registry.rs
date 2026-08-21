//! Tool Registry - All Tools Always Loaded
//!
//! This replaces lazy_tools.rs with a simple registry that:
//! - Loads ALL tools at startup
//! - Never evicts tools
//! - Provides fast lookup for execute_tool
//!
//! The compact mode meta-tools use this registry to:
//! - list_tools: Paginate through all registered tools
//! - search_tools: Filter by name/description/category
//! - get_tool_schema: Return input schema for a tool
//! - execute_tool: Look up and execute any tool

use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::server::{ToolExecutor, ToolInfo};
use op_core::ToolDefinition;

/// Tool trait - same as op_tools::Tool but standalone
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn category(&self) -> &str {
        "general"
    }
    fn namespace(&self) -> &str {
        "system"
    }
    fn tags(&self) -> Vec<String> {
        vec![]
    }
    async fn execute(&self, input: Value) -> Result<Value>;
}

pub type BoxedTool = Arc<dyn Tool>;

/// Simple tool registry - NO eviction, all tools always available
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, BoxedTool>>,
    definitions: RwLock<HashMap<String, ToolDefinition>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            definitions: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool (never evicted)
    pub async fn register(&self, tool: BoxedTool) -> Result<()> {
        let name = tool.name().to_string();
        let definition = ToolDefinition {
            name: name.clone(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            schema_version: String::new(),
            category: tool.category().to_string(),
            tags: tool.tags(),
            namespace: tool.namespace().to_string(),
        };

        self.tools.write().await.insert(name.clone(), tool);
        self.definitions
            .write()
            .await
            .insert(name.clone(), definition);

        debug!("Registered tool: {}", name);
        Ok(())
    }

    /// Get a tool by name (instant lookup, no loading)
    pub async fn get(&self, name: &str) -> Option<BoxedTool> {
        self.tools.read().await.get(name).cloned()
    }

    /// Execute a tool by name
    pub async fn execute(&self, name: &str, input: Value) -> Result<Value> {
        let tool = self
            .get(name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;
        tool.execute(input).await
    }

    /// Get tool definition
    pub async fn get_definition(&self, name: &str) -> Option<ToolDefinition> {
        self.definitions.read().await.get(name).cloned()
    }

    /// List all tools (paginated)
    pub async fn list(
        &self,
        offset: usize,
        limit: usize,
        category: Option<&str>,
    ) -> Vec<ToolDefinition> {
        let defs = self.definitions.read().await;

        let mut filtered: Vec<_> = defs
            .values()
            .filter(|d| category.is_none_or(|c| d.category == c))
            .cloned()
            .collect();
        filtered.sort_by(|left, right| left.name.cmp(&right.name));

        filtered.into_iter().skip(offset).take(limit).collect()
    }

    /// Search tools by query
    pub async fn search(&self, query: &str) -> Vec<ToolDefinition> {
        let query_lower = query.to_lowercase();
        let defs = self.definitions.read().await;

        let mut matches: Vec<_> = defs
            .values()
            .filter(|d| {
                d.name.to_lowercase().contains(&query_lower)
                    || d.description.to_lowercase().contains(&query_lower)
                    || d.category.to_lowercase().contains(&query_lower)
                    || d.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect();
        matches.sort_by(|left, right| left.name.cmp(&right.name));
        matches.truncate(50); // Reasonable limit for search results
        matches
    }

    /// Total tool count
    pub async fn count(&self) -> usize {
        self.tools.read().await.len()
    }

    /// Get all categories
    pub async fn categories(&self) -> Vec<String> {
        let defs = self.definitions.read().await;
        let mut cats: Vec<String> = defs
            .values()
            .map(|d| d.category.clone())
            .filter(|c| !c.is_empty())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry executor for CompactServer
pub struct RegistryExecutor {
    registry: Arc<ToolRegistry>,
}

impl RegistryExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ToolExecutor for RegistryExecutor {
    async fn execute_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        self.registry.execute(tool_name, arguments).await
    }

    async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.list(0, 1000, None).await;
        Ok(tools
            .into_iter()
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }

    async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>> {
        Ok(self
            .registry
            .get_definition(name)
            .await
            .map(|d| d.input_schema))
    }

    async fn search_tools(&self, query: &str, limit: usize) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.search(query).await;
        Ok(tools
            .into_iter()
            .take(limit)
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use simd_json::json;

    struct TestTool {
        name: &'static str,
        description: &'static str,
        category: &'static str,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            self.description
        }

        fn input_schema(&self) -> Value {
            json!({"type": "object"})
        }

        fn category(&self) -> &str {
            self.category
        }

        async fn execute(&self, input: Value) -> Result<Value> {
            Ok(input)
        }
    }

    async fn test_registry() -> ToolRegistry {
        let registry = ToolRegistry::new();
        for tool in [
            TestTool {
                name: "zeta_query",
                description: "Search operational state",
                category: "operations",
            },
            TestTool {
                name: "alpha_query",
                description: "Search cognitive memory",
                category: "cognitive",
            },
            TestTool {
                name: "beta_status",
                description: "Report current status",
                category: "operations",
            },
        ] {
            registry
                .register(Arc::new(tool))
                .await
                .expect("register test tool");
        }
        registry
    }

    #[tokio::test]
    async fn list_is_name_sorted_before_pagination_and_filtering() {
        let registry = test_registry().await;

        let names: Vec<_> = registry
            .list(0, usize::MAX, None)
            .await
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert_eq!(names, ["alpha_query", "beta_status", "zeta_query"]);

        let page = registry.list(1, 1, None).await;
        assert_eq!(page[0].name, "beta_status");

        let operations: Vec<_> = registry
            .list(0, usize::MAX, Some("operations"))
            .await
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert_eq!(operations, ["beta_status", "zeta_query"]);
    }

    #[tokio::test]
    async fn search_is_name_sorted_before_its_result_limit() {
        let registry = test_registry().await;

        let names: Vec<_> = registry
            .search("query")
            .await
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert_eq!(names, ["alpha_query", "zeta_query"]);
    }
}
