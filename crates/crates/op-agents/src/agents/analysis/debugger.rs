//! Debugger Agent

use async_trait::async_trait;
use std::process::Command;

use crate::agents::base::{validation, AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;

const ALLOWED_DIRS: &[&str] = &["/tmp", "/home", "/opt", "/var/log"];

pub struct DebuggerAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl DebuggerAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            profile: SecurityProfile::read_only_analysis(
                "debugger",
                vec!["strace", "ltrace", "gdb"],
            ),
        }
    }

    fn read_logs(&self, path: Option<&str>, lines: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("tail");

        let num_lines = lines.unwrap_or("100");
        validation::validate_args(num_lines)?;
        cmd.arg("-n").arg(num_lines);

        if let Some(file) = path {
            let validated_path = validation::validate_path(file, ALLOWED_DIRS)?;
            cmd.arg(validated_path);
        } else {
            return Err("Log file path required".to_string());
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Log output:\n{}\n{}", stdout, stderr))
    }

    fn journalctl(&self, unit: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("journalctl");
        cmd.arg("--no-pager").arg("-n").arg("100");

        if let Some(u) = unit {
            validation::validate_args(u)?;
            cmd.arg("-u").arg(u);
        }

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("Journal output:\n{}\n{}", stdout, stderr))
    }

    fn process_info(&self, pid: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new("ps");
        cmd.arg("aux");

        let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        if let Some(p) = pid {
            validation::validate_args(p)?;
            let filtered: String = stdout
                .lines()
                .filter(|line| line.contains(p))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!("Process info:\n{}", filtered))
        } else {
            Ok(format!("All processes:\n{}", stdout))
        }
    }
}

#[async_trait]
impl AgentTrait for DebuggerAgent {
    fn agent_type(&self) -> &str {
        "debugger"
    }
    fn name(&self) -> &str {
        "Debugger"
    }
    fn description(&self) -> &str {
        "Debug logs and process analysis"
    }

    fn operations(&self) -> Vec<String> {
        vec![
            "logs".to_string(),
            "journal".to_string(),
            "process".to_string(),
        ]
    }

    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }

    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        let result = match task.operation.as_str() {
            "logs" => self.read_logs(task.path.as_deref(), task.args.as_deref()),
            "journal" => self.journalctl(task.path.as_deref()),
            "process" => self.process_info(task.path.as_deref()),
            _ => Err(format!("Unknown operation: {}", task.operation)),
        };

        match result {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
