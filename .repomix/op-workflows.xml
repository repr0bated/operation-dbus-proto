This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-workflows/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-workflows/
              src/
                builtin/
                  dbus_node.rs
                  definitions.rs
                  mod.rs
                  plugin_node.rs
                  tool_node.rs
                context.rs
                engine.rs
                flow.rs
                history.rs
                lib.rs
                node.rs
                orchestrator.rs
                workflows.rs
              Cargo.toml
              compare-op-workflows.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/builtin/dbus_node.rs">
//! D-Bus Method Node - Calls a D-Bus method as a workflow node

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;

use crate::node::{NodePort, NodeResult, NodeState, WorkflowNode};

/// A workflow node that calls a D-Bus method
pub struct DbusMethodNode {
    id: String,
    name: String,
    service: String,
    path: String,
    interface: String,
    method: String,
    state: NodeState,
}

impl DbusMethodNode {
    /// Create a new D-Bus method node
    pub fn new(id: &str, service: &str, path: &str, interface: &str, method: &str) -> Self {
        Self {
            id: id.to_string(),
            name: format!(
                "{}.{}",
                interface.split('.').last().unwrap_or(interface),
                method
            ),
            service: service.to_string(),
            path: path.to_string(),
            interface: interface.to_string(),
            method: method.to_string(),
            state: NodeState::Idle,
        }
    }
}

#[async_trait]
impl WorkflowNode for DbusMethodNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> &str {
        "dbus-method"
    }

    fn inputs(&self) -> Vec<NodePort> {
        vec![NodePort::optional("args", "Arguments", "array")
            .with_description("Arguments to pass to the D-Bus method")]
    }

    fn outputs(&self) -> Vec<NodePort> {
        vec![NodePort::required("result", "Result", "object")
            .with_description("Result from the D-Bus method call")]
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) {
        self.state = state;
    }

    async fn execute(&mut self, inputs: HashMap<String, Value>) -> Result<NodeResult> {
        let start = std::time::Instant::now();
        let args = inputs.get("args").cloned().unwrap_or(json!([]));

        // In a real implementation, this would call the D-Bus method
        // For now, return mock data
        let mut outputs = HashMap::new();
        outputs.insert(
            "result".to_string(),
            json!({
                "service": self.service,
                "path": self.path,
                "interface": self.interface,
                "method": self.method,
                "args": args,
                "response": null,
                "success": true
            }),
        );

        Ok(NodeResult::success(outputs).with_duration(start.elapsed().as_millis() as u64))
    }

    fn config_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "D-Bus service name",
                    "default": self.service
                },
                "path": {
                    "type": "string",
                    "description": "D-Bus object path",
                    "default": self.path
                },
                "interface": {
                    "type": "string",
                    "description": "D-Bus interface name",
                    "default": self.interface
                },
                "method": {
                    "type": "string",
                    "description": "D-Bus method name",
                    "default": self.method
                }
            },
            "required": ["service", "path", "interface", "method"]
        })
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/builtin/definitions.rs">
//! Built-in Workflow Definitions
//!
//! Pre-defined workflows for common operations.

use crate::flow::{WorkflowDefinition, WorkflowNodeDef};
use crate::node::NodeConnection;
use simd_json::json;

/// Get all built-in workflow definitions
pub fn builtin_workflows() -> Vec<WorkflowDefinition> {
    vec![
        cargo_check_workflow(),
        service_status_workflow(),
        deploy_workflow(),
        code_review_workflow(),
    ]
}

