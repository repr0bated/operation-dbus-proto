use std::sync::Arc;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use crate::mcp::McpCompactDispatcher;
use super::session::ChatSession;
use crate::error::Result;

/// Intent types resolved from user input
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum Intent {
    Query(QueryIntent),
    ToolExecution(ToolExecutionIntent),
    WorkflowRequest(WorkflowIntent),
    MaintenanceRequest(MaintenanceIntent),
    InspectorRequest(InspectorIntent),
    DisasterRecoveryRequest(DisasterRecoveryIntent),
    Explanation(String),
    Unknown(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum QueryIntent {
    ListTools { filter: Option<String> },
    GetToolSchema { tool_name: String },
    SystemStatus,
    ExecutionHistory { limit: usize },
    ObjectState { object_id: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolExecutionIntent {
    pub tool_name: String,
    pub params: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkflowIntent {
    pub description: String,
    pub goal: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MaintenanceIntent {
    pub concern: String,
    pub scope: MaintenanceScope,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MaintenanceScope {
    System,
    Plugin(String),
    Object(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum InspectorIntent {
    Discover,
    PlanMigration { source: String },
    OnboardSchema { subsystem: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DisasterRecoveryIntent {
    Export { path: Option<String> },
    Import { path: String },
    Validate { path: String },
}

/// Resolves user messages into specific Intents
pub struct IntentResolver {
    mcp_compact: Arc<McpCompactDispatcher>,
}

impl IntentResolver {
    pub fn new(mcp_compact: Arc<McpCompactDispatcher>) -> Self {
        Self { mcp_compact }
    }

    pub async fn resolve(&self, message: &str, _session: &ChatSession) -> Result<Intent> {
        let input_lower = message.to_lowercase();
        let input_trim = message.trim();

        // 1. Direct Tool Execution (e.g. "run system.echo hello")
        if input_lower.starts_with("run ") || input_lower.starts_with("execute ") {
            let parts: Vec<&str> = input_trim.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                let tool_name = parts[1].to_string();
                // Simple parsing of params if 3rd part exists
                let params = if parts.len() > 2 {
                    // Try to parse as JSON, otherwise string
                    let mut json_str = parts[2].to_string();
                    unsafe { simd_json::from_str(&mut json_str) }.unwrap_or_else(|_| simd_json::json!({ "arg": parts[2] }))
                } else {
                    simd_json::json!({})
                };

                // Hallucination Check: Verify tool exists
                if !self.tool_exists(&tool_name).await {
                     // If tool doesn't exist, it might be a misunderstanding or "hallucination" of a tool name
                     return Ok(Intent::Unknown(message.to_string()));
                }

                return Ok(Intent::ToolExecution(ToolExecutionIntent {
                    tool_name,
                    params,
                }));
            }
        }

        // 2. Query Intents
        if input_lower.contains("list tools") {
            let filter = if input_lower.len() > 11 {
                Some(input_trim[10..].trim().to_string())
            } else {
                None
            };
            return Ok(Intent::Query(QueryIntent::ListTools { filter }));
        }

        if input_lower.contains("system status") || input_lower.contains("status") {
             return Ok(Intent::Query(QueryIntent::SystemStatus));
        }

        if input_lower.starts_with("show schema") {
             let parts: Vec<&str> = input_trim.split_whitespace().collect();
             if parts.len() > 2 {
                 return Ok(Intent::Query(QueryIntent::GetToolSchema { tool_name: parts[2].to_string() }));
             }
        }

        // 3. Maintenance
        if input_lower.contains("maintain") || input_lower.contains("fix") {
             return Ok(Intent::MaintenanceRequest(MaintenanceIntent {
                 concern: message.to_string(),
                 scope: MaintenanceScope::System, // simplified
             }));
        }

        // 4. Inspector
        if input_lower.contains("discover") 
            || input_lower.contains("ram") 
            || input_lower.contains("memory") 
            || input_lower.contains("cpu") 
            || input_lower.contains("disk") {
            return Ok(Intent::InspectorRequest(InspectorIntent::Discover));
        }

        // 5. Disaster Recovery
        if input_lower.contains("export") {
             return Ok(Intent::DisasterRecoveryRequest(DisasterRecoveryIntent::Export { path: None }));
        }

        // 6. Explanation
        if input_lower.starts_with("explain") || input_lower.starts_with("what is") {
             return Ok(Intent::Explanation(message.to_string()));
        }

        // Fallback
        Ok(Intent::Unknown(message.to_string()))
    }

    async fn tool_exists(&self, tool_name: &str) -> bool {
        // We use the mcp_compact dispatcher's registry reference
        // Assuming mcp_compact exposes tool_registry_ref() as seen in mod.rs
        self.mcp_compact.tool_registry_ref().get(tool_name).await.is_some()
    }
}