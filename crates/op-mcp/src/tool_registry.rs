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

/// Whether a registered tool is executable in this runtime.
///
/// The catalog is an operator-facing contract, so a schema-declared placeholder
/// must be distinguishable from a working tool before an agent attempts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolReadiness {
    Live,
    Mock { reason: String },
    Disabled { reason: String },
}

impl ToolReadiness {
    pub fn status(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Mock { .. } => "mock",
            Self::Disabled { .. } => "disabled",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Live => None,
            Self::Mock { reason } | Self::Disabled { reason } => Some(reason),
        }
    }
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
    fn readiness(&self) -> ToolReadiness {
        ToolReadiness::Live
    }
    async fn execute(&self, input: Value) -> Result<Value>;
}

pub type BoxedTool = Arc<dyn Tool>;

/// A tool's executable contract plus its runtime readiness.
#[derive(Debug, Clone)]
pub struct ToolCatalogEntry {
    pub definition: ToolDefinition,
    pub readiness: ToolReadiness,
}

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

        // Readiness is an executable contract, not merely catalog decoration.
        // A disabled integration must not be callable through an alternate
        // client path which bypasses the operator catalog.
        if let ToolReadiness::Disabled { reason } = tool.readiness() {
            anyhow::bail!("Tool '{}' is disabled: {}", name, reason);
        }

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
        let defs = self.definitions.read().await;

        let mut matches: Vec<_> = defs
            .values()
            .filter_map(|definition| {
                tool_search_score(definition, query).map(|score| (score, definition.clone()))
            })
            .collect();
        matches.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .cmp(left_score)
                .then_with(|| left.name.cmp(&right.name))
        });
        matches.truncate(50); // Reasonable limit for search results
        matches
            .into_iter()
            .map(|(_score, definition)| definition)
            .collect()
    }

    /// List tool definitions together with their executable readiness.
    /// Ordering and pagination match [`Self::list`], so a catalog can be
    /// rendered directly without accidentally marking a random page as live.
    pub async fn catalog(
        &self,
        offset: usize,
        limit: usize,
        category: Option<&str>,
    ) -> Vec<ToolCatalogEntry> {
        let definitions = self.definitions.read().await;
        let tools = self.tools.read().await;
        let mut entries: Vec<_> = definitions
            .values()
            .filter(|definition| category.is_none_or(|value| definition.category == value))
            .filter_map(|definition| {
                tools.get(&definition.name).map(|tool| ToolCatalogEntry {
                    definition: definition.clone(),
                    readiness: tool.readiness(),
                })
            })
            .collect();
        entries.sort_by(|left, right| left.definition.name.cmp(&right.definition.name));
        entries.into_iter().skip(offset).take(limit).collect()
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

/// Match human search phrases against catalog identifiers as well as ordinary
/// prose. Tool names routinely use `snake_case`, while an MCP client is much
/// more likely to ask for "design api". A literal substring check misses that
/// otherwise exact match.
fn tool_search_score(definition: &ToolDefinition, query: &str) -> Option<usize> {
    let query = query.trim();
    if query.is_empty() {
        return Some(0);
    }

    let fields = [
        definition.name.as_str(),
        definition.description.as_str(),
        definition.category.as_str(),
    ];
    let haystack = fields
        .iter()
        .copied()
        .chain(definition.tags.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    if haystack.to_lowercase().contains(&query.to_lowercase()) {
        // A phrase match within the tool name is the most specific result;
        // a phrase found only in prose remains useful but lower priority.
        return Some(
            if definition
                .name
                .to_lowercase()
                .contains(&query.to_lowercase())
            {
                10_000
            } else {
                1_000
            },
        );
    }

    let query_terms = search_terms(query);
    if query_terms.is_empty() {
        return None;
    }
    let haystack_terms = search_terms(&haystack);
    let matches_all_terms = query_terms
        .iter()
        .all(|term| haystack_terms.iter().any(|candidate| candidate == term));
    if !matches_all_terms {
        return None;
    }

    let name_terms = search_terms(&definition.name);
    let description_terms = search_terms(&definition.description);
    let category_terms = search_terms(&definition.category);
    let tag_terms = definition
        .tags
        .iter()
        .flat_map(|tag| search_terms(tag))
        .collect::<Vec<_>>();
    let score = query_terms.iter().fold(0, |score, term| {
        score
            + usize::from(name_terms.iter().any(|candidate| candidate == term)) * 100
            + usize::from(description_terms.iter().any(|candidate| candidate == term)) * 10
            + usize::from(category_terms.iter().any(|candidate| candidate == term)) * 5
            + usize::from(tag_terms.iter().any(|candidate| candidate == term)) * 3
    });
    Some(score)
}

fn search_terms(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
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
        let tools = self.registry.catalog(0, 1000, None).await;
        Ok(tools
            .into_iter()
            .filter(|entry| matches!(&entry.readiness, ToolReadiness::Live))
            .map(tool_catalog_entry_to_info)
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
        let mut live = Vec::with_capacity(limit);

        for definition in tools {
            if live.len() == limit {
                break;
            }
            let Some(tool) = self.registry.get(&definition.name).await else {
                continue;
            };
            if !matches!(&tool.readiness(), ToolReadiness::Live) {
                continue;
            }

            live.push(ToolInfo {
                name: definition.name,
                description: definition.description,
                input_schema: definition.input_schema,
                annotations: None,
            });
        }

        Ok(live)
    }
}

fn tool_catalog_entry_to_info(entry: ToolCatalogEntry) -> ToolInfo {
    ToolInfo {
        name: entry.definition.name,
        description: entry.definition.description,
        input_schema: entry.definition.input_schema,
        annotations: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use simd_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestTool {
        name: &'static str,
        description: &'static str,
        category: &'static str,
        readiness: ToolReadiness,
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

        fn readiness(&self) -> ToolReadiness {
            self.readiness.clone()
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
                readiness: ToolReadiness::Live,
            },
            TestTool {
                name: "alpha_query",
                description: "Search cognitive memory",
                category: "cognitive",
                readiness: ToolReadiness::Mock {
                    reason: "requires an external adapter".to_string(),
                },
            },
            TestTool {
                name: "beta_status",
                description: "Report current status",
                category: "operations",
                readiness: ToolReadiness::Live,
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

    #[tokio::test]
    async fn search_matches_human_phrases_against_snake_case_catalog_fields() {
        let registry = test_registry().await;

        let names: Vec<_> = registry
            .search("operational state")
            .await
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert_eq!(names, ["zeta_query"]);

        let names: Vec<_> = registry
            .search("zeta query")
            .await
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert_eq!(names, ["zeta_query"]);
    }

    #[test]
    fn search_ranking_prefers_operation_names_over_shared_agent_prose() {
        let definition = |name: &str| ToolDefinition {
            name: name.to_string(),
            description: "Backend architect specializing in scalable API design".to_string(),
            input_schema: json!({}),
            schema_version: String::new(),
            category: "agent".to_string(),
            tags: vec!["backend-architect".to_string()],
            namespace: "agents".to_string(),
        };
        let target = definition("agent_backend_architect_design_api");
        let broad = definition("agent_backend_architect_analyze");

        assert!(tool_search_score(&target, "design api") > tool_search_score(&broad, "design api"));
    }

    #[tokio::test]
    async fn catalog_carries_readiness_for_operator_surfaces() {
        let registry = test_registry().await;
        let catalog = registry.catalog(0, usize::MAX, None).await;

        assert_eq!(catalog[0].definition.name, "alpha_query");
        assert_eq!(catalog[0].readiness.status(), "mock");
        assert_eq!(
            catalog[0].readiness.reason(),
            Some("requires an external adapter")
        );
        assert_eq!(catalog[1].readiness.status(), "live");
        assert_eq!(catalog[1].readiness.reason(), None);
    }

    #[tokio::test]
    async fn mcp_executor_advertises_live_tools_only() {
        let executor = RegistryExecutor::new(Arc::new(test_registry().await));

        let listed: Vec<_> = executor
            .list_tools()
            .await
            .expect("list live tools")
            .into_iter()
            .map(|tool| tool.name)
            .collect();
        assert_eq!(listed, ["beta_status", "zeta_query"]);

        let searched: Vec<_> = executor
            .search_tools("query", 10)
            .await
            .expect("search live tools")
            .into_iter()
            .map(|tool| tool.name)
            .collect();
        assert_eq!(searched, ["zeta_query"]);
    }

    struct DisabledTool {
        executed: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Tool for DisabledTool {
        fn name(&self) -> &str {
            "disabled_tool"
        }

        fn description(&self) -> &str {
            "A tool which must never execute"
        }

        fn input_schema(&self) -> Value {
            json!({"type": "object"})
        }

        fn readiness(&self) -> ToolReadiness {
            ToolReadiness::Disabled {
                reason: "the backing service is unavailable".to_string(),
            }
        }

        async fn execute(&self, _input: Value) -> Result<Value> {
            self.executed.store(true, Ordering::SeqCst);
            Ok(json!({"unexpected": true}))
        }
    }

    #[tokio::test]
    async fn disabled_tools_are_rejected_before_execution() {
        let registry = ToolRegistry::new();
        let executed = Arc::new(AtomicBool::new(false));
        registry
            .register(Arc::new(DisabledTool {
                executed: executed.clone(),
            }))
            .await
            .expect("register disabled tool");

        let error = registry
            .execute("disabled_tool", json!({}))
            .await
            .expect_err("disabled tool must be rejected");

        assert_eq!(
            error.to_string(),
            "Tool 'disabled_tool' is disabled: the backing service is unavailable"
        );
        assert!(!executed.load(Ordering::SeqCst));
    }
}