/// Cargo check workflow
fn cargo_check_workflow() -> WorkflowDefinition {
    WorkflowDefinition::new(
        "cargo_check",
        "Cargo Check",
        "Run cargo check, clippy, and format",
    )
    .with_node(WorkflowNodeDef {
        id: "check".into(),
        node_type: "tool:cargo_check".into(),
        name: "Cargo Check".into(),
        config: json!({"path": "."}),
        position: Some((100.0, 100.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "clippy".into(),
        node_type: "tool:cargo_clippy".into(),
        name: "Cargo Clippy".into(),
        config: json!({"path": ".", "fix": false}),
        position: Some((300.0, 100.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "format".into(),
        node_type: "tool:cargo_fmt".into(),
        name: "Cargo Format".into(),
        config: json!({"path": ".", "check": true}),
        position: Some((500.0, 100.0)),
    })
    .with_connection(NodeConnection::new("check", "result", "clippy", "source"))
    .with_connection(NodeConnection::new("clippy", "result", "format", "source"))
}

/// Service status workflow
fn service_status_workflow() -> WorkflowDefinition {
    WorkflowDefinition::new(
        "service_status",
        "Service Status",
        "Check status of system services",
    )
    .with_node(WorkflowNodeDef {
        id: "list_units".into(),
        node_type: "tool:systemd_list_units".into(),
        name: "List Units".into(),
        config: json!({"pattern": "*.service"}),
        position: Some((100.0, 100.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "filter_failed".into(),
        node_type: "tool:filter".into(),
        name: "Filter Failed".into(),
        config: json!({"field": "active_state", "value": "failed"}),
        position: Some((300.0, 100.0)),
    })
    .with_connection(NodeConnection::new(
        "list_units",
        "units",
        "filter_failed",
        "input",
    ))
}

/// Deployment workflow
fn deploy_workflow() -> WorkflowDefinition {
    WorkflowDefinition::new(
        "deploy",
        "Deploy Application",
        "Build, test, and deploy application",
    )
    .with_node(WorkflowNodeDef {
        id: "build".into(),
        node_type: "tool:cargo_build".into(),
        name: "Build".into(),
        config: json!({"release": true}),
        position: Some((100.0, 100.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "test".into(),
        node_type: "tool:cargo_test".into(),
        name: "Test".into(),
        config: json!({}),
        position: Some((300.0, 100.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "deploy".into(),
        node_type: "tool:deploy".into(),
        name: "Deploy".into(),
        config: json!({"target": "production"}),
        position: Some((500.0, 100.0)),
    })
    .with_connection(NodeConnection::new("build", "binary", "test", "source"))
    .with_connection(NodeConnection::new("test", "result", "deploy", "artifact"))
}

/// Code review workflow
fn code_review_workflow() -> WorkflowDefinition {
    WorkflowDefinition::new(
        "code_review",
        "Code Review",
        "Multi-perspective code review",
    )
    .with_node(WorkflowNodeDef {
        id: "security".into(),
        node_type: "agent:security_reviewer".into(),
        name: "Security Review".into(),
        config: json!({"focus": "security"}),
        position: Some((100.0, 50.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "architecture".into(),
        node_type: "agent:architect".into(),
        name: "Architecture Review".into(),
        config: json!({"focus": "design"}),
        position: Some((100.0, 150.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "performance".into(),
        node_type: "agent:performance_analyst".into(),
        name: "Performance Review".into(),
        config: json!({"focus": "performance"}),
        position: Some((100.0, 250.0)),
    })
    .with_node(WorkflowNodeDef {
        id: "consolidate".into(),
        node_type: "merge".into(),
        name: "Consolidate".into(),
        config: json!({}),
        position: Some((300.0, 150.0)),
    })
    .with_connection(NodeConnection::new(
        "security",
        "findings",
        "consolidate",
        "security",
    ))
    .with_connection(NodeConnection::new(
        "architecture",
        "findings",
        "consolidate",
        "architecture",
    ))
    .with_connection(NodeConnection::new(
        "performance",
        "findings",
        "consolidate",
        "performance",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_workflows_valid() {
        let workflows = builtin_workflows();
        assert!(!workflows.is_empty());

        for wf in workflows {
            assert!(wf.validate().is_ok(), "Workflow '{}' is invalid", wf.id);
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/builtin/mod.rs">
//! Built-in workflow nodes
//!
//! Provides standard nodes for common operations:
//! - Plugin nodes (wrap StatePlugins)
//! - D-Bus nodes (call D-Bus methods)
//! - Tool nodes (execute tools)
//! - Control flow nodes (conditions, loops)

pub mod dbus_node;
pub mod definitions;
pub mod plugin_node;
pub mod tool_node;

pub use dbus_node::DbusMethodNode;
pub use definitions::builtin_workflows;
pub use plugin_node::PluginNode;
pub use tool_node::ToolNode;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

use crate::node::{NodePort, NodeResult, NodeState, WorkflowNode};

/// Log node - logs a message
pub struct LogNode {
    id: String,
    name: String,
    message: String,
    state: NodeState,
}

impl LogNode {
    pub fn new(id: &str, message: &str) -> Self {
        Self {
            id: id.to_string(),
            name: id.to_string(),
            message: message.to_string(),
            state: NodeState::Idle,
        }
    }
}

#[async_trait]
impl WorkflowNode for LogNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn node_type(&self) -> &str {
        "log"
    }

    fn inputs(&self) -> Vec<NodePort> {
        Vec::new()
    }

    fn outputs(&self) -> Vec<NodePort> {
        vec![
            NodePort::optional("logged", "Logged", "boolean"),
            NodePort::optional("message", "Message", "string"),
        ]
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) {
        self.state = state;
    }

    async fn execute(&mut self, _inputs: HashMap<String, Value>) -> Result<NodeResult> {
        tracing::info!("{}", self.message);
        let mut outputs = HashMap::new();
        outputs.insert("logged".to_string(), Value::from(true));
        outputs.insert("message".to_string(), Value::from(self.message.clone()));
        Ok(NodeResult::success(outputs))
    }
}

/// Delay node - waits for a duration
pub struct DelayNode {
    id: String,
    name: String,
    duration_ms: u64,
    state: NodeState,
}

impl DelayNode {
    pub fn new(id: &str, duration_ms: u64) -> Self {
        Self {
            id: id.to_string(),
            name: id.to_string(),
            duration_ms,
            state: NodeState::Idle,
        }
    }
}

#[async_trait]
impl WorkflowNode for DelayNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn node_type(&self) -> &str {
        "delay"
    }

    fn inputs(&self) -> Vec<NodePort> {
        Vec::new()
    }

    fn outputs(&self) -> Vec<NodePort> {
        vec![NodePort::optional("delayed_ms", "Delayed Ms", "number")]
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) {
        self.state = state;
    }

    async fn execute(&mut self, _inputs: HashMap<String, Value>) -> Result<NodeResult> {
        tokio::time::sleep(std::time::Duration::from_millis(self.duration_ms)).await;
        let mut outputs = HashMap::new();
        outputs.insert("slept".to_string(), Value::from(self.duration_ms));
        Ok(NodeResult::success(outputs))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/builtin/plugin_node.rs">
//! Plugin Node - Wraps a StatePlugin as a workflow node
//!
//! Converts StatePlugins into workflow nodes with:
//! - Input: desired_state
//! - Outputs: current_state, diff, apply_result

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;

use crate::node::{NodePort, NodeResult, NodeState, WorkflowNode};

/// A workflow node that wraps a StatePlugin
pub struct PluginNode {
    id: String,
    name: String,
    plugin_name: String,
    state: NodeState,
    operation: PluginOperation,
}

/// Operation to perform on the plugin
#[derive(Debug, Clone, Copy)]
pub enum PluginOperation {
    /// Query current state
    Query,
    /// Calculate diff between current and desired
    Diff,
    /// Apply desired state
    Apply,
}

impl PluginNode {
    /// Create a new plugin node
    pub fn new(id: &str, plugin_name: &str, operation: PluginOperation) -> Self {
        let op_name = match operation {
            PluginOperation::Query => "Query",
            PluginOperation::Diff => "Diff",
            PluginOperation::Apply => "Apply",
        };

        Self {
            id: id.to_string(),
            name: format!("{} {}", plugin_name, op_name),
            plugin_name: plugin_name.to_string(),
            state: NodeState::Idle,
            operation,
        }
    }
}

#[async_trait]
impl WorkflowNode for PluginNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> &str {
        "plugin"
    }

    fn inputs(&self) -> Vec<NodePort> {
        match self.operation {
            PluginOperation::Query => vec![],
            PluginOperation::Diff | PluginOperation::Apply => {
                vec![
                    NodePort::optional("desired_state", "Desired State", "object")
                        .with_description("The desired state to diff/apply"),
                ]
            }
        }
    }

    fn outputs(&self) -> Vec<NodePort> {
        match self.operation {
            PluginOperation::Query => {
                vec![
                    NodePort::required("current_state", "Current State", "object")
                        .with_description("The current system state"),
                ]
            }
            PluginOperation::Diff => vec![NodePort::required("diff", "State Diff", "object")
                .with_description("Difference between current and desired state")],
            PluginOperation::Apply => vec![NodePort::required("result", "Apply Result", "object")
                .with_description("Result of applying the state")],
        }
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) {
        self.state = state;
    }

    async fn execute(&mut self, inputs: HashMap<String, Value>) -> Result<NodeResult> {
        let start = std::time::Instant::now();

        // In a real implementation, this would call the actual plugin
        // For now, return mock data
        let result = match self.operation {
            PluginOperation::Query => {
                let mut outputs = HashMap::new();
                outputs.insert(
                    "current_state".to_string(),
                    json!({
                        "plugin": self.plugin_name,
                        "state": "queried",
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }),
                );
                NodeResult::success(outputs)
            }
            PluginOperation::Diff => {
                let desired = inputs.get("desired_state").cloned().unwrap_or(json!({}));
                let mut outputs = HashMap::new();
                outputs.insert(
                    "diff".to_string(),
                    json!({
                        "plugin": self.plugin_name,
                        "desired": desired,
                        "changes": [],
                        "has_changes": false
                    }),
                );
                NodeResult::success(outputs)
            }
            PluginOperation::Apply => {
                let mut outputs = HashMap::new();
                outputs.insert(
                    "result".to_string(),
                    json!({
                        "plugin": self.plugin_name,
                        "applied": true,
                        "changes_made": 0
                    }),
                );
                NodeResult::success(outputs)
            }
        };

        Ok(result.with_duration(start.elapsed().as_millis() as u64))
    }

    fn config_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "plugin_name": {
                    "type": "string",
                    "description": "Name of the plugin",
                    "default": self.plugin_name
                },
                "operation": {
                    "type": "string",
                    "enum": ["query", "diff", "apply"],
                    "description": "Operation to perform"
                }
            }
        })
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/builtin/tool_node.rs">
//! Tool Node - Executes a tool as a workflow node

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;

use crate::node::{NodePort, NodeResult, NodeState, WorkflowNode};

/// A workflow node that executes a tool
pub struct ToolNode {
    id: String,
    name: String,
    tool_name: String,
    state: NodeState,
}

impl ToolNode {
    /// Create a new tool node
    pub fn new(id: &str, tool_name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: format!("Tool: {}", tool_name),
            tool_name: tool_name.to_string(),
            state: NodeState::Idle,
        }
    }
}

#[async_trait]
impl WorkflowNode for ToolNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> &str {
        "tool"
    }

    fn inputs(&self) -> Vec<NodePort> {
        vec![NodePort::optional("arguments", "Arguments", "object")
            .with_description("Arguments to pass to the tool")]
    }

    fn outputs(&self) -> Vec<NodePort> {
        vec![NodePort::required("result", "Result", "object")
            .with_description("Result from tool execution")]
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) {
        self.state = state;
    }

    async fn execute(&mut self, inputs: HashMap<String, Value>) -> Result<NodeResult> {
        let start = std::time::Instant::now();
        let arguments = inputs.get("arguments").cloned().unwrap_or(json!({}));

        // In a real implementation, this would execute the tool via ToolRegistry
        // For now, return mock data
        let mut outputs = HashMap::new();
        outputs.insert(
            "result".to_string(),
            json!({
                "tool": self.tool_name,
                "arguments": arguments,
                "output": null,
                "success": true
            }),
        );

        Ok(NodeResult::success(outputs).with_duration(start.elapsed().as_millis() as u64))
    }

    fn config_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool to execute",
                    "default": self.tool_name
                }
            },
            "required": ["tool_name"]
        })
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/context.rs">
//! Workflow Context - Execution context for workflows
//!
//! Provides access to:
//! - Variables and state
//! - Plugin instances
//! - Tool execution
//! - Logging and metrics

use anyhow::Result;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Workflow execution context
pub struct WorkflowContext {
    /// Workflow ID
    pub workflow_id: String,
    /// Execution ID (unique per run)
    pub execution_id: String,
    /// Variables available during execution
    pub variables: Arc<RwLock<HashMap<String, Value>>>,
    /// Execution log
    log: Arc<RwLock<Vec<LogEntry>>>,
}

/// Log entry for workflow execution
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogLevel,
    pub node_id: Option<String>,
    pub message: String,
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl WorkflowContext {
    /// Create a new workflow context
    pub fn new(workflow_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            execution_id: uuid::Uuid::new_v4().to_string(),
            variables: Arc::new(RwLock::new(HashMap::new())),
            log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get a variable
    pub async fn get_variable(&self, name: &str) -> Option<Value> {
        let vars = self.variables.read().await;
        vars.get(name).cloned()
    }

    /// Set a variable
    pub async fn set_variable(&self, name: &str, value: Value) {
        let mut vars = self.variables.write().await;
        vars.insert(name.to_string(), value);
    }

    /// Get all variables
    pub async fn get_all_variables(&self) -> HashMap<String, Value> {
        let vars = self.variables.read().await;
        vars.clone()
    }

    /// Log a message
    pub async fn log(&self, level: LogLevel, node_id: Option<&str>, message: &str) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            node_id: node_id.map(String::from),
            message: message.to_string(),
        };
        let mut log = self.log.write().await;
        log.push(entry);
    }

    /// Log debug message
    pub async fn debug(&self, node_id: Option<&str>, message: &str) {
        self.log(LogLevel::Debug, node_id, message).await;
    }

    /// Log info message
    pub async fn info(&self, node_id: Option<&str>, message: &str) {
        self.log(LogLevel::Info, node_id, message).await;
    }

    /// Log warning message
    pub async fn warn(&self, node_id: Option<&str>, message: &str) {
        self.log(LogLevel::Warn, node_id, message).await;
    }

    /// Log error message
    pub async fn error(&self, node_id: Option<&str>, message: &str) {
        self.log(LogLevel::Error, node_id, message).await;
    }

    /// Get execution log
    pub async fn get_log(&self) -> Vec<LogEntry> {
        let log = self.log.read().await;
        log.clone()
    }

    /// Interpolate variables in a string
    pub async fn interpolate(&self, template: &str) -> String {
        let vars = self.variables.read().await;
        let mut result = template.to_string();

        for (name, value) in vars.iter() {
            let pattern = format!("${{{}}}", name);
            let replacement = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&pattern, &replacement);
        }

        result
    }

    /// Interpolate variables in a JSON value
    pub async fn interpolate_value(&self, value: &Value) -> Value {
        match value {
            Value::String(s) => Value::String(self.interpolate(s).await),
            Value::Object(obj) => {
                let mut new_obj = simd_json::value::owned::Object::new();
                for (k, v) in obj.iter() {
                    new_obj.insert(k.clone(), Box::pin(self.interpolate_value(v)).await);
                }
                Value::Object(Box::new(new_obj))
            }
            Value::Array(arr) => {
                let mut new_arr = Vec::new();
                for v in arr {
                    new_arr.push(Box::pin(self.interpolate_value(v)).await);
                }
                Value::Array(new_arr)
            }
            other => other.clone(),
        }
    }
}

impl Default for WorkflowContext {
    fn default() -> Self {
        Self::new("default")
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/engine.rs">
//! Workflow Engine - Executes workflow graphs
//!
//! The engine manages workflow execution:
//! - Determines execution order based on dependencies
//! - Manages parallel execution of independent nodes
//! - Handles errors and retries
//! - Collects results

use anyhow::Result;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::context::WorkflowContext;
use crate::flow::{Workflow, WorkflowDefinition, WorkflowState};
use crate::node::{NodeResult, NodeState, WorkflowNode};

/// Workflow execution result
#[derive(Debug, Clone)]
pub struct WorkflowExecutionResult {
    /// Whether workflow completed successfully
    pub success: bool,
    /// Workflow outputs
    pub outputs: HashMap<String, Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Node execution results
    pub node_results: HashMap<String, NodeResult>,
}

/// Factory for creating node instances from definitions
pub trait NodeFactory: Send + Sync {
    /// Create a node instance from a node type and config
    fn create_node(
        &self,
        node_type: &str,
        node_id: &str,
        config: &Value,
    ) -> Result<Box<dyn WorkflowNode>>;
}

/// Workflow Engine - Executes workflows
pub struct WorkflowEngine {
    /// Registered workflow definitions
    definitions: Arc<RwLock<HashMap<String, WorkflowDefinition>>>,
    /// Node factory for creating node instances
    node_factory: Arc<dyn NodeFactory>,
    /// Maximum parallel nodes
    max_parallel: usize,
}

impl WorkflowEngine {
    /// Create a new workflow engine
    pub fn new(node_factory: Arc<dyn NodeFactory>) -> Self {
        Self {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            node_factory,
            max_parallel: 10,
        }
    }

    /// Set maximum parallel node executions
    pub fn with_max_parallel(mut self, max: usize) -> Self {
        self.max_parallel = max;
        self
    }

    /// Register a workflow definition
    pub async fn register(&self, definition: WorkflowDefinition) -> Result<()> {
        definition.validate()?;
        let mut defs = self.definitions.write().await;
        info!(workflow_id = %definition.id, "Registering workflow");
        defs.insert(definition.id.clone(), definition);
        Ok(())
    }

    /// Get a workflow definition
    pub async fn get_definition(&self, workflow_id: &str) -> Option<WorkflowDefinition> {
        let defs = self.definitions.read().await;
        defs.get(workflow_id).cloned()
    }

    /// List all workflow definitions
    pub async fn list_definitions(&self) -> Vec<WorkflowDefinition> {
        let defs = self.definitions.read().await;
        defs.values().cloned().collect()
    }

    /// Execute a workflow by ID
    pub async fn execute(
        &self,
        workflow_id: &str,
        inputs: HashMap<String, Value>,
    ) -> Result<WorkflowExecutionResult> {
        let definition = self
            .get_definition(workflow_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Workflow not found: {}", workflow_id))?;

        self.execute_definition(definition, inputs).await
    }

    /// Execute a workflow definition
    pub async fn execute_definition(
        &self,
        definition: WorkflowDefinition,
        inputs: HashMap<String, Value>,
    ) -> Result<WorkflowExecutionResult> {
        let start = std::time::Instant::now();
        let workflow_id = definition.id.clone();

        info!(workflow_id = %workflow_id, "Starting workflow execution");

        // Create workflow instance
        let mut workflow = Workflow::new(definition.clone());
        workflow.state = WorkflowState::Running;

        // Set initial variables from inputs
        for (key, value) in inputs {
            workflow.variables.insert(key, value);
        }

        // Create node instances
        let mut nodes: HashMap<String, Box<dyn WorkflowNode>> = HashMap::new();
        for node_def in &definition.nodes {
            match self
                .node_factory
                .create_node(&node_def.node_type, &node_def.id, &node_def.config)
            {
                Ok(node) => {
                    nodes.insert(node_def.id.clone(), node);
                }
                Err(e) => {
                    error!(node_id = %node_def.id, error = %e, "Failed to create node");
                    return Ok(WorkflowExecutionResult {
                        success: false,
                        outputs: HashMap::new(),
                        error: Some(format!("Failed to create node '{}': {}", node_def.id, e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        node_results: HashMap::new(),
                    });
                }
            }
        }

        let mut node_results: HashMap<String, NodeResult> = HashMap::new();

        // Execute nodes in dependency order
        loop {
            // Check for completion
            if workflow.is_complete() {
                workflow.state = WorkflowState::Completed;
                break;
            }

            // Check for failure
            if workflow.has_failed() {
                workflow.state = WorkflowState::Failed;
                break;
            }

            // Get ready nodes
            let ready_nodes = workflow.get_ready_nodes();
            if ready_nodes.is_empty() {
                // No nodes ready but not complete - deadlock or all failed
                warn!(workflow_id = %workflow_id, "No nodes ready to execute");
                workflow.state = WorkflowState::Failed;
                break;
            }

            // Execute ready nodes (in parallel up to max_parallel)
            let batch: Vec<_> = ready_nodes.into_iter().take(self.max_parallel).collect();

            for node_id in batch {
                debug!(workflow_id = %workflow_id, node_id = %node_id, "Executing node");

                // Get inputs for this node
                let node_inputs = workflow.get_node_inputs(&node_id);

                // Get node instance
                if let Some(node) = nodes.get_mut(&node_id) {
                    // Update state
                    node.set_state(NodeState::Running);
                    workflow
                        .node_states
                        .insert(node_id.clone(), NodeState::Running);

                    // Execute
                    match node.execute(node_inputs).await {
                        Ok(result) => {
                            if result.success {
                                workflow.complete_node(&node_id, result.outputs.clone());
                                node.set_state(NodeState::Completed);
                            } else {
                                let error = result.error.clone().unwrap_or_default();
                                workflow.fail_node(&node_id, &error);
                                node.set_state(NodeState::Failed);
                            }
                            node_results.insert(node_id.clone(), result);
                        }
                        Err(e) => {
                            error!(node_id = %node_id, error = %e, "Node execution error");
                            workflow.fail_node(&node_id, &e.to_string());
                            node.set_state(NodeState::Failed);
                            node_results
                                .insert(node_id.clone(), NodeResult::failure(e.to_string()));
                        }
                    }
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let success = workflow.state == WorkflowState::Completed;

        info!(
            workflow_id = %workflow_id,
            success = success,
            duration_ms = duration_ms,
            "Workflow execution complete"
        );

        Ok(WorkflowExecutionResult {
            success,
            outputs: workflow.get_outputs(),
            error: if success {
                None
            } else {
                Some("Workflow execution failed".to_string())
            },
            duration_ms,
            node_results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockNodeFactory;

    impl NodeFactory for MockNodeFactory {
        fn create_node(
            &self,
            _node_type: &str,
            _node_id: &str,
            _config: &Value,
        ) -> Result<Box<dyn WorkflowNode>> {
            Err(anyhow::anyhow!("Mock factory"))
        }
    }

    #[tokio::test]
    async fn test_workflow_registration() {
        let factory = Arc::new(MockNodeFactory);
        let engine = WorkflowEngine::new(factory);

        let def = WorkflowDefinition::new("test", "Test Workflow", "A test workflow");
        engine.register(def).await.unwrap();

        assert!(engine.get_definition("test").await.is_some());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/flow.rs">
//! Workflow Flow - Graph of connected nodes
//!
//! A Workflow is a directed graph of nodes connected by edges.
//! Data flows from output ports to input ports.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

use crate::node::{NodeConnection, NodePort, NodeResult, NodeState};

/// Workflow definition (serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Category for organization
    pub category: String,
    /// Node definitions
    pub nodes: Vec<WorkflowNodeDef>,
    /// Connections between nodes
    pub connections: Vec<NodeConnection>,
    /// Input parameters for the workflow
    pub inputs: Vec<NodePort>,
    /// Output parameters from the workflow
    pub outputs: Vec<NodePort>,
    /// Tags for discovery
    pub tags: Vec<String>,
    /// Version
    pub version: String,
}

/// Node definition within a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNodeDef {
    /// Node ID (unique within workflow)
    pub id: String,
    /// Node type (e.g., "plugin:systemd", "dbus:org.freedesktop.systemd1")
    pub node_type: String,
    /// Display name
    pub name: String,
    /// Configuration for this node instance
    pub config: Value,
    /// Position for visual layout (optional)
    pub position: Option<(f32, f32)>,
}

/// Runtime workflow instance
pub struct Workflow {
    /// Definition
    pub definition: WorkflowDefinition,
    /// Current state of each node
    pub node_states: HashMap<String, NodeState>,
    /// Collected outputs from completed nodes
    pub node_outputs: HashMap<String, HashMap<String, Value>>,
    /// Workflow-level variables
    pub variables: HashMap<String, Value>,
    /// Overall workflow state
    pub state: WorkflowState,
}

/// Overall workflow state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    /// Not started
    Idle,
    /// Currently executing
    Running,
    /// Paused
    Paused,
    /// Completed successfully
    Completed,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
}

impl Default for WorkflowState {
    fn default() -> Self {
        Self::Idle
    }
}

impl Workflow {
    /// Create a new workflow from definition
    pub fn new(definition: WorkflowDefinition) -> Self {
        let mut node_states = HashMap::new();
        for node in &definition.nodes {
            node_states.insert(node.id.clone(), NodeState::Idle);
        }

        Self {
            definition,
            node_states,
            node_outputs: HashMap::new(),
            variables: HashMap::new(),
            state: WorkflowState::Idle,
        }
    }

    /// Get nodes that are ready to execute (all inputs satisfied)
    pub fn get_ready_nodes(&self) -> Vec<String> {
        let mut ready = Vec::new();

        for node_def in &self.definition.nodes {
            // Skip if not idle
            if self.node_states.get(&node_def.id) != Some(&NodeState::Idle) {
                continue;
            }

            // Check if all input connections are satisfied
            let inputs_satisfied = self.are_inputs_satisfied(&node_def.id);
            if inputs_satisfied {
                ready.push(node_def.id.clone());
            }
        }

        ready
    }

    /// Check if all inputs for a node are satisfied
    fn are_inputs_satisfied(&self, node_id: &str) -> bool {
        // Find all connections targeting this node
        for conn in &self.definition.connections {
            if conn.to_node == node_id {
                // Check if source node has completed
                if self.node_states.get(&conn.from_node) != Some(&NodeState::Completed) {
                    return false;
                }
                // Check if source output exists
                if let Some(outputs) = self.node_outputs.get(&conn.from_node) {
                    if !outputs.contains_key(&conn.from_port) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
        true
    }

    /// Get inputs for a node from connected outputs
    pub fn get_node_inputs(&self, node_id: &str) -> HashMap<String, Value> {
        let mut inputs = HashMap::new();

        for conn in &self.definition.connections {
            if conn.to_node == node_id {
                if let Some(outputs) = self.node_outputs.get(&conn.from_node) {
                    if let Some(value) = outputs.get(&conn.from_port) {
                        inputs.insert(conn.to_port.clone(), value.clone());
                    }
                }
            }
        }

        inputs
    }

    /// Mark a node as completed with outputs
    pub fn complete_node(&mut self, node_id: &str, outputs: HashMap<String, Value>) {
        self.node_states
            .insert(node_id.to_string(), NodeState::Completed);
        self.node_outputs.insert(node_id.to_string(), outputs);
    }

    /// Mark a node as failed
    pub fn fail_node(&mut self, node_id: &str, _error: &str) {
        self.node_states
            .insert(node_id.to_string(), NodeState::Failed);
    }

    /// Check if workflow is complete
    pub fn is_complete(&self) -> bool {
        self.definition.nodes.iter().all(|n| {
            matches!(
                self.node_states.get(&n.id),
                Some(NodeState::Completed) | Some(NodeState::Skipped)
            )
        })
    }

    /// Check if workflow has failed
    pub fn has_failed(&self) -> bool {
        self.definition
            .nodes
            .iter()
            .any(|n| self.node_states.get(&n.id) == Some(&NodeState::Failed))
    }

    /// Get workflow outputs (from designated output nodes)
    pub fn get_outputs(&self) -> HashMap<String, Value> {
        // Collect outputs from nodes that connect to workflow outputs
        let mut result = HashMap::new();

        // For now, collect all outputs from all completed nodes
        for (node_id, outputs) in &self.node_outputs {
            for (port_id, value) in outputs {
                result.insert(format!("{}.{}", node_id, port_id), value.clone());
            }
        }

        result
    }
}

impl WorkflowDefinition {
    /// Create a new workflow definition
    pub fn new(id: &str, name: &str, description: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            category: "general".to_string(),
            nodes: Vec::new(),
            connections: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            tags: Vec::new(),
            version: "1.0.0".to_string(),
        }
    }

    /// Add a node
    pub fn with_node(mut self, node: WorkflowNodeDef) -> Self {
        self.nodes.push(node);
        self
    }

    /// Add a connection
    pub fn with_connection(mut self, conn: NodeConnection) -> Self {
        self.connections.push(conn);
        self
    }

    /// Validate the workflow definition
    pub fn validate(&self) -> Result<()> {
        // Check for duplicate node IDs
        let mut seen_ids = std::collections::HashSet::new();
        for node in &self.nodes {
            if !seen_ids.insert(&node.id) {
                return Err(anyhow::anyhow!("Duplicate node ID: {}", node.id));
            }
        }

        // Check connections reference valid nodes
        for conn in &self.connections {
            if !seen_ids.contains(&conn.from_node) {
                return Err(anyhow::anyhow!(
                    "Connection references unknown source node: {}",
                    conn.from_node
                ));
            }
            if !seen_ids.contains(&conn.to_node) {
                return Err(anyhow::anyhow!(
                    "Connection references unknown target node: {}",
                    conn.to_node
                ));
            }
        }

        // Check for cycles (simple DFS)
        // TODO: Implement proper cycle detection

        Ok(())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/history.rs">
//! Workflow History - Durable Event Log
//!
//! Implements the Event Sourcing pattern for workflows.
//! Every state change is recorded as an immutable event.
//! Replaying these events reconstructs the workflow state.

use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single event in the workflow history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    /// Incremental event ID (1, 2, 3...)
    pub event_id: u64,
    /// Timestamp (UTC)
    pub timestamp: u64,
    /// The type of event and its data
    pub event_type: EventType,
}

impl HistoryEvent {
    pub fn new(event_id: u64, event_type: EventType) -> Self {
        Self {
            event_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            event_type,
        }
    }
}

/// Types of events that can occur in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Workflow execution started
    WorkflowExecutionStarted {
        workflow_type: String,
        workflow_id: String,
        inputs: Value,
    },
    /// Workflow execution completed
    WorkflowExecutionCompleted { result: Value },
    /// Workflow execution failed
    WorkflowExecutionFailed {
        error: String,
        details: Option<String>,
    },

    /// A node (task) was scheduled
    NodeTaskScheduled {
        node_id: String,
        node_type: String,
        inputs: Value,
    },
    /// A node task started execution (worker picked it up)
    NodeTaskStarted { node_id: String, attempt: u32 },
    /// A node task completed successfully
    NodeTaskCompleted { node_id: String, result: Value },
    /// A node task failed
    NodeTaskFailed {
        node_id: String,
        error: String,
        retryable: bool,
    },

    /// A timer was started
    TimerStarted {
        timer_id: String,
        duration_secs: u64,
    },
    /// A timer fired
    TimerFired { timer_id: String },

    /// A signal was received (external event)
    SignalReceived { signal_name: String, payload: Value },

    /// A marker recorded by the workflow (custom data)
    MarkerRecorded { marker_name: String, details: Value },
}

/// The full history of a workflow execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowHistory {
    pub events: Vec<HistoryEvent>,
}

impl WorkflowHistory {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add an event to the history
    pub fn add(&mut self, event_type: EventType) {
        let event_id = (self.events.len() as u64) + 1;
        self.events.push(HistoryEvent::new(event_id, event_type));
    }

    /// Get the last event ID
    pub fn last_event_id(&self) -> u64 {
        self.events.len() as u64
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/lib.rs">
//! op-workflows: Workflow engine with plugin/service nodes
//!
//! Features:
//! - PocketFlow-style flow-based programming
//! - Plugins and services as workflow nodes
//! - State transitions and event-driven execution
//! - Parallel and sequential execution modes

pub mod builtin;
pub mod context;
pub mod engine;
pub mod flow;
pub mod history;
pub mod node;
pub mod orchestrator;
pub mod workflows;

pub use orchestrator::{
    Orchestrator, OrchestratorConfig, OrchestratorStats, StepResult, WorkflowResult,
};

pub use context::WorkflowContext;
pub use engine::WorkflowEngine;
pub use flow::{Workflow, WorkflowDefinition, WorkflowState};
pub use node::{NodeConnection, NodePort, NodeResult, NodeState, WorkflowNode};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::context::WorkflowContext;
    pub use super::engine::WorkflowEngine;
    pub use super::flow::{Workflow, WorkflowDefinition, WorkflowState};
    pub use super::node::{NodeConnection, NodePort, NodeResult, NodeState, WorkflowNode};
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/node.rs">
//! Workflow Node - Plugin/Service as a workflow node
//!
//! Nodes are the fundamental building blocks of workflows.
//! Each node represents a plugin, service, or D-Bus method that can:
//! - Receive data through input ports
//! - Execute some operation
//! - Produce data through output ports

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// State of a workflow node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    /// Node is idle, waiting to be executed
    Idle,
    /// Node is waiting for input data
    WaitingForInput,
    /// Node is currently executing
    Running,
    /// Node completed successfully
    Completed,
    /// Node failed
    Failed,
    /// Node was skipped (condition not met)
    Skipped,
}

impl Default for NodeState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Result of node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Output data keyed by port name
    pub outputs: HashMap<String, Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Additional metadata
    pub metadata: HashMap<String, Value>,
}

impl NodeResult {
    /// Create a successful result
    pub fn success(outputs: HashMap<String, Value>) -> Self {
        Self {
            success: true,
            outputs,
            error: None,
            duration_ms: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            outputs: HashMap::new(),
            error: Some(error.into()),
            duration_ms: 0,
            metadata: HashMap::new(),
        }
    }

    /// Set duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }
}

/// A port on a workflow node (input or output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePort {
    /// Port identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Data type (e.g., "string", "number", "object", "state")
    pub data_type: String,
    /// Whether this port is required
    pub required: bool,
    /// Description
    pub description: Option<String>,
    /// Default value if not connected
    pub default_value: Option<Value>,
}

impl NodePort {
    /// Create a new required port
    pub fn required(id: &str, name: &str, data_type: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            data_type: data_type.to_string(),
            required: true,
            description: None,
            default_value: None,
        }
    }

    /// Create a new optional port
    pub fn optional(id: &str, name: &str, data_type: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            data_type: data_type.to_string(),
            required: false,
            description: None,
            default_value: None,
        }
    }

    /// Add description
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// Add default value
    pub fn with_default(mut self, value: Value) -> Self {
        self.default_value = Some(value);
        self
    }
}

/// Trait for workflow nodes
#[async_trait]
pub trait WorkflowNode: Send + Sync {
    /// Get the node's unique identifier
    fn id(&self) -> &str;

    /// Get the node's display name
    fn name(&self) -> &str;

    /// Get the node type (plugin, service, dbus-method, etc.)
    fn node_type(&self) -> &str;

    /// Get input ports
    fn inputs(&self) -> Vec<NodePort>;

    /// Get output ports
    fn outputs(&self) -> Vec<NodePort>;

    /// Get current state
    fn state(&self) -> NodeState;

    /// Set state
    fn set_state(&mut self, state: NodeState);

    /// Execute the node with given inputs
    async fn execute(&mut self, inputs: HashMap<String, Value>) -> Result<NodeResult>;

    /// Validate inputs before execution
    fn validate_inputs(&self, inputs: &HashMap<String, Value>) -> Result<()> {
        for port in self.inputs() {
            if port.required && !inputs.contains_key(&port.id) {
                if port.default_value.is_none() {
                    return Err(anyhow::anyhow!(
                        "Required input '{}' not provided for node '{}'",
                        port.id,
                        self.id()
                    ));
                }
            }
        }
        Ok(())
    }

    /// Get configuration schema (JSON Schema)
    fn config_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {}
        })
    }
}

/// A connection between two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConnection {
    /// Source node ID
    pub from_node: String,
    /// Source port ID
    pub from_port: String,
    /// Target node ID
    pub to_node: String,
    /// Target port ID
    pub to_port: String,
}

impl NodeConnection {
    /// Create a new connection
    pub fn new(from_node: &str, from_port: &str, to_node: &str, to_port: &str) -> Self {
        Self {
            from_node: from_node.to_string(),
            from_port: from_port.to_string(),
            to_node: to_node.to_string(),
            to_port: to_port.to_string(),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/orchestrator.rs">
//! Request orchestrator for tool execution and workstack routing
//!
//! Provides orchestration of tool execution with:
//! - Capability-based routing
//! - Workstack execution for multi-tool sequences
//! - Intermediate result caching
//! - Pattern tracking for optimization suggestions

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use op_core::error::Result;
use op_execution_tracker::{ExecutionRecord, ExecutionTracker};
use op_plugins::PluginCatalog;
use op_tools::registry::ToolRegistry;

// ============================================================================
// ORCHESTRATOR CONFIG
// ============================================================================

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Minimum tools to trigger workstack routing (default: 2)
    pub workstack_threshold: usize,
    /// Enable intermediate step caching
    pub enable_caching: bool,
    /// Track patterns for optimization suggestions
    pub track_patterns: bool,
    /// Promotion threshold (calls before suggesting promotion)
    pub promotion_threshold: u32,
    /// Maximum concurrent tool executions
    pub max_concurrent: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            workstack_threshold: 2,
            enable_caching: true,
            track_patterns: true,
            promotion_threshold: 3,
            max_concurrent: 10,
        }
    }
}

// ============================================================================
// EXECUTION RESULT
// ============================================================================

/// Workflow execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub request_id: String,
    pub success: bool,
    pub output: simd_json::OwnedValue,
    pub steps: Vec<StepResult>,
    pub total_latency_ms: u64,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub used_workstack: bool,
    pub resolved_tools: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Individual step result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_index: usize,
    pub tool_name: String,
    pub latency_ms: u64,
    pub cached: bool,
    pub success: bool,
    pub error: Option<String>,
}

// ============================================================================
// PATTERN TRACKING
// ============================================================================

/// Tracked execution pattern
#[derive(Debug, Clone)]
pub struct ExecutionPattern {
    pub tool_sequence: Vec<String>,
    pub call_count: u32,
    pub total_latency_ms: u64,
    pub suggested_name: Option<String>,
}

impl ExecutionPattern {
    pub fn avg_latency_ms(&self) -> u64 {
        if self.call_count == 0 {
            0
        } else {
            self.total_latency_ms / self.call_count as u64
        }
    }
}

/// Pattern tracker for optimization suggestions
pub struct PatternTracker {
    patterns: RwLock<HashMap<String, ExecutionPattern>>,
    promotion_threshold: u32,
}

impl PatternTracker {
    pub fn new(promotion_threshold: u32) -> Self {
        Self {
            patterns: RwLock::new(HashMap::new()),
            promotion_threshold,
        }
    }

    /// Record a tool sequence execution
    pub async fn record(&self, tools: &[String], latency_ms: u64) -> Option<String> {
        let key = tools.join("→");
        let mut patterns = self.patterns.write().await;

        let pattern = patterns.entry(key.clone()).or_insert(ExecutionPattern {
            tool_sequence: tools.to_vec(),
            call_count: 0,
            total_latency_ms: 0,
            suggested_name: None,
        });

        pattern.call_count += 1;
        pattern.total_latency_ms += latency_ms;

        if pattern.call_count >= self.promotion_threshold && pattern.suggested_name.is_none() {
            let name = format!("combined_{}", &Self::hash_sequence(tools)[..8]);
            pattern.suggested_name = Some(name.clone());
            Some(name)
        } else {
            None
        }
    }

    /// Get patterns ready for promotion
    pub async fn get_promotion_candidates(&self) -> Vec<ExecutionPattern> {
        self.patterns
            .read()
            .await
            .values()
            .filter(|p| p.call_count >= self.promotion_threshold)
            .cloned()
            .collect()
    }

    fn hash_sequence(tools: &[String]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tools.join(":").as_bytes());
        hex::encode(hasher.finalize())
    }
}

// ============================================================================
// INTERMEDIATE CACHE
// ============================================================================

/// Simple in-memory cache for intermediate results
pub struct IntermediateCache {
    cache: RwLock<HashMap<String, CachedResult>>,
    max_entries: usize,
}

#[derive(Clone)]
struct CachedResult {
    output: simd_json::OwnedValue,
    created_at: std::time::Instant,
}

impl IntermediateCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_entries,
        }
    }

    pub async fn get(&self, key: &str) -> Option<simd_json::OwnedValue> {
        let cache = self.cache.read().await;
        cache.get(key).map(|c| c.output.clone())
    }

    pub async fn put(&self, key: String, output: simd_json::OwnedValue) {
        let mut cache = self.cache.write().await;

        // Evict oldest if over limit
        if cache.len() >= self.max_entries {
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, v)| v.created_at)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(
            key,
            CachedResult {
                output,
                created_at: std::time::Instant::now(),
            },
        );
    }

    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        CacheStats {
            total_entries: cache.len(),
            max_entries: self.max_entries,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub max_entries: usize,
}

// ============================================================================
// ORCHESTRATOR
// ============================================================================

/// Main orchestrator for tool execution
pub struct Orchestrator {
    config: OrchestratorConfig,
    tool_registry: Arc<ToolRegistry>,
    /// Shared plugin catalog view used for lookup/routing only.
    ///
    /// The orchestrator is not allowed to become a second source of truth for
    /// plugin schema. It only consumes the already-registered catalog entries.
    #[allow(dead_code)]
    plugin_catalog: Arc<PluginCatalog>,
    execution_tracker: Arc<ExecutionTracker>,
    pattern_tracker: Arc<PatternTracker>,
    cache: Arc<IntermediateCache>,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub fn new(
        config: OrchestratorConfig,
        tool_registry: Arc<ToolRegistry>,
        plugin_catalog: Arc<PluginCatalog>,
    ) -> Self {
        let pattern_tracker = PatternTracker::new(config.promotion_threshold);
        let cache = IntermediateCache::new(1000);
        let execution_tracker = ExecutionTracker::new(1000);

        Self {
            config,
            tool_registry,
            plugin_catalog,
            execution_tracker: Arc::new(execution_tracker),
            pattern_tracker: Arc::new(pattern_tracker),
            cache: Arc::new(cache),
        }
    }

    /// Execute a single tool
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        input: simd_json::OwnedValue,
        session_id: Option<String>,
    ) -> Result<WorkflowResult> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let start_time = Instant::now();

        // Get the tool
        let tool = self
            .tool_registry
            .get(tool_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?;

        // Start tracking
        let exec_record = self
            .execution_tracker
            .start_execution(tool_name, Some(input.clone()), session_id)
            .await;

        // Execute
        let result = tool.execute(input.clone()).await;

        // Record result
        match &result {
            Ok(output) => {
                self.execution_tracker
                    .complete_execution(
                        &exec_record.id,
                        Some(simd_json::to_string(output).unwrap_or_default()),
                    )
                    .await;
            }
            Err(e) => {
                self.execution_tracker
                    .fail_execution(&exec_record.id, e.to_string())
                    .await;
            }
        }

        let output = result?;
        let latency_ms = start_time.elapsed().as_millis() as u64;

        Ok(WorkflowResult {
            request_id,
            success: true,
            output,
            steps: vec![StepResult {
                step_index: 0,
                tool_name: tool_name.to_string(),
                latency_ms,
                cached: false,
                success: true,
                error: None,
            }],
            total_latency_ms: latency_ms,
            cache_hits: 0,
            cache_misses: 1,
            used_workstack: false,
            resolved_tools: vec![tool_name.to_string()],
            error: None,
        })
    }

    /// Execute a sequence of tools (workstack)
    pub async fn execute_sequence(
        &self,
        tool_names: &[&str],
        initial_input: simd_json::OwnedValue,
        session_id: Option<String>,
    ) -> Result<WorkflowResult> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let start_time = Instant::now();

        if tool_names.is_empty() {
            return Ok(WorkflowResult {
                request_id,
                success: true,
                output: initial_input,
                steps: Vec::new(),
                total_latency_ms: 0,
                cache_hits: 0,
                cache_misses: 0,
                used_workstack: false,
                resolved_tools: Vec::new(),
                error: None,
            });
        }

        // Single tool - direct execution
        if tool_names.len() < self.config.workstack_threshold {
            return self
                .execute_tool(tool_names[0], initial_input, session_id)
                .await;
        }

        // Multi-tool workstack execution
        let mut steps = Vec::new();
        let mut current_input = initial_input;
        let mut cache_hits = 0usize;
        let mut cache_misses = 0usize;

        let workstack_id = format!(
            "ws-{}",
            &Self::hash_sequence_with_input(tool_names, &current_input)[..12]
        );

        for (step_index, tool_name) in tool_names.iter().enumerate() {
            let step_start = Instant::now();
            let cache_key = format!(
                "{}:{}:{}",
                workstack_id,
                step_index,
                Self::hash_input(&current_input)
            );

            // Try cache first
            let (output, cached) = if self.config.enable_caching {
                if let Some(cached_output) = self.cache.get(&cache_key).await {
                    cache_hits += 1;
                    (cached_output, true)
                } else {
                    cache_misses += 1;
                    let tool = self
                        .tool_registry
                        .get(tool_name)
                        .await
                        .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?;
                    let result = tool.execute(current_input.clone()).await?;

                    // Cache the result
                    self.cache.put(cache_key, result.clone()).await;

                    (result, false)
                }
            } else {
                cache_misses += 1;
                let tool = self
                    .tool_registry
                    .get(tool_name)
                    .await
                    .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?;
                let result = tool.execute(current_input.clone()).await?;
                (result, false)
            };

            let latency_ms = step_start.elapsed().as_millis() as u64;

            steps.push(StepResult {
                step_index,
                tool_name: tool_name.to_string(),
                latency_ms,
                cached,
                success: true,
                error: None,
            });

            current_input = output;
        }

        let total_latency_ms = start_time.elapsed().as_millis() as u64;

        // Track pattern
        if self.config.track_patterns {
            let tool_vec: Vec<String> = tool_names.iter().map(|s| s.to_string()).collect();
            if let Some(suggested_name) = self
                .pattern_tracker
                .record(&tool_vec, total_latency_ms)
                .await
            {
                tracing::info!(
                    "🔥 Pattern detected: '{}' ready for promotion",
                    suggested_name
                );
            }
        }

        Ok(WorkflowResult {
            request_id,
            success: true,
            output: current_input,
            steps,
            total_latency_ms,
            cache_hits: cache_hits as u32,
            cache_misses: cache_misses as u32,
            used_workstack: true,
            resolved_tools: tool_names.iter().map(|s| s.to_string()).collect(),
            error: None,
        })
    }

    /// Get orchestrator statistics
    pub async fn stats(&self) -> OrchestratorStats {
        let exec_stats = self.execution_tracker.get_stats().await;
        let cache_stats = self.cache.stats().await;
        let promotion_candidates = self.pattern_tracker.get_promotion_candidates().await;

        OrchestratorStats {
            total_executions: exec_stats.total_executions,
            successful_executions: exec_stats.successful_executions,
            failed_executions: exec_stats.failed_executions,
            avg_latency_ms: exec_stats.average_duration_ms(),
            cache_entries: cache_stats.total_entries,
            promotion_candidates: promotion_candidates.len(),
        }
    }

    fn hash_input(input: &simd_json::OwnedValue) -> String {
        let mut hasher = Sha256::new();
        hasher.update(simd_json::to_string(input).unwrap_or_default().as_bytes());
        hex::encode(hasher.finalize())[..16].to_string()
    }

    fn hash_sequence_with_input(tools: &[&str], input: &simd_json::OwnedValue) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tools.join("→").as_bytes());
        hasher.update(simd_json::to_string(input).unwrap_or_default().as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Orchestrator statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_latency_ms: f64,
    pub cache_entries: usize,
    pub promotion_candidates: usize,
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/src/workflows.rs">
//! MCP Workflows using PocketFlow
//! Flow-based programming for complex MCP agent interactions

use anyhow::Result;
use async_trait::async_trait;
use pocketflow_rs::{Context, Flow, Node, ProcessResult, ProcessState};
use serde_json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Workflow states for MCP operations
#[derive(Debug, Clone, PartialEq)]
pub enum McpWorkflowState {
    /// Initial state
    Start,
    /// Code analysis completed
    CodeAnalyzed,
    /// Tests written/generated
    TestsGenerated,
    /// Documentation updated
    DocsUpdated,
    /// Deployment ready
    ReadyToDeploy,
    /// Operation completed successfully
    Success,
    /// Operation failed
    Failure,
    /// Awaiting user input
    AwaitingInput,
}

impl Default for McpWorkflowState {
    fn default() -> Self {
        McpWorkflowState::Start
    }
}

impl ProcessState for McpWorkflowState {
    fn is_default(&self) -> bool {
        matches!(self, McpWorkflowState::Start)
    }

    fn to_condition(&self) -> String {
        match self {
            McpWorkflowState::Start => "start",
            McpWorkflowState::CodeAnalyzed => "code_analyzed",
            McpWorkflowState::TestsGenerated => "tests_generated",
            McpWorkflowState::DocsUpdated => "docs_updated",
            McpWorkflowState::ReadyToDeploy => "ready_to_deploy",
            McpWorkflowState::Success => "success",
            McpWorkflowState::Failure => "failure",
            McpWorkflowState::AwaitingInput => "awaiting_input",
        }
        .to_string()
    }
}

/// MCP Code Review Workflow Node
pub struct CodeReviewNode {
    language: String,
}

impl CodeReviewNode {
    pub fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
        }
    }
}

#[async_trait]
impl Node for CodeReviewNode {
    type State = McpWorkflowState;

    async fn prepare(&self, context: &mut Context) -> Result<()> {
        log::info!("🔍 Preparing code review for {} code", self.language);
        context.set(
            "review_language",
            serde_json::Value::String(self.language.clone()),
        );
        Ok(())
    }

    async fn execute(&self, context: &Context) -> Result<serde_json::Value> {
        log::info!("⚡ Executing code review workflow");

        // Get code from context
        let code = context.get("code").and_then(|v| v.as_str()).unwrap_or("");

        // Simulate calling MCP agents for code analysis
        log::info!(
            "📝 Analyzing {} lines of {} code",
            code.lines().count(),
            self.language
        );

        // In real implementation, this would call actual MCP agents
        // like rust_pro, python_pro, etc.

        Ok(serde_json::Value::String("code_analyzed".to_string()))
    }

    async fn post_process(
        &self,
        context: &mut Context,
        result: &Result<serde_json::Value>,
    ) -> Result<ProcessResult<Self::State>> {
        match result {
            Ok(value) if value.as_str() == Some("code_analyzed") => {
                context.set("analysis_complete", serde_json::Value::Bool(true));
                log::info!("✅ Code analysis completed");
                Ok(ProcessResult::new(
                    McpWorkflowState::CodeAnalyzed,
                    "Code review completed successfully".to_string(),
                ))
            }
            Ok(_) => {
                log::warn!("⚠️  Unexpected result from code review");
                Ok(ProcessResult::new(
                    McpWorkflowState::Failure,
                    "Unexpected result".to_string(),
                ))
            }
            Err(e) => {
                log::error!("❌ Code review failed: {}", e);
                Ok(ProcessResult::new(
                    McpWorkflowState::Failure,
                    format!("Code review failed: {}", e),
                ))
            }
        }
    }
}

/// Test Generation Node
pub struct TestGenerationNode;

#[async_trait]
impl Node for TestGenerationNode {
    type State = McpWorkflowState;

    async fn prepare(&self, _context: &mut Context) -> Result<()> {
        log::info!("🧪 Preparing test generation");
        Ok(())
    }

    async fn execute(&self, context: &Context) -> Result<serde_json::Value> {
        log::info!("⚡ Generating tests based on code analysis");

        // Check if code analysis was completed
        let analysis_done = context
            .get("analysis_complete")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !analysis_done {
            log::warn!("⚠️  Cannot generate tests without code analysis");
            return Ok(serde_json::Value::String("failed".to_string()));
        }

        // In real implementation, call test generation agents
        log::info!("📝 Generating comprehensive test suite");

        Ok(serde_json::Value::String("tests_generated".to_string()))
    }

    async fn post_process(
        &self,
        context: &mut Context,
        result: &Result<serde_json::Value>,
    ) -> Result<ProcessResult<Self::State>> {
        match result {
            Ok(value) if value.as_str() == Some("tests_generated") => {
                context.set("tests_generated", serde_json::Value::Bool(true));
                log::info!("✅ Tests generated");
                Ok(ProcessResult::new(
                    McpWorkflowState::TestsGenerated,
                    "Tests generated successfully".to_string(),
                ))
            }
            Ok(_) => Ok(ProcessResult::new(
                McpWorkflowState::Failure,
                "Unexpected result".to_string(),
            )),
            Err(e) => {
                log::error!("❌ Test generation failed: {}", e);
                Ok(ProcessResult::new(
                    McpWorkflowState::Failure,
                    format!("Test generation failed: {}", e),
                ))
            }
        }
    }
}

/// Documentation Update Node
pub struct DocumentationNode;

#[async_trait]
impl Node for DocumentationNode {
    type State = McpWorkflowState;

    async fn prepare(&self, _context: &mut Context) -> Result<()> {
        log::info!("📚 Preparing documentation update");
        Ok(())
    }

    async fn execute(&self, context: &Context) -> Result<serde_json::Value> {
        log::info!("⚡ Updating documentation");

        // Simulate documentation update
        let tests_done = context
            .get("tests_generated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !tests_done {
            log::warn!("⚠️  Tests should be generated before final documentation");
            return Ok(serde_json::Value::String("awaiting_input".to_string()));
        }

        log::info!("📝 Updating API documentation and README");

        Ok(serde_json::Value::String("docs_updated".to_string()))
    }

    async fn post_process(
        &self,
        context: &mut Context,
        result: &Result<serde_json::Value>,
    ) -> Result<ProcessResult<Self::State>> {
        match result {
            Ok(value) if value.as_str() == Some("docs_updated") => {
                context.set("docs_updated", serde_json::Value::Bool(true));
                log::info!("✅ Documentation updated");
                Ok(ProcessResult::new(
                    McpWorkflowState::DocsUpdated,
                    "Documentation updated successfully".to_string(),
                ))
            }
            Ok(value) if value.as_str() == Some("awaiting_input") => {
                log::info!("⏳ Documentation update paused - awaiting test completion");
                Ok(ProcessResult::new(
                    McpWorkflowState::AwaitingInput,
                    "Awaiting test completion".to_string(),
                ))
            }
            Ok(_) => Ok(ProcessResult::new(
                McpWorkflowState::Failure,
                "Unexpected result".to_string(),
            )),
            Err(e) => {
                log::error!("❌ Documentation update error: {}", e);
                Ok(ProcessResult::new(
                    McpWorkflowState::Failure,
                    format!("Documentation update error: {}", e),
                ))
            }
        }
    }
}

/// Deployment Preparation Node
pub struct DeploymentNode;

#[async_trait]
impl Node for DeploymentNode {
    type State = McpWorkflowState;

    async fn prepare(&self, _context: &mut Context) -> Result<()> {
        log::info!("🚀 Preparing deployment");
        Ok(())
    }

    async fn execute(&self, context: &Context) -> Result<serde_json::Value> {
        log::info!("⚡ Deploying system changes");

        // Simulate deployment
        log::info!("🚀 Starting deployment to production");

        Ok(serde_json::Value::String("ready_to_deploy".to_string()))
    }

    async fn post_process(
        &self,
        context: &mut Context,
        result: &Result<serde_json::Value>,
    ) -> Result<ProcessResult<Self::State>> {
        match result {
            Ok(value) if value.as_str() == Some("ready_to_deploy") => {
                context.set("deployment_ready", serde_json::Value::Bool(true));
                log::info!("✅ Deployment complete");
                Ok(ProcessResult::new(
                    McpWorkflowState::Success,
                    "Deployment finished".to_string(),
                ))
            }
            Ok(_) => Ok(ProcessResult::new(
                McpWorkflowState::Failure,
                "Unexpected result".to_string(),
            )),
            Err(e) => {
                log::error!("❌ Deployment preparation error: {}", e);
                Ok(ProcessResult::new(
                    McpWorkflowState::Failure,
                    format!("Deployment preparation error: {}", e),
                ))
            }
        }
    }
}

/// MCP Development Workflow Manager
pub struct McpWorkflowManager {
    flows: std::collections::HashMap<String, Flow<McpWorkflowState>>,
}

impl McpWorkflowManager {
    pub fn new() -> Self {
        Self {
            flows: std::collections::HashMap::new(),
        }
    }

    /// Create a standard code review workflow
    pub fn create_code_review_workflow(&mut self, language: &str) -> Result<()> {
        // Create nodes
        let code_review = Arc::new(CodeReviewNode::new(language));
        let test_gen = Arc::new(TestGenerationNode);
        let docs = Arc::new(DocumentationNode);
        let deploy = Arc::new(DeploymentNode);

        // Create flow starting with code review
        let mut flow = Flow::new("code_review", code_review);
        flow.add_node("test_generation", test_gen);
        flow.add_node("documentation", docs);
        flow.add_node("deployment", deploy);

        // Define workflow transitions
        flow.add_edge(
            "code_review",
            "test_generation",
            McpWorkflowState::CodeAnalyzed,
        );
        flow.add_edge(
            "test_generation",
            "documentation",
            McpWorkflowState::TestsGenerated,
        );
        flow.add_edge("documentation", "deployment", McpWorkflowState::DocsUpdated);
        flow.add_edge(
            "documentation",
            "documentation",
            McpWorkflowState::AwaitingInput,
        ); // Wait for tests
        flow.add_edge("deployment", "code_review", McpWorkflowState::ReadyToDeploy); // Loop back for next review

        self.flows.insert(format!("code_review_{}", language), flow);
        Ok(())
    }

    /// Execute a workflow with given context
    pub async fn run_workflow(
        &self,
        workflow_name: &str,
        context: Context,
    ) -> Result<serde_json::Value> {
        if let Some(workflow) = self.flows.get(workflow_name) {
            log::info!("🚀 Running workflow: {}", workflow_name);
            let result = workflow.run(context).await?;
            log::info!("✅ Workflow complete: {}", workflow_name);
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Workflow '{}' not found", workflow_name))
        }
    }

    /// List available workflows
    pub fn list_workflows(&self) -> Vec<String> {
        self.flows.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_code_review_workflow() {
        let mut manager = McpWorkflowManager::new();
        manager.create_code_review_workflow("rust").unwrap();

        let workflows = manager.list_workflows();
        assert!(workflows.contains(&"code_review_rust".to_string()));

        // Create test context
        let mut context = Context::new();
        context.set(
            "code",
            Value::String("fn main() { println!(\"Hello\"); }".to_string()),
        );

        // This would run the full workflow in a real test
        // let result = manager.run_workflow("code_review_rust", context).await;
        // assert!(result.is_ok());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/Cargo.toml">
[package]
name = "op-workflows"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Workflow engine with plugin/service nodes for op-dbus-v2"

[dependencies]
op-core = { workspace = true }
op-plugins = { path = "../op-plugins" }
op-tools = { path = "../op-tools" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
sha2 = { workspace = true }
hex = "0.4"
pocketflow_rs = "0.1"
op-execution-tracker = { path = "../op-execution-tracker" }
log = { workspace = true }
serde_json = { workspace = true }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/compare-op-workflows.md">
# compare-op-workflows

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 13 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 8 |
| Partial artifacts | 0 |
| Spec-listed source files | 12 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- Workflow engine with plugin/service nodes for op-dbus-v2
- Internal crate integrations: op-core, op-plugins, op-tools, op-execution-tracker.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/builtin/tool_node.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/tool_node.rs |
| `src/builtin/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/mod.rs |
| `src/builtin/definitions.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/definitions.rs |
| `src/builtin/dbus_node.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/dbus_node.rs |
| `src/builtin/plugin_node.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/plugin_node.rs |
| `src/workflows.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/workflows.rs |
| `src/orchestrator.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/orchestrator.rs |
| `src/node.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/node.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/flow.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/flow.rs |
| `src/engine.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/engine.rs |
| `src/context.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/context.rs |
| `builtin` | ✅ Present | builtin group | src/builtin/dbus_node.rs, src/builtin/definitions.rs, src/builtin/mod.rs, src/builtin/plugin_node.rs, src/builtin/tool_node.rs |
| `root` | ✅ Present | root source group | src/context.rs, src/engine.rs, src/flow.rs, src/history.rs, src/lib.rs, src/node.rs, src/orchestrator.rs, src/workflows.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| workflows | ✅ Implemented | src/workflows.rs | SPEC main module |
| orchestrator | ✅ Implemented | src/orchestrator.rs | SPEC main module |
| node | ✅ Implemented | src/node.rs | SPEC main module |
| flow | ✅ Implemented | src/flow.rs | SPEC main module |
| engine | ✅ Implemented | src/engine.rs | SPEC main module |
| context | ✅ Implemented | src/context.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-plugins` - documented in SPEC
- `op-tools` - documented in SPEC
- `op-execution-tracker` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `uuid` - documented in SPEC
- `chrono` - documented in SPEC
- `sha2` - documented in SPEC
- `hex` - documented in SPEC
- `pocketflow_rs` - documented in SPEC
- `log` - documented in SPEC
- `serde_json` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: builtin, context, engine, flow, history, node, orchestrator, workflows.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-workflows/SPEC.md">
# op-workflows - Specification

## Overview
**Crate**: `op-workflows`  
**Location**: `crates/op-workflows`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-workflows"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-workflows/src/builtin/tool_node.rs
op-workflows/src/builtin/mod.rs
op-workflows/src/builtin/definitions.rs
op-workflows/src/builtin/dbus_node.rs
op-workflows/src/builtin/plugin_node.rs
op-workflows/src/workflows.rs
op-workflows/src/orchestrator.rs
op-workflows/src/node.rs
op-workflows/src/lib.rs
op-workflows/src/flow.rs
op-workflows/src/engine.rs
op-workflows/src/context.rs
```

### Key Dependencies
```toml
op-core = { workspace = true }
op-plugins = { path = "../op-plugins" }
op-tools = { path = "../op-tools" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
sha2 = { workspace = true }
hex = "0.4"
pocketflow_rs = "0.1"
op-execution-tracker = { path = "../op-execution-tracker" }
log = { workspace = true }
serde_json = { workspace = true }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
      12 Rust source files

### Main Modules
workflows
orchestrator
node
flow
engine
context

## Purpose
Workflow engine with plugin/service nodes for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-plugins
- op-tools
- op-execution-tracker

---
*Generated from crate analysis*
</file>

</files>
