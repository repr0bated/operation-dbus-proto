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
pub use op_core::SecurityLevel;

/// Content returned by a tool execution
#[derive(Debug, Clone)]
pub enum ToolContent {
    Text(String),
    Json(Value),
}

impl ToolContent {
    pub fn text(s: impl Into<String>) -> Self {
        ToolContent::Text(s.into())
    }
    pub fn json(v: Value) -> Self {
        ToolContent::Json(v)
    }
}

/// Result of a tool execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(content: ToolContent) -> Self {
        ToolResult { content: vec![content], is_error: false }
    }
    pub fn error(content: ToolContent) -> Self {
        ToolResult { content: vec![content], is_error: true }
    }
}

impl From<ToolResult> for Value {
    fn from(r: ToolResult) -> Value {
        match r.content.into_iter().next() {
            Some(ToolContent::Text(s)) => Value::String(s),
            Some(ToolContent::Json(v)) => v,
            None => Value::Static(simd_json::StaticNode::Null),
        }
    }
}

/// Metadata describing a tool
#[derive(Debug, Clone)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub version: String,
    pub security_level: SecurityLevel,
    pub requires_auth: bool,
}

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
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            name: self.name().to_string(),
            description: self.description().to_string(),
            category: self.category().to_string(),
            tags: self.tags(),
            author: None,
            version: "1.0.0".to_string(),
            security_level: SecurityLevel::Public,
            requires_auth: false,
        }
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
            namespace: String::new(),
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

        let filtered: Vec<_> = defs
            .values()
            .filter(|d| category.is_none_or(|c| d.category == c))
            .cloned()
            .collect();

        filtered.into_iter().skip(offset).take(limit).collect()
    }

    /// Search tools by query
    pub async fn search(&self, query: &str) -> Vec<ToolDefinition> {
        let query_lower = query.to_lowercase();
        let defs = self.definitions.read().await;

        defs.values()
            .filter(|d| {
                d.name.to_lowercase().contains(&query_lower)
                    || d.description.to_lowercase().contains(&query_lower)
                    || d.category.to_lowercase().contains(&query_lower)
                    || d.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .take(50).cloned() // Reasonable limit for search results
            .collect()
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
