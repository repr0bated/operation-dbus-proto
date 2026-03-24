//! Network Engineer Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

pub struct NetworkEngineerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl NetworkEngineerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "network-engineer",
                vec!["ip", "ss", "netstat", "dig", "ping", "traceroute"],
            ),
        }
    }

    fn show_interfaces(&self) -> Result<String, String> {
        let mut cmd = Command::new("ip");
        cmd.arg("addr").arg("show");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Network interfaces:\n{}\n{}", stdout, stderr))
    }

    fn show_routes(&self) -> Result<String, String> {
        let mut cmd = Command::new("ip");
        cmd.arg("route").arg("show");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Routes:\n{}\n{}", stdout, stderr))
    }

    fn show_connections(&self) -> Result<String, String> {
        let mut cmd = Command::new("ss");
        cmd.arg("-tuln");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Active connections:\n{}\n{}", stdout, stderr))
    }

    fn dns_lookup(&self, host: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("dig");

        if let Some(h) = host {
            validation::validate_args(h)?;
            cmd.arg(h);
        } else {
            return Err("Host required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("DNS lookup:\n{}\n{}", stdout, stderr))
    }
}

#[async_trait]
impl AgentTrait for NetworkEngineerAgent {
    fn agent_type(&self) -> &str {
        "network-engineer"
    }
    fn name(&self) -> &str {
        "Network Engineer"
    }
    fn description(&self) -> &str {
        "Network diagnostics and analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "interfaces".to_string(),
            "routes".to_string(),
            "connections".to_string(),
            "dns".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "interfaces" => self.show_interfaces(),
            "routes" => self.show_routes(),
            "connections" => self.show_connections(),
            "dns" => self.dns_lookup(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
