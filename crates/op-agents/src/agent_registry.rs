//! Agent registry for dynamic agent management
//!
//! This eliminates tight coupling by allowing agents to be registered
//! dynamically without hardcoding in the orchestrator.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Unique agent type identifier
    pub agent_type: String,

    /// Human-readable name
    pub name: String,

    /// Description of agent functionality
    pub description: String,

    /// Command to execute the agent
    pub command: String,

    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory
    pub working_dir: Option<PathBuf>,

    /// Capabilities this agent provides
    pub capabilities: Vec<String>,

    /// Whether this agent requires root privileges
    #[serde(default)]
    pub requires_root: bool,

    /// Maximum number of instances
    #[serde(default = "default_max_instances")]
    pub max_instances: usize,

    /// Restart policy
    #[serde(default)]
    pub restart_policy: RestartPolicy,

    /// Health check configuration
    pub health_check: Option<HealthCheck>,
}

fn default_max_instances() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    #[default]
    Never,
    Always,
    OnFailure {
        max_retries: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// D-Bus method to call for health check
    pub method: String,

    /// Interval in seconds
    pub interval_secs: u64,

    /// Timeout in seconds
    pub timeout_secs: u64,

    /// Number of consecutive failures before marking unhealthy
    pub unhealthy_threshold: u32,
}

/// Agent instance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstance {
    pub id: String,
    pub agent_type: String,
    pub pid: Option<u32>,
    pub status: AgentStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
    pub restart_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Starting,
    Running,
    Healthy,
    Unhealthy,
    Stopping,
    Stopped,
    Failed { reason: String },
}

/// Agent factory for creating agents
#[async_trait]
pub trait AgentFactory: Send + Sync {
    /// Create a new agent instance
    async fn create_agent(&self, spec: &AgentSpec, instance_id: &str) -> Result<AgentHandle>;

    /// Check if an agent type is supported
    fn supports(&self, agent_type: &str) -> bool;
}

/// Handle to a running agent
pub struct AgentHandle {
    pub id: String,
    pub process: tokio::process::Child,
    pub spec: AgentSpec,
}

/// Default agent factory that spawns processes
pub struct ProcessAgentFactory;

#[async_trait]
impl AgentFactory for ProcessAgentFactory {
    async fn create_agent(&self, spec: &AgentSpec, instance_id: &str) -> Result<AgentHandle> {
        let mut cmd = tokio::process::Command::new(&spec.command);

        // Add arguments
        cmd.args(&spec.args);

        // Add instance ID as environment variable
        cmd.env("AGENT_ID", instance_id);
        cmd.env("AGENT_TYPE", &spec.agent_type);

        // Add custom environment variables
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(dir) = &spec.working_dir {
            cmd.current_dir(dir);
        }

        // Spawn the process
        let process = cmd
            .spawn()
            .context(format!("Failed to spawn agent: {}", spec.command))?;

        Ok(AgentHandle {
            id: instance_id.to_string(),
            process,
            spec: spec.clone(),
        })
    }

    fn supports(&self, _agent_type: &str) -> bool {
        true // Default factory supports all types
    }
}

/// Agent registry for managing agent specifications and instances
pub struct AgentRegistry {
    /// Registered agent specifications
    specs: Arc<RwLock<HashMap<String, AgentSpec>>>,

    /// Running agent instances
    instances: Arc<RwLock<HashMap<String, AgentInstance>>>,

    /// Agent factories
    factories: Arc<RwLock<Vec<Box<dyn AgentFactory>>>>,

    /// Agent handles
    handles: Arc<RwLock<HashMap<String, AgentHandle>>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    pub fn new() -> Self {
        let registry = Self {
            specs: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            factories: Arc::new(RwLock::new(Vec::new())),
            handles: Arc::new(RwLock::new(HashMap::new())),
        };

        // Register default factory
        let default_factory = Box::new(ProcessAgentFactory);

        // We need to do this in a blocking context since new() is not async
        let factories = registry.factories.clone();
        tokio::spawn(async move {
            let mut factories = factories.write().await;
            factories.push(default_factory);
        });

        registry
    }

