//! OP-DBUS Chatbot: Cognitive Control Plane
//!
//! The chatbot is the cognitive brain of OP-DBUS.
//! It reasons, plans, and coordinates - but NEVER executes directly.
//!
//! All execution flows through: Chatbot -> MCP -> Orchestrator -> Tools
//!
//! Wired into:
//! - Inspector Gadget (can request discovery)
//! - Policy Engine (checks policies before suggesting actions)
//! - Disaster Recovery (can export/import canonical state)
//! - Dependency Manager (can check/satisfy dependencies)
//! - Antigravity Tunnel (dev mode, same interface)

pub mod intent;
pub mod planner;
pub mod session;
pub mod maintenance;
pub mod cognitive;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::error::{OpDbusError, Result};
use crate::mcp::McpCompactDispatcher;
use crate::mcp_live::{McpLiveDispatcher, CognitiveStream};
use op_state_store::StateStore;
use crate::policy::{PolicyEngine, PolicyDecision};
use crate::inspector_gadget::InspectorGadget;
use crate::disaster_recovery::DisasterRecovery;
use crate::dependency::DependencyManager;
use op_tools::registry::ToolDefinition as ToolMetadata;

pub use intent::{
    Intent, IntentResolver, QueryIntent, ToolExecutionIntent, 
    WorkflowIntent, MaintenanceIntent, InspectorIntent, DisasterRecoveryIntent
};
pub use planner::WorkflowPlanner;
pub use session::{ChatSession, ChatMessage, MessageRole};
pub use maintenance::{MaintenanceHandler, MaintenanceTask, MaintenanceResult};

/// Chatbot configuration
#[derive(Debug, Clone)]
pub struct ChatbotConfig {
    pub max_history: usize,
    pub enable_maintenance: bool,
    pub enable_workflows: bool,
    pub enable_inspector: bool,
    pub enable_policy_check: bool,
    pub streaming: bool,
}

impl Default for ChatbotConfig {
    fn default() -> Self {
        Self {
            max_history: crate::constants::CHATBOT_MAX_HISTORY,
            enable_maintenance: true,
            enable_workflows: true,
            enable_inspector: true,
            enable_policy_check: true,
            streaming: true,
        }
    }
}

/// The OP-DBUS Chatbot
///
/// The cognitive brain. NEVER executes tools directly.
pub struct Chatbot {
    config: ChatbotConfig,
    mcp_compact: Arc<McpCompactDispatcher>,
    mcp_live: Arc<McpLiveDispatcher>,
    state_store: Arc<dyn StateStore>,
    policy_engine: Option<Arc<PolicyEngine>>,
    inspector: Option<Arc<InspectorGadget>>,
    disaster_recovery: Option<Arc<DisasterRecovery>>,
    dependency_manager: Option<Arc<DependencyManager>>,
    intent_resolver: IntentResolver,
    workflow_planner: WorkflowPlanner,
    maintenance_handler: MaintenanceHandler,
    sessions: dashmap::DashMap<String, ChatSession>,
    #[cfg(feature = "dev-antigravity")]
    bridge_client: Option<crate::antigravity::AntigravityBridgeClient>,
}

impl Chatbot {
    pub fn new(
        config: ChatbotConfig,
        mcp_compact: Arc<McpCompactDispatcher>,
        mcp_live: Arc<McpLiveDispatcher>,
        state_store: Arc<dyn StateStore>,
    ) -> Self {
        let registry = mcp_compact.tool_registry();
        
        Self {
            config: config.clone(),
            mcp_compact: mcp_compact.clone(),
            mcp_live,
            state_store: state_store.clone(),
            policy_engine: None,
            inspector: None,
            disaster_recovery: None,
            dependency_manager: None,
            intent_resolver: IntentResolver::new(mcp_compact.clone()),
            workflow_planner: WorkflowPlanner::new(registry),
            maintenance_handler: MaintenanceHandler::new(),
            sessions: dashmap::DashMap::new(),
            #[cfg(feature = "dev-antigravity")]
            bridge_client: crate::antigravity::AntigravityBridgeClient::from_env(),
        }
    }

    /// Wire policy engine
    pub fn with_policy_engine(mut self, engine: Arc<PolicyEngine>) -> Self {
        self.policy_engine = Some(engine);
        self
    }

