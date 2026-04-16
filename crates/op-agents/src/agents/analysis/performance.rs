//! Performance Engineer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct PerformanceEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl PerformanceEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "performance-engineer",
                vec!["top", "htop", "vmstat", "iostat"],
            ),
        }
    }

    fn system_stats(&self) -> Result<String, String> {
        let mut cmd = Command::new("vmstat");
        cmd.arg("1").arg("5");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("System stats:\n{}\n{}", stdout, stderr))
    }

    fn io_stats(&self) -> Result<String, String> {
        let mut cmd = Command::new("iostat");
        cmd.arg("-x").arg("1").arg("3");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("I/O stats:\n{}\n{}", stdout, stderr))
    }

    fn memory_info(&self) -> Result<String, String> {
        let mut cmd = Command::new("free");
        cmd.arg("-h");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Memory info:\n{}\n{}", stdout, stderr))
    }

    fn cpu_info(&self) -> Result<String, String> {
        let mut cmd = Command::new("lscpu");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("CPU info:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for PerformanceEngineerAgent {
    fn agent_type(&self) -> &str {
        "performance-engineer"
    }
    fn name(&self) -> &str {
        "Performance Engineer"
    }
    fn description(&self) -> &str {
        "System performance analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "vmstat".to_string(),
            "iostat".to_string(),
            "memory".to_string(),
            "cpu".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "vmstat" => self.system_stats(),
            "iostat" => self.io_stats(),
            "memory" => self.memory_info(),
            "cpu" => self.cpu_info(),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
