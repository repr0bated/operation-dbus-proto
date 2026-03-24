//! Base Persona Agent Implementation

use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashSet;

use super::super::agent_trait::{
    UnifiedAgent, AgentCategory, AgentCapability, AgentRequest, AgentResponse
};
use crate::security::SecurityProfile;

/// Base implementation for persona agents (LLM-only, no execution)
pub struct PersonaAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub system_prompt: String,
    pub knowledge: String,
    pub capabilities: HashSet<AgentCapability>,
    pub examples: Vec<(String, String)>,
}

impl PersonaAgent {
    /// Create a new persona agent
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        domain: &str,
        system_prompt: &str,
        knowledge: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            domain: domain.to_string(),
            system_prompt: system_prompt.to_string(),
            knowledge: knowledge.to_string(),
            capabilities: HashSet::new(),
            examples: vec![],
        }
    }

    /// Add a capability
    pub fn with_capability(mut self, cap: AgentCapability) -> Self {
        self.capabilities.insert(cap);
        self
    }

    /// Add an example interaction
    pub fn with_example(mut self, question: &str, answer: &str) -> Self {
        self.examples.push((question.to_string(), answer.to_string()));
        self
    }

    /// Generate augmented prompt for LLM
    pub fn augmented_prompt(&self, user_query: &str) -> String {
        let mut prompt = self.system_prompt.clone();
        
        if !self.knowledge.is_empty() {
            prompt.push_str("\n\n## Domain Knowledge\n");
            prompt.push_str(&self.knowledge);
        }

        if !self.examples.is_empty() {
            prompt.push_str("\n\n## Example Interactions\n");
            for (q, a) in &self.examples {
                prompt.push_str(&format!("\nQ: {}\nA: {}\n", q, a));
            }
        }

        prompt.push_str(&format!("\n\n## Current Query\n{}", user_query));
        prompt
    }
}

#[async_trait]
impl UnifiedAgent for PersonaAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> AgentCategory {
        AgentCategory::Persona
    }

    fn capabilities(&self) -> HashSet<AgentCapability> {
        self.capabilities.clone()
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    fn knowledge_base(&self) -> Option<&str> {
        if self.knowledge.is_empty() {
            None
        } else {
            Some(&self.knowledge)
        }
    }

    fn examples(&self) -> Vec<(&str, &str)> {
        self.examples.iter()
            .map(|(q, a)| (q.as_str(), a.as_str()))
            .collect()
    }

    fn security_profile(&self) -> Option<&SecurityProfile> {
        None // Persona agents don't execute code
    }

    fn operations(&self) -> Vec<&str> {
        vec!["consult", "review", "explain", "recommend"]
    }

    async fn execute(&self, request: AgentRequest) -> AgentResponse {
        // Persona agents don't execute - they augment LLM prompts
        // Return the augmented prompt for the LLM to process
        let query = request.args.get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let augmented = self.augmented_prompt(query);

        AgentResponse::success(
            json!({
                "augmented_prompt": augmented,
                "domain": self.domain,
                "agent": self.id,
            }),
            "Prompt augmented with domain expertise"
        )
    }
}
