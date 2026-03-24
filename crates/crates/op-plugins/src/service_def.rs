//! Systemd plugin for service management
//!
//! Schema-as-code: These types ARE the schema. Validation happens at parse time.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

/// Service name - validated on construction
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServiceName(String);

impl ServiceName {
    pub fn new(name: impl Into<String>) -> Result<Self, ValidationError> {
        let name = name.into();
        if name.is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if name.len() > 64 {
            return Err(ValidationError::NameTooLong(name.len()));
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '@')
        {
            return Err(ValidationError::InvalidChars(name));
        }
        if name.starts_with('-') || name.starts_with('.') {
            return Err(ValidationError::InvalidStart(name));
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ServiceName {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}
impl From<ServiceName> for String {
    fn from(n: ServiceName) -> String {
        n.0
    }
}
impl std::fmt::Display for ServiceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("service name cannot be empty")]
    EmptyName,
    #[error("service name exceeds 64 chars: {0}")]
    NameTooLong(usize),
    #[error("service name contains invalid characters: {0}")]
    InvalidChars(String),
    #[error("service name cannot start with - or .: {0}")]
    InvalidStart(String),
    #[error("command path must be absolute: {0}")]
    RelativePath(String),
    #[error("invalid resource limit: {0}")]
    InvalidResource(String),
}

/// Service type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    #[default]
    Simple,
    Forking {
        pid_file: Option<PathBuf>,
    },
    Oneshot,
    Notify,
}

/// Active state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActiveState {
    Active,
    Inactive,
    Activating,
    Deactivating,
    Failed,
    Reloading,
}

/// Command to execute - validated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecCommand {
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
}

impl ExecCommand {
    pub fn new(program: impl Into<PathBuf>, args: Vec<String>) -> Result<Self, ValidationError> {
        let program = program.into();
        if !program.is_absolute() {
            return Err(ValidationError::RelativePath(program.display().to_string()));
        }
        Ok(Self { program, args })
    }

    pub fn to_command_line(&self) -> String {
        let mut cmd = self.program.display().to_string();
        for arg in &self.args {
            cmd.push(' ');
            if arg.contains(' ') {
                cmd.push('"');
                cmd.push_str(arg);
                cmd.push('"');
            } else {
                cmd.push_str(arg);
            }
        }
        cmd
    }
}

/// Resource limits
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub memory_max: Option<u64>,
    pub cpu_quota: Option<f32>,
    pub tasks_max: Option<u32>,
}

