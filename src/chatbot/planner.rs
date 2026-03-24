//! Workflow planner.
//! 
//! The planner takes cognitive results and produces execution plans.
//! It NEVER executes - only plans. Execution is delegated to orchestrator.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use uuid::Uuid;

use crate::error::Result;
use op_tools::registry::ToolRegistry;
use crate::work_stack::{WorkStack, WorkStackBuilder};

use super::cognitive::{CognitiveResult, ReasoningTrace, ToolMapping};

/// A planned step in the workflow
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlanStep {
    /// Step order
    pub order: u32,
    /// Tool to execute
    pub tool_name: String,
    /// Parameters for tool
    pub params: Value,
    /// Dependencies on other steps (by order)
    pub depends_on: Vec<u32>,
    /// Expected output schema (for validation)
    pub expected_output: Option<Value>,
    /// Rationale for this step
    pub rationale: String,
}

/// A complete planned workflow
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlannedWorkflow {
    /// Unique workflow identifier
    pub workflow_id: String,
    /// Steps in the workflow
    pub steps: Vec<PlanStep>,
    /// Reasoning trace that produced this plan
    pub reasoning_trace: Option<ReasoningTrace>,
    /// Overall confidence in the plan
    pub confidence: f64,
}

impl PlannedWorkflow {
    /// Check if this is a single-tool workflow
    pub fn is_single_tool(&self) -> bool {
        self.steps.len() == 1
    }

    /// Get the primary tool (first/only tool)
    pub fn primary_tool(&self) -> Option<&str> {
        self.steps.first().map(|s| s.tool_name.as_str())
    }
}

/// Workflow planner
///
/// Plans execution but NEVER executes.
pub struct WorkflowPlanner {
    tool_registry: Arc<ToolRegistry>,
}

impl WorkflowPlanner {
    pub fn new(
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            tool_registry,
        }
    }

    /// Plan a workflow from cognitive result
    ///
    /// This produces a plan - it does NOT execute.
    pub async fn plan(&self, cognitive: &CognitiveResult) -> Result<PlannedWorkflow> {
        let workflow_id = Uuid::new_v4().to_string();
        let mut steps = Vec::new();

        // If no tools mapped, plan a user.respond
        if cognitive.tool_mappings.is_empty() {
            steps.push(PlanStep {
                order: 0,
                tool_name: "user.respond".to_string(),
                params: simd_json::json!({
                    "message": "I could not determine an appropriate action for your request.",
                    "format": "text",
                }),
                depends_on: Vec::new(),
                expected_output: None,
                rationale: "No tools matched intent - responding with clarification request".to_string(),
            });

            return Ok(PlannedWorkflow {
                workflow_id,
                steps,
                reasoning_trace: Some(cognitive.reasoning_trace.clone()),
                confidence: 0.3,
            });
        }

        // Single tool case
        if cognitive.tool_mappings.len() == 1 {
            let mapping = &cognitive.tool_mappings[0];
            steps.push(self.create_step_from_mapping(0, mapping));
        } else {
            // Multi-tool case - create dependency chain
            for (i, mapping) in cognitive.tool_mappings.iter().enumerate() {
                let mut step = self.create_step_from_mapping(i as u32, mapping);
                
                // Each step depends on previous (simple chain)
                // Production would analyze data dependencies
                if i > 0 {
                    step.depends_on.push((i - 1) as u32);
                }
                
                steps.push(step);
            }
        }

        // Always end with user.respond to report results
        let needs_respond = !steps.iter().any(|s| s.tool_name == "user.respond");
        if needs_respond && !steps.is_empty() {
            let last_order = steps.len() as u32;
            steps.push(PlanStep {
                order: last_order,
                tool_name: "user.respond".to_string(),
                params: simd_json::json!({
                    "message": "{{previous_output}}",
                    "format": "json",
                }),
                depends_on: vec![last_order - 1],
                expected_output: None,
                rationale: "Report execution results to user".to_string(),
            });
        }

        let confidence = cognitive.reasoning_trace.confidence;

        Ok(PlannedWorkflow {
            workflow_id,
            steps,
            reasoning_trace: Some(cognitive.reasoning_trace.clone()),
            confidence,
        })
    }

    /// Plan from a workflow intent (simpler interface for chatbot)
    pub async fn plan_from_intent(
        &self,
        intent: &super::intent::WorkflowIntent,
        _session: &super::session::ChatSession,
    ) -> Result<PlannedWorkflow> {
        // Create a simple plan based on the workflow description
        // In production, this would use LLM to map intent to tools
        let workflow_id = Uuid::new_v4().to_string();
        
        let step = PlanStep {
            order: 0,
            tool_name: "user.respond".to_string(),
            params: simd_json::json!({
                "message": format!("Workflow planning for: {}\n\nGoal: {}", intent.description, intent.goal),
                "format": "text",
            }),
            depends_on: Vec::new(),
            expected_output: None,
            rationale: intent.description.clone(),
        };

        Ok(PlannedWorkflow {
            workflow_id,
            steps: vec![step],
            reasoning_trace: None,
            confidence: 0.5,
        })
    }

    /// Create a plan step from a tool mapping
    fn create_step_from_mapping(&self, order: u32, mapping: &ToolMapping) -> PlanStep {
        PlanStep {
            order,
            tool_name: mapping.tool_name.clone(),
            params: mapping.params.clone(),
            depends_on: Vec::new(),
            expected_output: None,
            rationale: mapping.rationale.clone(),
        }
    }

    /// Build a work stack from a planned workflow
    ///
    /// This converts the plan to an orchestrator work stack.
    /// Still does NOT execute - that's the orchestrator's job.
    pub fn build_work_stack(&self, workflow: &PlannedWorkflow) -> Result<WorkStack> {
        let mut builder = WorkStackBuilder::new()
            .stack_id(&workflow.workflow_id);

        for step in &workflow.steps {
            let node_id = format!("step_{}", step.order);
            builder = builder.add_tool(&node_id, &step.tool_name, step.params.clone());

            // Add edges for dependencies
            for dep in &step.depends_on {
                let dep_node_id = format!("step_{}", dep);
                builder = builder.add_edge(&dep_node_id, &node_id);
            }
        }

        Ok(builder.build())
    }

    /// Validate a workflow plan
    pub async fn validate(&self, workflow: &PlannedWorkflow) -> Result<()> {
        // Check all tools exist
        for step in &workflow.steps {
            if self.tool_registry.get(&step.tool_name).await.is_none()
                && !step.tool_name.starts_with("mcp.")
                && step.tool_name != "user.respond"
            {
                return Err(crate::error::OpDbusError::ToolNotFound(
                    step.tool_name.clone()
                ));
            }
        }

        // Check dependency graph is valid (no cycles)
        // Simplified - production would do full cycle detection
        for step in &workflow.steps {
            for dep in &step.depends_on {
                if *dep >= step.order {
                    return Err(crate::error::OpDbusError::InvalidWorkStack(
                        format!("Step {} depends on later step {}", step.order, dep)
                    ));
                }
            }
        }

        Ok(())
    }
}