    /// Wire inspector gadget
    pub fn with_inspector(mut self, inspector: Arc<InspectorGadget>) -> Self {
        self.inspector = Some(inspector);
        self
    }

    /// Wire disaster recovery
    pub fn with_disaster_recovery(mut self, dr: Arc<DisasterRecovery>) -> Self {
        self.disaster_recovery = Some(dr);
        self
    }

    /// Wire dependency manager
    pub fn with_dependency_manager(mut self, dm: Arc<DependencyManager>) -> Self {
        self.dependency_manager = Some(dm);
        self
    }

    /// Get or create session
    pub fn session(&self, session_id: &str) -> ChatSession {
        self.sessions
            .entry(session_id.to_string())
            .or_insert_with(|| ChatSession::new(crate::chatbot::session::SessionId(session_id.to_string())))
            .clone()
    }

    /// Process a chat message
    pub async fn chat(
        &self,
        session_id: &str,
        message: &str,
    ) -> Result<ChatResponse> {
        let mut session = self.session(session_id);
        session.add_message(ChatMessage::user(message));

        // Resolve intent
        let intent = self.intent_resolver.resolve(message, &session).await?;

        // Process based on intent
        let response = match &intent {
            Intent::Query(query) => self.handle_query(query, &session).await?,
            Intent::ToolExecution(tool_req) => {
                // Check policy before execution
                if self.config.enable_policy_check {
                    if let Some(ref engine) = self.policy_engine {
                        let params_serde = serde_json::to_value(&tool_req.params).unwrap_or(serde_json::Value::Null);
                        match engine.check_tool_execution(&tool_req.tool_name, &params_serde) {
                            PolicyDecision::Denied { reason, .. } => {
                                return Ok(ChatResponse {
                                    message: format!(
                                        "⛔ Policy denied execution of `{}`\n\n**Reason:** {}",
                                        tool_req.tool_name, reason
                                    ),
                                    intent: intent.clone(),
                                    actions_taken: vec![],
                                    suggestions: vec!["Contact administrator for policy override".into()],
                                });
                            }
                            PolicyDecision::RequiresApproval { approvers, .. } => {
                                return Ok(ChatResponse {
                                    message: format!(
                                        "⚠️ Execution of `{}` requires approval from: {:?}",
                                        tool_req.tool_name, approvers
                                    ),
                                    intent: intent.clone(),
                                    actions_taken: vec![],
                                    suggestions: vec!["Request approval".into()],
                                });
                            }
                            PolicyDecision::Allowed { .. } => {}
                        }
                    }
                }
                self.handle_tool_execution(tool_req, &session).await?
            }
            Intent::WorkflowRequest(workflow_req) => self.handle_workflow(workflow_req, &session).await?,
            Intent::MaintenanceRequest(maint_req) => self.handle_maintenance(maint_req, &session).await?,
            Intent::InspectorRequest(inspector_req) => self.handle_inspector(inspector_req, &session).await?,
            Intent::DisasterRecoveryRequest(dr_req) => self.handle_disaster_recovery(dr_req, &session).await?,
            Intent::Explanation(topic) => self.handle_explanation(topic, &session).await?,
            Intent::Unknown(raw) => self.handle_unknown(raw, &session).await?,
        };

        session.add_message(ChatMessage::assistant(&response.message));
        self.sessions.insert(session_id.to_string(), session);

        Ok(response)
    }

    /// Stream cognitive response
    pub async fn chat_stream(
        &self,
        session_id: &str,
        message: &str,
    ) -> Result<CognitiveStream> {
        let session = self.session(session_id);

        // Create a stream channel - the actual implementation would use the live dispatcher
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        // Start the cognitive stream in the background
        let message_clone = message.to_string();
        tokio::spawn(async move {
            // Send a thinking chunk
            let _ = tx.send(crate::mcp_live::CognitiveChunk {
                chunk_type: crate::mcp_live::CognitiveChunkType::Thinking,
                content: format!("Processing: {}", message_clone),
                metadata: None,
            }).await;
        });

        Ok(crate::mcp_live::CognitiveStream::new(rx))
    }