impl ResourceLimits {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(mem) = self.memory_max {
            if mem < 1024 * 1024 {
                return Err(ValidationError::InvalidResource("memory_max < 1MB".into()));
            }
        }
        if let Some(cpu) = self.cpu_quota {
            if cpu <= 0.0 || cpu > 100.0 {
                return Err(ValidationError::InvalidResource(
                    "cpu_quota not in 0-100".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Restart condition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestartCondition {
    #[default]
    Never,
    Always,
    OnFailure,
}

/// Log type for dinit
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogType {
    #[default]
    None,
    Buffer,
    Syslog,
    File(PathBuf),
}

/// Ready notification mechanism
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadyNotification {
    #[default]
    None,
    Pipefd(u32),
    SdNotify,
}

/// Restart policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartPolicy {
    #[serde(default)]
    pub condition: RestartCondition,
    #[serde(default = "default_delay")]
    pub delay_secs: u64,
    pub max_retries: Option<u32>,
}

fn default_delay() -> u64 {
    1
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            condition: RestartCondition::Never,
            delay_secs: 1,
            max_retries: None,
        }
    }
}

/// Service definition - the schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDef {
    pub name: ServiceName,
    #[serde(default)]
    pub service_type: ServiceType,
    pub exec_start: ExecCommand,
    pub exec_stop: Option<ExecCommand>,
    pub working_dir: Option<PathBuf>,
    pub user: Option<String>,
    pub group: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<ServiceName>,
    #[serde(default)]
    pub waits_for: Vec<ServiceName>,
    #[serde(default)]
    pub restart: RestartPolicy,
    #[serde(default)]
    pub environment: HashMap<String, String>,
    #[serde(default)]
    pub env_file: Option<PathBuf>,
    #[serde(default)]
    pub resources: Option<ResourceLimits>,
    #[serde(default)]
    pub log_type: LogType,
    #[serde(default)]
    pub ready_notification: ReadyNotification,
    #[serde(default)]
    pub chain_to: Option<ServiceName>,
    #[serde(default)]
    pub smooth_recovery: bool,
    #[serde(default)]
    pub enabled: bool,
}

impl ServiceDef {
    /// Generate dinit service file content from validated schema
    pub fn to_dinit(&self) -> String {
        let mut out = String::new();

        // Type
        out.push_str(&format!(
            "type = {}\n",
            match self.service_type {
                ServiceType::Simple => "process",
                ServiceType::Forking { .. } => "bgprocess",
                ServiceType::Oneshot => "scripted",
                ServiceType::Notify => "process",
            }
        ));

        // Command
        out.push_str(&format!(
            "command = {}\n",
            self.exec_start.to_command_line()
        ));

        // Stop command
        if let Some(ref stop) = self.exec_stop {
            out.push_str(&format!("stop-command = {}\n", stop.to_command_line()));
        }

        // PID file for forking services
        if let ServiceType::Forking {
            pid_file: Some(ref p),
        } = self.service_type
        {
            out.push_str(&format!("pid-file = {}\n", p.display()));
        }

        // Working directory
        if let Some(ref dir) = self.working_dir {
            out.push_str(&format!("working-dir = {}\n", dir.display()));
        }

        // User/group
        if let Some(ref user) = self.user {
            out.push_str(&format!("run-as = {}\n", user));
        }
        if let Some(ref group) = self.group {
            // dinit usually implies group from run-as, but explicit group can be set via setgid wrapper or future dinit features
            // For now, we note it in comments or if dinit adds direct group support
            out.push_str(&format!("# group = {}\n", group));
        }

        // Hard Dependencies (depends-on)
        for dep in &self.depends_on {
            out.push_str(&format!("depends-on = {}\n", dep));
        }

        // Soft Dependencies (waits-for)
        for wait in &self.waits_for {
            out.push_str(&format!("waits-for = {}\n", wait));
        }

        // Chain To
        if let Some(ref chain) = self.chain_to {
            out.push_str(&format!("chain-to = {}\n", chain));
        }

        // Restart policy
        match self.restart.condition {
            RestartCondition::Always => out.push_str("restart = yes\n"),
            RestartCondition::OnFailure => out.push_str("restart = on-failure\n"),
            RestartCondition::Never => out.push_str("restart = false\n"),
        }
        if self.restart.delay_secs > 0 {
            out.push_str(&format!("restart-delay = {}\n", self.restart.delay_secs));
        }
        if self.smooth_recovery {
            out.push_str("smooth-recovery = true\n");
        }

        // Environment
        if let Some(ref env_file) = self.env_file {
            out.push_str(&format!("env-file = {}\n", env_file.display()));
        }
        for (k, v) in &self.environment {
            out.push_str(&format!("env = {}={}\n", k, v));
        }

        // Logging
        match &self.log_type {
            LogType::Buffer => out.push_str("log-type = buffer\n"),
            LogType::Syslog => out.push_str("log-type = syslog\n"),
            LogType::File(path) => {
                out.push_str("log-type = file\n");
                out.push_str(&format!("logfile = {}\n", path.display()));
            }
            LogType::None => {}
        }

        // Ready Notification
        match self.ready_notification {
            ReadyNotification::Pipefd(fd) => {
                out.push_str(&format!("ready-notification = pipefd:{}\n", fd))
            }
            ReadyNotification::SdNotify => {
                // dinit doesn't support sd_notify natively in the same way, usually requires a wrapper or pipefd usage.
                // However, newer versions or plugins might. For now, we map it to a comment or specific wrapper if defined.
                // Assuming standard dinit:
                out.push_str("# ready-notification = sd_notify (requires wrapper)\n");
            }
            ReadyNotification::None => {}
        }

        out
    }

