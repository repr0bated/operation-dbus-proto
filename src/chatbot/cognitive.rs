//! Cognitive reasoning engine.
//!
//! The cognitive engine understands user intent and maps it to tools.
//! It reasons but NEVER executes - all execution is delegated.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;

use crate::error::{OpDbusError, Result};
use crate::mcp::McpCompactDispatcher;
use op_tools::registry::{ToolRegistry, ToolDefinition};

use super::session::ChatSession;

/// Reasoning trace for audit and explanation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReasoningTrace {
    /// Steps in the reasoning process
    pub steps: Vec<ReasoningStep>,
    /// Tools considered
    pub tools_considered: Vec<String>,
    /// Tools selected
    pub tools_selected: Vec<String>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Anti-hallucination checks passed
    pub hallucination_checks: Vec<HallucinationCheck>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReasoningStep {
    pub step_number: u32,
    pub description: String,
    pub input: Value,
    pub output: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HallucinationCheck {
    pub check_type: String,
    pub passed: bool,
    pub details: String,
}

/// Result of cognitive reasoning
#[derive(Debug, Clone)]
pub struct CognitiveResult {
    /// Understood user intent
    pub intent: UserIntent,
    /// Mapped tools
    pub tool_mappings: Vec<ToolMapping>,
    /// Reasoning trace
    pub reasoning_trace: ReasoningTrace,
    /// Session context used
    pub session_context: SessionContext,
}

/// Parsed user intent
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserIntent {
    /// Primary action requested
    pub action: IntentAction,
    /// Target entities
    pub targets: Vec<String>,
    /// Parameters extracted
    pub parameters: Value,
    /// Confidence in understanding
    pub confidence: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IntentAction {
    /// Execute a specific operation
    Execute,
    /// Query/retrieve information
    Query,
    /// List/enumerate resources
    List,
    /// Create a new resource
    Create,
    /// Update an existing resource
    Update,
    /// Delete a resource
    Delete,
    /// Explain something
    Explain,
    /// Help/guidance request
    Help,
    /// Maintenance operation
    Maintain,
    /// Unknown - requires clarification
    Unknown,
}

/// Mapping from intent to tool
#[derive(Debug, Clone)]
pub struct ToolMapping {
    pub tool_name: String,
    pub params: Value,
    pub confidence: f64,
    pub rationale: String,
}

/// Session context for reasoning
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    pub previous_tools: Vec<String>,
    pub previous_intents: Vec<IntentAction>,
    pub entities_mentioned: Vec<String>,
}

/// Cognitive reasoning engine
pub struct CognitiveEngine {
    max_depth: u32,
    anti_hallucination: bool,
    tool_registry: Arc<ToolRegistry>,
}

impl CognitiveEngine {
    pub fn new(
        max_depth: u32,
        anti_hallucination: bool,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            max_depth,
            anti_hallucination,
            tool_registry,
        }
    }

    /// Reason about user input and map to tools
    ///
    /// This is the core cognitive function.
    /// It understands intent but NEVER executes.
    pub async fn reason(
        &self,
        input: &str,
        session: &ChatSession,
        mcp: &McpCompactDispatcher,
    ) -> Result<CognitiveResult> {
        let mut trace = ReasoningTrace {
            steps: Vec::new(),
            tools_considered: Vec::new(),
            tools_selected: Vec::new(),
            confidence: 0.0,
            hallucination_checks: Vec::new(),
        };

        // Step 1: Parse intent
        let intent = self.parse_intent(input, &mut trace)?;

        // Step 2: Build session context
        let session_context = self.build_session_context(session);

        // Step 3: Discover available tools
        let available_tools = self.discover_tools(mcp, &mut trace).await?;

        // Step 4: Map intent to tools
        let tool_mappings = self.map_intent_to_tools(
            &intent,
            &available_tools,
            &session_context,
            &mut trace,
        )?;

        // Step 5: Anti-hallucination checks
        if self.anti_hallucination {
            let tool_names: Vec<String> = available_tools.iter().map(|t| t.name.clone()).collect();
            self.run_hallucination_checks(&tool_mappings, &tool_names, &mut trace)?;
        }

        // Calculate overall confidence
        trace.confidence = if tool_mappings.is_empty() {
            0.0
        } else {
            tool_mappings.iter().map(|m| m.confidence).sum::<f64>() 
                / tool_mappings.len() as f64
        };

        Ok(CognitiveResult {
            intent,
            tool_mappings,
            reasoning_trace: trace,
            session_context,
        })
    }

    /// Parse user intent from natural language
    fn parse_intent(
        &self,
        input: &str,
        trace: &mut ReasoningTrace,
    ) -> Result<UserIntent> {
        let input_lower = input.to_lowercase();

        // Keyword-based intent detection (production would use NLU)
        let action = if input_lower.contains("list") || input_lower.contains("show") {
            IntentAction::List
        } else if input_lower.contains("create") || input_lower.contains("add") || input_lower.contains("new") {
            IntentAction::Create
        } else if input_lower.contains("delete") || input_lower.contains("remove") {
            IntentAction::Delete
        } else if input_lower.contains("update") || input_lower.contains("modify") || input_lower.contains("change") {
            IntentAction::Update
        } else if input_lower.contains("explain") || input_lower.contains("what is") || input_lower.contains("how") {
            IntentAction::Explain
        } else if input_lower.contains("help") {
            IntentAction::Help
        } else if input_lower.contains("get") || input_lower.contains("fetch") || input_lower.contains("query") {
            IntentAction::Query
        } else if input_lower.contains("maintain") || input_lower.contains("fix") || input_lower.contains("repair") {
            IntentAction::Maintain
        } else if input_lower.contains("run") || input_lower.contains("execute") || input_lower.contains("do") {
            IntentAction::Execute
        } else {
            IntentAction::Unknown
        };

        // Extract targets (simplified - production would use NER)
        let targets = self.extract_targets(input);

        // Extract parameters
        let parameters = self.extract_parameters(input);

        let confidence = match action {
            IntentAction::Unknown => 0.3,
            _ => 0.7,
        };

        trace.steps.push(ReasoningStep {
            step_number: 1,
            description: "Intent parsing".to_string(),
            input: simd_json::json!({"raw_input": input}),
            output: simd_json::json!({
                "action": action,
                "confidence": confidence,
            }),
        });

        Ok(UserIntent {
            action,
            targets,
            parameters,
            confidence,
        })
    }

    /// Extract target entities from input
    fn extract_targets(&self, input: &str) -> Vec<String> {
        let mut targets = Vec::new();
        
        // Look for common target patterns
        let keywords = ["interface", "network", "service", "container", "pod", 
                       "user", "plugin", "tool", "policy", "object",
                       "ram", "memory", "cpu", "disk"];
        
        for keyword in keywords {
            if input.to_lowercase().contains(keyword) {
                targets.push(keyword.to_string());
            }
        }

        targets
    }

    /// Extract parameters from input
    fn extract_parameters(&self, input: &str) -> Value {
        // Simplified parameter extraction
        simd_json::json!({
            "raw_input": input,
        })
    }

    /// Build session context
    fn build_session_context(&self, session: &ChatSession) -> SessionContext {
        SessionContext {
            previous_tools: session.executed_tools.clone(),
            previous_intents: Vec::new(), // Would track from history
            entities_mentioned: session.entities.clone(),
        }
    }

    /// Discover available tools via MCP
    async fn discover_tools(
        &self,
        _mcp: &McpCompactDispatcher,
        trace: &mut ReasoningTrace,
    ) -> Result<Vec<ToolDefinition>> {
        // Use MCP compact mode to discover tools
        let tools = self.tool_registry.list().await;
        
        trace.tools_considered = tools.iter().map(|t| t.name.clone()).collect();
        
        trace.steps.push(ReasoningStep {
            step_number: 2,
            description: "Tool discovery via MCP".to_string(),
            input: simd_json::json!({}),
            output: simd_json::json!({
                "tools_found": tools.len(),
            }),
        });

        Ok(tools)
    }

    /// Map intent to specific tools
    fn map_intent_to_tools(
        &self,
        intent: &UserIntent,
        available_tools: &[ToolDefinition],
        context: &SessionContext,
        trace: &mut ReasoningTrace,
    ) -> Result<Vec<ToolMapping>> {
        let mut mappings = Vec::new();

        // Match intent action to tool patterns
        for tool_def in available_tools {
            let tool_name = &tool_def.name;
            let confidence = self.calculate_tool_match_confidence(
                intent,
                tool_name,
                context,
            );

            if confidence > 0.5 {
                mappings.push(ToolMapping {
                    tool_name: tool_name.clone(),
                    params: intent.parameters.clone(),
                    confidence,
                    rationale: format!(
                        "Tool '{}' matches intent action '{:?}' with confidence {:.2}",
                        tool_name, intent.action, confidence
                    ),
                });
            }
        }

        // Sort by confidence
        mappings.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        // Take top matches
        let selected: Vec<_> = mappings.into_iter().take(5).collect();
        
        trace.tools_selected = selected.iter().map(|m| m.tool_name.clone()).collect();
        
        trace.steps.push(ReasoningStep {
            step_number: 3,
            description: "Tool mapping".to_string(),
            input: simd_json::json!({"intent": intent}),
            output: simd_json::json!({
                "mappings": trace.tools_selected.len(),
            }),
        });

        Ok(selected)
    }

    /// Calculate confidence of tool matching intent
    fn calculate_tool_match_confidence(
        &self,
        intent: &UserIntent,
        tool_name: &str,
        _context: &SessionContext,
    ) -> f64 {
        let mut confidence: f64 = 0.0;

        // Match by action prefix
        let tool_lower = tool_name.to_lowercase();
        
        match intent.action {
            IntentAction::List => {
                if tool_lower.contains("list") { confidence += 0.8; }
            }
            IntentAction::Create => {
                if tool_lower.contains("create") { confidence += 0.8; }
            }
            IntentAction::Delete => {
                if tool_lower.contains("delete") { confidence += 0.8; }
            }
            IntentAction::Query => {
                if tool_lower.contains("get") || tool_lower.contains("query") { 
                    confidence += 0.8; 
                }
            }
            IntentAction::Help => {
                if tool_lower.contains("help") || tool_lower == "mcp.list_tools" {
                    confidence += 0.9;
                }
            }
            IntentAction::Execute => {
                // General execution - match target entities
                for target in &intent.targets {
                    if tool_lower.contains(&target.to_lowercase()) {
                        confidence += 0.3;
                    }
                }
            }
            IntentAction::Explain => {
                if tool_lower == "user.respond" {
                    confidence += 0.9; // Explanations go through respond
                }
            }
            _ => {}
        }

        // Match by target entities
        for target in &intent.targets {
            if tool_lower.contains(&target.to_lowercase()) {
                confidence += 0.2;
            }
            
            // Special mapping for system resources
            if (target == "ram" || target == "memory" || target == "cpu" || target == "disk") 
                && tool_name == "inspector.discover" {
                confidence += 0.8;
            }
        }

        // Special case: user.respond always has baseline confidence
        if tool_name == "user.respond" {
            confidence = confidence.max(0.6f64);
        }

        confidence.min(1.0f64)
    }

    /// Run anti-hallucination checks
    fn run_hallucination_checks(
        &self,
        mappings: &[ToolMapping],
        available_tools: &[String],
        trace: &mut ReasoningTrace,
    ) -> Result<()> {
        // Check 1: All selected tools must exist
        let tools_exist_check = {
            let all_exist = mappings.iter().all(|m| {
                available_tools.contains(&m.tool_name)
            });
            HallucinationCheck {
                check_type: "tools_exist".to_string(),
                passed: all_exist,
                details: if all_exist {
                    "All selected tools exist in registry".to_string()
                } else {
                    "Some selected tools do not exist".to_string()
                },
            }
        };
        trace.hallucination_checks.push(tools_exist_check.clone());

        if !tools_exist_check.passed {
            return Err(OpDbusError::PolicyViolation(
                "Anti-hallucination: Referenced non-existent tools".into()
            ));
        }

        // Check 2: Confidence threshold
        let confidence_check = {
            let sufficient_confidence = mappings.iter().any(|m| m.confidence >= 0.5);
            HallucinationCheck {
                check_type: "confidence_threshold".to_string(),
                passed: sufficient_confidence || mappings.is_empty(),
                details: format!("Confidence threshold check: {}", sufficient_confidence),
            }
        };
        trace.hallucination_checks.push(confidence_check);

        trace.steps.push(ReasoningStep {
            step_number: 4,
            description: "Anti-hallucination checks".to_string(),
            input: simd_json::json!({}),
            output: simd_json::json!({
                "checks_passed": trace.hallucination_checks.iter().all(|c| c.passed),
            }),
        });

        Ok(())
    }
}