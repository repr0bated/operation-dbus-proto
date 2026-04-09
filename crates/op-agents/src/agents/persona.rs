use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::{profiles::presets, SecurityProfile};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaConfig {
    pub agent_type: String,
    pub name: String,
    pub description: String,
    pub operations: Vec<String>,
    pub system_prompt: String,
    pub capabilities: Vec<String>,
    pub security_profile: String,
}

pub struct PersonaAgent {
    agent_id: String,
    config: PersonaConfig,
    profile: SecurityProfile,
}

impl PersonaAgent {
    pub fn new(agent_id: String, config: PersonaConfig) -> Self {
        let profile = match config.security_profile.as_str() {
            "python_pro" => presets::python_pro(),
            "rust_pro" => presets::rust_pro(),
            "golang_pro" => presets::golang_pro(),
            "javascript_pro" => presets::javascript_pro(),
            "typescript_pro" => presets::typescript_pro(),
            "code_reviewer" => presets::code_reviewer(),
            "security_auditor" => presets::security_auditor(),
            "docs_architect" => presets::docs_architect(),
            "tdd_orchestrator" => presets::tdd_orchestrator(),
            "default" | _ => SecurityProfile::content_generation(&config.agent_type),
        };

        Self {
            agent_id,
            config,
            profile,
        }
    }

    pub fn config(&self) -> &PersonaConfig {
        &self.config
    }
}

#[async_trait]
impl AgentTrait for PersonaAgent {
    fn agent_type(&self) -> &str {
        &self.config.agent_type
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn operations(&self) -> Vec<String> {
        self.config.operations.clone()
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != self.config.agent_type {
            return Err(format!("Invalid task type: {}", task.task_type));
        }

        let result_json = simd_json::json!({
            "system_prompt": self.config.system_prompt,
            "operation": task.operation,
            "args": task.args,
            "message": format!("{} executing {}", self.name(), task.operation)
        });

        let data = simd_json::to_string(&result_json).unwrap_or_default();
        Ok(TaskResult::success(&task.operation, data))
    }

    fn get_status(&self) -> String {
        format!("{} agent {} is running", self.name(), self.agent_id)
    }
}