    /// Register an agent specification
    pub async fn register_spec(&self, spec: AgentSpec) -> Result<()> {
        let mut specs = self.specs.write().await;

        if specs.contains_key(&spec.agent_type) {
            return Err(anyhow::anyhow!(
                "Agent type '{}' is already registered",
                spec.agent_type
            ));
        }

        specs.insert(spec.agent_type.clone(), spec);
        Ok(())
    }

    /// Load specifications from a configuration file
    pub async fn load_specs_from_file(&self, path: &PathBuf) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read agent specifications file")?;

        let mut content = content;
        let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }
            .context("Failed to parse agent specifications")?;

        for spec in specs {
            self.register_spec(spec).await?;
        }

        Ok(())
    }

    /// Load specifications from a directory
    pub async fn load_specs_from_directory(&self, dir: &PathBuf) -> Result<()> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .context("Failed to read specifications directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Err(e) = self.load_specs_from_file(&path).await {
                    tracing::warn!("Failed to load spec from {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Register a custom agent factory
    pub async fn register_factory(&self, factory: Box<dyn AgentFactory>) {
        let mut factories = self.factories.write().await;
        factories.push(factory);
    }

    /// Spawn an agent instance
    pub async fn spawn_agent(
        &self,
        agent_type: &str,
        _config: Option<OwnedValue>,
    ) -> Result<String> {
        // Get the specification
        let specs = self.specs.read().await;
        let spec = specs
            .get(agent_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown agent type: {}", agent_type))?
            .clone();

        // Check max instances
        let instances = self.instances.read().await;
        let current_count = instances
            .values()
            .filter(|i| {
                i.agent_type == agent_type
                    && matches!(i.status, AgentStatus::Running | AgentStatus::Healthy)
            })
            .count();

        if current_count >= spec.max_instances {
            return Err(anyhow::anyhow!(
                "Maximum instances ({}) reached for agent type '{}'",
                spec.max_instances,
                agent_type
            ));
        }
        drop(instances);

        // Generate instance ID
        let instance_id = format!("{}-{}", agent_type, uuid::Uuid::new_v4());

        // Find suitable factory
        let factories = self.factories.read().await;
        let factory = factories
            .iter()
            .find(|f| f.supports(agent_type))
            .ok_or_else(|| anyhow::anyhow!("No factory supports agent type: {}", agent_type))?;

        // Create the agent
        let handle = factory.create_agent(&spec, &instance_id).await?;
        let pid = handle.process.id();

        // Store the handle
        let mut handles = self.handles.write().await;
        handles.insert(instance_id.clone(), handle);

        // Create instance record
        let instance = AgentInstance {
            id: instance_id.clone(),
            agent_type: agent_type.to_string(),
            pid,
            status: AgentStatus::Starting,
            started_at: chrono::Utc::now(),
            last_health_check: None,
            restart_count: 0,
        };

        // Store instance
        let mut instances = self.instances.write().await;
        instances.insert(instance_id.clone(), instance);

        // TODO: Start health check task if configured

        Ok(instance_id)
    }

    /// Kill an agent instance
    pub async fn kill_agent(&self, instance_id: &str) -> Result<()> {
        let mut handles = self.handles.write().await;

        if let Some(mut handle) = handles.remove(instance_id) {
            // Try graceful shutdown first
            handle
                .process
                .kill()
                .await
                .context("Failed to kill agent process")?;

            // Update instance status
            let mut instances = self.instances.write().await;
            if let Some(instance) = instances.get_mut(instance_id) {
                instance.status = AgentStatus::Stopped;
            }

            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Agent instance '{}' not found",
                instance_id
            ))
        }
    }

    /// Get agent instance status
    pub async fn get_instance_status(&self, instance_id: &str) -> Result<AgentInstance> {
        let instances = self.instances.read().await;
        instances
            .get(instance_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Agent instance '{}' not found", instance_id))
    }

    /// List all agent instances
    pub async fn list_instances(&self) -> Vec<AgentInstance> {
        let instances = self.instances.read().await;
        instances.values().cloned().collect()
    }

    /// List all registered agent types
    pub async fn list_agent_types(&self) -> Vec<String> {
        let specs = self.specs.read().await;
        specs.keys().cloned().collect()
    }

    /// Get agent specification
    pub async fn get_spec(&self, agent_type: &str) -> Option<AgentSpec> {
        let specs = self.specs.read().await;
        specs.get(agent_type).cloned()
    }
}

/// Load default agent specifications
pub async fn load_default_specs(registry: &AgentRegistry) -> Result<()> {
    let specs = vec![
        AgentSpec {
            agent_type: "executor".to_string(),
            name: "Command Executor".to_string(),
            description: "Executes whitelisted shell commands".to_string(),
            command: "dbus-agent-executor".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["execute".to_string()],
            requires_root: false,
            max_instances: 3,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "file".to_string(),
            name: "File Manager".to_string(),
            description: "Manages file operations".to_string(),
            command: "dbus-agent-file".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "read".to_string(),
                "write".to_string(),
                "delete".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "network".to_string(),
            name: "Network Manager".to_string(),
            description: "Manages network configuration".to_string(),
            command: "dbus-agent-network".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["network".to_string()],
            requires_root: true,
            max_instances: 2,
            restart_policy: RestartPolicy::Always,
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "systemd".to_string(),
            name: "Systemd Controller".to_string(),
            description: "Controls systemd services".to_string(),
            command: "dbus-agent-systemd".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["service".to_string()],
            requires_root: true,
            max_instances: 2,
            restart_policy: RestartPolicy::Always,
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "monitor".to_string(),
            name: "System Monitor".to_string(),
            description: "Monitors system resources".to_string(),
            command: "dbus-agent-monitor".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec!["monitor".to_string()],
            requires_root: false,
            max_instances: 1,
            restart_policy: RestartPolicy::Always,
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "packagekit".to_string(),
            name: "Package Manager".to_string(),
            description: "Manages system packages via PackageKit".to_string(),
            command: "dbus-agent-packagekit".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "install".to_string(),
                "remove".to_string(),
                "update".to_string(),
            ],
            requires_root: true,
            max_instances: 2,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "python-pro".to_string(),
            name: "Python Professional".to_string(),
            description: "Python development and execution environment".to_string(),
            command: "dbus-agent-python-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "analyze".to_string(),
                "format".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "rust-pro".to_string(),
            name: "Rust Professional".to_string(),
            description: "Rust development and compilation environment".to_string(),
            command: "dbus-agent-rust-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "check".to_string(),
                "test".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "c-pro".to_string(),
            name: "C Professional".to_string(),
            description: "C development and compilation environment".to_string(),
            command: "dbus-agent-c-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "debug".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "cpp-pro".to_string(),
            name: "C++ Professional".to_string(),
            description: "C++ development and compilation environment".to_string(),
            command: "dbus-agent-cpp-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "debug".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "golang-pro".to_string(),
            name: "Go Professional".to_string(),
            description: "Go development and compilation environment".to_string(),
            command: "dbus-agent-golang-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "compile".to_string(),
                "test".to_string(),
                "build".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "javascript-pro".to_string(),
            name: "JavaScript Professional".to_string(),
            description: "JavaScript development and execution environment".to_string(),
            command: "dbus-agent-javascript-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "format".to_string(),
                "lint".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "php-pro".to_string(),
            name: "PHP Professional".to_string(),
            description: "PHP development and execution environment".to_string(),
            command: "dbus-agent-php-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "lint".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
        AgentSpec {
            agent_type: "sql-pro".to_string(),
            name: "SQL Professional".to_string(),
            description: "SQL development and query execution environment".to_string(),
            command: "dbus-agent-sql-pro".to_string(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            capabilities: vec![
                "execute".to_string(),
                "optimize".to_string(),
                "analyze".to_string(),
            ],
            requires_root: false,
            max_instances: 5,
            restart_policy: RestartPolicy::OnFailure { max_retries: 3 },
            health_check: Some(HealthCheck {
                method: "GetStatus".to_string(),
                interval_secs: 30,
                timeout_secs: 5,
                unhealthy_threshold: 3,
            }),
        },
    ];

    for spec in specs {
        registry.register_spec(spec).await?;
    }

    Ok(())
}
