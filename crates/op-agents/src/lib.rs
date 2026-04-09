//! op-agents: Agent implementations for op-dbus
//!
//! This crate provides agent types and the factory function to create them.
//! Agents are domain-specific AI assistants that can be invoked via D-Bus or MCP.

pub mod agent_catalog;
pub mod agent_registry;
pub mod agents;
pub mod dbus_service;
pub mod router;
pub mod security;

// Re-export key types
pub use agent_catalog::{builtin_agent_descriptors, AgentDescriptor};
pub use agent_registry::{AgentRegistry, AgentStatus};
pub use agents::base::{AgentTask, AgentTrait, TaskResult};
pub use agents::*;
pub use router::{create_router, AgentsServiceRouter, AgentsState};

/// Create an agent by type name
///
/// This is the factory function that agent tools and D-Bus services use.
///
/// # Arguments
/// * `agent_type` - The type of agent (e.g., "rust-pro", "memory", "sequential-thinking")
/// * `agent_id` - Unique identifier for this agent instance
///
/// # Returns
/// A boxed agent trait object, or error if type is unknown
pub fn create_agent(
    agent_type: &str,
    agent_id: String,
) -> Result<Box<dyn AgentTrait + Send + Sync>, String> {
    use agents::{
        aiml::{
            AIEngineerAgent, DataEngineerAgent, DataScientistAgent, MLEngineerAgent,
            MLOpsEngineerAgent, PromptEngineerAgent,
        },
        operations::{DevOpsTroubleshooterAgent, IncidentResponderAgent, TestAutomatorAgent},
        orchestration::{
            ContextManagerAgent, DxOptimizerAgent, MemoryAgent, SequentialThinkingAgent,
            TddOrchestratorAgent,
        },
        persona::PersonaAgent,
    };

    let agent_type_normalized = agent_type.replace("_", "-");

    let agent: Option<Box<dyn AgentTrait + Send + Sync>> = match agent_type_normalized.as_str() {
        // Orchestration agents
        "memory" => Some(Box::new(MemoryAgent::new(agent_id.clone()))),
        "context-manager" => Some(Box::new(ContextManagerAgent::new(agent_id.clone()))),
        "sequential-thinking" => Some(Box::new(SequentialThinkingAgent::new(agent_id.clone()))),
        "dx-optimizer" => Some(Box::new(DxOptimizerAgent::new(agent_id.clone()))),
        "tdd-orchestrator" => Some(Box::new(TddOrchestratorAgent::new(agent_id.clone()))),

        // AI/ML agents
        "prompt-engineer" => Some(Box::new(PromptEngineerAgent::new(agent_id.clone()))),
        "ai-engineer" => Some(Box::new(AIEngineerAgent::new(agent_id.clone()))),
        "ml-engineer" => Some(Box::new(MLEngineerAgent::new(agent_id.clone()))),
        "mlops-engineer" => Some(Box::new(MLOpsEngineerAgent::new(agent_id.clone()))),
        "data-scientist" => Some(Box::new(DataScientistAgent::new(agent_id.clone()))),
        "data-engineer" => Some(Box::new(DataEngineerAgent::new(agent_id.clone()))),

        // Operations agents
        "devops-troubleshooter" => Some(Box::new(DevOpsTroubleshooterAgent::new(agent_id.clone()))),
        "incident-responder" => Some(Box::new(IncidentResponderAgent::new(agent_id.clone()))),
        "test-automator" => Some(Box::new(TestAutomatorAgent::new(agent_id.clone()))),

        _ => None,
    };

    if let Some(a) = agent {
        return Ok(a);
    }

    // Look up in personas.yaml
    let path = std::env::var("OP_AGENT_PERSONAS_PATH")
        .unwrap_or_else(|_| "config/agents/personas.yaml".to_string());

    let personas = agent_catalog::load_builtin_personas(&path);
    if let Some(config) = personas
        .into_iter()
        .find(|p| p.agent_type == agent_type_normalized)
    {
        return Ok(Box::new(PersonaAgent::new(agent_id, config)));
    }

    Err(format!("Unknown agent type: {}", agent_type))
}

/// List all available agent types
pub fn list_agent_types() -> Vec<String> {
    let mut types = vec![
        "memory".to_string(),
        "context-manager".to_string(),
        "sequential-thinking".to_string(),
        "dx-optimizer".to_string(),
        "tdd-orchestrator".to_string(),
        "prompt-engineer".to_string(),
        "ai-engineer".to_string(),
        "ml-engineer".to_string(),
        "mlops-engineer".to_string(),
        "data-scientist".to_string(),
        "data-engineer".to_string(),
        "devops-troubleshooter".to_string(),
        "incident-responder".to_string(),
        "test-automator".to_string(),
    ];

    let path = std::env::var("OP_AGENT_PERSONAS_PATH")
        .unwrap_or_else(|_| "config/agents/personas.yaml".to_string());
    let personas = agent_catalog::load_builtin_personas(&path);
    for p in personas {
        types.push(p.agent_type);
    }

    types
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_agent() {
        let agent = create_agent("memory", "test-1".to_string());
        assert!(agent.is_ok());
        let agent = agent.unwrap();
        assert_eq!(agent.agent_type(), "memory");
    }

    #[test]
    fn test_create_agent_underscore_variant() {
        let agent = create_agent("dx_optimizer", "test-2".to_string());
        assert!(agent.is_ok());
    }

    #[test]
    fn test_unknown_agent() {
        let agent = create_agent("unknown-agent", "test-3".to_string());
        assert!(agent.is_err());
    }
}