    /// Write dinit service file to /etc/dinit.d/
    pub fn install(&self) -> std::io::Result<()> {
        let path = format!("/etc/dinit.d/{}", self.name);
        std::fs::write(&path, self.to_dinit())
    }
}

/// Current service state (from systemctl)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: ServiceName,
    pub active_state: ActiveState,
    pub sub_state: String,
    pub load_state: String,
}

/// Internal manager state (state machine)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManagerState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

/// Service status (runtime)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: ServiceName,
    pub state: ManagerState,
    pub pid: Option<u32>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Desired state for apply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    pub name: ServiceName,
    pub active: Option<ActiveState>,
    pub enabled: Option<bool>,
}

/// Systemd plugin
#[derive(Debug, Clone, Default)]
pub struct SystemdPlugin {
    pub services: Vec<ServiceName>,
}

impl SystemdPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_state(&self) -> Result<Vec<ServiceState>> {
        let names: Vec<&str> = if self.services.is_empty() {
            vec!["dbus", "sshd"]
        } else {
            self.services.iter().map(|s| s.as_str()).collect()
        };

        let mut states = Vec::new();
        for name in names {
            if let Ok(state) = self.get_service_status(name).await {
                states.push(state);
            }
        }
        Ok(states)
    }

    pub async fn apply(&self, desired: &[DesiredState]) -> Result<()> {
        for d in desired {
            if let Some(active) = d.active {
                match active {
                    ActiveState::Active => self.start(d.name.as_str()).await?,
                    ActiveState::Inactive => self.stop(d.name.as_str()).await?,
                    _ => {}
                }
            }
            if let Some(enabled) = d.enabled {
                if enabled {
                    self.enable(d.name.as_str()).await?;
                } else {
                    self.disable(d.name.as_str()).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn start(&self, name: &str) -> Result<()> {
        self.ctl(name, "start").await
    }
    pub async fn stop(&self, name: &str) -> Result<()> {
        self.ctl(name, "stop").await
    }
    pub async fn restart(&self, name: &str) -> Result<()> {
        self.ctl(name, "restart").await
    }
    pub async fn enable(&self, name: &str) -> Result<()> {
        self.ctl(name, "enable").await
    }
    pub async fn disable(&self, name: &str) -> Result<()> {
        self.ctl(name, "disable").await
    }

    pub async fn get_service_status(&self, name: &str) -> Result<ServiceState> {
        let out = tokio::process::Command::new("systemctl")
            .args(["show", name, "--property=ActiveState,SubState,LoadState"])
            .output()
            .await?;

        if !out.status.success() {
            anyhow::bail!("systemctl show failed for {}", name);
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut active = "unknown";
        let mut sub = "unknown";
        let mut load = "unknown";

        for line in stdout.lines() {
            if let Some((k, v)) = line.split_once('=') {
                match k {
                    "ActiveState" => active = v,
                    "SubState" => sub = v,
                    "LoadState" => load = v,
                    _ => {}
                }
            }
        }

        let active_state = match active {
            "active" => ActiveState::Active,
            "inactive" => ActiveState::Inactive,
            "activating" => ActiveState::Activating,
            "deactivating" => ActiveState::Deactivating,
            "failed" => ActiveState::Failed,
            "reloading" => ActiveState::Reloading,
            _ => ActiveState::Inactive,
        };

        Ok(ServiceState {
            name: ServiceName::new(name)?,
            active_state,
            sub_state: sub.to_string(),
            load_state: load.to_string(),
        })
    }

    async fn ctl(&self, name: &str, action: &str) -> Result<()> {
        info!("systemctl {} {}", action, name);
        let out = tokio::process::Command::new("systemctl")
            .args([action, name])
            .output()
            .await?;

        if !out.status.success() {
            anyhow::bail!(
                "systemctl {} {} failed: {}",
                action,
                name,
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }
}
