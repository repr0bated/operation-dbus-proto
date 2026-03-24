//! Tool trait and registry with extended built-in tools.
//!
//! Tools are the ONLY legal mechanism for state mutation.
//! A JSON-RPC request maps to exactly one tool invocation.

mod builtin;
pub mod dbus;
pub mod ovs;
pub mod incus;
pub mod dynamic;

pub use builtin::*;

use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::error::{OpDbusError, Result};
use op_execution_tracker::ExecutionRecord;
use op_state::EffectivePlugin;

/// Tool execution context
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Effective plugin configuration (core + resolved tunables)
    pub plugin: Arc<EffectivePlugin>,
    /// Policy ID governing this execution
    pub policy_id: String,
    /// Previous execution hash for chaining
    pub prev_hash: String,
    /// Caller identity/session
    pub caller_id: String,
    /// Session ID for tracking
    pub session_id: Option<String>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            plugin: Arc::new(EffectivePlugin::default()),
            policy_id: "default".to_string(),
            prev_hash: "genesis".to_string(),
            caller_id: "anonymous".to_string(),
            session_id: None,
        }
    }
}

/// Tool execution result
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub output: Value,
    pub execution_record: ExecutionRecord,
}

/// Tool trait - the only mutation mechanism in OP-DBUS
///
/// Every tool must:
/// - Have a unique name
/// - Provide a JSON schema for validation
/// - Execute deterministically
/// - Return a complete execution record
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name (e.g., "network.interface.create")
    fn name(&self) -> &str;

    /// JSON Schema for input validation
    fn input_schema(&self) -> Value;

    /// JSON Schema for output
    fn output_schema(&self) -> Value;

    /// Human-readable description
    fn description(&self) -> &str;

    /// Execute the tool
    ///
    /// Tools must:
    /// - Validate input against schema
    /// - Perform the mutation
    /// - Return complete output
    /// - Never perform side effects outside the contract
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult>;

    /// Validate input against schema (default implementation)
    fn validate_input(&self, input: &Value) -> Result<()> {
        // Basic validation - production would use jsonschema crate
        if input.is_null() && !self.input_schema().get("nullable").is_some() {
            // Allow null if schema permits
        }
        Ok(())
    }
}

/// Tool metadata for discovery and documentation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub plugin: String,
    pub version: String,
}

impl ToolMetadata {
    pub fn from_tool(tool: &dyn Tool, plugin: &str, version: &str) -> Self {
        Self {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            output_schema: tool.output_schema(),
            plugin: plugin.to_string(),
            version: version.to_string(),
        }
    }
}

/// Thread-safe tool registry
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn Tool>>,
    metadata: DashMap<String, ToolMetadata>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
            metadata: DashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(
        &self,
        tool: Arc<dyn Tool>,
        plugin: &str,
        version: &str,
    ) -> Result<()> {
        let name = tool.name().to_string();

        if self.tools.contains_key(&name) {
            return Err(OpDbusError::ToolAlreadyRegistered(name));
        }

        let metadata = ToolMetadata::from_tool(tool.as_ref(), plugin, version);
        self.metadata.insert(name.clone(), metadata);
        self.tools.insert(name, tool);

        Ok(())
    }

    /// Unregister a tool
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.metadata.remove(name);
        self.tools.remove(name).map(|(_, t)| t)
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).map(|t| Arc::clone(&t))
    }

    /// Get tool metadata
    pub fn get_metadata(&self, name: &str) -> Option<ToolMetadata> {
        self.metadata.get(name).map(|m| m.clone())
    }

    /// List all tool names
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.iter().map(|e| e.key().clone()).collect()
    }

    /// List all tools (returns Arc<dyn Tool>)
    pub fn list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.iter().map(|e| Arc::clone(e.value())).collect()
    }

    /// List all tool metadata
    pub fn list_metadata(&self) -> Vec<ToolMetadata> {
        self.metadata.iter().map(|e| e.value().clone()).collect()
    }

    /// Search tools by prefix or pattern
    pub fn search(&self, query: &str) -> Vec<ToolMetadata> {
        let query_lower = query.to_lowercase();
        self.metadata
            .iter()
            .filter(|e| {
                e.key().to_lowercase().contains(&query_lower)
                    || e.value().description.to_lowercase().contains(&query_lower)
            })
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get tool count
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
