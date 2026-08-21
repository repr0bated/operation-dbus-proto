//! MCP tool registry adapter for built-in agents.
//!
//! The agent catalog is the local schema source for exposing agents as tools.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use op_agents::{builtin_agent_descriptors, create_agent, AgentDescriptor, AgentTask};
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn register_agent_tools(registry: &ToolRegistry) -> Result<usize> {
    let mut count = 0;

    for descriptor in builtin_agent_descriptors() {
        for operation in &descriptor.operations {
            let schema = descriptor.schema_for(operation).cloned().ok_or_else(|| {
                anyhow!(
                    "agent catalog is missing a schema for {}:{}",
                    descriptor.agent_type,
                    operation
                )
            })?;
            registry
                .register(Arc::new(AgentCatalogTool {
                    descriptor: descriptor.clone(),
                    operation: operation.clone(),
                    input_schema: schema.input_schema,
                    tool_name: format!(
                        "agent_{}_{}",
                        sanitize_tool_name(&descriptor.agent_type),
                        sanitize_tool_name(operation)
                    ),
                }) as BoxedTool)
                .await?;
            count += 1;
        }
    }

    Ok(count)
}

struct AgentCatalogTool {
    descriptor: AgentDescriptor,
    operation: String,
    input_schema: Value,
    tool_name: String,
}

#[async_trait]
impl Tool for AgentCatalogTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.descriptor.description
    }

    fn category(&self) -> &str {
        "agent"
    }

    fn namespace(&self) -> &str {
        "agents"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "agent".to_string(),
            self.descriptor.agent_type.clone(),
            self.operation.clone(),
        ]
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let agent_id = format!(
            "tool-registry-{}",
            sanitize_tool_name(&self.descriptor.agent_type)
        );
        let agent =
            create_agent(&self.descriptor.agent_type, agent_id).map_err(|err| anyhow!(err))?;

        let mut task = AgentTask::new(&self.descriptor.agent_type, &self.operation);
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            task = task.with_path(path);
        }
        if let Some(args) = agent_argument_text(&input) {
            task = task.with_args(&args);
        }
        task.config = agent_config(&input);

        let result = agent.execute(task).await.map_err(|err| anyhow!(err))?;
        Ok(json!({
            "success": result.success,
            "agent_type": self.descriptor.agent_type,
            "agent_name": self.descriptor.name,
            "operation": result.operation,
            "data": result.data,
            "metadata": result.metadata,
        }))
    }
}

/// Convert the catalog-level operation schema into the legacy `AgentTask`
/// transport without losing meaningful data. Concrete agents still consume a
/// textual argument field, but schema-driven callers can use `query`,
/// `context`, `code`, or structured `args` as advertised.
fn agent_argument_text(input: &Value) -> Option<String> {
    let direct = input
        .get("args")
        .or_else(|| input.get("query"))
        .or_else(|| input.get("code"))
        .or_else(|| input.get("context"))
        .or_else(|| input.get("workflow"));

    direct.and_then(|value| {
        if value.is_null() {
            None
        } else if let Some(text) = value.as_str() {
            Some(text.to_string())
        } else {
            simd_json::to_string(value).ok()
        }
    })
}

/// Keep explicitly supplied configuration and preserve operation-specific
/// fields for agents which inspect `AgentTask::config` (for example limits,
/// tags, or workflow flags). Transport fields are excluded because they have
/// dedicated `AgentTask` members.
fn agent_config(input: &Value) -> HashMap<String, Value> {
    let mut config = input
        .get("config")
        .and_then(|value| value.as_object())
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    if let Some(values) = input.as_object() {
        for (key, value) in values {
            if !matches!(key.as_ref(), "args" | "path" | "config" | "timeout") {
                config
                    .entry(key.to_string())
                    .or_insert_with(|| value.clone());
            }
        }
    }

    config
}

fn sanitize_tool_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_agents::AgentOperationSchema;

    fn sequential_descriptor() -> AgentDescriptor {
        builtin_agent_descriptors()
            .into_iter()
            .find(|descriptor| descriptor.agent_type == "sequential-thinking")
            .expect("sequential-thinking catalog descriptor")
    }

    fn sequential_tool() -> AgentCatalogTool {
        let descriptor = sequential_descriptor();
        let operation = "analyze".to_string();
        let schema: AgentOperationSchema = descriptor
            .schema_for(&operation)
            .cloned()
            .expect("operation schema");
        AgentCatalogTool {
            tool_name: format!(
                "agent_{}_{}",
                sanitize_tool_name(&descriptor.agent_type),
                sanitize_tool_name(&operation)
            ),
            descriptor,
            operation,
            input_schema: schema.input_schema,
        }
    }

    #[test]
    fn agent_tool_uses_the_catalog_operation_schema() {
        let tool = sequential_tool();
        assert!(tool
            .input_schema()
            .get("properties")
            .and_then(|properties| properties.get("context"))
            .is_some());
    }

    #[tokio::test]
    async fn agent_tool_forwards_schema_level_context_to_legacy_agent_task() {
        let result = sequential_tool()
            .execute(json!({ "context": "review the cognitive MCP ingress" }))
            .await
            .expect("agent result");
        let data = result
            .get("data")
            .and_then(|value| value.as_str())
            .expect("agent result data");
        assert!(data.contains("review the cognitive MCP ingress"));
    }

    #[test]
    fn structured_agent_args_and_operation_fields_are_preserved() {
        let input = json!({
            "args": { "target": "tool registry" },
            "limit": 4,
            "config": { "mode": "careful" },
            "timeout": 15
        });
        assert_eq!(
            agent_argument_text(&input).as_deref(),
            Some(r#"{"target":"tool registry"}"#)
        );
        let config = agent_config(&input);
        assert_eq!(
            config.get("mode").and_then(|value| value.as_str()),
            Some("careful")
        );
        assert_eq!(
            config.get("limit").and_then(|value| value.as_u64()),
            Some(4)
        );
        assert!(!config.contains_key("timeout"));
    }
}