    async fn handle_query(
        &self,
        query: &QueryIntent,
        _session: &ChatSession,
    ) -> Result<ChatResponse> {
        match query {
            QueryIntent::ListTools { filter } => {
                let tools = self.get_filtered_tools(filter.as_deref()).await?;
                Ok(ChatResponse {
                    message: format!("Found {} tools:\n{}",
                        tools.len(),
                        tools.iter()
                            .map(|t| format!("\u{0009}• **{}**: {}", t.name, t.description))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    intent: Intent::Query(query.clone()),
                    actions_taken: vec![],
                    suggestions: vec!["Try `execute <tool_name>` to run a tool".into()],
                })
            }
            QueryIntent::GetToolSchema { tool_name } => {
                match self.mcp_compact.tool_registry_ref().get_definition(tool_name).await {
                    Some(meta) => Ok(ChatResponse {
                        message: format!(
                            "## {}

**Description:** {}

**Input Schema:**
```json
{}
```",
                            meta.name,
                            meta.description,
                            simd_json::to_string_pretty(&meta.input_schema).unwrap_or_default(),
                        ),
                        intent: Intent::Query(query.clone()),
                        actions_taken: vec![],
                        suggestions: vec![format!("Execute with: `run {} {{}}`", tool_name)],
                    }),
                    None => Ok(ChatResponse {
                        message: format!("Tool '{}' not found.", tool_name),
                        intent: Intent::Query(query.clone()),
                        actions_taken: vec![],
                        suggestions: vec!["list tools".into()],
                    }),
                }
            }
            QueryIntent::SystemStatus => {
                let tool_count = self.mcp_compact.tool_count().await;
                let has_policy = self.policy_engine.is_some();
                let has_inspector = self.inspector.is_some();
                let has_dr = self.disaster_recovery.is_some();

                Ok(ChatResponse {
                    message: format!(
                        "## System Status\n\n\t• **Tools Available:** {}
	• **Policy Engine:** {}
	• **Inspector Gadget:** {}
	• **Disaster Recovery:** {}
	• **Database:** Connected
	• **MCP Mode:** Compact + Live

						All systems operational.",
                        tool_count,
                        if has_policy { "Active" } else { "Not configured" },
                        if has_inspector { "Available" } else { "Not configured" },
                        if has_dr { "Available" } else { "Not configured" },
                    ),
                    intent: Intent::Query(query.clone()),
                    actions_taken: vec![],
                    suggestions: vec!["list tools".into(), "discover system".into()],
                })
            }
            QueryIntent::ExecutionHistory { limit } => {
                Ok(ChatResponse {
                    message: format!("Showing last {} executions.", limit),
                    intent: Intent::Query(query.clone()),
                    actions_taken: vec![],
                    suggestions: vec![],
                })
            }
            QueryIntent::ObjectState { object_id } => {
                match self.state_store.get_object(object_id).await? {
                    Some(obj) => Ok(ChatResponse {
                        message: format!(
                            "## Object: {}

**Type:** {}
```json
{}
```",
                            obj.id, obj.object_type,
                            simd_json::to_string_pretty(&obj.data).unwrap_or_default()
                        ),
                        intent: Intent::Query(query.clone()),
                        actions_taken: vec![],
                        suggestions: vec![],
                    }),
                    None => Ok(ChatResponse {
                        message: format!("Object '{}' not found.", object_id),
                        intent: Intent::Query(query.clone()),
                        actions_taken: vec![],
                        suggestions: vec![],
                    }),
                }
            }
        }
    }

    async fn handle_tool_execution(
        &self,
        request: &ToolExecutionIntent,
        _session: &ChatSession,
    ) -> Result<ChatResponse> {
        // Validate tool exists
        if !self.mcp_compact.tool_registry_ref().is_loaded(&request.tool_name).await {
            return Ok(ChatResponse {
                message: format!("❌ Tool '{}' does not exist.", request.tool_name),
                intent: Intent::ToolExecution(request.clone()),
                actions_taken: vec![],
                suggestions: vec!["list tools".into()],
            });
        }

        // Execute through MCP (NOT directly)
        let mcp_request = crate::mcp::McpRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(simd_json::json!({
                "name": "execute_tool",
                "arguments": {
                    "tool_name": request.tool_name,
                    "arguments": request.params,
                }
            })),
            id: Some(Value::from(1u64)),
        };

        let result = self.mcp_compact.handle_request(mcp_request).await;

        if result.error.is_none() {
            let data = result.result.unwrap_or(Value::from(()));
            Ok(ChatResponse {
                message: format!(
                    "✅ Executed `{}`\n\n**Result:**\n```json
{}
```",
                    request.tool_name,
                    simd_json::to_string_pretty(&data).unwrap_or_default()
                ),
                intent: Intent::ToolExecution(request.clone()),
                actions_taken: vec![ActionTaken {
                    tool: request.tool_name.clone(),
                    params: request.params.clone(),
                    success: true,
                    execution_id: data.get("execution_id")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                }],
                suggestions: vec![],
            })
        } else {
            let error_msg = result.error
                .map(|e| e.message)
                .unwrap_or_else(|| "Unknown error".to_string());
            Ok(ChatResponse {
                message: format!(
                    "❌ Execution failed: {}",
                    error_msg
                ),
                intent: Intent::ToolExecution(request.clone()),
                actions_taken: vec![],
                suggestions: vec![],
            })
        }
    }

    async fn handle_workflow(
        &self,
        request: &WorkflowIntent,
        session: &ChatSession,
    ) -> Result<ChatResponse> {
        if !self.config.enable_workflows {
            return Ok(ChatResponse {
                message: "Workflow composition is disabled.".into(),
                intent: Intent::WorkflowRequest(request.clone()),
                actions_taken: vec![],
                suggestions: vec![],
            });
        }

        let plan = self.workflow_planner.plan_from_intent(request, session).await?;

        // Validate all tools exist
        for step in &plan.steps {
            if !self.mcp_compact.tool_registry_ref().is_loaded(&step.tool_name).await {
                return Ok(ChatResponse {
                    message: format!("❌ Workflow uses unknown tool: {}", step.tool_name),
                    intent: Intent::WorkflowRequest(request.clone()),
                    actions_taken: vec![],
                    suggestions: vec!["list tools".into()],
                });
            }
        }

        Ok(ChatResponse {
            message: format!(
                "## Proposed Workflow: {}

**Steps:**
{}

Type `approve workflow` to execute.",
                plan.workflow_id,
                plan.steps.iter().enumerate()
                    .map(|(i, s)| format!("{}. `{}` - {}", i + 1, s.tool_name, s.rationale))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            intent: Intent::WorkflowRequest(request.clone()),
            actions_taken: vec![],
            suggestions: vec!["approve workflow".into(), "cancel".into()],
        })
    }

    async fn handle_maintenance(
        &self,
        request: &MaintenanceIntent,
        _session: &ChatSession,
    ) -> Result<ChatResponse> {
        if !self.config.enable_maintenance {
            return Ok(ChatResponse {
                message: "Maintenance reasoning is disabled.".into(),
                intent: Intent::MaintenanceRequest(request.clone()),
                actions_taken: vec![],
                suggestions: vec![],
            });
        }

        // MaintenanceHandler doesn't have analyze method - use list_tasks for now
        let tasks = self.maintenance_handler.list_tasks();

        Ok(ChatResponse {
            message: format!(
                "## Maintenance Tasks\n\n{}
",
                tasks.iter().map(|t| format!("• {} - {}", t.name, t.description)).collect::<Vec<_>>().join("\n")
            ),
            intent: Intent::MaintenanceRequest(request.clone()),
            actions_taken: vec![],
            suggestions: vec![],
        })
    }

    async fn handle_inspector(
        &self,
        request: &InspectorIntent,
        _session: &ChatSession,
    ) -> Result<ChatResponse> {
        if !self.config.enable_inspector {
            return Ok(ChatResponse {
                message: "Inspector Gadget is disabled.".into(),
                intent: Intent::InspectorRequest(request.clone()),
                actions_taken: vec![],
                suggestions: vec![],
            });
        }

        let inspector = self.inspector.as_ref() 
            .ok_or_else(|| OpDbusError::ConfigError("Inspector not configured".into()))?;

        match request {
            InspectorIntent::Discover => {
                let result = inspector.discover().await?;
                Ok(ChatResponse {
                    message: format!(
                        "## Discovery Complete\n\n\t**ID:** {}
	**Objects Found:** {}
	**New Schemas:** {}
	**Duration:** {}ms

						⚠️ This was a ONE-SHOT discovery. Database is now authoritative.",
                        result.discovery_id,
                        result.stats.total_objects,
                        result.stats.new_schemas,
                        result.stats.duration_ms
                    ),
                    intent: Intent::InspectorRequest(request.clone()),
                    actions_taken: vec![],
                    suggestions: vec!["import discovery".into()],
                })
            }
            InspectorIntent::PlanMigration { source } => {
                let plan = inspector.plan_migration(source, "op-dbus").await?;
                Ok(ChatResponse {
                    message: format!(
                        "## Migration Plan: {}

						**From:** {}
						**Steps:** {}
						**Risk:** {}
						**Rollback:** {}

						Type `execute migration` to proceed.",
                        plan.plan_id,
                        plan.source_system,
                        plan.steps.len(),
                        plan.risk_level,
                        if plan.rollback_possible { "Yes" } else { "No" }
                    ),
                    intent: Intent::InspectorRequest(request.clone()),
                    actions_taken: vec![],
                    suggestions: vec!["execute migration".into(), "cancel".into()],
                })
            }
            InspectorIntent::OnboardSchema { subsystem } => {
                let result = inspector.onboard_schema(subsystem).await?;
                Ok(ChatResponse {
                    message: format!(
                        "## Schema Onboarding\n\n\t**Subsystem:** {}
	**Schemas Discovered:** {}
	**Registered:** {}",
                        result.subsystem,
                        result.schemas_discovered.len(),
                        result.schemas_registered
                    ),
                    intent: Intent::InspectorRequest(request.clone()),
                    actions_taken: vec![],
                    suggestions: vec![],
                })
            }
        }
    }

    async fn handle_disaster_recovery(
        &self,
        request: &DisasterRecoveryIntent,
        _session: &ChatSession,
    ) -> Result<ChatResponse> {
        let dr = self.disaster_recovery.as_ref() 
            .ok_or_else(|| OpDbusError::ConfigError("Disaster recovery not configured".into()))?;

        match request {
            DisasterRecoveryIntent::Export { path } => {
                if let Some(p) = path {
                    let stats = dr.export_to_file(std::path::Path::new(p)).await?;
                    Ok(ChatResponse {
                        message: format!(
                            "## Export Complete\n\n\t**File:** {}
	**Objects:** {}
	**Size:** {} bytes",
                            p, stats.object_count, stats.total_size_bytes
                        ),
                        intent: Intent::DisasterRecoveryRequest(request.clone()),
                        actions_taken: vec![],
                        suggestions: vec![],
                    })
                } else {
                    let export = dr.export().await?;
                    Ok(ChatResponse {
                        message: format!(
                            "## Export Ready\n\n\t**Objects:** {}
	**Plugins:** {}
	**Executions:** {}",
                            export.stats.object_count,
                            export.stats.plugin_count,
                            export.stats.execution_count
                        ),
                        intent: Intent::DisasterRecoveryRequest(request.clone()),
                        actions_taken: vec![],
                        suggestions: vec!["export /path/to/file.json".into()],
                    })
                }
            }
            DisasterRecoveryIntent::Import { path } => {
                let result = dr.import_from_file(std::path::Path::new(path), None).await?;
                Ok(ChatResponse {
                    message: format!(
                        "## Import Complete\n\n\t**Imported:** {}
	**Skipped:** {}
	**Errors:** {}",
                        result.objects_imported,
                        result.objects_skipped,
                        result.errors.len()
                    ),
                    intent: Intent::DisasterRecoveryRequest(request.clone()),
                    actions_taken: vec![],
                    suggestions: vec![],
                })
            }
            DisasterRecoveryIntent::Validate { path } => {
                let content = tokio::fs::read(path).await?;
                let mut content_mut = content;
                let export: crate::disaster_recovery::CanonicalExport = simd_json::from_slice(&mut content_mut)
                    .map_err(|e| OpDbusError::Serialization(e.to_string()))?;
                let result = dr.validate_export(&export)?;
                Ok(ChatResponse {
                    message: format!(
                        "## Validation Result\n\n\t**Valid:** {}
	**Errors:** {}
	**Warnings:** {}",
                        result.valid,
                        result.errors.len(),
                        result.warnings.len()
                    ),
                    intent: Intent::DisasterRecoveryRequest(request.clone()),
                    actions_taken: vec![],
                    suggestions: if result.valid {
                        vec![format!("import {}", path)]
                    } else {
                        vec![]
                    },
                })
            }
        }
    }

    async fn handle_explanation(
        &self,
        topic: &str,
        _session: &ChatSession,
    ) -> Result<ChatResponse> {
        let explanation = match topic.to_lowercase().as_str() {
            "tools" => "Tools are the ONLY mutation mechanism in OP-DBUS.".into(),
            "mcp" => "MCP is ingress-only. It does NOT execute tools directly.".into(),
            "orchestrator" => "The orchestrator expands requests into work stacks.".into(),
            "inspector" | "inspector gadget" => 
                "Inspector Gadget is ONE-SHOT only for discovery, migration, and schema onboarding. 
                It does NOT run continuously. After import, database is authoritative.".into(),
            "policy" => "Policies are first-class state objects that govern what can be executed.".into(),
            "disaster recovery" | "dr" => 
                "Disaster recovery exports/imports canonical JSON. Recovery uses the same execution contract.".into(),
            "antigravity" | "tunnel" =>
                "Antigravity tunnel is DEVELOPMENT-ONLY for IDE code-assist billing. Removed in production.".into(),
            _ => format!("No specific explanation for '{}'", topic),
        };

        Ok(ChatResponse {
            message: explanation,
            intent: Intent::Explanation(topic.to_string()),
            actions_taken: vec![],
            suggestions: vec![],
        })
    }

    async fn handle_unknown(
        &self,
        raw: &str,
        session: &ChatSession,
    ) -> Result<ChatResponse> {
        // Fallback to Antigravity Bridge if available
        #[cfg(feature = "dev-antigravity")]
        if let Some(bridge) = &self.bridge_client {
            let system_prompt = format!(
                "You are the OP-DBUS cognitive control plane. 
                The user has asked: '{}'. 
                You have access to the current session history. 
                Your goal is to help the user by suggesting relevant tools or explaining concepts. 
                Do NOT execute tools directly, but suggest the commands. 
                Current Context: {{:?}}", 
                raw,
                session.messages.last().map(|m| &m.content)
            );

            match bridge.chat(raw, Some(&system_prompt)).await {
                Ok(reply) => {
                     return Ok(ChatResponse {
                        message: reply,
                        intent: Intent::Unknown(raw.to_string()),
                        actions_taken: vec![],
                        suggestions: vec![],
                    });
                }
                Err(e) => {
                    tracing::warn!("Bridge fallback failed: {}", e);
                }
            }
        }

        Ok(ChatResponse {
            message: format!(
                "I'm not sure how to help with: \"{}\"\n\n\nI can help you with:\n\t• **list tools** - see available tools\n\t• **run <tool>** - execute a tool\n\t• **discover system** - one-shot system discovery\n\t• **export/import** - disaster recovery\n\t• **system status** - check system health",
                raw
            ),
            intent: Intent::Unknown(raw.to_string()),
            actions_taken: vec![],
            suggestions: vec!["list tools".into(), "system status".into()],
        })
    }

    async fn get_available_tools(&self) -> Result<Vec<ToolMetadata>> {
        Ok(self.mcp_compact.tool_registry_ref().list().await)
    }

    async fn get_filtered_tools(&self, filter: Option<&str>) -> Result<Vec<ToolMetadata>> {
        let all_tools = self.get_available_tools().await?;
        match filter {
            Some(f) => Ok(all_tools.into_iter()
                .filter(|t| t.name.contains(f) || t.description.to_lowercase().contains(&f.to_lowercase()))
                .collect()),
            None => Ok(all_tools),
        }
    }
}

// Intent types are now imported from intent.rs

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatResponse {
    pub message: String,
    pub intent: Intent,
    pub actions_taken: Vec<ActionTaken>,
    pub suggestions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionTaken {
    pub tool: String,
    pub params: Value,
    pub success: bool,
    pub execution_id: Option<String>,
}