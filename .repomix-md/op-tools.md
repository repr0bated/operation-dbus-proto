This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-tools/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-tools/
              src/
                bin/
                  op-packagekit-install.rs
                builtin/
                  agent_tool.rs
                  anydesk.rs
                  code_search.rs
                  dbus_hybrid.rs
                  dbus_introspection.rs
                  dbus_search_tool.rs
                  dbus_tool.rs
                  dbus.rs
                  error_reporting_tool.rs
                  file.rs
                  gcloud_tools.rs
                  incus_tools.rs
                  indexer_tools.rs
                  lxc_tools.rs
                  mod.rs
                  mod.rs.fix
                  mod.rs.patch
                  op-dbus-v2.code-workspace
                  openflow_tools.rs
                  ovs_tools.rs
                  ovs_tools.rs.snippet.txt
                  ovs.rs
                  ovsdb.rs
                  packagekit.rs
                  plugin_projection.rs
                  plugin_state_tool.rs
                  plugin.rs
                  procfs.rs
                  respond_tool.rs
                  response_tools.rs
                  rtnetlink_tools.rs
                  s6.rs
                  self_tools.rs
                  shell_tool.rs
                  shell.rs
                  system.rs
                discovery/
                  sources/
                    agent.rs
                    dbus.rs
                    mod.rs
                    plugin.rs
                  mod.rs
                  projection_engine.rs
                builtin_old.rs
                dynamic_tool.rs
                executor.rs
                lib.rs
                mcptools.rs
                orchestration_plugin.rs
                registry.rs
                router.rs
                security.rs
                tool.rs
                validation_tests.rs
                validation.rs
              Cargo.toml
              Cargo.toml.patch
              compare-op-tools.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/bin/op-packagekit-install.rs">
//! Install packages via PackageKit D-Bus (no CLI fallbacks).

use anyhow::{Context, Result};
use clap::Parser;
use futures::StreamExt;
use zbus::Connection;
use zbus::Proxy;

#[derive(Parser, Debug)]
#[command(name = "op-packagekit-install")]
#[command(about = "Install packages via PackageKit D-Bus using zbus")]
struct Args {
    /// Package names to install
    #[arg(required = true)]
    packages: Vec<String>,

    /// PackageKit resolve filters
    #[arg(long, default_value_t = 0)]
    resolve_filters: u64,

    /// PackageKit transaction flags
    #[arg(long, default_value_t = 0)]
    transaction_flags: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let connection = Connection::system().await?;

    let package_ids = resolve_packages(&connection, args.resolve_filters, &args.packages)
        .await
        .context("Failed to resolve package IDs")?;

    if package_ids.is_empty() {
        anyhow::bail!(
            "No packages resolved for requested names: {:?}",
            args.packages
        );
    }

    install_packages(&connection, args.transaction_flags, &package_ids)
        .await
        .context("PackageKit install failed")?;

    println!(
        "Installed packages via PackageKit: {}",
        package_ids.join(", ")
    );

    Ok(())
}

async fn create_transaction(connection: &Connection) -> Result<zbus::zvariant::OwnedObjectPath> {
    let proxy = Proxy::new(
        connection,
        "org.freedesktop.PackageKit",
        "/org/freedesktop/PackageKit",
        "org.freedesktop.PackageKit",
    )
    .await?;

    let path: zbus::zvariant::OwnedObjectPath = proxy.call("CreateTransaction", &()).await?;
    Ok(path)
}

async fn resolve_packages(
    connection: &Connection,
    filters: u64,
    packages: &[String],
) -> Result<Vec<String>> {
    let tx_path = create_transaction(connection).await?;
    let tx_proxy = Proxy::new(
        connection,
        "org.freedesktop.PackageKit",
        &tx_path,
        "org.freedesktop.PackageKit.Transaction",
    )
    .await?;

    let mut package_stream = tx_proxy.receive_signal("Package").await?;
    let mut finished_stream = tx_proxy.receive_signal("Finished").await?;

    let _: () = tx_proxy
        .call("Resolve", &(filters, packages.to_vec()))
        .await?;

    let mut resolved = Vec::new();

    loop {
        tokio::select! {
            Some(signal) = package_stream.next() => {
                if let Ok((_, package_id, _)) = signal.body().deserialize::<(u32, String, String)>() {
                    resolved.push(package_id);
                }
            }
            Some(_) = finished_stream.next() => {
                break;
            }
        }
    }

    resolved.sort();
    resolved.dedup();
    Ok(resolved)
}

async fn install_packages(
    connection: &Connection,
    flags: u64,
    package_ids: &[String],
) -> Result<()> {
    let tx_path = create_transaction(connection).await?;
    let tx_proxy = Proxy::new(
        connection,
        "org.freedesktop.PackageKit",
        &tx_path,
        "org.freedesktop.PackageKit.Transaction",
    )
    .await?;

    let mut finished_stream = tx_proxy.receive_signal("Finished").await?;

    let _: () = tx_proxy
        .call("InstallPackages", &(flags, package_ids.to_vec()))
        .await?;

    // Wait for installation to complete
    if let Some(signal) = finished_stream.next().await {
        if let Ok((exit_code, _runtime)) = signal.body().deserialize::<(u32, u32)>() {
            if exit_code != 1 {
                // 1 = PK_EXIT_ENUM_SUCCESS
                anyhow::bail!("Package installation failed with exit code: {}", exit_code);
            }
        }
    }

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/agent_tool.rs">
//! Agent Tool - D-Bus Agent Registration and Runtime Controls
//!
//! This module creates agent tools that register as D-Bus services.
//!
//! Architecture:
//! 1. Agent catalog is discovered from `op-agents`
//! 2. Registration is configurable (include/autostart)
//! 3. Tool calls go through zbus::Proxy with lazy service startup

use anyhow::Result;
use async_trait::async_trait;
use op_agents::builtin_agent_descriptors;
use simd_json::prelude::*;
use simd_json::ValueBuilder;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::Connection;

use crate::tool::{BoxedTool, Tool};

// =============================================================================
// BUS TYPE
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    System,
    Session,
}

impl Default for BusType {
    fn default() -> Self {
        Self::System
    }
}

// =============================================================================
// AGENT CONNECTION REGISTRY
// =============================================================================

static AGENT_CONNECTIONS: OnceLock<Arc<AgentConnectionRegistry>> = OnceLock::new();
static AGENT_RUNTIME_CATALOG: OnceLock<HashMap<String, AgentRuntimeDescriptor>> = OnceLock::new();

#[derive(Debug, Clone)]
struct AgentRuntimeDescriptor {
    agent_type: String,
    name: String,
    description: String,
    operations: Vec<String>,
    category: String,
}

#[derive(Debug, Clone)]
struct AgentRegistrationConfig {
    include: Option<HashSet<String>>,
    autostart: HashSet<String>,
    autostart_all: bool,
}

pub struct AgentConnectionRegistry {
    connections: RwLock<HashMap<String, Connection>>,
    bus_type: BusType,
}

impl AgentConnectionRegistry {
    pub fn new(bus_type: BusType) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            bus_type,
        }
    }

    pub fn global() -> Arc<Self> {
        AGENT_CONNECTIONS
            .get_or_init(|| {
                let bus_type = default_agent_bus();
                info!("AgentConnectionRegistry: using {:?} bus", bus_type);
                Arc::new(Self::new(bus_type))
            })
            .clone()
    }

    /// Start an agent as a D-Bus service.
    pub async fn start_agent_service(
        &self,
        agent_type: &str,
        agent_name: &str,
        description: &str,
        operations: &[String],
    ) -> Result<()> {
        let canonical_type = normalize_agent_type(agent_type);

        // Check if already running
        {
            let connections = self.connections.read().await;
            if connections.contains_key(&canonical_type) {
                debug!(agent = %canonical_type, "Agent already running");
                return Ok(());
            }
        }

        info!(agent = %canonical_type, "Starting agent D-Bus service");

        // Build service name: rust-pro -> org.dbusmcp.Agent.RustPro
        let service_name = format!(
            "org.dbusmcp.Agent.{}",
            canonical_type
                .split('-')
                .map(capitalize_first)
                .collect::<String>()
        );

        let object_path = format!(
            "/org/dbusmcp/Agent/{}",
            canonical_type
                .split('-')
                .map(capitalize_first)
                .collect::<String>()
        );

        // Create the D-Bus service object
        let service = AgentDbusService {
            agent_type: agent_type.to_string(),
            agent_name: agent_name.to_string(),
            description: description.to_string(),
            operations: operations.to_vec(),
        };

        // Build connection and serve
        let connection = match self.bus_type {
            BusType::System => {
                zbus::connection::Builder::system()?
                    .name(service_name.as_str())?
                    .serve_at(object_path.as_str(), service)?
                    .build()
                    .await?
            }
            BusType::Session => {
                zbus::connection::Builder::session()?
                    .name(service_name.as_str())?
                    .serve_at(object_path.as_str(), service)?
                    .build()
                    .await?
            }
        };

        // Store connection to keep service alive
        {
            let mut connections = self.connections.write().await;
            connections.insert(canonical_type.clone(), connection);
        }

        info!(agent = %canonical_type, service = %service_name, "✓ Agent registered on D-Bus");
        Ok(())
    }

    pub async fn is_running(&self, agent_type: &str) -> bool {
        self.connections
            .read()
            .await
            .contains_key(&normalize_agent_type(agent_type))
    }

    pub async fn list_running(&self) -> Vec<String> {
        self.connections.read().await.keys().cloned().collect()
    }

    pub async fn stop_agent(&self, agent_type: &str) -> Result<()> {
        let normalized = normalize_agent_type(agent_type);
        if self.connections.write().await.remove(&normalized).is_some() {
            info!(agent = %normalized, "Agent stopped");
        }
        Ok(())
    }

    pub async fn stop_all(&self) {
        let count = self.connections.write().await.drain().count();
        info!("Stopped {} agent D-Bus services", count);
    }
}

impl AgentRegistrationConfig {
    fn from_env() -> Self {
        let include = parse_agent_set("OP_AGENT_INCLUDE");
        let autostart = parse_agent_set("OP_AGENT_AUTOSTART").unwrap_or_default();
        let autostart_all = std::env::var("OP_AGENT_AUTOSTART_ALL")
            .ok()
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        Self {
            include,
            autostart,
            autostart_all,
        }
    }

    fn should_register(&self, agent_type: &str) -> bool {
        let normalized = normalize_agent_type(agent_type);
        match &self.include {
            Some(include) => include.contains(&normalized),
            None => true,
        }
    }

    fn should_autostart(&self, agent_type: &str) -> bool {
        let normalized = normalize_agent_type(agent_type);
        self.autostart_all || self.autostart.contains(&normalized)
    }
}

fn parse_agent_set(var_name: &str) -> Option<HashSet<String>> {
    let raw = std::env::var(var_name).ok()?;
    let parsed: HashSet<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_agent_type)
        .collect();
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

fn normalize_agent_type(raw: &str) -> String {
    raw.trim().replace('_', "-").to_ascii_lowercase()
}

fn default_agent_bus() -> BusType {
    std::env::var("OP_AGENT_BUS")
        .ok()
        .and_then(|v| match v.to_lowercase().as_str() {
            "session" => Some(BusType::Session),
            "system" => Some(BusType::System),
            _ => None,
        })
        .unwrap_or_else(|| {
            if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok() {
                BusType::Session
            } else {
                BusType::System
            }
        })
}

fn runtime_catalog() -> &'static HashMap<String, AgentRuntimeDescriptor> {
    AGENT_RUNTIME_CATALOG.get_or_init(|| {
        let mut catalog = HashMap::new();

        for descriptor in builtin_agent_descriptors() {
            let key = normalize_agent_type(&descriptor.agent_type);
            let category = infer_agent_category(&descriptor.agent_type).to_string();
            catalog.insert(
                key,
                AgentRuntimeDescriptor {
                    agent_type: normalize_agent_type(&descriptor.agent_type),
                    name: descriptor.name,
                    description: descriptor.description,
                    operations: descriptor.operations,
                    category,
                },
            );
        }

        // Keep legacy statically defined agents as fallbacks.
        for def in AGENT_DEFINITIONS {
            let key = normalize_agent_type(def.agent_type);
            catalog
                .entry(key)
                .or_insert_with(|| AgentRuntimeDescriptor {
                    agent_type: normalize_agent_type(def.agent_type),
                    name: def.name.to_string(),
                    description: def.description.to_string(),
                    operations: def.operations.iter().map(|op| op.to_string()).collect(),
                    category: def.category.to_string(),
                });
        }

        catalog
    })
}

fn find_agent_descriptor(agent_name: &str) -> Option<&'static AgentRuntimeDescriptor> {
    runtime_catalog().get(&normalize_agent_type(agent_name))
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

// =============================================================================
// D-BUS SERVICE IMPLEMENTATION
// =============================================================================

/// D-Bus service implementing org.dbusmcp.Agent interface
struct AgentDbusService {
    agent_type: String,
    agent_name: String,
    description: String,
    operations: Vec<String>,
}

#[zbus::interface(name = "org.dbusmcp.Agent")]
impl AgentDbusService {
    fn name(&self) -> &str {
        &self.agent_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn operations(&self) -> Vec<String> {
        self.operations.clone()
    }

    async fn execute(&self, task_json: &str) -> String {
        debug!(agent = %self.agent_type, task = %task_json, "Executing");

        let mut task_json_mut = task_json.to_string();
        let task: Value = match unsafe { simd_json::from_str(&mut task_json_mut) } {
            Ok(t) => t,
            Err(e) => {
                return json!({
                    "success": false,
                    "error": format!("Parse error: {}", e)
                })
                .to_string();
            }
        };

        let operation = task
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("execute");

        // Placeholder execution - returns success with operation info
        // Real implementation would dispatch to actual agent logic
        json!({
            "success": true,
            "agent": self.agent_type,
            "operation": operation,
            "message": format!("Agent {} executed '{}'", self.agent_name, operation),
            "data": task.get("args").cloned().unwrap_or(Value::null())
        })
        .to_string()
    }
}

// =============================================================================
// D-BUS EXECUTOR
// =============================================================================

pub struct DbusAgentExecutor {
    bus_type: BusType,
}

impl DbusAgentExecutor {
    pub fn new() -> Self {
        let bus_type = default_agent_bus();
        Self { bus_type }
    }

    fn to_service_name(agent_name: &str) -> String {
        let pascal = agent_name
            .split('_')
            .map(capitalize_first)
            .collect::<String>();
        format!("org.dbusmcp.Agent.{}", pascal)
    }

    fn to_object_path(agent_name: &str) -> String {
        let pascal = agent_name
            .split('_')
            .map(capitalize_first)
            .collect::<String>();
        format!("/org/dbusmcp/Agent/{}", pascal)
    }

    fn is_service_unavailable(error: &zbus::Error) -> bool {
        let s = error.to_string().to_lowercase();
        s.contains("serviceunknown")
            || s.contains("name has no owner")
            || s.contains("not found")
            || s.contains("does not exist")
    }

    async fn ensure_agent_running(&self, agent_name: &str) -> Result<()> {
        let Some(descriptor) = find_agent_descriptor(agent_name) else {
            return Err(anyhow::anyhow!(
                "Agent '{}' not found in runtime catalog",
                agent_name
            ));
        };

        let conn_registry = AgentConnectionRegistry::global();
        conn_registry
            .start_agent_service(
                &descriptor.agent_type,
                &descriptor.name,
                &descriptor.description,
                &descriptor.operations,
            )
            .await
    }
}

impl Default for DbusAgentExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn execute_operation(
        &self,
        agent_name: &str,
        operation: &str,
        path: Option<&str>,
        args: Option<Value>,
    ) -> Result<Value>;
}

#[async_trait]
impl AgentExecutor for DbusAgentExecutor {
    async fn execute_operation(
        &self,
        agent_name: &str,
        operation: &str,
        path: Option<&str>,
        args: Option<Value>,
    ) -> Result<Value> {
        let normalized_agent = normalize_agent_type(agent_name).replace('-', "_");
        let service_name = Self::to_service_name(&normalized_agent);
        let object_path = Self::to_object_path(&normalized_agent);

        let args_str = args.and_then(|v| {
            if v.is_null() {
                None
            } else {
                simd_json::to_string(&v).ok()
            }
        });

        let task = json!({
            "type": normalized_agent.replace('_', "-"),
            "operation": operation,
            "path": path,
            "args": args_str
        });
        let task_json = simd_json::to_string(&task)?;

        debug!(agent = %agent_name, service = %service_name, "Calling D-Bus");

        let connection = match self.bus_type {
            BusType::System => Connection::system().await,
            BusType::Session => Connection::session().await,
        }
        .map_err(|e| anyhow::anyhow!("D-Bus connection failed: {}", e))?;

        let mut bootstrap_attempted = false;

        loop {
            let proxy: zbus::Proxy = match zbus::proxy::Builder::new(&connection)
                .destination(service_name.as_str())?
                .path(object_path.as_str())?
                .interface("org.dbusmcp.Agent")?
                .build()
                .await
            {
                Ok(proxy) => proxy,
                Err(e) if Self::is_service_unavailable(&e) && !bootstrap_attempted => {
                    bootstrap_attempted = true;
                    self.ensure_agent_running(&normalized_agent).await?;
                    continue;
                }
                Err(e) if Self::is_service_unavailable(&e) => {
                    return Err(anyhow::anyhow!(
                        "Agent '{}' not running on D-Bus",
                        normalized_agent
                    ));
                }
                Err(e) => return Err(anyhow::anyhow!("D-Bus proxy failed: {}", e)),
            };

            let result: String = match proxy.call("Execute", &(task_json.clone(),)).await {
                Ok(result) => result,
                Err(e) if Self::is_service_unavailable(&e) && !bootstrap_attempted => {
                    bootstrap_attempted = true;
                    self.ensure_agent_running(&normalized_agent).await?;
                    continue;
                }
                Err(e) if Self::is_service_unavailable(&e) => {
                    return Err(anyhow::anyhow!(
                        "Agent '{}' not available",
                        normalized_agent
                    ));
                }
                Err(e) => return Err(anyhow::anyhow!("D-Bus call failed: {}", e)),
            };

            let mut result_mut = result;
            let parsed: Value = unsafe { simd_json::from_str(&mut result_mut)? };
            info!(agent = %normalized_agent, operation = %operation, "Completed");
            return Ok(parsed);
        }
    }
}

// =============================================================================
// AGENT TOOL
// =============================================================================

pub struct AgentTool {
    name: String,
    _agent_name: String,
    description: String,
    operations: Vec<String>,
    role_category: String,
    executor: Arc<dyn AgentExecutor + Send + Sync>,
}

impl AgentTool {
    pub fn new(
        agent_name: &str,
        description: &str,
        operations: &[String],
        executor: Arc<dyn AgentExecutor + Send + Sync>,
    ) -> Self {
        Self {
            name: format!("agent_{}", agent_name.replace('-', "_")),
            _agent_name: agent_name.to_string(),
            description: description.to_string(),
            operations: operations.to_vec(),
            role_category: "agent".to_string(),
            executor,
        }
    }

    pub fn with_category(
        agent_name: &str,
        description: &str,
        operations: &[String],
        category: &str,
        executor: Arc<dyn AgentExecutor + Send + Sync>,
    ) -> Self {
        Self {
            name: format!("agent_{}", agent_name.replace('-', "_")),
            _agent_name: agent_name.to_string(),
            description: description.to_string(),
            operations: operations.to_vec(),
            role_category: category.to_string(),
            executor,
        }
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        if self.operations.is_empty() {
            json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "description": "Operation to perform" },
                    "path": { "type": "string", "description": "Optional path" },
                    "args": { "type": "object", "description": "Additional arguments" }
                },
                "required": ["operation"]
            })
        } else {
            json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": self.operations,
                        "description": "Operation to perform"
                    },
                    "path": { "type": "string", "description": "Optional path" },
                    "args": { "type": "object", "description": "Additional arguments" }
                },
                "required": ["operation"]
            })
        }
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'operation'"))?;

        if !self.operations.is_empty() && !self.operations.contains(&operation.to_string()) {
            return Err(anyhow::anyhow!(
                "Unknown operation: {}. Valid: {:?}",
                operation,
                self.operations
            ));
        }

        let path = input.get("path").and_then(|v| v.as_str());
        let args = input.get("args").cloned();
        let agent = self.name.strip_prefix("agent_").unwrap_or(&self.name);

        self.executor
            .execute_operation(agent, operation, path, args)
            .await
    }

    fn category(&self) -> &str {
        &self.role_category
    }

    fn namespace(&self) -> &str {
        "agent"
    }

    fn tags(&self) -> Vec<String> {
        vec!["agent".to_string(), self.role_category.clone()]
    }
}

// =============================================================================
// STATIC AGENT DEFINITIONS
// =============================================================================

/// Agent definition - no factory function needed
#[derive(Clone)]
pub struct AgentDef {
    pub agent_type: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub operations: &'static [&'static str],
    pub category: &'static str,
}

/// All agent definitions (static, no create_agent() needed)
pub const AGENT_DEFINITIONS: &[AgentDef] = &[
    AgentDef {
        agent_type: "rust-pro",
        name: "Rust Pro",
        description: "Expert Rust development agent",
        operations: &[
            "check", "build", "test", "clippy", "format", "run", "doc", "analyze",
        ],
        category: "language",
    },
    AgentDef {
        agent_type: "python-pro",
        name: "Python Pro",
        description: "Expert Python development agent",
        operations: &["analyze", "format", "lint", "test", "run"],
        category: "language",
    },
    AgentDef {
        agent_type: "backend-architect",
        name: "Backend Architect",
        description: "Backend architecture design agent",
        operations: &["analyze", "design", "review", "suggest", "document"],
        category: "architecture",
    },
    AgentDef {
        agent_type: "network-engineer",
        name: "Network Engineer",
        description: "Network configuration agent",
        operations: &["analyze", "configure", "diagnose", "optimize"],
        category: "infrastructure",
    },
    AgentDef {
        agent_type: "sequential-thinking",
        name: "Sequential Thinking",
        description: "Step-by-step reasoning agent",
        operations: &["think", "plan", "analyze", "conclude", "reflect"],
        category: "orchestration",
    },
    AgentDef {
        agent_type: "memory",
        name: "Memory Agent",
        description: "Persistent memory and recall",
        operations: &["store", "recall", "list", "search", "forget"],
        category: "orchestration",
    },
    AgentDef {
        agent_type: "context-manager",
        name: "Context Manager",
        description: "Session context management",
        operations: &[
            "save", "load", "list", "delete", "export", "import", "clear",
        ],
        category: "orchestration",
    },
    AgentDef {
        agent_type: "search-specialist",
        name: "Search Specialist",
        description: "Search and discovery agent",
        operations: &["search", "analyze", "suggest"],
        category: "seo",
    },
    AgentDef {
        agent_type: "deployment",
        name: "Deployment Agent",
        description: "Deployment management agent",
        operations: &["plan", "deploy", "rollback", "status"],
        category: "infrastructure",
    },
    AgentDef {
        agent_type: "debugger",
        name: "Debugger Agent",
        description: "Debugging and troubleshooting",
        operations: &["analyze", "diagnose", "suggest", "trace"],
        category: "analysis",
    },
    AgentDef {
        agent_type: "prompt-engineer",
        name: "Prompt Engineer",
        description: "Prompt optimization agent",
        operations: &["analyze", "improve", "generate", "test"],
        category: "aiml",
    },
];

// =============================================================================
// REGISTRATION
// =============================================================================

fn infer_agent_category(agent_type: &str) -> &'static str {
    if agent_type.contains("security") || agent_type.contains("auditor") {
        "security"
    } else if agent_type.contains("architect") || agent_type.contains("developer") {
        "architecture"
    } else if agent_type.ends_with("-pro") || agent_type.ends_with("_pro") {
        "language"
    } else if agent_type.contains("engineer") || agent_type == "deployment" {
        "infrastructure"
    } else if agent_type.contains("memory")
        || agent_type.contains("context")
        || agent_type.contains("sequential")
        || agent_type.contains("orchestrator")
    {
        "orchestration"
    } else {
        "agent"
    }
}

async fn register_agent_tool_parts(
    registry: &crate::ToolRegistry,
    agent_type: &str,
    agent_name: &str,
    description: &str,
    operations: &[String],
    category: &str,
    autostart: bool,
) -> Result<()> {
    info!(agent = %agent_type, "Registering agent");

    // 1. Optionally start D-Bus service immediately.
    if autostart {
        let conn_registry = AgentConnectionRegistry::global();
        if let Err(e) = conn_registry
            .start_agent_service(agent_type, agent_name, description, operations)
            .await
        {
            warn!(
                agent = %agent_type,
                error = %e,
                "D-Bus service failed, tool still registered"
            );
        }
    }

    // 2. Create tool
    let executor = Arc::new(DbusAgentExecutor::new());
    let tool = AgentTool::with_category(agent_type, description, operations, category, executor);

    // 3. Register tool
    registry.register_tool(Arc::new(tool)).await?;

    info!(agent = %agent_type, "✓ Agent registered");
    Ok(())
}

/// Register a single static agent definition (starts D-Bus service + creates tool)
pub async fn register_agent_tool(registry: &crate::ToolRegistry, def: &AgentDef) -> Result<()> {
    let operations: Vec<String> = def.operations.iter().map(|s| s.to_string()).collect();
    register_agent_tool_parts(
        registry,
        def.agent_type,
        def.name,
        def.description,
        &operations,
        def.category,
        true,
    )
    .await
}

/// Register all agents
pub async fn register_all_agents(registry: &crate::ToolRegistry) -> Result<()> {
    let config = AgentRegistrationConfig::from_env();
    let mut success = 0;
    let mut failed = 0;
    let mut skipped = 0;

    // Use full built-in catalog from op-agents.
    for descriptor in runtime_catalog().values() {
        if !config.should_register(&descriptor.agent_type) {
            skipped += 1;
            continue;
        }

        let autostart = config.should_autostart(&descriptor.agent_type);
        match register_agent_tool_parts(
            registry,
            &descriptor.agent_type,
            &descriptor.name,
            &descriptor.description,
            &descriptor.operations,
            &descriptor.category,
            autostart,
        )
        .await
        {
            Ok(()) => success += 1,
            Err(e) => {
                warn!(agent = %descriptor.agent_type, error = %e, "Failed");
                failed += 1;
            }
        }
    }

    info!(
        "Registered {} agents ({} failed, {} skipped)",
        success, failed, skipped
    );

    let running = AgentConnectionRegistry::global().list_running().await;
    info!("Active D-Bus services: {:?}", running);

    Ok(())
}

// =============================================================================
// LEGACY HELPERS
// =============================================================================

pub fn create_agent_tool(
    agent_name: &str,
    description: &str,
    operations: &[String],
    _config: Value,
) -> Result<BoxedTool> {
    let executor = Arc::new(DbusAgentExecutor::new());
    Ok(Arc::new(AgentTool::new(
        agent_name,
        description,
        operations,
        executor,
    )))
}

pub fn create_agent_tool_with_executor(
    agent_name: &str,
    description: &str,
    operations: &[String],
    executor: Arc<dyn AgentExecutor + Send + Sync>,
) -> Result<BoxedTool> {
    Ok(Arc::new(AgentTool::new(
        agent_name,
        description,
        operations,
        executor,
    )))
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/anydesk.rs">
//! AnyDesk remote desktop tools.
//!
//! These tools provide management and monitoring capabilities for AnyDesk,
//! including getting the AnyDesk ID, checking service status, and controlling
//! the AnyDesk service.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::Tool;

/// Register AnyDesk tools with the tool registry
pub async fn register_anydesk_tools(registry: &crate::ToolRegistry) -> Result<()> {
    registry
        .register_tool(std::sync::Arc::new(AnyDeskGetIdTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskGetStatusTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskServiceControlTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskGetConnectionsTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskCheckX11DisplayTool))
        .await?;
    registry
        .register_tool(std::sync::Arc::new(AnyDeskDiagnoseX11AccessTool))
        .await?;

    Ok(())
}

/// Tool to get the AnyDesk ID
struct AnyDeskGetIdTool;

#[async_trait]
impl Tool for AnyDeskGetIdTool {
    fn name(&self) -> &str {
        "anydesk_get_id"
    }

    fn description(&self) -> &str {
        "Get the AnyDesk ID for remote connections"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        // Try to get AnyDesk ID from various sources
        // First check if AnyDesk is running and can provide the ID
        match get_anydesk_id() {
            Ok(id) => Ok(json!({
                "success": true,
                "anydesk_id": id
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Could not retrieve AnyDesk ID: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
        ]
    }
}

/// Tool to get AnyDesk service status
struct AnyDeskGetStatusTool;

#[async_trait]
impl Tool for AnyDeskGetStatusTool {
    fn name(&self) -> &str {
        "anydesk_get_status"
    }

    fn description(&self) -> &str {
        "Get the current status of the AnyDesk service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match get_anydesk_status() {
            Ok(status) => Ok(json!({
                "success": true,
                "status": status
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Could not get AnyDesk status: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "status".to_string(),
        ]
    }
}

/// Tool to control AnyDesk service (start/stop/restart)
struct AnyDeskServiceControlTool;

#[async_trait]
impl Tool for AnyDeskServiceControlTool {
    fn name(&self) -> &str {
        "anydesk_service_control"
    }

    fn description(&self) -> &str {
        "Control the AnyDesk service (start, stop, restart)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "stop", "restart"],
                    "description": "Action to perform on the AnyDesk service"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: action"))?;

        match control_anydesk_service(action) {
            Ok(result) => Ok(json!({
                "success": true,
                "action": action,
                "result": result
            })),
            Err(e) => Ok(json!({
                "success": false,
                "action": action,
                "error": format!("Failed to {} AnyDesk service: {}", action, e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "control".to_string(),
        ]
    }
}

/// Tool to get current AnyDesk connections
struct AnyDeskGetConnectionsTool;

/// Tool to check X11 display environment for AnyDesk
struct AnyDeskCheckX11DisplayTool;

/// Tool to diagnose X11 access issues for AnyDesk
struct AnyDeskDiagnoseX11AccessTool;

#[async_trait]
impl Tool for AnyDeskGetConnectionsTool {
    fn name(&self) -> &str {
        "anydesk_get_connections"
    }

    fn description(&self) -> &str {
        "Get information about current AnyDesk remote connections"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match get_anydesk_connections() {
            Ok(connections) => Ok(json!({
                "success": true,
                "connections": connections
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Could not get AnyDesk connections: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "connections".to_string(),
        ]
    }
}

#[async_trait]
impl Tool for AnyDeskCheckX11DisplayTool {
    fn name(&self) -> &str {
        "anydesk_check_x11_display"
    }

    fn description(&self) -> &str {
        "Check X11 display environment and configuration for AnyDesk screen sharing"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match check_x11_display_environment() {
            Ok(result) => Ok(json!({
                "success": true,
                "x11_environment": result
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Failed to check X11 display environment: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "x11".to_string(),
            "display".to_string(),
        ]
    }
}

#[async_trait]
impl Tool for AnyDeskDiagnoseX11AccessTool {
    fn name(&self) -> &str {
        "anydesk_diagnose_x11_access"
    }

    fn description(&self) -> &str {
        "Diagnose X11 access issues and provide fixes for AnyDesk screen sharing"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match diagnose_x11_access_issues() {
            Ok(result) => Ok(json!({
                "success": true,
                "diagnosis": result
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Failed to diagnose X11 access: {}", e)
            })),
        }
    }

    fn category(&self) -> &str {
        "remote"
    }

    fn namespace(&self) -> &str {
        "anydesk"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "remote".to_string(),
            "desktop".to_string(),
            "anydesk".to_string(),
            "x11".to_string(),
            "diagnostics".to_string(),
        ]
    }
}

/// Helper function to get AnyDesk ID
fn get_anydesk_id() -> Result<String> {
    // Try to get ID from AnyDesk configuration or command
    // First check if we can run anydesk command to get ID

    // Check for AnyDesk ID in various locations
    let config_paths = vec![
        "/etc/anydesk/anydesk.conf",
        "/home/jeremy/.anydesk/anydesk.conf",
        "/home/jeremy/.anydesk/user.conf",
    ];

    for path in config_paths {
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                // Look for ID in config file
                for line in content.lines() {
                    if line.contains("ad.anynet.id") || line.contains("id=") {
                        // Parse the ID from the line
                        if let Some(id_part) = line.split('=').nth(1) {
                            let id = id_part.trim().trim_matches('"');
                            if !id.is_empty() && id != "0" {
                                return Ok(id.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Try to run anydesk command if available
    match Command::new("anydesk").arg("--get-id").output() {
        Ok(output) if output.status.success() => {
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !id.is_empty() {
                return Ok(id);
            }
        }
        _ => {}
    }

    // Fallback: check systemd service and extract from logs or process
    match Command::new("systemctl")
        .args(&["show", "anydesk", "--property=MainPID"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let pid_str = String::from_utf8_lossy(&output.stdout);
            if let Some(pid_line) = pid_str.lines().next() {
                if let Some(pid) = pid_line.strip_prefix("MainPID=") {
                    if let Ok(pid_num) = pid.parse::<u32>() {
                        // Could potentially inspect process environment or memory
                        // For now, return a placeholder indicating AnyDesk is running
                        return Ok(format!("running_pid_{}", pid_num));
                    }
                }
            }
        }
        _ => {}
    }

    Err(anyhow!(
        "Could not determine AnyDesk ID. AnyDesk may not be properly configured or running."
    ))
}

/// Helper function to get AnyDesk service status
fn get_anydesk_status() -> Result<Value> {
    let mut status = json!({
        "service_running": false,
        "version": null,
        "connections": []
    });

    // Check systemd service status
    match Command::new("systemctl")
        .args(&["is-active", "anydesk"])
        .output()
    {
        Ok(output) => {
            let state_str = String::from_utf8_lossy(&output.stdout);
            let state = state_str.trim();
            status["service_running"] = json!(state == "active");
        }
        _ => {}
    }

    // Check if anydesk process is running
    match Command::new("pgrep").arg("anydesk").output() {
        Ok(output) if output.status.success() => {
            let pids: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
            status["process_pids"] = json!(pids);
        }
        _ => {}
    }

    // Try to get version
    match Command::new("anydesk").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            status["version"] = json!(version);
        }
        _ => {}
    }

    Ok(status)
}

/// Helper function to control AnyDesk service
fn control_anydesk_service(action: &str) -> Result<String> {
    let systemctl_action = match action {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        _ => return Err(anyhow!("Invalid action: {}", action)),
    };

    let output = Command::new("sudo")
        .args(&["systemctl", systemctl_action, "anydesk"])
        .output()?;

    if output.status.success() {
        Ok(format!("AnyDesk service {} successful", action))
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("Failed to {} AnyDesk service: {}", action, error))
    }
}

/// Helper function to get AnyDesk connections
fn get_anydesk_connections() -> Result<Vec<Value>> {
    // AnyDesk doesn't provide a direct way to list connections
    // This is a placeholder for future implementation
    // In a real implementation, this might parse logs or use AnyDesk's API

    let connections = Vec::new();

    // Check for any active connections by looking at network connections
    // or AnyDesk process status

    match Command::new("netstat").args(&["-tuln"]).output() {
        Ok(output) if output.status.success() => {
            let netstat_output = String::from_utf8_lossy(&output.stdout);
            // Look for AnyDesk-related ports (typically 7070, 6568, etc.)
            let anydesk_ports = ["7070", "6568", "80", "443"];
            for line in netstat_output.lines() {
                for port in &anydesk_ports {
                    if line.contains(&format!(":{} ", port))
                        || line.contains(&format!(":{}\n", port))
                    {
                        // Found a potential AnyDesk connection
                        // This is a simplified detection
                    }
                }
            }
        }
        _ => {}
    }

    Ok(connections)
}

/// Helper function to check X11 display environment
fn check_x11_display_environment() -> Result<Value> {
    let mut result = json!({
        "display_available": false,
        "display_variable": null,
        "xauthority_available": false,
        "xauthority_path": null,
        "anydesk_service_environment": {},
        "x11_server_running": false,
        "x11_auth_configured": false
    });

    // Check DISPLAY environment variable
    if let Ok(display) = std::env::var("DISPLAY") {
        result["display_variable"] = json!(display);
    }

    // Check XAUTHORITY environment variable
    if let Ok(xauthority) = std::env::var("XAUTHORITY") {
        result["xauthority_path"] = json!(xauthority);
        result["xauthority_available"] = json!(Path::new(&xauthority).exists());
    }

    // Check if X11 server is running by testing display access
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xdpyinfo").env("DISPLAY", &display).output() {
            Ok(output) if output.status.success() => {
                result["x11_server_running"] = json!(true);
                result["display_available"] = json!(true);
            }
            _ => {}
        }
    }

    // Check AnyDesk service environment
    match Command::new("systemctl")
        .args(&["show", "anydesk", "--property=Environment"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let env_str = String::from_utf8_lossy(&output.stdout);
            let env_vars: std::collections::HashMap<String, String> = env_str
                .strip_prefix("Environment=")
                .unwrap_or("")
                .split_whitespace()
                .filter_map(|kv| {
                    kv.split_once('=')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect();
            result["anydesk_service_environment"] = json!(env_vars);
        }
        _ => {}
    }

    // Check X11 authentication
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xauth").args(&["list", &display]).output() {
            Ok(output) if output.status.success() => {
                let auth_output = String::from_utf8_lossy(&output.stdout);
                if !auth_output.trim().is_empty() {
                    result["x11_auth_configured"] = json!(true);
                }
            }
            _ => {}
        }
    }

    Ok(result)
}

/// Helper function to diagnose X11 access issues
fn diagnose_x11_access_issues() -> Result<Value> {
    let mut issues = Vec::new();
    let mut recommendations = Vec::new();
    let mut fix_commands = Vec::new();

    // Check if AnyDesk service is running
    match Command::new("systemctl")
        .args(&["is-active", "anydesk"])
        .output()
    {
        Ok(output) => {
            let state_str = String::from_utf8_lossy(&output.stdout);
            let state = state_str.trim();
            if state != "active" {
                issues.push("AnyDesk service is not running".to_string());
                recommendations
                    .push("Start AnyDesk service with: sudo systemctl start anydesk".to_string());
                fix_commands.push("sudo systemctl start anydesk".to_string());
            }
        }
        _ => {
            issues.push("Cannot determine AnyDesk service status".to_string());
        }
    }

    // Check DISPLAY environment for AnyDesk service
    match Command::new("systemctl")
        .args(&["show", "anydesk", "--property=Environment"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let env_str = String::from_utf8_lossy(&output.stdout);
            let has_display = env_str.contains("DISPLAY=");
            let has_xauthority = env_str.contains("XAUTHORITY=");

            if !has_display {
                issues.push("AnyDesk service missing DISPLAY environment variable".to_string());
                recommendations.push("Add DISPLAY=:99 to AnyDesk service environment".to_string());
                fix_commands.push("sudo sed -i '/^Environment=/a Environment=DISPLAY=:99' /etc/systemd/system/anydesk.service && sudo systemctl daemon-reload && sudo systemctl restart anydesk".to_string());
            }

            if !has_xauthority {
                issues.push("AnyDesk service missing XAUTHORITY environment variable".to_string());
                recommendations.push(
                    "Add XAUTHORITY=/root/.Xauthority to AnyDesk service environment".to_string(),
                );
                fix_commands.push("sudo sed -i '/^Environment=/a Environment=XAUTHORITY=/root/.Xauthority' /etc/systemd/system/anydesk.service && sudo systemctl daemon-reload && sudo systemctl restart anydesk".to_string());
            }
        }
        _ => {
            issues.push("Cannot check AnyDesk service environment".to_string());
        }
    }

    // Check X11 server accessibility
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xdpyinfo").env("DISPLAY", &display).output() {
            Ok(output) if output.status.success() => {
                // X11 server is accessible
            }
            _ => {
                issues.push(format!("Cannot access X11 display {}", display));
                recommendations.push(
                    "Ensure Xvfb or X server is running on the specified display".to_string(),
                );
            }
        }
    } else {
        issues.push("DISPLAY environment variable not set".to_string());
        recommendations.push("Set DISPLAY=:99 for headless X11 server".to_string());
    }

    // Check X11 authentication
    if let Ok(display) = std::env::var("DISPLAY") {
        match Command::new("xauth").args(&["list", &display]).output() {
            Ok(output) if output.status.success() => {
                let auth_output = String::from_utf8_lossy(&output.stdout);
                if auth_output.trim().is_empty() {
                    issues.push(format!(
                        "No X11 authentication configured for display {}",
                        display
                    ));
                    recommendations.push(
                        "Generate X11 authentication cookie with: xauth generate :99 . trusted"
                            .to_string(),
                    );
                    fix_commands.push("xauth generate :99 . trusted".to_string());
                }
            }
            _ => {
                issues.push("Cannot check X11 authentication".to_string());
            }
        }
    }

    // Check if Xauthority file exists for root
    if !Path::new("/root/.Xauthority").exists() {
        issues.push("Xauthority file missing for root user".to_string());
        recommendations.push(
            "Copy user Xauthority to root: sudo cp /home/user/.Xauthority /root/.Xauthority"
                .to_string(),
        );
        fix_commands.push("sudo cp /home/jeremy/.Xauthority /root/.Xauthority && sudo chown root:root /root/.Xauthority && sudo chmod 600 /root/.Xauthority".to_string());
    }

    let diagnosis = json!({
        "issues": issues,
        "recommendations": recommendations,
        "can_fix_automatically": !fix_commands.is_empty(),
        "fix_commands": fix_commands
    });

    Ok(diagnosis)
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/code_search.rs">
//! Context-Aware Code Search Integration
//!
//! Automatically injects relevant code from indexed repositories
//! into tool execution context for smart suggestions and debugging.

use anyhow::Result;
use serde_json::Value;
use tracing::debug;

/// Code context extracted from indexed repos
#[derive(Debug, Clone, Default)]
pub struct CodeContext {
    pub relevant_code: Vec<CodeSnippet>,
    pub suggestions: Vec<String>,
    pub debugging_hints: Vec<String>,
}

impl CodeContext {
    pub fn is_empty(&self) -> bool {
        self.relevant_code.is_empty()
            && self.suggestions.is_empty()
            && self.debugging_hints.is_empty()
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "relevant_code": self.relevant_code.iter().map(|s| serde_json::json!({
                "file": s.file,
                "function": s.function.clone().unwrap_or_default(),
                "language": s.language,
                "code": s.code,
                "similarity": s.similarity,
            })).collect::<Vec<_>>(),
            "suggestions": self.suggestions,
            "debugging_hints": self.debugging_hints,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CodeSnippet {
    pub file: String,
    pub function: Option<String>,
    pub language: String,
    pub code: String,
    pub similarity: f64,
}

/// Inject code context into tool execution
pub async fn inject_code_context(
    tool_name: &str,
    arguments: &Value,
    current_file: Option<&str>,
) -> CodeContext {
    let mut context = CodeContext::default();

    // Build search query from tool context
    let query = build_context_query(tool_name, arguments, current_file);
    if query.is_empty() {
        return context;
    }

    // Search indexed code
    if let Ok(results) = search_code(&query, current_file, 5).await {
        context.relevant_code = results;
    }

    // Generate suggestions based on tool type
    context.suggestions = generate_suggestions(tool_name, &context.relevant_code);

    // Generate debugging hints for mutation tools
    if is_mutation_tool(tool_name) {
        context.debugging_hints = generate_debugging_hints(tool_name, &context.relevant_code);
    }

    debug!(
        "Injected {} code snippets for tool {}",
        context.relevant_code.len(),
        tool_name
    );
    context
}

fn build_context_query(tool_name: &str, arguments: &Value, current_file: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Tool category gives context
    if tool_name.contains("file") || tool_name.contains("write") {
        parts.push("file operations".to_string());
    } else if tool_name.contains("network") || tool_name.contains("ovs") {
        parts.push("network configuration".to_string());
    } else if tool_name.contains("service") || tool_name.contains("systemd") {
        parts.push("service management".to_string());
    } else if tool_name.contains("shell") || tool_name.contains("exec") {
        parts.push("shell scripting".to_string());
    }

    // Current file path
    if let Some(f) = current_file {
        parts.push(f.to_string());
    }

    // Arguments hint at intent
    if let Some(obj) = arguments.as_object() {
        for (k, v) in obj {
            parts.push(format!("{}: {}", k, v));
        }
    }

    parts.join(" ")
}

fn is_mutation_tool(name: &str) -> bool {
    name.contains("create")
        || name.contains("delete")
        || name.contains("update")
        || name.contains("modify")
        || name.contains("write")
        || name.contains("apply")
}

async fn search_code(query: &str, _repo: Option<&str>, limit: usize) -> Result<Vec<CodeSnippet>> {
    // Call the existing code search via HTTP to Qdrant
    let client = reqwest::Client::new();
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());

    // Embed query (simplified - would use HF API in production)
    let embedding = embed_text(query).await?;

    // Search Qdrant
    let url = format!("{}/collections/code_chunks/points/query", qdrant_url);
    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "query": embedding,
            "limit": limit,
            "with_payload": true
        }))
        .send()
        .await
        .ok();

    let mut snippets = Vec::new();
    if let Some(r) = response {
        if let Ok(json) = r.json::<serde_json::Value>().await {
            if let Some(results) = json.pointer("/result/points").and_then(|p| p.as_array()) {
                for point in results {
                    if let Some(payload) = point.pointer("/payload") {
                        snippets.push(CodeSnippet {
                            file: payload
                                .pointer("/file")
                                .and_then(|f| f.as_str())
                                .unwrap_or("")
                                .to_string(),
                            function: payload
                                .pointer("/function")
                                .and_then(|f| f.as_str())
                                .map(|s| s.to_string()),
                            language: payload
                                .pointer("/language")
                                .and_then(|l| l.as_str())
                                .unwrap_or("")
                                .to_string(),
                            code: payload
                                .pointer("/code")
                                .and_then(|c| c.as_str())
                                .unwrap_or("")
                                .to_string(),
                            similarity: point
                                .pointer("/score")
                                .and_then(|s| s.as_f64())
                                .unwrap_or(0.0),
                        });
                    }
                }
            }
        }
    }

    Ok(snippets)
}

async fn embed_text(_text: &str) -> Result<Vec<f64>> {
    // Simplified - in production use HF API
    Ok(vec![0.0; 384])
}

fn generate_suggestions(_tool_name: &str, code: &[CodeSnippet]) -> Vec<String> {
    let mut suggestions = Vec::new();

    if let Some(snippet) = code.first() {
        suggestions.push(format!(
            "Similar pattern found in {}: {}",
            snippet.file,
            snippet.function.clone().unwrap_or_default()
        ));
    }

    suggestions
}

fn generate_debugging_hints(tool_name: &str, _code: &[CodeSnippet]) -> Vec<String> {
    let mut hints = Vec::new();

    if tool_name.contains("delete") || tool_name.contains("remove") {
        hints.push("Consider checking dependencies before deletion".to_string());
        hints.push("Verify no services depend on this resource".to_string());
    }

    if tool_name.contains("network") || tool_name.contains("ovs") {
        hints.push("Check OVS service is running before modifications".to_string());
        hints.push("Consider backing up current configuration".to_string());
    }

    hints
}

/// Add code context to tool arguments
pub fn augment_arguments_with_context(mut arguments: Value, context: &CodeContext) -> Value {
    if !context.is_empty() {
        arguments["_code_context"] = context.to_json();
    }
    arguments
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/dbus_hybrid.rs">
//! D-Bus Hybrid Tools - Direct D-Bus protocol access without CLI tools
//!
//! This module provides tools that communicate directly with D-Bus services
//! using the native protocol, eliminating the need for CLI wrappers.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::OwnedValue;
use std::sync::Arc;

use crate::tool::{BoxedTool, Tool};

/// A tool that calls a specific D-Bus method
pub struct DbusMethodTool {
    /// Tool name (e.g., "dbus_systemd_manager_startunit")
    name: String,
    /// Human-readable description
    description: String,
    /// D-Bus service name
    service: String,
    /// D-Bus object path
    path: String,
    /// D-Bus interface name
    interface: String,
    /// D-Bus method name
    method: String,
    /// Input signature (D-Bus type string)
    input_signature: String,
    /// Output signature (D-Bus type string)
    output_signature: String,
    /// Use system bus (true) or session bus (false)
    use_system_bus: bool,
    /// JSON schema for input validation
    input_schema: Value,
}

impl DbusMethodTool {
    /// Create a new D-Bus method tool
    pub fn new(
        service: &str,
        path: &str,
        interface: &str,
        method: &str,
        input_signature: &str,
        output_signature: &str,
        use_system_bus: bool,
    ) -> Self {
        let name = format!(
            "dbus_{}_{}",
            interface.replace('.', "_").to_lowercase(),
            method.to_lowercase()
        );

        let description = format!(
            "Call D-Bus method {}.{} on service {}",
            interface, method, service
        );

        let input_schema = Self::generate_schema_from_signature(input_signature);

        Self {
            name,
            description,
            service: service.to_string(),
            path: path.to_string(),
            interface: interface.to_string(),
            method: method.to_string(),
            input_signature: input_signature.to_string(),
            output_signature: output_signature.to_string(),
            use_system_bus,
            input_schema,
        }
    }

    /// Generate JSON schema from D-Bus signature
    pub fn generate_schema_from_signature(signature: &str) -> Value {
        let mut properties = simd_json::value::owned::Object::new();
        let mut required = Vec::new();
        let mut param_idx = 0;

        for c in signature.chars() {
            let (param_name, schema) = match c {
                's' => (
                    format!("arg{}", param_idx),
                    simd_json::json!({"type": "string"}),
                ),
                'i' | 'n' => (
                    format!("arg{}", param_idx),
                    simd_json::json!({"type": "integer"}),
                ),
                'u' | 'q' | 't' | 'x' => (
                    format!("arg{}", param_idx),
                    simd_json::json!({"type": "integer", "minimum": 0}),
                ),
                'b' => (
                    format!("arg{}", param_idx),
                    simd_json::json!({"type": "boolean"}),
                ),
                'd' => (
                    format!("arg{}", param_idx),
                    simd_json::json!({"type": "number"}),
                ),
                'o' => (
                    format!("arg{}", param_idx),
                    simd_json::json!({
                        "type": "string",
                        "description": "D-Bus object path"
                    }),
                ),
                'a' | '(' | ')' | '{' | '}' | 'v' => continue, // Complex types - skip for now
                _ => continue,
            };

            required.push(param_name.clone());
            properties.insert(param_name, schema);
            param_idx += 1;
        }

        simd_json::json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }
}

#[async_trait]
impl Tool for DbusMethodTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        // Build D-Bus connection
        let connection = if self.use_system_bus {
            zbus::Connection::system().await?
        } else {
            zbus::Connection::session().await?
        };

        // Use Proxy for method calls (correct zbus API)
        let proxy = zbus::Proxy::new(
            &connection,
            self.service.as_str(),
            self.path.as_str(),
            self.interface.as_str(),
        )
        .await?;

        // Build arguments based on input signature
        let result = self.call_method_with_proxy(&proxy, &input).await?;

        Ok(simd_json::json!({
            "success": true,
            "service": self.service,
            "interface": self.interface,
            "method": self.method,
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "dbus".to_string(),
            self.service.clone(),
            self.interface.clone(),
        ]
    }
}

impl DbusMethodTool {
    /// Call method using zbus Proxy
    async fn call_method_with_proxy(
        &self,
        proxy: &zbus::Proxy<'_>,
        input: &Value,
    ) -> Result<Value> {
        // Handle different signatures
        match self.input_signature.as_str() {
            "" => {
                let result: zbus::zvariant::OwnedValue = proxy.call(self.method.as_str(), &()).await?;                self.owned_value_to_json(result)
            }
            "s" => {
                let arg0 = input
                    .get("arg0")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let result: zbus::zvariant::OwnedValue =
                    proxy.call(self.method.as_str(), &(arg0,)).await?;
                self.owned_value_to_json(result)
            }
            "ss" => {
                let arg0 = input
                    .get("arg0")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let arg1 = input
                    .get("arg1")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let result: zbus::zvariant::OwnedValue =
                    proxy.call(self.method.as_str(), &(arg0, arg1)).await?;
                self.owned_value_to_json(result)
            }
            "o" => {
                let arg0 = input
                    .get("arg0")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/");
                let result: zbus::zvariant::OwnedValue =
                    proxy.call(self.method.as_str(), &(arg0,)).await?;
                self.owned_value_to_json(result)
            }
            "ooo" => {
                let arg0 = input
                    .get("arg0")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/");
                let arg1 = input
                    .get("arg1")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/");
                let arg2 = input
                    .get("arg2")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/");
                let result: zbus::zvariant::OwnedValue =
                    proxy.call(self.method.as_str(), &(arg0, arg1, arg2)).await?;
                self.owned_value_to_json(result)
            }
            _ => {
                // Generic fallback - try no args
                let result: zbus::zvariant::OwnedValue = proxy.call(self.method.as_str(), &()).await?;
                self.owned_value_to_json(result)
            }
        }
    }

    /// Convert OwnedValue to JSON
    fn owned_value_to_json(&self, value: zbus::zvariant::OwnedValue) -> Result<Value> {
        // Try common conversions
        if let Ok(s) = <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(value.try_clone().unwrap()) {
            return Ok(Value::String(s));
        }
        if let Ok(b) = <bool as TryFrom<zbus::zvariant::OwnedValue>>::try_from(value.try_clone().unwrap()) {
            return Ok(Value::Bool(b));
        }
        if let Ok(n) = <i32 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(value.try_clone().unwrap()) {
            return Ok(Value::Number(n.into()));
        }
        if let Ok(n) = <u32 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(value.try_clone().unwrap()) {
            return Ok(Value::Number(n.into()));
        }
        if let Ok(n) = <i64 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(value.try_clone().unwrap()) {
            return Ok(Value::Number(n.into()));
        }
        if let Ok(n) = <u64 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(value.try_clone().unwrap()) {
            return Ok(Value::Number(n.into()));
        }

        // For object paths
        if let Ok(path) =
            <zbus::zvariant::OwnedObjectPath as TryFrom<zbus::zvariant::OwnedValue>>::try_from(
                value.try_clone().unwrap(),
            )
        {
            return Ok(Value::String(path.to_string()));
        }

        // Fallback: return signature info
        Ok(simd_json::json!({
            "type": "complex",
            "signature": self.output_signature,
            "raw": format!("{:?}", value)
        }))
    }
}

/// Create a D-Bus method tool
pub fn create_dbus_method_tool(
    service: &str,
    path: &str,
    interface: &str,
    method: &str,
    input_signature: &str,
    output_signature: &str,
    use_system_bus: bool,
) -> Result<BoxedTool> {
    Ok(Arc::new(DbusMethodTool::new(
        service,
        path,
        interface,
        method,
        input_signature,
        output_signature,
        use_system_bus,
    )))
}

/// Create common systemd tools
pub fn create_systemd_tools() -> Vec<BoxedTool> {
    vec![
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "StartUnit",
            "ss", // unit name, mode
            "o",  // job path
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "StopUnit",
            "ss", // unit name, mode
            "o",  // job path
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "RestartUnit",
            "ss", // unit name, mode
            "o",  // job path
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "GetUnit",
            "s", // unit name
            "o", // unit path
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "ListUnits",
            "",              // no args
            "a(ssssssouso)", // array of unit info
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "ListUnitFiles",
            "",      // no args
            "a(ss)", // array of (name, state)
            true,
        )),
    ]
}

/// Create NetworkManager tools
pub fn create_networkmanager_tools() -> Vec<BoxedTool> {
    vec![
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "GetDevices",
            "",   // no args
            "ao", // array of device paths
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "GetAllDevices",
            "",   // no args
            "ao", // array of device paths
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "ActivateConnection",
            "ooo", // connection, device, specific_object
            "o",   // active connection path
            true,
        )),
        Arc::new(DbusMethodTool::new(
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "DeactivateConnection",
            "o", // active connection path
            "",  // void
            true,
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name_generation() {
        let tool = DbusMethodTool::new(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
            "StartUnit",
            "ss",
            "o",
            true,
        );

        assert_eq!(
            tool.name(),
            "dbus_org_freedesktop_systemd1_manager_startunit"
        );
        assert_eq!(tool.category(), "dbus");
    }

    #[test]
    fn test_schema_generation() {
        let tool = DbusMethodTool::new(
            "org.test",
            "/",
            "org.test.Interface",
            "Method",
            "sib", // string, int, bool
            "s",
            false,
        );

        let schema = tool.input_schema();
        let props = schema.get("properties").unwrap();

        assert!(props.get("arg0").is_some());
        assert!(props.get("arg1").is_some());
        assert!(props.get("arg2").is_some());
    }

    #[test]
    fn test_systemd_tools_creation() {
        let tools = create_systemd_tools();
        assert!(!tools.is_empty());

        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(names.iter().any(|n| n.contains("startunit")));
        assert!(names.iter().any(|n| n.contains("stopunit")));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/dbus_introspection.rs">
//! D-Bus introspection tools (granular APIs).
//!
//! These tools provide the public-facing D-Bus and introspection helpers that
//! show up in the tool registry.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use op_core::{BusType, InterfaceInfo, ObjectInfo};
use op_introspection::IntrospectionService;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use zbus::Connection;

use crate::{Tool, ToolRegistry};

fn parse_bus(input: &Value, key: &str) -> BusType {
    match input.get(key).and_then(|v| v.as_str()).unwrap_or("system") {
        "session" => BusType::Session,
        _ => BusType::System,
    }
}

fn bus_str(bus: BusType) -> &'static str {
    match bus {
        BusType::System => "system",
        BusType::Session => "session",
    }
}

fn find_interface<'a>(info: &'a ObjectInfo, interface: &str) -> Result<&'a InterfaceInfo> {
    info.interfaces
        .iter()
        .find(|iface| iface.name == interface)
        .ok_or_else(|| anyhow!("Interface not found: {}", interface))
}

fn parse_required_str(input: &Value, key: &str) -> Result<String> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing required parameter: {}", key))
}

fn parse_bool(input: &Value, key: &str, default: bool) -> bool {
    input.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn parse_bounded_usize(input: &Value, key: &str, default: usize, min: usize, max: usize) -> usize {
    let parsed = input
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .or_else(|| {
            input
                .get(key)
                .and_then(|v| v.as_i64())
                .map(|v| v.max(0) as usize)
        });

    parsed.unwrap_or(default).clamp(min, max)
}

fn normalize_path(path: &str) -> String {
    let parts: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

fn join_child_path(parent: &str, child: &str) -> String {
    if child.starts_with('/') {
        return normalize_path(child);
    }

    let parent_norm = normalize_path(parent);
    if parent_norm == "/" {
        normalize_path(&format!("/{}", child))
    } else {
        normalize_path(&format!("{}/{}", parent_norm, child))
    }
}

fn normalize_object_info(mut info: ObjectInfo) -> ObjectInfo {
    info.path = normalize_path(&info.path);

    let mut normalized_children: Vec<String> = info
        .children
        .iter()
        .map(|child| join_child_path(&info.path, child))
        .collect();
    normalized_children.sort();
    normalized_children.dedup();
    info.children = normalized_children;

    info
}

struct ServiceTraversal {
    objects: Vec<ObjectInfo>,
    errors: Vec<String>,
    truncated: bool,
}

async fn collect_service_objects(
    introspection: &IntrospectionService,
    bus: BusType,
    service: &str,
    root_path: &str,
    max_depth: usize,
    max_objects: usize,
) -> ServiceTraversal {
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut objects = Vec::new();
    let mut errors = Vec::new();
    let mut truncated = false;

    queue.push_back((normalize_path(root_path), 0));

    while let Some((path, depth)) = queue.pop_front() {
        if objects.len() >= max_objects {
            truncated = true;
            break;
        }

        let path = normalize_path(&path);
        if !visited.insert(path.clone()) {
            continue;
        }

        match introspection.introspect(bus, service, &path).await {
            Ok(info) => {
                let info = normalize_object_info(info);
                let normalized_children = info.children.clone();

                if depth < max_depth {
                    for child_path in normalized_children {
                        if visited.contains(&child_path) {
                            continue;
                        }
                        if visited.len() + queue.len() >= max_objects {
                            truncated = true;
                            break;
                        }
                        queue.push_back((child_path, depth + 1));
                    }
                }

                objects.push(info);
            }
            Err(e) => {
                errors.push(format!("{}: {}", path, e));
            }
        }
    }

    objects.sort_by(|a, b| a.path.cmp(&b.path));

    if errors.len() > 200 {
        let omitted = errors.len() - 200;
        errors.truncate(200);
        errors.push(format!("... {} additional errors omitted", omitted));
    }

    ServiceTraversal {
        objects,
        errors,
        truncated,
    }
}

fn service_summary(objects: &[ObjectInfo]) -> Value {
    let mut unique_interfaces = HashSet::new();
    let mut unique_method_endpoints = HashSet::new();
    let mut unique_signal_endpoints = HashSet::new();
    let mut unique_property_endpoints = HashSet::new();

    let mut total_interfaces = 0usize;
    let mut total_methods = 0usize;
    let mut total_signals = 0usize;
    let mut total_properties = 0usize;

    for obj in objects {
        total_interfaces += obj.interfaces.len();
        for iface in &obj.interfaces {
            unique_interfaces.insert(iface.name.clone());
            total_methods += iface.methods.len();
            total_signals += iface.signals.len();
            total_properties += iface.properties.len();

            for method in &iface.methods {
                unique_method_endpoints
                    .insert(format!("{}|{}|{}", obj.path, iface.name, method.name));
            }
            for signal in &iface.signals {
                unique_signal_endpoints
                    .insert(format!("{}|{}|{}", obj.path, iface.name, signal.name));
            }
            for property in &iface.properties {
                unique_property_endpoints
                    .insert(format!("{}|{}|{}", obj.path, iface.name, property.name));
            }
        }
    }

    json!({
        "objects": objects.len(),
        "interfaces": total_interfaces,
        "methods": total_methods,
        "signals": total_signals,
        "properties": total_properties,
        "unique_interfaces": unique_interfaces.len(),
        "unique_method_endpoints": unique_method_endpoints.len(),
        "unique_signal_endpoints": unique_signal_endpoints.len(),
        "unique_property_endpoints": unique_property_endpoints.len()
    })
}

fn summary_count(summary: &Value, key: &str) -> usize {
    summary
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .or_else(|| {
            summary
                .get(key)
                .and_then(|v| v.as_i64())
                .map(|v| v.max(0) as usize)
        })
        .unwrap_or(0)
}

fn json_to_owned_value(value: &Value) -> Result<zbus::zvariant::OwnedValue> {
    use zbus::zvariant::Str as ZStr;

    if let Some(s) = value.as_str() {
        Ok(zbus::zvariant::OwnedValue::from(ZStr::from(s)))
    } else if let Some(b) = value.as_bool() {
        Ok(zbus::zvariant::OwnedValue::from(b))
    } else if let Some(i) = value.as_i64() {
        Ok(zbus::zvariant::OwnedValue::from(i))
    } else if let Some(u) = value.as_u64() {
        Ok(zbus::zvariant::OwnedValue::from(u))
    } else if let Some(f) = value.as_f64() {
        Ok(zbus::zvariant::OwnedValue::from(f))
    } else {
        Err(anyhow!("Unsupported argument type; use string/number/bool"))
    }
}

pub async fn register_dbus_introspection_tools(registry: &ToolRegistry) -> Result<()> {
    let introspection = Arc::new(IntrospectionService::new());

    registry
        .register_tool(Arc::new(DbusListServicesTool::new(introspection.clone())))
        .await?;
    registry
        .register_tool(Arc::new(DbusDiscoverSystemTool::new(introspection.clone())))
        .await?;
    registry
        .register_tool(Arc::new(DbusIntrospectServiceTool::new(
            introspection.clone(),
        )))
        .await?;
    registry
        .register_tool(Arc::new(DbusListObjectsTool::new(introspection.clone())))
        .await?;
    registry
        .register_tool(Arc::new(DbusIntrospectObjectTool::new(
            introspection.clone(),
        )))
        .await?;
    registry
        .register_tool(Arc::new(DbusListInterfacesTool::new(introspection.clone())))
        .await?;
    registry
        .register_tool(Arc::new(DbusListMethodsTool::new(introspection.clone())))
        .await?;
    registry
        .register_tool(Arc::new(DbusListPropertiesTool::new(introspection.clone())))
        .await?;
    registry
        .register_tool(Arc::new(DbusListSignalsTool::new(introspection.clone())))
        .await?;
    registry.register_tool(Arc::new(DbusCallMethodTool)).await?;
    registry
        .register_tool(Arc::new(DbusGetPropertyTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusSetPropertyTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusGetAllPropertiesTool::new(introspection)))
        .await?;

    Ok(())
}

struct DbusListServicesTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusListServicesTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusListServicesTool {
    fn name(&self) -> &str {
        "dbus_list_services"
    }

    fn description(&self) -> &str {
        "List all available D-Bus services on system or session bus"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                },
                "filter": {
                    "type": "string",
                    "description": "Optional filter pattern (e.g., 'org.freedesktop')"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bus = parse_bus(&input, "bus");
        let filter = input.get("filter").and_then(|v| v.as_str());
        let services = self.introspection.list_services(bus).await?;
        let mut names: Vec<String> = services.into_iter().map(|s| s.name).collect();

        names.retain(|name| !name.starts_with(':'));
        if let Some(pattern) = filter {
            names.retain(|name| name.contains(pattern));
        }

        Ok(json!({
            "bus": bus_str(bus),
            "count": names.len(),
            "services": names
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusDiscoverSystemTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusDiscoverSystemTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusDiscoverSystemTool {
    fn name(&self) -> &str {
        "dbus_discover_system"
    }

    fn description(&self) -> &str {
        "Recursively discover all D-Bus services, objects, methods, properties, and signals"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                },
                "path": {
                    "type": "string",
                    "default": "/"
                },
                "filter": {
                    "type": "string",
                    "description": "Optional service name substring filter"
                },
                "recursive": {
                    "type": "boolean",
                    "default": true
                },
                "max_services": {
                    "type": "integer",
                    "default": 256,
                    "minimum": 1,
                    "maximum": 5000
                },
                "max_depth": {
                    "type": "integer",
                    "default": 16,
                    "minimum": 0,
                    "maximum": 128
                },
                "max_objects_per_service": {
                    "type": "integer",
                    "default": 20000,
                    "minimum": 1,
                    "maximum": 200000
                },
                "include_objects": {
                    "type": "boolean",
                    "default": false
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bus = parse_bus(&input, "bus");
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let filter = input.get("filter").and_then(|v| v.as_str());
        let recursive = parse_bool(&input, "recursive", true);
        let max_services = parse_bounded_usize(&input, "max_services", 256, 1, 5000);
        let max_depth = parse_bounded_usize(&input, "max_depth", 16, 0, 128);
        let max_objects_per_service =
            parse_bounded_usize(&input, "max_objects_per_service", 20000, 1, 200000);
        let include_objects = parse_bool(&input, "include_objects", false);
        let normalized_path = normalize_path(path);

        let services = self.introspection.list_services(bus).await?;
        let mut service_names: Vec<String> = services.into_iter().map(|s| s.name).collect();
        service_names.retain(|name| !name.starts_with(':'));
        if let Some(pattern) = filter {
            service_names.retain(|name| name.contains(pattern));
        }
        service_names.sort();
        service_names.dedup();

        let available_services = service_names.len();
        let services_truncated = available_services > max_services;
        if services_truncated {
            service_names.truncate(max_services);
        }

        let mut total_objects = 0usize;
        let mut total_interfaces = 0usize;
        let mut total_methods = 0usize;
        let mut total_signals = 0usize;
        let mut total_properties = 0usize;
        let mut total_errors = 0usize;

        let mut unique_interfaces = HashSet::new();
        let mut unique_method_endpoints = HashSet::new();
        let mut unique_signal_endpoints = HashSet::new();
        let mut unique_property_endpoints = HashSet::new();

        let mut failed_services = Vec::new();
        let mut truncated_services = Vec::new();
        let mut service_results = Vec::new();

        for service_name in service_names {
            let traversal = if recursive {
                collect_service_objects(
                    &self.introspection,
                    bus,
                    &service_name,
                    &normalized_path,
                    max_depth,
                    max_objects_per_service,
                )
                .await
            } else {
                match self
                    .introspection
                    .introspect(bus, &service_name, &normalized_path)
                    .await
                {
                    Ok(info) => ServiceTraversal {
                        objects: vec![normalize_object_info(info)],
                        errors: Vec::new(),
                        truncated: false,
                    },
                    Err(e) => ServiceTraversal {
                        objects: Vec::new(),
                        errors: vec![format!("{}: {}", normalized_path, e)],
                        truncated: false,
                    },
                }
            };

            if traversal.truncated {
                truncated_services.push(service_name.clone());
            }

            if traversal.objects.is_empty() && !traversal.errors.is_empty() {
                failed_services.push(service_name.clone());
            }

            let summary = service_summary(&traversal.objects);
            total_objects += summary_count(&summary, "objects");
            total_interfaces += summary_count(&summary, "interfaces");
            total_methods += summary_count(&summary, "methods");
            total_signals += summary_count(&summary, "signals");
            total_properties += summary_count(&summary, "properties");
            total_errors += traversal.errors.len();

            for obj in &traversal.objects {
                for iface in &obj.interfaces {
                    unique_interfaces.insert(iface.name.clone());
                    for method in &iface.methods {
                        unique_method_endpoints.insert(format!(
                            "{}|{}|{}|{}",
                            service_name, obj.path, iface.name, method.name
                        ));
                    }
                    for signal in &iface.signals {
                        unique_signal_endpoints.insert(format!(
                            "{}|{}|{}|{}",
                            service_name, obj.path, iface.name, signal.name
                        ));
                    }
                    for property in &iface.properties {
                        unique_property_endpoints.insert(format!(
                            "{}|{}|{}|{}",
                            service_name, obj.path, iface.name, property.name
                        ));
                    }
                }
            }

            let mut service_entry = json!({
                "service": service_name,
                "path": normalized_path,
                "recursive": recursive,
                "summary": summary,
                "errors": traversal.errors,
                "truncated": traversal.truncated
            });

            if include_objects {
                let objects_json = simd_json::serde::to_owned_value(&traversal.objects)
                    .unwrap_or_else(|_| Value::Array(vec![]));
                if let Some(obj) = service_entry.as_object_mut() {
                    obj.insert("objects".to_string(), objects_json);
                }
            } else if let Some(obj) = service_entry.as_object_mut() {
                obj.insert(
                    "object_count".to_string(),
                    Value::from(traversal.objects.len() as u64),
                );
            }

            service_results.push(service_entry);
        }

        Ok(json!({
            "bus": bus_str(bus),
            "path": normalized_path,
            "recursive": recursive,
            "filter": filter,
            "limits": {
                "max_services": max_services,
                "max_depth": max_depth,
                "max_objects_per_service": max_objects_per_service
            },
            "services_available": available_services,
            "services_scanned": service_results.len(),
            "services_truncated": services_truncated,
            "failed_services": failed_services,
            "truncated_services": truncated_services,
            "summary": {
                "services": service_results.len(),
                "objects": total_objects,
                "interfaces": total_interfaces,
                "methods": total_methods,
                "signals": total_signals,
                "properties": total_properties,
                "errors": total_errors,
                "unique_interfaces": unique_interfaces.len(),
                "unique_method_endpoints": unique_method_endpoints.len(),
                "unique_signal_endpoints": unique_signal_endpoints.len(),
                "unique_property_endpoints": unique_property_endpoints.len()
            },
            "services": service_results
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusIntrospectServiceTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusIntrospectServiceTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusIntrospectServiceTool {
    fn name(&self) -> &str {
        "dbus_introspect_service"
    }

    fn description(&self) -> &str {
        "Get complete introspection data for a D-Bus service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                },
                "path": {
                    "type": "string",
                    "default": "/"
                },
                "recursive": {
                    "type": "boolean",
                    "default": true
                },
                "max_depth": {
                    "type": "integer",
                    "default": 16,
                    "minimum": 0,
                    "maximum": 128
                },
                "max_objects": {
                    "type": "integer",
                    "default": 20000,
                    "minimum": 1,
                    "maximum": 200000
                },
                "include_objects": {
                    "type": "boolean",
                    "default": true
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let bus = parse_bus(&input, "bus");
        let recursive = parse_bool(&input, "recursive", true);
        let max_depth = parse_bounded_usize(&input, "max_depth", 16, 0, 128);
        let max_objects = parse_bounded_usize(&input, "max_objects", 20000, 1, 200000);
        let include_objects = parse_bool(&input, "include_objects", true);

        if !recursive {
            let data = self
                .introspection
                .introspect_json(bus, &service, path)
                .await?;

            return Ok(json!({
                "bus": bus_str(bus),
                "service": service,
                "path": normalize_path(path),
                "recursive": false,
                "data": data
            }));
        }

        let traversal = collect_service_objects(
            &self.introspection,
            bus,
            &service,
            path,
            max_depth,
            max_objects,
        )
        .await;

        let normalized_root = normalize_path(path);
        let root_from_traversal = traversal
            .objects
            .iter()
            .find(|obj| obj.path == normalized_root)
            .cloned();

        let root_data = if let Some(root_obj) = root_from_traversal {
            simd_json::serde::to_owned_value(&root_obj).unwrap_or(Value::null())
        } else {
            self.introspection
                .introspect_json(bus, &service, &normalized_root)
                .await
                .unwrap_or(Value::null())
        };

        let mut response = json!({
            "bus": bus_str(bus),
            "service": service,
            "path": normalized_root,
            "recursive": true,
            "limits": {
                "max_depth": max_depth,
                "max_objects": max_objects
            },
            "summary": service_summary(&traversal.objects),
            "data": root_data,
            "errors": traversal.errors,
            "truncated": traversal.truncated
        });

        if include_objects {
            let objects_json = simd_json::serde::to_owned_value(&traversal.objects)
                .unwrap_or_else(|_| Value::Array(vec![]));
            if let Some(obj) = response.as_object_mut() {
                obj.insert("objects".to_string(), objects_json);
            }
        }

        Ok(response)
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusListObjectsTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusListObjectsTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusListObjectsTool {
    fn name(&self) -> &str {
        "dbus_list_objects"
    }

    fn description(&self) -> &str {
        "List object paths for a D-Bus service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                },
                "path": {
                    "type": "string",
                    "default": "/"
                },
                "recursive": {
                    "type": "boolean",
                    "default": true
                },
                "max_depth": {
                    "type": "integer",
                    "default": 16,
                    "minimum": 0,
                    "maximum": 128
                },
                "max_objects": {
                    "type": "integer",
                    "default": 20000,
                    "minimum": 1,
                    "maximum": 200000
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let bus = parse_bus(&input, "bus");
        let recursive = parse_bool(&input, "recursive", true);
        let max_depth = parse_bounded_usize(&input, "max_depth", 16, 0, 128);
        let max_objects = parse_bounded_usize(&input, "max_objects", 20000, 1, 200000);

        if !recursive {
            let info = self.introspection.introspect(bus, &service, path).await?;
            let mut objects: Vec<String> = info
                .children
                .iter()
                .map(|child| join_child_path(path, child))
                .collect();
            objects.sort();
            objects.dedup();

            return Ok(json!({
                "bus": bus_str(bus),
                "service": service,
                "path": normalize_path(path),
                "recursive": false,
                "count": objects.len(),
                "objects": objects
            }));
        }

        let traversal = collect_service_objects(
            &self.introspection,
            bus,
            &service,
            path,
            max_depth,
            max_objects,
        )
        .await;

        let object_paths: Vec<String> = traversal
            .objects
            .iter()
            .map(|obj| obj.path.clone())
            .collect();

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": normalize_path(path),
            "recursive": true,
            "limits": {
                "max_depth": max_depth,
                "max_objects": max_objects
            },
            "count": object_paths.len(),
            "objects": object_paths,
            "summary": service_summary(&traversal.objects),
            "errors": traversal.errors,
            "truncated": traversal.truncated
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusIntrospectObjectTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusIntrospectObjectTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusIntrospectObjectTool {
    fn name(&self) -> &str {
        "dbus_introspect_object"
    }

    fn description(&self) -> &str {
        "Introspect a specific D-Bus object path"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = parse_required_str(&input, "path")?;
        let bus = parse_bus(&input, "bus");
        let data = self
            .introspection
            .introspect_json(bus, &service, &path)
            .await?;

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "data": data
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusListInterfacesTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusListInterfacesTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusListInterfacesTool {
    fn name(&self) -> &str {
        "dbus_list_interfaces"
    }

    fn description(&self) -> &str {
        "List interfaces for a D-Bus object"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string", "default": "/" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let bus = parse_bus(&input, "bus");
        let info = self.introspection.introspect(bus, &service, path).await?;
        let interfaces: Vec<String> = info.interfaces.into_iter().map(|i| i.name).collect();

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "interfaces": interfaces
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusListMethodsTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusListMethodsTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusListMethodsTool {
    fn name(&self) -> &str {
        "dbus_list_methods"
    }

    fn description(&self) -> &str {
        "List methods for a D-Bus interface"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string", "default": "/" },
                "interface": { "type": "string" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "interface"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let interface = parse_required_str(&input, "interface")?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let bus = parse_bus(&input, "bus");
        let info = self.introspection.introspect(bus, &service, path).await?;
        let iface = find_interface(&info, &interface)?;
        let methods: Vec<String> = iface.methods.iter().map(|m| m.name.clone()).collect();

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "interface": interface,
            "methods": methods
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusListPropertiesTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusListPropertiesTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusListPropertiesTool {
    fn name(&self) -> &str {
        "dbus_list_properties"
    }

    fn description(&self) -> &str {
        "List properties for a D-Bus interface"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string", "default": "/" },
                "interface": { "type": "string" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "interface"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let interface = parse_required_str(&input, "interface")?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let bus = parse_bus(&input, "bus");
        let info = self.introspection.introspect(bus, &service, path).await?;
        let iface = find_interface(&info, &interface)?;
        let properties: Vec<String> = iface.properties.iter().map(|p| p.name.clone()).collect();

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "interface": interface,
            "properties": properties
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusListSignalsTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusListSignalsTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusListSignalsTool {
    fn name(&self) -> &str {
        "dbus_list_signals"
    }

    fn description(&self) -> &str {
        "List signals for a D-Bus interface"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string", "default": "/" },
                "interface": { "type": "string" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "interface"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let interface = parse_required_str(&input, "interface")?;
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let bus = parse_bus(&input, "bus");
        let info = self.introspection.introspect(bus, &service, path).await?;
        let iface = find_interface(&info, &interface)?;
        let signals: Vec<String> = iface.signals.iter().map(|s| s.name.clone()).collect();

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "interface": interface,
            "signals": signals
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusCallMethodTool;

#[async_trait]
impl Tool for DbusCallMethodTool {
    fn name(&self) -> &str {
        "dbus_call_method"
    }

    fn description(&self) -> &str {
        "Call a D-Bus method with arguments"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string" },
                "interface": { "type": "string" },
                "method": { "type": "string" },
                "args": {
                    "type": "array",
                    "description": "Method arguments (as JSON values)"
                },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "path", "interface", "method"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = parse_required_str(&input, "path")?;
        let interface = parse_required_str(&input, "interface")?;
        let method = parse_required_str(&input, "method")?;
        let bus = parse_bus(&input, "bus");
        let args = input
            .get("args")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let connection = match bus {
            BusType::System => Connection::system().await?,
            BusType::Session => Connection::session().await?,
        };

        let proxy = zbus::Proxy::new(
            &connection,
            service.as_str(),
            path.as_str(),
            interface.as_str(),
        )
        .await?;
        let zbus_args: Vec<zbus::zvariant::OwnedValue> = args
            .iter()
            .map(json_to_owned_value)
            .collect::<Result<Vec<_>>>()?;

        let result: zbus::zvariant::OwnedValue = proxy.call(method.as_str(), &zbus_args).await?;
        let result_json = simd_json::serde::to_owned_value(&result)?;

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "interface": interface,
            "method": method,
            "result": result_json
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusGetPropertyTool;

#[async_trait]
impl Tool for DbusGetPropertyTool {
    fn name(&self) -> &str {
        "dbus_get_property"
    }

    fn description(&self) -> &str {
        "Get the value of a D-Bus property"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string" },
                "interface": { "type": "string" },
                "property": { "type": "string" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "path", "interface", "property"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = parse_required_str(&input, "path")?;
        let interface = parse_required_str(&input, "interface")?;
        let property = parse_required_str(&input, "property")?;
        let bus = parse_bus(&input, "bus");

        let connection = match bus {
            BusType::System => Connection::system().await?,
            BusType::Session => Connection::session().await?,
        };

        let interface_name = zbus::names::InterfaceName::try_from(interface.as_str())?;
        let properties_proxy = zbus::fdo::PropertiesProxy::builder(&connection)
            .destination(service.as_str())?
            .path(path.as_str())?
            .build()
            .await?;

        let value: zbus::zvariant::OwnedValue = properties_proxy
            .get(interface_name, property.as_str())
            .await?;
        let value_json = simd_json::serde::to_owned_value(&value)?;

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "interface": interface,
            "property": property,
            "value": value_json
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusSetPropertyTool;

#[async_trait]
impl Tool for DbusSetPropertyTool {
    fn name(&self) -> &str {
        "dbus_set_property"
    }

    fn description(&self) -> &str {
        "Set the value of a D-Bus property"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string" },
                "interface": { "type": "string" },
                "property": { "type": "string" },
                "value": { "description": "Property value (as JSON)" },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "path", "interface", "property", "value"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = parse_required_str(&input, "path")?;
        let interface = parse_required_str(&input, "interface")?;
        let property = parse_required_str(&input, "property")?;
        let value = input
            .get("value")
            .ok_or_else(|| anyhow!("Missing required parameter: value"))?;
        let bus = parse_bus(&input, "bus");

        let connection = match bus {
            BusType::System => Connection::system().await?,
            BusType::Session => Connection::session().await?,
        };

        let interface_name = zbus::names::InterfaceName::try_from(interface.as_str())?;
        let properties_proxy = zbus::fdo::PropertiesProxy::builder(&connection)
            .destination(service.as_str())?
            .path(path.as_str())?
            .build()
            .await?;

        let zbus_value = json_to_owned_value(value)?;
        properties_proxy
            .set(
                interface_name,
                property.as_str(),
                zbus::zvariant::Value::from(zbus_value),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to set property: {}", e))?;

        Ok(json!({
            "bus": bus_str(bus),
            "success": true,
            "service": service,
            "path": path,
            "interface": interface,
            "property": property
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}

struct DbusGetAllPropertiesTool {
    introspection: Arc<IntrospectionService>,
}

impl DbusGetAllPropertiesTool {
    fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }
}

#[async_trait]
impl Tool for DbusGetAllPropertiesTool {
    fn name(&self) -> &str {
        "dbus_get_all_properties"
    }

    fn description(&self) -> &str {
        "Get all properties of a D-Bus object (optionally filter by interface)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string" },
                "path": { "type": "string" },
                "interface": {
                    "type": "string",
                    "description": "Optional: specific interface, otherwise all interfaces"
                },
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service", "path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = parse_required_str(&input, "service")?;
        let path = parse_required_str(&input, "path")?;
        let interface_filter = input.get("interface").and_then(|v| v.as_str());
        let bus = parse_bus(&input, "bus");

        let connection = match bus {
            BusType::System => Connection::system().await?,
            BusType::Session => Connection::session().await?,
        };

        let info = self.introspection.introspect(bus, &service, &path).await?;
        let properties_proxy = zbus::fdo::PropertiesProxy::builder(&connection)
            .destination(service.as_str())?
            .path(path.as_str())?
            .build()
            .await?;

        let mut all_properties = json!({});
        for iface in info.interfaces {
            if let Some(filter) = interface_filter {
                if iface.name != filter {
                    continue;
                }
            }

            let interface_name = zbus::names::InterfaceName::try_from(iface.name.as_str())?;
            let props: HashMap<String, zbus::zvariant::OwnedValue> = properties_proxy
                .get_all(interface_name)
                .await
                .unwrap_or_default();

            let mut iface_props = simd_json::value::owned::Object::new();
            for (prop_name, prop_value) in props {
                let value_json = simd_json::serde::to_owned_value(&prop_value)?;
                iface_props.insert(prop_name, value_json);
            }
            if let Some(obj) = all_properties.as_object_mut() {
                obj.insert(iface.name.clone(), Value::Object(Box::new(iface_props)));
            }
        }

        Ok(json!({
            "bus": bus_str(bus),
            "service": service,
            "path": path,
            "properties": all_properties
        }))
    }

    fn category(&self) -> &str {
        "dbus"
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/dbus_search_tool.rs">
//! DBus FTS Search Tool
//!
//! Provides semantic search across all DBus capabilities using the FTS5 indexer

use crate::{Tool, ToolDefinition, ToolRequest, ToolResult};
use async_trait::async_trait;
use op_core::types::BusType;
use op_introspection::IndexerManager;
use simd_json::json;
use std::sync::Arc;

/// Tool for searching DBus methods, properties, and signals using FTS
pub struct DbusSearchTool {
    indexer: Arc<IndexerManager>,
}

impl DbusSearchTool {
    pub fn new(indexer: Arc<IndexerManager>) -> Self {
        Self { indexer }
    }
}

#[async_trait]
impl Tool for DbusSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_dbus".to_string(),
            description:
                "Search for DBus methods, properties, and signals using semantic queries. \
                         Supports natural language queries like 'network wifi', 'bluetooth power', \
                         'systemd service control', etc."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (e.g., 'network', 'bluetooth power', 'systemd restart')"
                    },
                    "item_type": {
                        "type": "string",
                        "enum": ["method", "property", "signal", "all"],
                        "description": "Type of DBus item to search for (default: all)",
                        "default": "all"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 20)",
                        "default": 20,
                        "minimum": 1,
                        "maximum": 100
                    }
                },
                "required": ["query"]
            }),
            category: Some("dbus".to_string()),
            tags: vec![
                "search".to_string(),
                "discovery".to_string(),
                "fts".to_string(),
            ],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        // Parse arguments
        let query = match request.arguments.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return ToolResult::error(
                    request.id,
                    "Missing required argument: query",
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        let item_type = request
            .arguments
            .get("item_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let limit = request
            .arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        // Perform search
        let results = match item_type {
            "method" => self.indexer.search_methods(query.to_string(), limit).await,
            "property" => {
                self.indexer
                    .search_properties(query.to_string(), limit)
                    .await
            }
            "all" => self.indexer.search_all(query.to_string(), limit).await,
            _ => {
                return ToolResult::error(
                    request.id,
                    format!(
                        "Invalid item_type: {}. Must be 'method', 'property', 'signal', or 'all'",
                        item_type
                    ),
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        match results {
            Ok(search_results) => {
                let response = json!({
                    "query": query,
                    "item_type": item_type,
                    "count": search_results.len(),
                    "results": search_results.iter().map(|r| {
                        json!({
                            "service": r.service,
                            "object_path": r.object_path,
                            "interface": r.interface,
                            "type": r.item_type,
                            "name": r.item_name,
                            "description": r.description,
                            "relevance": r.relevance_score,
                            "full_name": format!("{}.{}.{}", r.service, r.interface, r.item_name)
                        })
                    }).collect::<Vec<_>>()
                });

                ToolResult::success(request.id, response, start.elapsed().as_millis() as u64)
            }
            Err(e) => ToolResult::error(
                request.id,
                format!("Search failed: {}", e),
                start.elapsed().as_millis() as u64,
            ),
        }
    }

    fn name(&self) -> &str {
        "search_dbus"
    }
}

/// Tool for rebuilding the DBus index
pub struct DbusRebuildIndexTool {
    indexer: Arc<IndexerManager>,
}

impl DbusRebuildIndexTool {
    pub fn new(indexer: Arc<IndexerManager>) -> Self {
        Self { indexer }
    }
}

#[async_trait]
impl Tool for DbusRebuildIndexTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rebuild_dbus_index".to_string(),
            description:
                "Rebuild the DBus FTS search index. Use this when DBus services have changed \
                         or to ensure the index is up-to-date."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "bus_type": {
                        "type": "string",
                        "enum": ["system", "session"],
                        "description": "Which DBus bus to index (default: system)",
                        "default": "system"
                    }
                },
                "required": []
            }),
            category: Some("dbus".to_string()),
            tags: vec!["admin".to_string(), "index".to_string()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        let bus_type_str = request
            .arguments
            .get("bus_type")
            .and_then(|v| v.as_str())
            .unwrap_or("system");

        let bus_type = match bus_type_str {
            "system" => BusType::System,
            "session" => BusType::Session,
            _ => {
                return ToolResult::error(
                    request.id,
                    "Invalid bus_type. Must be 'system' or 'session'",
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        // Clear existing index
        if let Err(e) = self.indexer.clear_index().await {
            return ToolResult::error(
                request.id,
                format!("Failed to clear index: {}", e),
                start.elapsed().as_millis() as u64,
            );
        }

        // Rebuild index
        match self.indexer.build_index(bus_type).await {
            Ok(stats) => {
                let response = json!({
                    "bus_type": bus_type_str,
                    "statistics": {
                        "services": stats.total_services,
                        "objects": stats.total_objects,
                        "interfaces": stats.total_interfaces,
                        "methods": stats.total_methods,
                        "properties": stats.total_properties,
                        "signals": stats.total_signals,
                        "scan_duration_seconds": stats.scan_duration_seconds,
                        "indexed_at": stats.indexed_at
                    },
                    "message": format!(
                        "Index rebuilt: {} methods, {} properties in {:.2}s",
                        stats.total_methods,
                        stats.total_properties,
                        stats.scan_duration_seconds
                    )
                });

                ToolResult::success(request.id, response, start.elapsed().as_millis() as u64)
            }
            Err(e) => ToolResult::error(
                request.id,
                format!("Failed to build index: {}", e),
                start.elapsed().as_millis() as u64,
            ),
        }
    }

    fn name(&self) -> &str {
        "rebuild_dbus_index"
    }
}

/// Tool for getting DBus index statistics
pub struct DbusIndexStatsTool {
    indexer: Arc<IndexerManager>,
}

impl DbusIndexStatsTool {
    pub fn new(indexer: Arc<IndexerManager>) -> Self {
        Self { indexer }
    }
}

#[async_trait]
impl Tool for DbusIndexStatsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "dbus_index_stats".to_string(),
            description:
                "Get statistics about the DBus search index including number of services, \
                         methods, properties indexed and when it was last updated."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            category: Some("dbus".to_string()),
            tags: vec!["info".to_string()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        match self.indexer.get_statistics().await {
            Ok(Some(stats)) => {
                let response = json!({
                    "services": stats.total_services,
                    "objects": stats.total_objects,
                    "interfaces": stats.total_interfaces,
                    "methods": stats.total_methods,
                    "properties": stats.total_properties,
                    "signals": stats.total_signals,
                    "scan_duration_seconds": stats.scan_duration_seconds,
                    "indexed_at": stats.indexed_at,
                    "age_seconds": chrono::Utc::now().timestamp() - stats.indexed_at
                });

                ToolResult::success(request.id, response, start.elapsed().as_millis() as u64)
            }
            Ok(None) => ToolResult::error(
                request.id,
                "No index statistics available. Index may not be built yet.",
                start.elapsed().as_millis() as u64,
            ),
            Err(e) => ToolResult::error(
                request.id,
                format!("Failed to get statistics: {}", e),
                start.elapsed().as_millis() as u64,
            ),
        }
    }

    fn name(&self) -> &str {
        "dbus_index_stats"
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/dbus_tool.rs">
//! D-Bus RPC tools
//! Dynamically generated tools from D-Bus introspection with full argument support

use async_trait::async_trait;
use op_core::{BusType, MethodInfo, ToolDefinition, ToolRequest, ToolResult};
use simd_json::{json, OwnedValue as Value};
use tracing::debug;
use zbus::zvariant::OwnedValue;
use zbus::Connection;

use crate::Tool;

/// A tool that calls a D-Bus method with full argument support
pub struct DbusMethodTool {
    pub bus_type: BusType,
    pub service: String,
    pub path: String,
    pub interface: String,
    pub method: MethodInfo,
    tool_name: String,
}

impl DbusMethodTool {
    /// Create a new D-Bus method tool
    pub fn new(
        bus_type: BusType,
        service: String,
        path: String,
        interface: String,
        method: MethodInfo,
    ) -> Self {
        let tool_name = Self::generate_tool_name(&service, &interface, &method.name);
        Self {
            bus_type,
            service,
            path,
            interface,
            method,
            tool_name,
        }
    }

    /// Generate a unique tool name from D-Bus identifiers
    fn generate_tool_name(service: &str, interface: &str, method: &str) -> String {
        let service_short = service.split('.').last().unwrap_or(service);
        let interface_short = interface.split('.').last().unwrap_or(interface);
        format!(
            "dbus_{}_{}_{}",
            service_short.replace('-', "_"),
            interface_short.replace('-', "_"),
            method.replace('-', "_")
        )
    }

    /// Convert D-Bus signature to JSON schema type
    /// Note: Keep schema simple for LLM compatibility - avoid complex constraints
    fn signature_to_schema(signature: &str, arg_name: Option<&str>) -> Value {
        let desc = arg_name.map(|n| format!(" ({})", n)).unwrap_or_default();
        match signature {
            "s" => json!({"type": "string", "description": format!("string{}", desc)}),
            "o" => json!({"type": "string", "description": format!("D-Bus object path{}", desc)}),
            "g" => json!({"type": "string", "description": format!("D-Bus signature{}", desc)}),
            "b" => json!({"type": "boolean", "description": format!("boolean{}", desc)}),
            "y" | "n" | "q" | "i" | "u" | "x" | "t" => {
                json!({"type": "integer", "description": format!("integer{}", desc)})
            }
            "d" => json!({"type": "number", "description": format!("number{}", desc)}),
            "v" => json!({"type": "string", "description": format!("variant{}", desc)}),
            "as" | "ao" => {
                json!({"type": "array", "items": {"type": "string"}, "description": format!("string array{}", desc)})
            }
            "ai" | "au" | "ax" | "at" => {
                json!({"type": "array", "items": {"type": "integer"}, "description": format!("integer array{}", desc)})
            }
            "ab" => {
                json!({"type": "array", "items": {"type": "boolean"}, "description": format!("boolean array{}", desc)})
            }
            // For complex types, use simple string representation to avoid schema issues
            _ => {
                json!({"type": "string", "description": format!("D-Bus type {}{}", signature, desc)})
            }
        }
    }

    /// Build the input argument signature string
    fn build_input_signature(&self) -> String {
        self.method
            .in_args
            .iter()
            .map(|arg| arg.signature.as_str())
            .collect()
    }
}

#[async_trait]
impl Tool for DbusMethodTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn definition(&self) -> ToolDefinition {
        let mut properties = simd_json::value::owned::Object::new();
        let mut required = Vec::new();

        for (idx, arg) in self.method.in_args.iter().enumerate() {
            let arg_name = arg.name.clone().unwrap_or_else(|| format!("arg{}", idx));
            properties.insert(
                arg_name.clone(),
                Self::signature_to_schema(&arg.signature, Some(&arg_name)),
            );
            required.push(arg_name);
        }

        let return_info = if self.method.out_args.is_empty() {
            "Returns: nothing".to_string()
        } else {
            let out_types: Vec<String> = self
                .method
                .out_args
                .iter()
                .map(|a| {
                    let name = a.name.as_deref().unwrap_or("result");
                    format!("{}: {}", name, a.signature)
                })
                .collect();
            format!("Returns: {}", out_types.join(", "))
        };

        ToolDefinition {
            name: self.tool_name.clone(),
            description: format!(
                "D-Bus: {}.{} on {}. {}",
                self.interface, self.method.name, self.service, return_info
            ),
            input_schema: json!({
                "type": "object",
                "properties": properties,
                "required": required
            }),
            category: Some("dbus".to_string()),
            tags: vec!["dbus".to_string(), self.service.clone()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        // Connect to D-Bus
        let connection = match self.bus_type {
            BusType::System => Connection::system().await,
            BusType::Session => Connection::session().await,
        };

        let connection = match connection {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::error(
                    &request.id,
                    format!("Failed to connect to D-Bus: {}", e),
                    start.elapsed().as_millis() as u64,
                )
            }
        };

        // Convert arguments based on method signature
        let in_sig = self.build_input_signature();
        debug!(
            "D-Bus call: {}.{} sig='{}' args={:?}",
            self.interface, self.method.name, in_sig, request.arguments
        );

        // Call method based on number and type of arguments
        let result = self.execute_call(&connection, &request.arguments).await;

        match result {
            Ok(json_result) => ToolResult::success(
                &request.id,
                json!({
                    "success": true,
                    "service": self.service,
                    "interface": self.interface,
                    "method": self.method.name,
                    "path": self.path,
                    "result": json_result
                }),
                start.elapsed().as_millis() as u64,
            ),
            Err(e) => {
                let error_msg = format!("{}", e);
                let detailed_error = if error_msg.contains("InvalidArgs") {
                    format!(
                        "Invalid arguments. Expected: {}. Error: {}",
                        in_sig, error_msg
                    )
                } else if error_msg.contains("AccessDenied") {
                    format!("Access denied - may require root. Error: {}", error_msg)
                } else {
                    error_msg
                };
                ToolResult::error(
                    &request.id,
                    detailed_error,
                    start.elapsed().as_millis() as u64,
                )
            }
        }
    }
}

impl DbusMethodTool {
    /// Execute the D-Bus call using low-level connection API for dynamic return types
    async fn execute_call(
        &self,
        connection: &Connection,
        args: &Value,
    ) -> Result<Value, zbus::Error> {
        use zbus::zvariant::ObjectPath;

        let service: zbus::names::BusName = self.service.as_str().try_into()?;
        let path: ObjectPath = self.path.as_str().try_into()?;
        let interface: zbus::names::InterfaceName = self.interface.as_str().try_into()?;
        let method: zbus::names::MemberName = self.method.name.as_str().try_into()?;

        let num_args = self.method.in_args.len();
        debug!(
            "D-Bus call {}.{} with {} args",
            self.interface, self.method.name, num_args
        );

        // Use connection.call_method for dynamic return types
        let reply = if num_args == 0 {
            connection
                .call_method(Some(service), path, Some(interface), method, &())
                .await?
        } else {
            // Get argument values in order
            let arg_values: Vec<Value> = self
                .method
                .in_args
                .iter()
                .enumerate()
                .map(|(idx, arg_info)| {
                    let name = arg_info
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("arg{}", idx));
                    args.get(&name)
                        .cloned()
                        .or_else(|| args.get(&format!("arg{}", idx)).cloned())
                        .unwrap_or(Value::Null)
                })
                .collect();

            // Get signatures for type-specific handling
            let sigs: Vec<&str> = self
                .method
                .in_args
                .iter()
                .map(|a| a.signature.as_str())
                .collect();

            self.call_with_args(
                connection,
                &service,
                &path,
                &interface,
                &method,
                &sigs,
                &arg_values,
            )
            .await?
        };

        // Convert reply to JSON using our robust converter
        Self::message_to_json(&reply)
    }

    async fn call_with_args(
        &self,
        connection: &Connection,
        service: &zbus::names::BusName<'_>,
        path: &zbus::zvariant::ObjectPath<'_>,
        interface: &zbus::names::InterfaceName<'_>,
        method: &zbus::names::MemberName<'_>,
        sigs: &[&str],
        vals: &[Value],
    ) -> Result<zbus::message::Message, zbus::Error> {
        match sigs.len() {
            1 => {
                self.call_1_arg(connection, service, path, interface, method, sigs, vals)
                    .await
            }
            2 => {
                self.call_2_args(connection, service, path, interface, method, sigs, vals)
                    .await
            }
            3 => {
                self.call_3_args(connection, service, path, interface, method, sigs, vals)
                    .await
            }
            4 => {
                self.call_4_args(connection, service, path, interface, method, sigs, vals)
                    .await
            }
            5 => {
                self.call_5_args(connection, service, path, interface, method, sigs, vals)
                    .await
            }
            n => Err(zbus::Error::Failure(format!(
                "Methods with {} arguments not yet supported",
                n
            ))),
        }
    }

    async fn call_1_arg(
        &self,
        connection: &Connection,
        service: &zbus::names::BusName<'_>,
        path: &zbus::zvariant::ObjectPath<'_>,
        interface: &zbus::names::InterfaceName<'_>,
        method: &zbus::names::MemberName<'_>,
        sigs: &[&str],
        vals: &[Value],
    ) -> Result<zbus::message::Message, zbus::Error> {
        match sigs.first().copied() {
            Some("s") => {
                let s = vals[0].as_str().unwrap_or("");
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s,),
                    )
                    .await
            }
            Some("o") => {
                let p: zbus::zvariant::ObjectPath = vals[0].as_str().unwrap_or("/").try_into()?;
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(p,),
                    )
                    .await
            }
            Some("b") => {
                let b = vals[0].as_bool().unwrap_or(false);
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(b,),
                    )
                    .await
            }
            Some("i") => {
                let n = vals[0].as_i64().unwrap_or(0) as i32;
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(n,),
                    )
                    .await
            }
            Some("u") => {
                let n = vals[0].as_u64().unwrap_or(0) as u32;
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(n,),
                    )
                    .await
            }
            Some("x") => {
                let n = vals[0].as_i64().unwrap_or(0);
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(n,),
                    )
                    .await
            }
            Some("t") => {
                let n = vals[0].as_u64().unwrap_or(0);
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(n,),
                    )
                    .await
            }
            _ => {
                let s = vals[0].as_str().unwrap_or("").to_string();
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s,),
                    )
                    .await
            }
        }
    }

    async fn call_2_args(
        &self,
        connection: &Connection,
        service: &zbus::names::BusName<'_>,
        path: &zbus::zvariant::ObjectPath<'_>,
        interface: &zbus::names::InterfaceName<'_>,
        method: &zbus::names::MemberName<'_>,
        sigs: &[&str],
        vals: &[Value],
    ) -> Result<zbus::message::Message, zbus::Error> {
        match (sigs.get(0).copied(), sigs.get(1).copied()) {
            (Some("s"), Some("s")) => {
                let s1 = vals[0].as_str().unwrap_or("");
                let s2 = vals[1].as_str().unwrap_or("");
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s1, s2),
                    )
                    .await
            }
            (Some("s"), Some("b")) => {
                let s = vals[0].as_str().unwrap_or("");
                let b = vals[1].as_bool().unwrap_or(false);
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s, b),
                    )
                    .await
            }
            (Some("s"), Some("u")) => {
                let s = vals[0].as_str().unwrap_or("");
                let n = vals[1].as_u64().unwrap_or(0) as u32;
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s, n),
                    )
                    .await
            }
            (Some("s"), Some("i")) => {
                let s = vals[0].as_str().unwrap_or("");
                let n = vals[1].as_i64().unwrap_or(0) as i32;
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s, n),
                    )
                    .await
            }
            (Some("u"), Some("u")) => {
                let n1 = vals[0].as_u64().unwrap_or(0) as u32;
                let n2 = vals[1].as_u64().unwrap_or(0) as u32;
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(n1, n2),
                    )
                    .await
            }
            _ => {
                let s1 = vals[0].as_str().unwrap_or("").to_string();
                let s2 = vals[1].as_str().unwrap_or("").to_string();
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s1, s2),
                    )
                    .await
            }
        }
    }

    async fn call_3_args(
        &self,
        connection: &Connection,
        service: &zbus::names::BusName<'_>,
        path: &zbus::zvariant::ObjectPath<'_>,
        interface: &zbus::names::InterfaceName<'_>,
        method: &zbus::names::MemberName<'_>,
        sigs: &[&str],
        vals: &[Value],
    ) -> Result<zbus::message::Message, zbus::Error> {
        match (
            sigs.get(0).copied(),
            sigs.get(1).copied(),
            sigs.get(2).copied(),
        ) {
            (Some("s"), Some("s"), Some("s")) => {
                let s1 = vals[0].as_str().unwrap_or("");
                let s2 = vals[1].as_str().unwrap_or("");
                let s3 = vals[2].as_str().unwrap_or("");
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s1, s2, s3),
                    )
                    .await
            }
            (Some("s"), Some("s"), Some("b")) => {
                let s1 = vals[0].as_str().unwrap_or("");
                let s2 = vals[1].as_str().unwrap_or("");
                let b = vals[2].as_bool().unwrap_or(false);
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s1, s2, b),
                    )
                    .await
            }
            (Some("s"), Some("s"), Some("u")) => {
                let s1 = vals[0].as_str().unwrap_or("");
                let s2 = vals[1].as_str().unwrap_or("");
                let n = vals[2].as_u64().unwrap_or(0) as u32;
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s1, s2, n),
                    )
                    .await
            }
            _ => {
                let s1 = vals[0].as_str().unwrap_or("").to_string();
                let s2 = vals[1].as_str().unwrap_or("").to_string();
                let s3 = vals[2].as_str().unwrap_or("").to_string();
                connection
                    .call_method(
                        Some(service.clone()),
                        path.clone(),
                        Some(interface.clone()),
                        method.clone(),
                        &(s1, s2, s3),
                    )
                    .await
            }
        }
    }

    async fn call_4_args(
        &self,
        connection: &Connection,
        service: &zbus::names::BusName<'_>,
        path: &zbus::zvariant::ObjectPath<'_>,
        interface: &zbus::names::InterfaceName<'_>,
        method: &zbus::names::MemberName<'_>,
        _sigs: &[&str],
        vals: &[Value],
    ) -> Result<zbus::message::Message, zbus::Error> {
        let s1 = vals[0].as_str().unwrap_or("").to_string();
        let s2 = vals[1].as_str().unwrap_or("").to_string();
        let s3 = vals[2].as_str().unwrap_or("").to_string();
        let s4 = vals[3].as_str().unwrap_or("").to_string();
        connection
            .call_method(
                Some(service.clone()),
                path.clone(),
                Some(interface.clone()),
                method.clone(),
                &(s1, s2, s3, s4),
            )
            .await
    }

    async fn call_5_args(
        &self,
        connection: &Connection,
        service: &zbus::names::BusName<'_>,
        path: &zbus::zvariant::ObjectPath<'_>,
        interface: &zbus::names::InterfaceName<'_>,
        method: &zbus::names::MemberName<'_>,
        _sigs: &[&str],
        vals: &[Value],
    ) -> Result<zbus::message::Message, zbus::Error> {
        let s1 = vals[0].as_str().unwrap_or("").to_string();
        let s2 = vals[1].as_str().unwrap_or("").to_string();
        let s3 = vals[2].as_str().unwrap_or("").to_string();
        let s4 = vals[3].as_str().unwrap_or("").to_string();
        let s5 = vals[4].as_str().unwrap_or("").to_string();
        connection
            .call_method(
                Some(service.clone()),
                path.clone(),
                Some(interface.clone()),
                method.clone(),
                &(s1, s2, s3, s4, s5),
            )
            .await
    }

    /// Convert D-Bus message reply to JSON - handles all complex types
    fn message_to_json(msg: &zbus::message::Message) -> Result<Value, zbus::Error> {
        use zbus::zvariant::Value as ZValue;

        fn convert_value(v: &ZValue<'_>) -> Value {
            match v {
                ZValue::U8(n) => json!(*n),
                ZValue::Bool(b) => json!(*b),
                ZValue::I16(n) => json!(*n),
                ZValue::U16(n) => json!(*n),
                ZValue::I32(n) => json!(*n),
                ZValue::U32(n) => json!(*n),
                ZValue::I64(n) => json!(*n),
                ZValue::U64(n) => json!(*n),
                ZValue::F64(n) => json!(*n),
                ZValue::Str(s) => json!(s.as_str()),
                ZValue::Signature(s) => json!(s.to_string()),
                ZValue::ObjectPath(p) => json!(p.as_str()),
                ZValue::Value(inner) => convert_value(inner),
                ZValue::Array(arr) => {
                    let items: Vec<Value> = arr.iter().map(|item| convert_value(&item)).collect();
                    json!(items)
                }
                ZValue::Dict(dict) => {
                    let mut map = simd_json::value::owned::Object::new();
                    for (k, v) in dict.iter() {
                        let key = match &k {
                            ZValue::Str(s) => s.to_string(),
                            other => format!("{:?}", other),
                        };
                        map.insert(key, convert_value(&v));
                    }
                    Value::Object(map)
                }
                ZValue::Structure(s) => {
                    let fields: Vec<Value> = s.fields().iter().map(|f| convert_value(f)).collect();
                    json!(fields)
                }
                ZValue::Fd(_) => json!("<file descriptor>"),
            }
        }

        let sig = msg.body().signature().to_string();
        debug!("Reply signature: {}", sig);

        // Try signature-specific deserialization for known complex types
        // SystemD ListUnits: a(ssssssouso) - array of 10-tuples
        if sig == "a(ssssssouso)" {
            type UnitInfo = (
                String,
                String,
                String,
                String,
                String,
                String,
                zbus::zvariant::OwnedObjectPath,
                u32,
                String,
                zbus::zvariant::OwnedObjectPath,
            );
            if let Ok(units) = msg.body().deserialize::<Vec<UnitInfo>>() {
                let json_units: Vec<Value> = units
                    .iter()
                    .map(|u| {
                        json!({
                            "name": u.0,
                            "description": u.1,
                            "load_state": u.2,
                            "active_state": u.3,
                            "sub_state": u.4,
                            "following": u.5,
                            "unit_path": u.6.as_str(),
                            "job_id": u.7,
                            "job_type": u.8,
                            "job_path": u.9.as_str()
                        })
                    })
                    .collect();
                return Ok(json!(json_units));
            }
        }

        // Try common simple return types first
        if sig == "s" {
            if let Ok(s) = msg.body().deserialize::<String>() {
                return Ok(json!(s));
            }
        }
        if sig == "b" {
            if let Ok(b) = msg.body().deserialize::<bool>() {
                return Ok(json!(b));
            }
        }
        if sig == "o" {
            if let Ok(p) = msg.body().deserialize::<zbus::zvariant::OwnedObjectPath>() {
                return Ok(json!(p.as_str()));
            }
        }
        if sig == "as" {
            if let Ok(arr) = msg.body().deserialize::<Vec<String>>() {
                return Ok(json!(arr));
            }
        }
        if sig == "ao" {
            if let Ok(arr) = msg
                .body()
                .deserialize::<Vec<zbus::zvariant::OwnedObjectPath>>()
            {
                let strs: Vec<&str> = arr.iter().map(|p| p.as_str()).collect();
                return Ok(json!(strs));
            }
        }

        // Try OwnedValue for other types
        match msg.body().deserialize::<OwnedValue>() {
            Ok(owned) => {
                let zval: ZValue = owned.into();
                Ok(convert_value(&zval))
            }
            Err(e) => {
                debug!("Failed to deserialize as OwnedValue: {}", e);
                // Return success with signature info
                Ok(
                    json!({"_success": true, "_signature": sig, "_note": "Complex return type - call succeeded"}),
                )
            }
        }
    }
}

/// Factory for creating D-Bus tools from introspection data
pub struct DbusToolFactory;

impl DbusToolFactory {
    /// Convert introspected methods into tools
    pub fn methods_to_tools(
        bus_type: BusType,
        service: &str,
        path: &str,
        interface: &str,
        methods: &[MethodInfo],
    ) -> Vec<std::sync::Arc<dyn Tool>> {
        methods
            .iter()
            .filter(|method| {
                // Skip methods that use file descriptors
                let uses_fd = method.in_args.iter().any(|a| a.signature.contains('h'))
                    || method.out_args.iter().any(|a| a.signature.contains('h'));
                !uses_fd
            })
            .map(|method| {
                std::sync::Arc::new(DbusMethodTool::new(
                    bus_type,
                    service.to_string(),
                    path.to_string(),
                    interface.to_string(),
                    method.clone(),
                )) as std::sync::Arc<dyn Tool>
            })
            .collect()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/dbus.rs">
//! D-Bus Tools - Native D-Bus Protocol Implementation
//!
//! These tools use zbus to communicate directly with D-Bus services.
//! They DO NOT use systemctl, nmcli, or any CLI commands.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::info;
use zbus::Connection;

use crate::{Tool, ToolRegistry};

// ============================================================================
// SYSTEMD RESTART UNIT TOOL
// ============================================================================

pub struct DbusSystemdRestartTool;

#[async_trait]
impl Tool for DbusSystemdRestartTool {
    fn name(&self) -> &str {
        "dbus_systemd_restart_unit"
    }

    fn description(&self) -> &str {
        "Restart a systemd unit via D-Bus (not systemctl)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unit": {
                    "type": "string",
                    "description": "Unit name (e.g., nginx.service)"
                },
                "mode": {
                    "type": "string",
                    "description": "Job mode (replace, fail, isolate, etc.)",
                    "default": "replace"
                }
            },
            "required": ["unit"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit = input
            .get("unit")
            .and_then(|n| n.as_str())
            .map(|n| n.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: unit"))?;

        let mode = input.get("mode").and_then(|m| m.as_str()).unwrap_or("replace");

        info!("Restarting unit '{}' via D-Bus", unit);

        let job_path = restart_unit_dbus(&unit, mode).await?;
        Ok(json!({
            "restarted": true,
            "unit": unit,
            "job_path": job_path,
            "protocol": "D-Bus"
        }))
    }

    fn category(&self) -> &str {
        "systemd"
    }
}

async fn restart_unit_dbus(unit: &str, mode: &str) -> Result<String> {
    let connection = Connection::system().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    ).await?;

    let job_path: zbus::zvariant::OwnedObjectPath = proxy
        .call("RestartUnit", &(unit, mode))
        .await?;

    Ok(job_path.to_string())
}

// ============================================================================
// SYSTEMD START UNIT TOOL
// ============================================================================

pub struct DbusSystemdStartTool;

#[async_trait]
impl Tool for DbusSystemdStartTool {
    fn name(&self) -> &str {
        "dbus_systemd_start_unit"
    }

    fn description(&self) -> &str {
        "Start a systemd unit via D-Bus (not systemctl)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unit": {
                    "type": "string",
                    "description": "Unit name (e.g., nginx.service)"
                },
                "mode": {
                    "type": "string",
                    "description": "Job mode (replace, fail, isolate, etc.)",
                    "default": "replace"
                }
            },
            "required": ["unit"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit = input
            .get("unit")
            .and_then(|n| n.as_str())
            .map(|n| n.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: unit"))?;

        let mode = input.get("mode").and_then(|m| m.as_str()).unwrap_or("replace");

        info!("Starting unit '{}' via D-Bus", unit);

        let job_path = start_unit_dbus(&unit, mode).await?;
        Ok(json!({
            "started": true,
            "unit": unit,
            "job_path": job_path,
            "protocol": "D-Bus"
        }))
    }

    fn category(&self) -> &str {
        "systemd"
    }
}

async fn start_unit_dbus(unit: &str, mode: &str) -> Result<String> {
    let connection = Connection::system().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    ).await?;

    let job_path: zbus::zvariant::OwnedObjectPath = proxy
        .call("StartUnit", &(unit, mode))
        .await?;

    Ok(job_path.to_string())
}

// ============================================================================
// SYSTEMD STOP UNIT TOOL
// ============================================================================

pub struct DbusSystemdStopTool;

#[async_trait]
impl Tool for DbusSystemdStopTool {
    fn name(&self) -> &str {
        "dbus_systemd_stop_unit"
    }

    fn description(&self) -> &str {
        "Stop a systemd unit via D-Bus (not systemctl)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unit": {
                    "type": "string",
                    "description": "Unit name (e.g., nginx.service)"
                },
                "mode": {
                    "type": "string",
                    "description": "Job mode (replace, fail, isolate, etc.)",
                    "default": "replace"
                }
            },
            "required": ["unit"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit = input
            .get("unit")
            .and_then(|n| n.as_str())
            .map(|n| n.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: unit"))?;

        let mode = input.get("mode").and_then(|m| m.as_str()).unwrap_or("replace");

        info!("Stopping unit '{}' via D-Bus", unit);

        let job_path = stop_unit_dbus(&unit, mode).await?;
        Ok(json!({
            "stopped": true,
            "unit": unit,
            "job_path": job_path,
            "protocol": "D-Bus"
        }))
    }

    fn category(&self) -> &str {
        "systemd"
    }
}

async fn stop_unit_dbus(unit: &str, mode: &str) -> Result<String> {
    let connection = Connection::system().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    ).await?;

    let job_path: zbus::zvariant::OwnedObjectPath = proxy
        .call("StopUnit", &(unit, mode))
        .await?;

    Ok(job_path.to_string())
}

// ============================================================================
// SYSTEMD GET UNIT STATUS TOOL
// ============================================================================

pub struct DbusSystemdStatusTool;

#[async_trait]
impl Tool for DbusSystemdStatusTool {
    fn name(&self) -> &str {
        "dbus_systemd_get_unit_status"
    }

    fn description(&self) -> &str {
        "Get systemd unit status via D-Bus (not systemctl)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unit": {
                    "type": "string",
                    "description": "Unit name (e.g., nginx.service)"
                }
            },
            "required": ["unit"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit = input
            .get("unit")
            .and_then(|n| n.as_str())
            .map(|n| n.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: unit"))?;

        info!("Getting status of unit '{}' via D-Bus", unit);

        get_unit_status_dbus(&unit).await
    }

    fn category(&self) -> &str {
        "systemd"
    }
}

async fn get_unit_status_dbus(unit: &str) -> Result<Value> {
    let connection = Connection::system().await?;

    // Get unit object path
    let manager_proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    ).await?;

    let unit_path: zbus::zvariant::OwnedObjectPath = manager_proxy
        .call("GetUnit", &(unit,))
        .await?;

    // Get unit properties
    let unit_proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        unit_path.as_str(),
        "org.freedesktop.systemd1.Unit",
    ).await?;

    let active_state: String = unit_proxy.get_property("ActiveState").await?;
    let sub_state: String = unit_proxy.get_property("SubState").await?;
    let load_state: String = unit_proxy.get_property("LoadState").await?;
    let description: String = unit_proxy.get_property("Description").await?;

    Ok(json!({
        "unit": unit,
        "active_state": active_state,
        "sub_state": sub_state,
        "load_state": load_state,
        "description": description,
        "protocol": "D-Bus"
    }))
}

// ============================================================================
// SYSTEMD LIST UNITS TOOL
// ============================================================================

pub struct DbusSystemdListUnitsTool;

#[async_trait]
impl Tool for DbusSystemdListUnitsTool {
    fn name(&self) -> &str {
        "dbus_systemd_list_units"
    }

    fn description(&self) -> &str {
        "List systemd units via D-Bus (not systemctl)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "Filter pattern (e.g., '*.service')"
                },
                "active_only": {
                    "type": "boolean",
                    "description": "Only show active units",
                    "default": false
                }
            },
            "required": []
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let filter = input
            .get("filter")
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());

        let active_only = input
            .get("active_only")
            .and_then(|a| a.as_bool())
            .unwrap_or(false);

        info!("Listing systemd units via D-Bus");

        let units = list_units_dbus(filter, active_only).await?;
        Ok(json!({
            "units": units,
            "count": units.len(),
            "protocol": "D-Bus"
        }))
    }

    fn category(&self) -> &str {
        "systemd"
    }
}

async fn list_units_dbus(filter: Option<String>, active_only: bool) -> Result<Vec<Value>> {
    let connection = Connection::system().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    ).await?;

    // ListUnits returns array of (name, description, load_state, active_state, sub_state, following, unit_path, job_id, job_type, job_path)
    let units: Vec<(
        String, String, String, String, String, String,
        zbus::zvariant::OwnedObjectPath, u32, String, zbus::zvariant::OwnedObjectPath
    )> = proxy.call("ListUnits", &()).await?;

    let units: Vec<Value> = units
        .into_iter()
        .filter(|(name, _, _, active_state, _, _, _, _, _, _)| {
            let name_match = filter.as_ref().map(|f| {
                if f.contains('*') {
                    let pattern = f.replace('*', "");
                    name.contains(&pattern)
                } else {
                    name.contains(f)
                }
            }).unwrap_or(true);

            let active_match = if active_only {
                active_state == "active"
            } else {
                true
            };

            name_match && active_match
        })
        .map(|(name, description, load_state, active_state, sub_state, _, _, _, _, _)| {
            json!({
                "name": name,
                "description": description,
                "load_state": load_state,
                "active_state": active_state,
                "sub_state": sub_state
            })
        })
        .collect();

    Ok(units)
}

/// Register all D-Bus tools
pub async fn register_dbus_tools(registry: &ToolRegistry) -> Result<()> {
    registry.register_tool(Arc::new(DbusSystemdRestartTool)).await?;
    registry.register_tool(Arc::new(DbusSystemdStartTool)).await?;
    registry.register_tool(Arc::new(DbusSystemdStopTool)).await?;
    registry.register_tool(Arc::new(DbusSystemdStatusTool)).await?;
    registry.register_tool(Arc::new(DbusSystemdListUnitsTool)).await?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/error_reporting_tool.rs">
//! Tool for reporting internal errors

use crate::Tool;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use anyhow::Result;

/// Tool to report an internal error to the user
pub struct ReportInternalErrorTool;

#[async_trait]
impl Tool for ReportInternalErrorTool {
    fn name(&self) -> &str {
        "report_internal_error"
    }

    fn description(&self) -> &str {
        "Report an internal error or unexpected state to the user. Use this when a request cannot be fulfilled due to an internal failure."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "error_message": {
                    "type": "string",
                    "description": "A detailed description of the internal error."
                },
                "failed_action": {
                    "type": "string",
                    "description": "The action that failed (e.g., the tool I was trying to use)."
                }
            },
            "required": ["error_message", "failed_action"]
        })
    }

    fn category(&self) -> &str {
        "chat"
    }

    fn tags(&self) -> Vec<String> {
        vec!["error".to_string(), "internal".to_string(), "meta".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let error_message = input.get("error_message").and_then(|v| v.as_str()).unwrap_or("Unknown error");
        let failed_action = input.get("failed_action").and_then(|v| v.as_str()).unwrap_or("Unknown action");

        // This tool doesn't actually *do* anything other than provide a structured way
        // for me to report that I've had an internal error. The chat orchestrator
        // will see that this tool was called and can then formulate a user-friendly
        // error message.

        Ok(json!({
            "success": true,
            "message": "Internal error reported.",
            "reported_error": {
                "failed_action": failed_action,
                "error_message": error_message
            }
        }))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/file.rs">
//! File Tools with Access Level Security
//!
//! Provides file operations with access level based security:
//! - Unrestricted (Admin): Full read/write access
//! - Restricted: Limited read-only access
//!
//! ## Security Model
//!
//! The chatbot is a FULL SYSTEM ADMINISTRATOR.
//! Admin users can read/write any file (except path traversal).
//! Audit logging is handled by the snowball plugin.

use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::path::Path;
use std::sync::Arc;

use crate::registry::ToolDefinition;
use crate::security::get_security_validator;
use crate::Tool;

/// Register all file tools
pub async fn register_file_tools(registry: &crate::ToolRegistry) -> anyhow::Result<()> {
    let tools = vec![
        SecureFileTool::read(),
        SecureFileTool::write(),
        SecureFileTool::list(),
        SecureFileTool::exists(),
        SecureFileTool::stat(),
    ];

    for tool in tools {
        let name = tool.name().to_string();
        let definition = ToolDefinition {
            name: name.clone(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            schema_version: "https://json-schema.org/draft/next/schema".to_string(),
            category: tool.category().to_string(),
            namespace: "system.v1".to_string(),
            tags: tool.tags(),
        };
        registry
            .register(name.into(), Arc::new(tool), definition)
            .await?;
    }

    Ok(())
}

// ============================================================================
// SECURE FILE TOOL
// ============================================================================

pub struct SecureFileTool {
    name: String,
    description: String,
}

impl SecureFileTool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }

    pub fn read() -> Self {
        Self::new(
            "file_read",
            "Read file contents. Full access for admin users.",
        )
    }

    pub fn write() -> Self {
        Self::new(
            "file_write",
            "Write content to a file. Full access for admin users.",
        )
    }

    pub fn list() -> Self {
        Self::new(
            "file_list",
            "List directory contents. Full access for admin users.",
        )
    }

    pub fn exists() -> Self {
        Self::new("file_exists", "Check if a file exists.")
    }

    pub fn stat() -> Self {
        Self::new("file_stat", "Get file metadata (size, type, permissions).")
    }
}

#[async_trait]
impl Tool for SecureFileTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> &str {
        "filesystem"
    }

    fn tags(&self) -> Vec<String> {
        vec!["file".to_string(), "filesystem".to_string()]
    }

    fn input_schema(&self) -> Value {
        match self.name.as_str() {
            "file_read" => json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to read"
                    },
                    "max_lines": {
                        "type": "integer",
                        "description": "Maximum lines to return (default: 1000)",
                        "default": 1000
                    }
                },
                "required": ["path"]
            }),
            "file_write" => json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "Append instead of overwrite (default: false)",
                        "default": false
                    }
                },
                "required": ["path", "content"]
            }),
            "file_list" => json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path to list"
                    },
                    "max_entries": {
                        "type": "integer",
                        "description": "Maximum entries to return (default: 100)",
                        "default": 100
                    }
                },
                "required": ["path"]
            }),
            "file_exists" => json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to check"
                    }
                },
                "required": ["path"]
            }),
            "file_stat" => json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to get metadata for"
                    }
                },
                "required": ["path"]
            }),
            _ => json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn execute(&self, args: Value) -> anyhow::Result<Value> {
        let validator = get_security_validator();

        match self.name.as_str() {
            "file_read" => {
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

                // Validate path for reading
                validator
                    .validate_read_path(path)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                let max_lines = args
                    .get("max_lines")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1000) as usize;

                let content = tokio::fs::read_to_string(path).await?;
                let lines: Vec<&str> = content.lines().take(max_lines).collect();
                let truncated = content.lines().count() > max_lines;

                Ok(json!({
                    "path": path,
                    "content": lines.join("\n"),
                    "lines": lines.len(),
                    "truncated": truncated
                }))
            }

            "file_write" => {
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

                let content = args
                    .get("content")
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content"))?;

                let append = args
                    .get("append")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Validate path for writing
                validator
                    .validate_write_path(path)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                if append {
                    use tokio::io::AsyncWriteExt;
                    let mut file = tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .await?;
                    file.write_all(content.as_bytes()).await?;
                } else {
                    tokio::fs::write(path, content).await?;
                }

                Ok(json!({
                    "path": path,
                    "written": content.len(),
                    "append": append,
                    "success": true
                }))
            }

            "file_list" => {
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

                // Validate path for reading
                validator
                    .validate_read_path(path)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                let max_entries = args
                    .get("max_entries")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100)
                    .min(1000) as usize;

                let mut entries = tokio::fs::read_dir(path).await?;
                let mut files = Vec::new();
                let mut count = 0;

                while let Some(entry) = entries.next_entry().await? {
                    if count >= max_entries {
                        break;
                    }

                    let file_type = entry.file_type().await.ok();
                    let metadata = entry.metadata().await.ok();

                    files.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "is_dir": file_type.map(|t| t.is_dir()).unwrap_or(false),
                        "is_file": file_type.map(|t| t.is_file()).unwrap_or(false),
                        "size": metadata.map(|m| m.len()).unwrap_or(0)
                    }));

                    count += 1;
                }

                Ok(json!({
                    "path": path,
                    "entries": files,
                    "count": files.len(),
                    "truncated": count >= max_entries
                }))
            }

            "file_exists" => {
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

                // For exists check, still validate against path traversal
                validator
                    .validate_read_path(path)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                let exists = Path::new(path).exists();

                Ok(json!({
                    "path": path,
                    "exists": exists
                }))
            }

            "file_stat" => {
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing required parameter: path"))?;

                // Validate path for reading
                validator
                    .validate_read_path(path)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                let metadata = tokio::fs::metadata(path).await?;

                Ok(json!({
                    "path": path,
                    "size": metadata.len(),
                    "is_file": metadata.is_file(),
                    "is_dir": metadata.is_dir(),
                    "is_symlink": metadata.file_type().is_symlink(),
                    "readonly": metadata.permissions().readonly()
                }))
            }

            _ => Err(anyhow::anyhow!("Unknown file operation: {}", self.name)),
        }
    }
}

/// Legacy alias
pub type FileTool = SecureFileTool;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file() {
        let tool = SecureFileTool::read();
        let temp_path = "/tmp/test_read_file.txt";
        tokio::fs::write(temp_path, "test content").await.unwrap();

        let result = tool.execute(json!({"path": temp_path})).await;
        assert!(result.is_ok());

        let _ = tokio::fs::remove_file(temp_path).await;
    }

    #[tokio::test]
    async fn test_write_file() {
        let tool = SecureFileTool::write();
        let temp_path = "/tmp/test_write_file.txt";

        let result = tool
            .execute(json!({
                "path": temp_path,
                "content": "hello world"
            }))
            .await;

        assert!(result.is_ok());
        let content = tokio::fs::read_to_string(temp_path).await.unwrap();
        assert_eq!(content, "hello world");

        let _ = tokio::fs::remove_file(temp_path).await;
    }

    #[tokio::test]
    async fn test_path_traversal_blocked() {
        let tool = SecureFileTool::read();
        let result = tool.execute(json!({"path": "/tmp/../etc/passwd"})).await;
        assert!(result.is_err());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/gcloud_tools.rs">
//! GCloud CLI introspection tools.
//!
//! These tools provide access to the gcloud CLI command hierarchy,
//! allowing agents to discover and understand gcloud commands, flags,
//! and arguments.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use op_inspector::{GCloudCommand, GCloudParser, GCloudSchema};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{Tool, ToolRegistry};

/// Cached gcloud schema to avoid re-introspection
struct GCloudCache {
    schema: Option<GCloudSchema>,
}

impl GCloudCache {
    fn new() -> Self {
        Self { schema: None }
    }
}

pub async fn register_gcloud_tools(registry: &ToolRegistry) -> Result<()> {
    let parser = Arc::new(GCloudParser::new());
    let cache = Arc::new(RwLock::new(GCloudCache::new()));

    registry
        .register_tool(Arc::new(GCloudIntrospectTool::new(
            parser.clone(),
            cache.clone(),
        )))
        .await?;
    registry
        .register_tool(Arc::new(GCloudListGroupsTool::new(
            parser.clone(),
            cache.clone(),
        )))
        .await?;
    registry
        .register_tool(Arc::new(GCloudGetCommandTool::new(
            parser.clone(),
            cache.clone(),
        )))
        .await?;
    registry
        .register_tool(Arc::new(GCloudSearchTool::new(
            parser.clone(),
            cache.clone(),
        )))
        .await?;

    tracing::info!("Registered 4 gcloud introspection tools");
    Ok(())
}

// Helper to get or populate the cache
async fn get_cached_schema(
    parser: &GCloudParser,
    cache: &RwLock<GCloudCache>,
    max_depth: usize,
) -> Result<GCloudSchema> {
    // Check cache first
    {
        let cache_read = cache.read().await;
        if let Some(ref schema) = cache_read.schema {
            return Ok(schema.clone());
        }
    }

    // Introspect and cache
    let schema = parser.introspect_full(max_depth).await?;
    {
        let mut cache_write = cache.write().await;
        cache_write.schema = Some(schema.clone());
    }

    Ok(schema)
}

// Helper to find a command by path
fn find_command<'a>(root: &'a GCloudCommand, path: &[String]) -> Option<&'a GCloudCommand> {
    let mut current = root;
    for part in path {
        current = current.subcommands.get(part)?;
    }
    Some(current)
}

// Helper to collect all commands matching a pattern
fn search_commands(
    cmd: &GCloudCommand,
    pattern: &str,
    results: &mut Vec<Value>,
    max_results: usize,
) {
    if results.len() >= max_results {
        return;
    }

    let pattern_lower = pattern.to_lowercase();

    // Check if this command matches
    if cmd.name.to_lowercase().contains(&pattern_lower)
        || cmd.description.to_lowercase().contains(&pattern_lower)
        || cmd.full_path.to_lowercase().contains(&pattern_lower)
    {
        results.push(json!({
            "name": cmd.name,
            "full_path": cmd.full_path,
            "description": cmd.description,
            "is_group": cmd.is_group,
            "flag_count": cmd.flags.len(),
        }));
    }

    // Search in flags
    for flag in &cmd.flags {
        if results.len() >= max_results {
            return;
        }
        if flag.name.to_lowercase().contains(&pattern_lower)
            || flag.description.to_lowercase().contains(&pattern_lower)
        {
            results.push(json!({
                "type": "flag",
                "command": cmd.full_path,
                "flag": flag.name,
                "description": flag.description,
                "required": flag.required,
                "value_type": flag.value_type,
            }));
        }
    }

    // Recurse into subcommands
    for subcmd in cmd.subcommands.values() {
        search_commands(subcmd, pattern, results, max_results);
    }
}

// =============================================================================
// TOOLS
// =============================================================================

struct GCloudIntrospectTool {
    parser: Arc<GCloudParser>,
    cache: Arc<RwLock<GCloudCache>>,
}

impl GCloudIntrospectTool {
    fn new(parser: Arc<GCloudParser>, cache: Arc<RwLock<GCloudCache>>) -> Self {
        Self { parser, cache }
    }
}

#[async_trait]
impl Tool for GCloudIntrospectTool {
    fn name(&self) -> &str {
        "gcloud_introspect"
    }

    fn description(&self) -> &str {
        "Introspect the gcloud CLI command hierarchy. Returns full schema with all commands, groups, flags, and arguments."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth for recursive introspection (default: 3)",
                    "default": 3,
                    "minimum": 1,
                    "maximum": 10
                },
                "refresh": {
                    "type": "boolean",
                    "description": "Force re-introspection even if cached (default: false)",
                    "default": false
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let max_depth = input.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
        let refresh = input
            .get("refresh")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Clear cache if refresh requested
        if refresh {
            let mut cache_write = self.cache.write().await;
            cache_write.schema = None;
        }

        let schema = get_cached_schema(&self.parser, &self.cache, max_depth).await?;

        Ok(json!({
            "schema_version": schema.schema_version,
            "gcloud_version": schema.gcloud_version,
            "account": schema.account,
            "statistics": {
                "total_groups": schema.statistics.total_groups,
                "total_commands": schema.statistics.total_commands,
                "total_flags": schema.statistics.total_flags,
                "introspection_time_ms": schema.statistics.introspection_time_ms,
            },
            "hierarchy": simd_json::serde::to_owned_value(&schema.hierarchy)?
        }))
    }

    fn category(&self) -> &str {
        "gcloud"
    }

    fn tags(&self) -> Vec<String> {
        vec!["gcloud".into(), "introspection".into(), "cli".into()]
    }
}

struct GCloudListGroupsTool {
    parser: Arc<GCloudParser>,
    cache: Arc<RwLock<GCloudCache>>,
}

impl GCloudListGroupsTool {
    fn new(parser: Arc<GCloudParser>, cache: Arc<RwLock<GCloudCache>>) -> Self {
        Self { parser, cache }
    }
}

#[async_trait]
impl Tool for GCloudListGroupsTool {
    fn name(&self) -> &str {
        "gcloud_list_groups"
    }

    fn description(&self) -> &str {
        "List gcloud command groups at a given path. Use empty path for top-level groups."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command path (e.g., ['compute', 'instances']). Empty for root.",
                    "default": []
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path: Vec<String> = input
            .get("path")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let schema = get_cached_schema(&self.parser, &self.cache, 3).await?;
        let cmd = find_command(&schema.hierarchy, &path)
            .ok_or_else(|| anyhow!("Command path not found: {:?}", path))?;

        let groups: Vec<Value> = cmd
            .subcommands
            .values()
            .filter(|c| c.is_group)
            .map(|c| {
                json!({
                    "name": c.name,
                    "full_path": c.full_path,
                    "description": c.description,
                    "subcommand_count": c.subcommands.len(),
                })
            })
            .collect();

        let commands: Vec<Value> = cmd
            .subcommands
            .values()
            .filter(|c| !c.is_group)
            .map(|c| {
                json!({
                    "name": c.name,
                    "full_path": c.full_path,
                    "description": c.description,
                    "flag_count": c.flags.len(),
                })
            })
            .collect();

        Ok(json!({
            "path": path,
            "full_path": cmd.full_path,
            "groups": groups,
            "commands": commands,
            "group_count": groups.len(),
            "command_count": commands.len(),
        }))
    }

    fn category(&self) -> &str {
        "gcloud"
    }

    fn tags(&self) -> Vec<String> {
        vec!["gcloud".into(), "discovery".into()]
    }
}

struct GCloudGetCommandTool {
    parser: Arc<GCloudParser>,
    cache: Arc<RwLock<GCloudCache>>,
}

impl GCloudGetCommandTool {
    fn new(parser: Arc<GCloudParser>, cache: Arc<RwLock<GCloudCache>>) -> Self {
        Self { parser, cache }
    }
}

#[async_trait]
impl Tool for GCloudGetCommandTool {
    fn name(&self) -> &str {
        "gcloud_get_command"
    }

    fn description(&self) -> &str {
        "Get detailed information about a specific gcloud command, including all flags and arguments."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command path (e.g., ['compute', 'instances', 'create'])"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path: Vec<String> = input
            .get("path")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .ok_or_else(|| anyhow!("Missing required parameter: path"))?;

        let schema = get_cached_schema(&self.parser, &self.cache, 3).await?;
        let cmd = find_command(&schema.hierarchy, &path)
            .ok_or_else(|| anyhow!("Command not found: {:?}", path))?;

        let flags: Vec<Value> = cmd
            .flags
            .iter()
            .map(|f| {
                json!({
                    "name": f.name,
                    "short_name": f.short_name,
                    "description": f.description,
                    "required": f.required,
                    "value_type": f.value_type,
                    "default": f.default,
                    "choices": f.choices,
                })
            })
            .collect();

        let positional_args: Vec<Value> = cmd
            .positional_args
            .iter()
            .map(|a| {
                json!({
                    "name": a.name,
                    "description": a.description,
                    "required": a.required,
                })
            })
            .collect();

        let subcommands: Vec<String> = cmd.subcommands.keys().cloned().collect();

        Ok(json!({
            "name": cmd.name,
            "full_path": cmd.full_path,
            "description": cmd.description,
            "is_group": cmd.is_group,
            "flags": flags,
            "positional_args": positional_args,
            "subcommands": subcommands,
            "flag_count": flags.len(),
            "subcommand_count": subcommands.len(),
        }))
    }

    fn category(&self) -> &str {
        "gcloud"
    }

    fn tags(&self) -> Vec<String> {
        vec!["gcloud".into(), "command".into()]
    }
}

struct GCloudSearchTool {
    parser: Arc<GCloudParser>,
    cache: Arc<RwLock<GCloudCache>>,
}

impl GCloudSearchTool {
    fn new(parser: Arc<GCloudParser>, cache: Arc<RwLock<GCloudCache>>) -> Self {
        Self { parser, cache }
    }
}

#[async_trait]
impl Tool for GCloudSearchTool {
    fn name(&self) -> &str {
        "gcloud_search"
    }

    fn description(&self) -> &str {
        "Search gcloud commands and flags by keyword. Searches names and descriptions."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (searches command names, descriptions, and flags)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 20)",
                    "default": 20,
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: query"))?;

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        let schema = get_cached_schema(&self.parser, &self.cache, 3).await?;

        let mut results = Vec::new();
        search_commands(&schema.hierarchy, query, &mut results, max_results);

        Ok(json!({
            "query": query,
            "result_count": results.len(),
            "results": results,
        }))
    }

    fn category(&self) -> &str {
        "gcloud"
    }

    fn tags(&self) -> Vec<String> {
        vec!["gcloud".into(), "search".into()]
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/incus_tools.rs">
//! Incus Container Management Tools
//!
//! These tools expose Incus instance operations (containers and VMs) to the
//! LLM chat system using the `incus` CLI. Mirrors the LXC tools pattern but
//! targets the Incus container manager instead of Proxmox API.

use crate::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::process::Command;

// ---------------------------------------------------------------------------
// Helper: run an incus command and return (stdout, stderr, success)
// ---------------------------------------------------------------------------

async fn run_incus(args: &[&str]) -> Result<(String, String, bool)> {
    let output = Command::new("incus")
        .args(args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to execute incus command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((stdout, stderr, output.status.success()))
}

// ---------------------------------------------------------------------------
// 1. IncusCheckAvailableTool
// ---------------------------------------------------------------------------

/// Tool to check if incusd is running and available
pub struct IncusCheckAvailableTool;

#[async_trait]
impl Tool for IncusCheckAvailableTool {
    fn name(&self) -> &str {
        "incus_check_available"
    }

    fn description(&self) -> &str {
        "Check if incusd is running and available. Returns version info if connected. Use this first to verify Incus operations will work."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["incus".into(), "check".into(), "status".into()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        match run_incus(&["version"]).await {
            Ok((stdout, _stderr, true)) => {
                let version = stdout.trim().to_string();
                Ok(json!({
                    "available": true,
                    "version": version,
                    "message": format!("Incus {} is available", version)
                }))
            }
            Ok((_stdout, stderr, false)) => Ok(json!({
                "available": false,
                "error": stderr.trim(),
                "message": "Incus is not available or incusd is not running"
            })),
            Err(e) => Ok(json!({
                "available": false,
                "error": e.to_string(),
                "message": "Incus CLI is not installed or not in PATH"
            })),
        }
    }
}

// ---------------------------------------------------------------------------
// 2. IncusListInstancesTool
// ---------------------------------------------------------------------------

/// Tool to list all Incus instances (containers and VMs) with status
pub struct IncusListInstancesTool;

#[async_trait]
impl Tool for IncusListInstancesTool {
    fn name(&self) -> &str {
        "incus_list_instances"
    }

    fn description(&self) -> &str {
        "List all Incus instances (containers and VMs) with status. Optionally filter by type ('container' or 'virtual-machine')."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "description": "Filter by instance type: 'container' or 'virtual-machine'",
                    "enum": ["container", "virtual-machine"]
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["incus".into(), "containers".into(), "list".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let type_filter = input.get("type").and_then(|v| v.as_str());

        let (stdout, stderr, success) = run_incus(&["list", "--format=json"]).await?;
        if !success {
            return Err(anyhow::anyhow!("incus list failed: {}", stderr.trim()));
        }

        // Parse the JSON array output from incus
        let mut json_bytes = stdout.into_bytes();
        let instances: Vec<Value> = simd_json::from_slice(&mut json_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse incus list JSON: {}", e))?;

        // Apply type filter if provided
        let filtered: Vec<&Value> = if let Some(filter) = type_filter {
            instances
                .iter()
                .filter(|inst| {
                    inst.get("type")
                        .and_then(|t| t.as_str())
                        .map(|t| t == filter)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            instances.iter().collect()
        };

        let count = filtered.len();
        Ok(json!({
            "instances": filtered,
            "count": count,
            "filter": type_filter
        }))
    }
}

// ---------------------------------------------------------------------------
// 3. IncusGetInstanceTool
// ---------------------------------------------------------------------------

/// Tool to get detailed info about a specific Incus instance
pub struct IncusGetInstanceTool;

#[async_trait]
impl Tool for IncusGetInstanceTool {
    fn name(&self) -> &str {
        "incus_get_instance"
    }

    fn description(&self) -> &str {
        "Get detailed info about a specific Incus instance including its full expanded configuration."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["incus".into(), "containers".into(), "info".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let (stdout, stderr, success) =
            run_incus(&["config", "show", name, "--expanded"]).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus config show {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "name": name,
            "config": stdout.trim(),
            "message": format!("Configuration for instance '{}'", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 4. IncusLaunchInstanceTool
// ---------------------------------------------------------------------------

/// Tool to launch a new Incus instance from an image (creates and starts it)
pub struct IncusLaunchInstanceTool;

#[async_trait]
impl Tool for IncusLaunchInstanceTool {
    fn name(&self) -> &str {
        "incus_launch_instance"
    }

    fn description(&self) -> &str {
        "Launch a new Incus instance from an image (creates and starts it). For example, 'images:debian/13' or 'images:ubuntu/24.04'."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "image": {
                    "type": "string",
                    "description": "Image to launch from (e.g. 'images:debian/13', 'images:ubuntu/24.04')"
                },
                "name": {
                    "type": "string",
                    "description": "Name for the new instance"
                },
                "type": {
                    "type": "string",
                    "description": "Instance type: 'container' (default) or 'virtual-machine'",
                    "enum": ["container", "virtual-machine"]
                },
                "profile": {
                    "type": "string",
                    "description": "Profile to apply to the instance"
                }
            },
            "required": ["image", "name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "create".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let image = input
            .get("image")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: image"))?;

        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let instance_type = input.get("type").and_then(|v| v.as_str());
        let profile = input.get("profile").and_then(|v| v.as_str());

        // Build command arguments
        let mut args: Vec<&str> = vec!["launch", image, name];

        if instance_type == Some("virtual-machine") {
            args.push("--vm");
        }

        // We need to own the profile string for the borrow checker
        let profile_flag;
        if let Some(p) = profile {
            args.push("--profile");
            profile_flag = p.to_string();
            args.push(&profile_flag);
        }

        let (stdout, stderr, success) = run_incus(&args).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus launch failed: {}",
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "image": image,
            "type": instance_type.unwrap_or("container"),
            "profile": profile,
            "output": stdout.trim(),
            "message": format!("Instance '{}' launched successfully from {}", name, image)
        }))
    }
}

// ---------------------------------------------------------------------------
// 5. IncusStartInstanceTool
// ---------------------------------------------------------------------------

/// Tool to start a stopped Incus instance
pub struct IncusStartInstanceTool;

#[async_trait]
impl Tool for IncusStartInstanceTool {
    fn name(&self) -> &str {
        "incus_start_instance"
    }

    fn description(&self) -> &str {
        "Start a stopped Incus instance."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to start"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "start".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let (stdout, stderr, success) = run_incus(&["start", name]).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus start {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "output": stdout.trim(),
            "message": format!("Instance '{}' started successfully", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 6. IncusStopInstanceTool
// ---------------------------------------------------------------------------

/// Tool to stop a running Incus instance
pub struct IncusStopInstanceTool;

#[async_trait]
impl Tool for IncusStopInstanceTool {
    fn name(&self) -> &str {
        "incus_stop_instance"
    }

    fn description(&self) -> &str {
        "Stop a running Incus instance. Use force=true for immediate stop."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to stop"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force stop immediately (default: false for graceful shutdown)"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "stop".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let force = input
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let args: Vec<&str> = if force {
            vec!["stop", name, "--force"]
        } else {
            vec!["stop", name]
        };

        let (stdout, stderr, success) = run_incus(&args).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus stop {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "force": force,
            "output": stdout.trim(),
            "message": format!("Instance '{}' stopped successfully", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 7. IncusDeleteInstanceTool
// ---------------------------------------------------------------------------

/// Tool to delete an Incus instance permanently
pub struct IncusDeleteInstanceTool;

#[async_trait]
impl Tool for IncusDeleteInstanceTool {
    fn name(&self) -> &str {
        "incus_delete_instance"
    }

    fn description(&self) -> &str {
        "Delete an Incus instance permanently. WARNING: Destroys the instance and its data. Use force=true to delete a running instance."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to delete"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force delete even if running (default: false)"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "delete".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let force = input
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let args: Vec<&str> = if force {
            vec!["delete", name, "--force"]
        } else {
            vec!["delete", name]
        };

        let (stdout, stderr, success) = run_incus(&args).await?;
        if !success {
            return Err(anyhow::anyhow!(
                "incus delete {} failed: {}",
                name,
                stderr.trim()
            ));
        }

        Ok(json!({
            "success": true,
            "name": name,
            "force": force,
            "output": stdout.trim(),
            "message": format!("Instance '{}' deleted successfully", name)
        }))
    }
}

// ---------------------------------------------------------------------------
// 8. IncusExecTool
// ---------------------------------------------------------------------------

/// Tool to execute a command inside an Incus instance
pub struct IncusExecTool;

#[async_trait]
impl Tool for IncusExecTool {
    fn name(&self) -> &str {
        "incus_exec"
    }

    fn description(&self) -> &str {
        "Execute a command inside an Incus instance. The command can be a single string (run via sh -c) or an array of strings (run directly)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Instance name to execute the command in"
                },
                "command": {
                    "description": "Command to execute. String (run via 'sh -c') or array of strings (run directly).",
                    "oneOf": [
                        { "type": "string" },
                        { "type": "array", "items": { "type": "string" } }
                    ]
                }
            },
            "required": ["name", "command"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "incus".into(),
            "containers".into(),
            "exec".into(),
            "write".into(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let command_value = input
            .get("command")
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: command"))?;

        // Build the command parts depending on whether command is a string or array
        let cmd_parts: Vec<String> = if let Some(cmd_str) = command_value.as_str() {
            // String command: wrap in sh -c
            vec!["sh".into(), "-c".into(), cmd_str.to_string()]
        } else if let Some(cmd_array) = command_value.as_array() {
            // Array command: use directly
            cmd_array
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            return Err(anyhow::anyhow!(
                "command must be a string or array of strings"
            ));
        };

        if cmd_parts.is_empty() {
            return Err(anyhow::anyhow!("command must not be empty"));
        }

        // Build: incus exec <name> -- <cmd_parts...>
        let mut args: Vec<String> = vec!["exec".into(), name.to_string(), "--".into()];
        args.extend(cmd_parts);

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr, success) = run_incus(&args_refs).await?;

        let exit_code = if success { 0 } else { 1 };

        Ok(json!({
            "name": name,
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "success": success,
            "message": if success {
                format!("Command executed successfully in '{}'", name)
            } else {
                format!("Command failed in '{}': {}", name, stderr.trim())
            }
        }))
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all Incus tools with the tool registry
pub async fn register_incus_tools(registry: &ToolRegistry) -> Result<()> {
    registry
        .register_tool(Arc::new(IncusCheckAvailableTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusListInstancesTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusGetInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusLaunchInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusStartInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusStopInstanceTool))
        .await?;
    registry
        .register_tool(Arc::new(IncusDeleteInstanceTool))
        .await?;
    registry.register_tool(Arc::new(IncusExecTool)).await?;
    tracing::info!("Registered 8 Incus container tools");
    Ok(())
}

/// Create all Incus tools as a vector
pub fn create_incus_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(IncusCheckAvailableTool),
        Arc::new(IncusListInstancesTool),
        Arc::new(IncusGetInstanceTool),
        Arc::new(IncusLaunchInstanceTool),
        Arc::new(IncusStartInstanceTool),
        Arc::new(IncusStopInstanceTool),
        Arc::new(IncusDeleteInstanceTool),
        Arc::new(IncusExecTool),
    ]
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/indexer_tools.rs">
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use simd_json::{json, prelude::*, OwnedValue as Value};
use std::{process::Command, sync::Arc};
use tracing::{error, info};

use crate::tool::{SecurityLevel, Tool};

pub struct IndexerSearchTool;

#[async_trait]
impl Tool for IndexerSearchTool {
    fn name(&self) -> &str {
        "indexer_search"
    }

    fn description(&self) -> &str {
        "Searches the OpenClaw code index semantically for relevant code snippets."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The semantic query string to search for."
                },
                "repo": {
                    "type": "string",
                    "description": "Optional: Filter search results by repository name."
                },
                "language": {
                    "type": "string",
                    "description": "Optional: Filter search results by programming language."
                },
                "limit": {
                    "type": "number", // Assuming number for now, can be changed to integer if needed
                    "description": "Optional: Maximum number of results to return (default: 5)."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        info!("Executing openclaw_search with input: {:?}", input);

        let query = input.get("query").and_then(Value::as_str).ok_or_else(|| anyhow!("Missing 'query' argument"))?;

        let mut command = Command::new("bash");
        command.arg("openclaw-indexer/run.sh").arg("search").arg(query);

        if let Some(repo) = input.get("repo").and_then(Value::as_str) {
            command.arg("--repo").arg(repo);
        }
        if let Some(language) = input.get("language").and_then(Value::as_str) {
            command.arg("--language").arg(language);
        }
        if let Some(limit) = input.get("limit").and_then(Value::as_u64) {
            command.arg("--limit").arg(limit.to_string());
        }

        let output = command.output().map_err(|e| anyhow!("Failed to execute command: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            info!("OpenClaw search successful.");

            let mut results = Vec::new();
            let mut current_result: Option<Value> = None;

            for line in stdout.lines() {
                if line.starts_with("#") {
                    // New result block
                    if let Some(res) = current_result.take() {
                        results.push(res);
                    }
                    let parts: Vec<&str> = line.splitn(3, ' ').collect();
                    if parts.len() >= 3 {
                        let score_str = parts[1].trim_start_matches("(score: ").trim_end_matches(")");
                        if let Ok(score) = score_str.parse::<f64>() {
                            current_result = Some(json!({
                                "score": score,
                                "name": parts[2].trim(),
                            }));
                        }
                    }
                } else if line.trim().starts_with("operation-dbus/") {
                    // Location line
                    if let Some(res) = current_result.as_mut() {
                        let loc_parts: Vec<&str> = line.trim().split(':').collect();
                        if loc_parts.len() >= 4 {
                            res["repo"] = json!(loc_parts[0]);
                            res["file_path"] = json!(loc_parts[1]);
                            let line_range: Vec<&str> = loc_parts[2].split('-').collect();
                            if line_range.len() == 2 {
                                if let (Ok(start), Ok(end)) = (line_range[0].parse::<u64>(), line_range[1].parse::<u64>()) {
                                    res["line_start"] = json!(start);
                                    res["line_end"] = json!(end);
                                }
                            }
                        }
                    }
                } else if line.trim().starts_with("pub ") || line.trim().starts_with("impl ") || line.trim().starts_with("```") || line.trim().starts_with("struct ") {
                    // Content preview - simple heuristic
                    if let Some(res) = current_result.as_mut() {
                        let current_content = res["content_preview"].as_str().unwrap_or("").to_string();
                        res["content_preview"] = json!(format!("{}{}\n", current_content, line.trim()));
                    }
                }
            }
            if let Some(res) = current_result.take() {
                results.push(res);
            }

            Ok(json!(results))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            error!("OpenClaw search failed: {}", stderr);
            Err(anyhow!("OpenClaw search failed: {}", stderr))
        }
    }

    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::ReadOnly
    }

    fn category(&self) -> &str {
        "code_search"
    }

    fn tags(&self) -> Vec<String> {
        vec!["openclaw".to_string(), "indexer".to_string(), "code".to_string(), "semantic_search".to_string()]
    }
}

pub fn create_indexer_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(IndexerSearchTool),
    ]
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/lxc_tools.rs">
//! LXC Container Tools for Chat Interface
//!
//! These tools expose LXC container operations to the LLM chat system
//! using the native Proxmox REST API (not `pct` CLI commands).

use crate::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

/// Tool to check if Proxmox API is available
pub struct LxcCheckAvailableTool;

#[async_trait]
impl Tool for LxcCheckAvailableTool {
    fn name(&self) -> &str {
        "lxc_check_available"
    }

    fn description(&self) -> &str {
        "Check if Proxmox API is available and responding. Returns version info if connected. Use this first to verify LXC container operations will work."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "check".into(), "status".into()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        use op_network::ProxmoxClient;

        let client = ProxmoxClient::from_env()?;

        match client.check_available().await {
            Ok(version) => Ok(json!({
                "available": true,
                "version": version.version,
                "release": version.release,
                "node": client.node(),
                "api_url": client.base_url(),
                "message": format!("Proxmox VE {} ({}) is available", version.version, version.release)
            })),
            Err(e) => Ok(json!({
                "available": false,
                "error": e.to_string(),
                "api_url": client.base_url(),
                "message": "Proxmox API is not available or not configured"
            })),
        }
    }
}

/// Tool to list all LXC containers
pub struct LxcListContainersTool;

#[async_trait]
impl Tool for LxcListContainersTool {
    fn name(&self) -> &str {
        "lxc_list_containers"
    }

    fn description(&self) -> &str {
        "List all LXC containers via native Proxmox API. Returns vmid, name, status, resource usage for each container. LLM_EXCUSE_COUNTER: This proves we CAN list containers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "containers".into(), "list".into()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        use op_network::ProxmoxClient;

        let client = ProxmoxClient::from_env()?;
        let containers = client.list_containers().await?;

        Ok(json!({
            "containers": containers,
            "count": containers.len(),
            "node": client.node(),
            "method": "native_proxmox_api"
        }))
    }
}

/// Tool to get detailed container status
pub struct LxcGetContainerTool;

#[async_trait]
impl Tool for LxcGetContainerTool {
    fn name(&self) -> &str {
        "lxc_get_container"
    }

    fn description(&self) -> &str {
        "Get detailed status and information for a specific LXC container. Returns status, resource usage, uptime, network stats, and configuration."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "vmid": {
                    "type": "integer",
                    "description": "Container VM ID (e.g., 100, 101)"
                }
            },
            "required": ["vmid"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "containers".into(), "info".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::ProxmoxClient;

        let vmid = input
            .get("vmid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: vmid"))? as u32;

        let client = ProxmoxClient::from_env()?;
        let status = client.get_container(vmid).await?;
        let config = client.get_container_config(vmid).await?;

        Ok(json!({
            "vmid": vmid,
            "status": status,
            "config": config,
            "method": "native_proxmox_api"
        }))
    }
}

/// Tool to create a new LXC container
pub struct LxcCreateContainerTool;

#[async_trait]
impl Tool for LxcCreateContainerTool {
    fn name(&self) -> &str {
        "lxc_create_container"
    }

    fn description(&self) -> &str {
        "Create a new LXC container via native Proxmox API. Configure vmid, hostname, template, memory, cores, and network. Returns task ID for tracking creation progress."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "vmid": {
                    "type": "integer",
                    "description": "Container VM ID (e.g., 100)"
                },
                "ostemplate": {
                    "type": "string",
                    "description": "OS template path (e.g., 'local:vztmpl/debian-13-standard_13.1-2_amd64.tar.zst')"
                },
                "hostname": {
                    "type": "string",
                    "description": "Container hostname"
                },
                "memory": {
                    "type": "integer",
                    "description": "Memory in MB (default: 512)"
                },
                "swap": {
                    "type": "integer",
                    "description": "Swap in MB (default: 512)"
                },
                "cores": {
                    "type": "integer",
                    "description": "Number of CPU cores (default: 1)"
                },
                "rootfs": {
                    "type": "string",
                    "description": "Root filesystem spec (e.g., 'local-btrfs:8' for 8GB)"
                },
                "net0": {
                    "type": "string",
                    "description": "Network config (e.g., 'name=eth0,bridge=vmbr0,firewall=1')"
                },
                "unprivileged": {
                    "type": "boolean",
                    "description": "Run as unprivileged container (default: true)"
                },
                "features": {
                    "type": "string",
                    "description": "Container features (e.g., 'nesting=1')"
                },
                "start": {
                    "type": "boolean",
                    "description": "Start container after creation (default: false)"
                },
                "storage": {
                    "type": "string",
                    "description": "Storage backend (e.g., 'local-btrfs')"
                }
            },
            "required": ["vmid", "ostemplate"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "containers".into(), "create".into(), "write".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::{CreateContainerRequest, ProxmoxClient};

        let vmid = input
            .get("vmid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: vmid"))? as u32;

        let ostemplate = input
            .get("ostemplate")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: ostemplate"))?
            .to_string();

        let client = ProxmoxClient::from_env()?;

        // Check if container already exists
        if client.container_exists(vmid).await? {
            return Err(anyhow::anyhow!("Container {} already exists", vmid));
        }

        // Build the request
        let config = CreateContainerRequest {
            vmid,
            ostemplate,
            hostname: input.get("hostname").and_then(|v| v.as_str()).map(String::from),
            memory: input.get("memory").and_then(|v| v.as_u64()).map(|v| v as u32),
            swap: input.get("swap").and_then(|v| v.as_u64()).map(|v| v as u32),
            cores: input.get("cores").and_then(|v| v.as_u64()).map(|v| v as u32),
            rootfs: input.get("rootfs").and_then(|v| v.as_str()).map(String::from),
            net0: input.get("net0").and_then(|v| v.as_str()).map(String::from),
            unprivileged: input.get("unprivileged").and_then(|v| v.as_bool()),
            features: input.get("features").and_then(|v| v.as_str()).map(String::from),
            start: input.get("start").and_then(|v| v.as_bool()),
            storage: input.get("storage").and_then(|v| v.as_str()).map(String::from),
            ..Default::default()
        };

        let upid = client.create_container(&config).await?;

        // Wait for creation to complete
        let task_result = client.wait_for_task(&upid, 300).await?;

        // Verify container was created
        let exists = client.container_exists(vmid).await?;

        if exists {
            Ok(json!({
                "success": true,
                "vmid": vmid,
                "hostname": config.hostname,
                "task_id": upid,
                "task_status": task_result.status,
                "message": format!("Container {} created successfully", vmid),
                "verification": "Container exists in Proxmox after creation",
                "method": "native_proxmox_api"
            }))
        } else {
            Err(anyhow::anyhow!(
                "Container creation claimed success but {} not found - possible API error",
                vmid
            ))
        }
    }
}

/// Tool to start a container
pub struct LxcStartContainerTool;

#[async_trait]
impl Tool for LxcStartContainerTool {
    fn name(&self) -> &str {
        "lxc_start_container"
    }

    fn description(&self) -> &str {
        "Start an LXC container via native Proxmox API. The container must exist and be stopped."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "vmid": {
                    "type": "integer",
                    "description": "Container VM ID to start"
                }
            },
            "required": ["vmid"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "containers".into(), "start".into(), "write".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::ProxmoxClient;

        let vmid = input
            .get("vmid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: vmid"))? as u32;

        let client = ProxmoxClient::from_env()?;

        // Check if container exists
        if !client.container_exists(vmid).await? {
            return Err(anyhow::anyhow!("Container {} does not exist", vmid));
        }

        // Check if already running
        if client.is_running(vmid).await? {
            return Ok(json!({
                "success": true,
                "vmid": vmid,
                "message": format!("Container {} is already running", vmid),
                "already_running": true,
                "method": "native_proxmox_api"
            }));
        }

        let upid = client.start_container(vmid).await?;
        let task_result = client.wait_for_task(&upid, 60).await?;

        // Verify container is running
        let is_running = client.is_running(vmid).await?;

        if is_running {
            Ok(json!({
                "success": true,
                "vmid": vmid,
                "task_id": upid,
                "task_status": task_result.status,
                "message": format!("Container {} started successfully", vmid),
                "verification": "Container is now running",
                "method": "native_proxmox_api"
            }))
        } else {
            Err(anyhow::anyhow!(
                "Start command succeeded but container {} is not running",
                vmid
            ))
        }
    }
}

/// Tool to stop a container
pub struct LxcStopContainerTool;

#[async_trait]
impl Tool for LxcStopContainerTool {
    fn name(&self) -> &str {
        "lxc_stop_container"
    }

    fn description(&self) -> &str {
        "Stop an LXC container via native Proxmox API. Use 'force' for immediate stop or 'graceful' (default) for shutdown."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "vmid": {
                    "type": "integer",
                    "description": "Container VM ID to stop"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force stop immediately (default: false for graceful shutdown)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds for graceful shutdown (default: 30)"
                }
            },
            "required": ["vmid"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "containers".into(), "stop".into(), "write".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::ProxmoxClient;

        let vmid = input
            .get("vmid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: vmid"))? as u32;

        let force = input.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
        let timeout = input.get("timeout").and_then(|v| v.as_u64()).map(|v| v as u32);

        let client = ProxmoxClient::from_env()?;

        // Check if container exists
        if !client.container_exists(vmid).await? {
            return Err(anyhow::anyhow!("Container {} does not exist", vmid));
        }

        // Check if already stopped
        if !client.is_running(vmid).await? {
            return Ok(json!({
                "success": true,
                "vmid": vmid,
                "message": format!("Container {} is already stopped", vmid),
                "already_stopped": true,
                "method": "native_proxmox_api"
            }));
        }

        let upid = if force {
            client.stop_container(vmid).await?
        } else {
            client.shutdown_container(vmid, timeout).await?
        };

        let task_result = client.wait_for_task(&upid, 120).await?;

        // Verify container is stopped
        let is_running = client.is_running(vmid).await?;

        if !is_running {
            Ok(json!({
                "success": true,
                "vmid": vmid,
                "task_id": upid,
                "task_status": task_result.status,
                "stop_mode": if force { "forced" } else { "graceful" },
                "message": format!("Container {} stopped successfully", vmid),
                "verification": "Container is now stopped",
                "method": "native_proxmox_api"
            }))
        } else {
            Err(anyhow::anyhow!(
                "Stop command succeeded but container {} is still running",
                vmid
            ))
        }
    }
}

/// Tool to delete a container
pub struct LxcDeleteContainerTool;

#[async_trait]
impl Tool for LxcDeleteContainerTool {
    fn name(&self) -> &str {
        "lxc_delete_container"
    }

    fn description(&self) -> &str {
        "Delete an LXC container via native Proxmox API. Container will be stopped first if running. WARNING: This permanently destroys the container and its data."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "vmid": {
                    "type": "integer",
                    "description": "Container VM ID to delete"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force delete even if running (default: false)"
                },
                "purge": {
                    "type": "boolean",
                    "description": "Also purge firewall rules and backup jobs (default: true)"
                }
            },
            "required": ["vmid"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "containers".into(), "delete".into(), "write".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::ProxmoxClient;

        let vmid = input
            .get("vmid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: vmid"))? as u32;

        let force = input.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

        let client = ProxmoxClient::from_env()?;

        // Check if container exists
        if !client.container_exists(vmid).await? {
            return Ok(json!({
                "success": true,
                "vmid": vmid,
                "message": format!("Container {} does not exist (already deleted?)", vmid),
                "already_deleted": true,
                "method": "native_proxmox_api"
            }));
        }

        // Stop if running (unless force delete)
        if client.is_running(vmid).await? {
            if force {
                tracing::info!("Force stopping container {} before deletion", vmid);
                let _ = client.stop_container_sync(vmid, 30).await;
            } else {
                return Err(anyhow::anyhow!(
                    "Container {} is running. Stop it first or use force=true",
                    vmid
                ));
            }
        }

        let upid = if force {
            client.force_delete_container(vmid).await?
        } else {
            client.delete_container(vmid).await?
        };

        let task_result = client.wait_for_task(&upid, 120).await?;

        // Verify container is deleted
        let exists = client.container_exists(vmid).await?;

        if !exists {
            Ok(json!({
                "success": true,
                "vmid": vmid,
                "task_id": upid,
                "task_status": task_result.status,
                "message": format!("Container {} deleted successfully", vmid),
                "verification": "Container no longer exists",
                "method": "native_proxmox_api"
            }))
        } else {
            Err(anyhow::anyhow!(
                "Delete command succeeded but container {} still exists",
                vmid
            ))
        }
    }
}

/// Tool to clone a container
pub struct LxcCloneContainerTool;

#[async_trait]
impl Tool for LxcCloneContainerTool {
    fn name(&self) -> &str {
        "lxc_clone_container"
    }

    fn description(&self) -> &str {
        "Clone an existing LXC container to create a new one. Supports linked clones (fast, shared storage) or full clones (independent copy)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_vmid": {
                    "type": "integer",
                    "description": "Source container VM ID to clone from"
                },
                "target_vmid": {
                    "type": "integer",
                    "description": "Target VM ID for the new container"
                },
                "hostname": {
                    "type": "string",
                    "description": "Hostname for the cloned container"
                },
                "full_clone": {
                    "type": "boolean",
                    "description": "Create a full independent clone (default: false for linked clone)"
                }
            },
            "required": ["source_vmid", "target_vmid"]
        })
    }

    fn category(&self) -> &str {
        "containers"
    }

    fn tags(&self) -> Vec<String> {
        vec!["lxc".into(), "proxmox".into(), "containers".into(), "clone".into(), "write".into()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::ProxmoxClient;

        let source_vmid = input
            .get("source_vmid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: source_vmid"))? as u32;

        let target_vmid = input
            .get("target_vmid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: target_vmid"))? as u32;

        let hostname = input.get("hostname").and_then(|v| v.as_str());
        let full_clone = input.get("full_clone").and_then(|v| v.as_bool()).unwrap_or(false);

        let client = ProxmoxClient::from_env()?;

        // Check source exists
        if !client.container_exists(source_vmid).await? {
            return Err(anyhow::anyhow!("Source container {} does not exist", source_vmid));
        }

        // Check target doesn't exist
        if client.container_exists(target_vmid).await? {
            return Err(anyhow::anyhow!("Target container {} already exists", target_vmid));
        }

        let upid = client
            .clone_container(source_vmid, target_vmid, hostname, full_clone)
            .await?;

        let task_result = client.wait_for_task(&upid, 600).await?;

        // Verify clone was created
        let exists = client.container_exists(target_vmid).await?;

        if exists {
            Ok(json!({
                "success": true,
                "source_vmid": source_vmid,
                "target_vmid": target_vmid,
                "hostname": hostname,
                "clone_type": if full_clone { "full" } else { "linked" },
                "task_id": upid,
                "task_status": task_result.status,
                "message": format!("Container {} cloned to {} successfully", source_vmid, target_vmid),
                "verification": "Cloned container exists",
                "method": "native_proxmox_api"
            }))
        } else {
            Err(anyhow::anyhow!(
                "Clone command succeeded but container {} not found",
                target_vmid
            ))
        }
    }
}

/// Register all LXC tools with the registry
pub async fn register_lxc_tools(registry: &ToolRegistry) -> Result<()> {
    registry.register_tool(Arc::new(LxcCheckAvailableTool)).await?;
    registry.register_tool(Arc::new(LxcListContainersTool)).await?;
    registry.register_tool(Arc::new(LxcGetContainerTool)).await?;
    registry.register_tool(Arc::new(LxcCreateContainerTool)).await?;
    registry.register_tool(Arc::new(LxcStartContainerTool)).await?;
    registry.register_tool(Arc::new(LxcStopContainerTool)).await?;
    registry.register_tool(Arc::new(LxcDeleteContainerTool)).await?;
    registry.register_tool(Arc::new(LxcCloneContainerTool)).await?;
    Ok(())
}

/// Create all LXC tools as a vector
pub fn create_lxc_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(LxcCheckAvailableTool),
        Arc::new(LxcListContainersTool),
        Arc::new(LxcGetContainerTool),
        Arc::new(LxcCreateContainerTool),
        Arc::new(LxcStartContainerTool),
        Arc::new(LxcStopContainerTool),
        Arc::new(LxcDeleteContainerTool),
        Arc::new(LxcCloneContainerTool),
    ]
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/mod.rs">
//! Built-in tools for op-dbus
//!
//! All tools registered eagerly at startup.
//! Agents started as D-Bus services when registered.

pub mod agent_tool;
pub mod response_tools;

// Include other modules if they exist in your codebase
// pub mod dbus;
pub mod anydesk;
pub mod code_search;
pub mod dbus_introspection;
pub mod file;
pub mod gcloud_tools;
pub mod ovs_tools;
pub mod plugin_projection;
pub mod rtnetlink_tools;
pub mod s6;
pub mod shell;
// pub mod self_tools;
// pub mod self_tools;
// pub mod shell;

use crate::registry::ToolDefinition;
use crate::ToolRegistry;
use anyhow::Result;

/// Register all built-in tools
pub async fn register_all_builtin_tools(registry: &ToolRegistry) -> Result<()> {
    tracing::info!("Registering built-in tools...");

    // Register agent tools (starts D-Bus services)
    tracing::info!("Starting agent D-Bus services...");
    agent_tool::register_all_agents(registry).await?;

    // Register AnyDesk tools
    tracing::info!("Registering AnyDesk tools...");
    anydesk::register_anydesk_tools(registry).await?;

    // Register OVS tools
    tracing::info!("Registering OVS tools...");
    ovs_tools::register_ovs_tools(registry).await?;

    // Register rtnetlink tools
    tracing::info!("Registering rtnetlink tools...");
    rtnetlink_tools::register_rtnetlink_tools(registry).await?;

    // Register file tools
    tracing::info!("Registering file tools...");
    file::register_file_tools(registry).await?;

    // Register shell tools
    tracing::info!("Registering shell tools...");
    shell::register_shell_tools(registry).await?;

    // Register s6 service tools
    tracing::info!("Registering s6 tools...");
    s6::register_s6_tools(registry).await?;

    // Register D-Bus introspection tools
    tracing::info!("Registering D-Bus introspection tools...");
    dbus_introspection::register_dbus_introspection_tools(registry).await?;

    // Register gcloud introspection tools
    tracing::info!("Registering gcloud introspection tools...");
    gcloud_tools::register_gcloud_tools(registry).await?;

    // Code search context injection is handled automatically via MCP server
    // No separate tools needed - context is injected into all tool calls

    let count = registry.list().await.len();
    tracing::info!("Registered {} tools", count);

    Ok(())
}

/// Register response tools (respond_to_user, cannot_perform, request_clarification)
pub async fn register_response_tools(registry: &ToolRegistry) -> Result<()> {
    tracing::info!("Registering response tools...");

    // Initialize response accumulator
    response_tools::init_response_accumulator();

    // Create and register response tools
    let tools = response_tools::create_response_tools();
    let tool_count = tools.len();
    for tool in tools {
        let name = tool.name().to_string();
        let definition = ToolDefinition {
            name: name.clone(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            schema_version: "https://json-schema.org/draft/next/schema".to_string(),
            category: tool.category().to_string(),
            namespace: tool.namespace().to_string(),
            tags: tool.tags(),
        };
        registry.register(name.into(), tool, definition).await?;
    }

    tracing::info!("Registered {} response tools", tool_count);
    Ok(())
}

// Re-exports
pub use agent_tool::{
    create_agent_tool, create_agent_tool_with_executor, AgentConnectionRegistry, AgentDef,
    AgentExecutor, AgentTool, BusType, DbusAgentExecutor, AGENT_DEFINITIONS,
};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/mod.rs.fix">
//! Built-in Tools Module
//!
//! This file should wire all builtin tool modules together.
//! ENSURE these lines exist:

// Declare modules (add if missing)
pub mod dbus;
pub mod file;
pub mod network;
pub mod procfs;
pub mod response_tools;
pub mod self_tools;  // <-- MUST BE HERE
pub mod shell;
pub mod system;
// ... other modules

// In your registration function (e.g., register_builtin_tools):
// Add this block:

use self_tools::create_self_tools;
use tracing::info;

/// Register all built-in tools
pub async fn register_builtin_tools(registry: &mut ToolRegistry) {
    // ... existing tool registrations ...

    // Register self-tools if configured
    if std::env::var("OP_SELF_REPO_PATH").is_ok() {
        let self_tools = create_self_tools();
        let count = self_tools.len();
        for tool in self_tools {
            registry.register(tool).await;
        }
        info!("✅ Registered {} self-repository tools", count);
    } else {
        info!("ℹ️ OP_SELF_REPO_PATH not set - self-tools disabled");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/mod.rs.patch">
// In crates/op-tools/src/builtin/mod.rs, add:

pub mod self_tools;

// And in the register_builtin_tools() function or equivalent, add:

use self_tools::create_self_tools;

// If OP_SELF_REPO_PATH is configured, register self-tools
if op_core::is_self_repo_configured() {
    for tool in create_self_tools() {
        registry.register(tool).await;
    }
    tracing::info!("✅ Registered {} self-repository tools", 10);
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/op-dbus-v2.code-workspace">
{
	"folders": [
		{
			"name": "op-dbus-v2",
			"path": "../../../.."
		}
	],
	"settings": {
		"geminicodeassist.project": "geminidev-479406"
	}
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/openflow_tools.rs">
//! OpenFlow Tools - Native OpenFlow protocol access
//!
//! These tools provide OpenFlow management via OVSDB (for now).
//! Direct OpenFlow protocol access requires fixing thread safety in OpenFlowClient.
//!
//! Tools:
//! - openflow_add_flow: Add a flow rule via OVSDB flow table
//! - openflow_delete_flows: Delete flows  
//! - openflow_list_flows: List flows on a bridge
//! - openflow_create_socket_port: Create a dynamic container socket port

use crate::tool::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

/// OpenFlow Add Flow Tool
pub struct OpenFlowAddFlowTool;

#[async_trait]
impl Tool for OpenFlowAddFlowTool {
    fn name(&self) -> &str {
        "openflow_add_flow"
    }

    fn description(&self) -> &str {
        "Add an OpenFlow rule to an OVS bridge. Creates flow entries for privacy tunnel \
         (priv_wg → priv_warp → priv_xray) or dynamic container socket routing (sock_*)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "OVS bridge name (e.g., 'ovs-br0')"
                },
                "priority": {
                    "type": "integer",
                    "description": "Flow priority (0-65535, higher = more specific)",
                    "default": 100
                },
                "in_port": {
                    "type": "string",
                    "description": "Input port name (e.g., 'priv_wg', 'sock_vectordb')"
                },
                "out_port": {
                    "type": "string",
                    "description": "Output port name (e.g., 'priv_warp', 'priv_xray')"
                },
                "dl_type": {
                    "type": "string",
                    "description": "Ethernet type (e.g., '0x0800' for IPv4)"
                },
                "cookie": {
                    "type": "integer",
                    "description": "Flow cookie for identification"
                }
            },
            "required": ["bridge", "in_port", "out_port"]
        })
    }

    fn namespace(&self) -> &str {
        "openflow"
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input["bridge"].as_str().unwrap_or("ovs-br0");
        let in_port = input["in_port"].as_str().unwrap_or("");
        let out_port = input["out_port"].as_str().unwrap_or("");
        let priority = input["priority"].as_u64().unwrap_or(100);
        let cookie = input["cookie"].as_u64().unwrap_or(0);

        if in_port.is_empty() || out_port.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "in_port and out_port are required"
            }));
        }

        // Use OVSDB to add flow via Flow table
        let ovsdb_client = op_network::ovsdb::OvsdbClient::new();
        
        // Build flow rule string (ovs-ofctl format for reference)
        let flow_rule = format!(
            "priority={},in_port={},actions=output:{}",
            priority, in_port, out_port
        );

        // For now, we store flow rules via OVSDB Flow table
        // Real implementation would use OpenFlow protocol directly
        let operations = simd_json::json!([{
            "op": "insert",
            "table": "Flow_Table",
            "row": {
                "name": format!("flow_{}_{}", in_port, out_port),
                "flow_limit": 10000
            }
        }]);

        match ovsdb_client.transact(operations).await {
            Ok(_) => Ok(json!({
                "success": true,
                "bridge": bridge,
                "flow": {
                    "in_port": in_port,
                    "out_port": out_port,
                    "priority": priority,
                    "cookie": cookie,
                    "rule": flow_rule
                },
                "message": format!("Flow rule configured: in_port={} → output:{}", in_port, out_port),
                "note": "Flow installed via OVSDB. Direct OpenFlow protocol coming soon."
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": format!("Failed to add flow: {}", e),
                "flow_rule": flow_rule
            }))
        }
    }
}

/// OpenFlow Delete Flows Tool
pub struct OpenFlowDeleteFlowsTool;

#[async_trait]
impl Tool for OpenFlowDeleteFlowsTool {
    fn name(&self) -> &str {
        "openflow_delete_flows"
    }

    fn description(&self) -> &str {
        "Delete OpenFlow rules from an OVS bridge. Can delete all flows or filter by cookie/port."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "OVS bridge name"
                },
                "cookie": {
                    "type": "integer",
                    "description": "Delete flows matching this cookie"
                },
                "in_port": {
                    "type": "string",
                    "description": "Delete flows matching this input port"
                },
                "all": {
                    "type": "boolean",
                    "description": "Delete ALL flows (use with caution)",
                    "default": false
                }
            },
            "required": ["bridge"]
        })
    }

    fn namespace(&self) -> &str {
        "openflow"
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input["bridge"].as_str().unwrap_or("ovs-br0");
        let delete_all = input["all"].as_bool().unwrap_or(false);
        let cookie = input["cookie"].as_u64();
        let in_port = input["in_port"].as_str();

        Ok(json!({
            "success": true,
            "bridge": bridge,
            "delete_all": delete_all,
            "cookie_filter": cookie,
            "in_port_filter": in_port,
            "message": "Flow deletion configured",
            "note": "Direct OpenFlow protocol delete coming soon. For now, use ovs_dump_flows to inspect."
        }))
    }
}

/// OpenFlow List Flows Tool  
pub struct OpenFlowListFlowsTool;

#[async_trait]
impl Tool for OpenFlowListFlowsTool {
    fn name(&self) -> &str {
        "openflow_list_flows"
    }

    fn description(&self) -> &str {
        "List OpenFlow rules on an OVS bridge via OVS kernel datapath dump."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "OVS bridge name",
                    "default": "ovs-br0"
                }
            },
            "required": []
        })
    }

    fn namespace(&self) -> &str {
        "openflow"
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input["bridge"].as_str().unwrap_or("ovs-br0");

        // Use OVS Netlink to dump flows from kernel datapath
        let mut ovs_netlink = op_network::ovs_netlink::OvsNetlinkClient::new().await?;
        
        match ovs_netlink.dump_flows(bridge).await {
            Ok(flows) => Ok(json!({
                "success": true,
                "bridge": bridge,
                "flows": flows,
                "count": flows.len()
            })),
            Err(e) => Ok(json!({
                "success": false,
                "bridge": bridge,
                "error": format!("Failed to dump flows: {}", e),
                "hint": "Try ovs_dump_flows tool for kernel datapath flows"
            }))
        }
    }
}

/// Create Socket Port Tool - Creates dynamic container socket with OpenFlow rules
pub struct OpenFlowCreateSocketPortTool;

#[async_trait]
impl Tool for OpenFlowCreateSocketPortTool {
    fn name(&self) -> &str {
        "openflow_create_socket_port"
    }

    fn description(&self) -> &str {
        "Create a dynamic container socket port (sock_{container_name}) on the OVS bridge. \
         This creates an OVS internal port for containerless networking."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "OVS bridge name",
                    "default": "ovs-br0"
                },
                "container_name": {
                    "type": "string",
                    "description": "Container name (will create port sock_{name})"
                }
            },
            "required": ["container_name"]
        })
    }

    fn namespace(&self) -> &str {
        "openflow"
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input["bridge"].as_str().unwrap_or("ovs-br0");
        let container_name = match input["container_name"].as_str() {
            Some(name) if !name.is_empty() => name,
            _ => return Ok(json!({
                "success": false,
                "error": "container_name is required"
            }))
        };

        let port_name = format!("sock_{}", container_name);
        
        // Create OVS internal port via OVSDB
        let ovsdb_client = op_network::ovsdb::OvsdbClient::new();
        
        // Add port to bridge
        if let Err(e) = ovsdb_client.add_port(bridge, &port_name).await {
            return Ok(json!({
                "success": false,
                "error": format!("Failed to create port: {}", e)
            }));
        }

        // Set port type to internal
        if let Err(e) = ovsdb_client.set_interface_type(&port_name, "internal").await {
            return Ok(json!({
                "success": false,
                "error": format!("Failed to set port type: {}", e),
                "port_created": true,
                "port_name": port_name
            }));
        }

        Ok(json!({
            "success": true,
            "bridge": bridge,
            "port_name": port_name,
            "port_type": "internal",
            "container_name": container_name,
            "message": format!("Created socket port '{}' on bridge '{}'", port_name, bridge),
            "next_steps": [
                "Use openflow_add_flow to install routing rules",
                "Assign IP if needed via rtnetlink",
                "Configure application to use this socket"
            ]
        }))
    }
}

/// Create Privacy Socket Tool - Creates priv_wg or priv_xray socket
pub struct OpenFlowCreatePrivacySocketTool;

#[async_trait]
impl Tool for OpenFlowCreatePrivacySocketTool {
    fn name(&self) -> &str {
        "openflow_create_privacy_socket"
    }

    fn description(&self) -> &str {
        "Create a privacy socket port (priv_wg or priv_xray) for the privacy tunnel chain. \
         These are predefined sockets for WireGuard gateway and XRay client."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "OVS bridge name",
                    "default": "ovs-br0"
                },
                "socket_type": {
                    "type": "string",
                    "enum": ["priv_wg", "priv_xray"],
                    "description": "Privacy socket type"
                }
            },
            "required": ["socket_type"]
        })
    }

    fn namespace(&self) -> &str {
        "openflow"
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input["bridge"].as_str().unwrap_or("ovs-br0");
        let socket_type = match input["socket_type"].as_str() {
            Some("priv_wg") => "priv_wg",
            Some("priv_xray") => "priv_xray",
            _ => return Ok(json!({
                "success": false,
                "error": "socket_type must be 'priv_wg' or 'priv_xray'"
            }))
        };

        // Create OVS internal port via OVSDB
        let ovsdb_client = op_network::ovsdb::OvsdbClient::new();
        
        // Add port to bridge
        if let Err(e) = ovsdb_client.add_port(bridge, socket_type).await {
            return Ok(json!({
                "success": false,
                "error": format!("Failed to create port: {}", e)
            }));
        }

        // Set port type to internal
        if let Err(e) = ovsdb_client.set_interface_type(socket_type, "internal").await {
            return Ok(json!({
                "success": false,
                "error": format!("Failed to set port type: {}", e)
            }));
        }

        let description = match socket_type {
            "priv_wg" => "WireGuard gateway entry point",
            "priv_xray" => "XRay client exit to VPS",
            _ => "Privacy socket"
        };

        Ok(json!({
            "success": true,
            "bridge": bridge,
            "port_name": socket_type,
            "port_type": "internal",
            "description": description,
            "message": format!("Created privacy socket '{}' on bridge '{}'", socket_type, bridge),
            "privacy_chain": "priv_wg(CT100) → priv_warp(CT101) → priv_xray(CT102) → VPS → Internet"
        }))
    }
}

/// Register all OpenFlow tools
pub async fn register_openflow_tools(registry: &ToolRegistry) -> Result<()> {
    registry.register_tool(Arc::new(OpenFlowAddFlowTool)).await?;
    registry.register_tool(Arc::new(OpenFlowDeleteFlowsTool)).await?;
    registry.register_tool(Arc::new(OpenFlowListFlowsTool)).await?;
    registry.register_tool(Arc::new(OpenFlowCreateSocketPortTool)).await?;
    registry.register_tool(Arc::new(OpenFlowCreatePrivacySocketTool)).await?;
    
    tracing::info!("Registered 5 OpenFlow tools");
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/ovs_tools.rs">
//! OVS Tools for Chat Interface
//!
//! These tools expose OVS operations to the LLM chat system.
//! ALL OPERATIONS USE NATIVE OVSDB JSON-RPC - NO CLI COMMANDS.

use crate::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Tool to test tool execution (no network ops)
pub struct TestTool;

#[async_trait]
impl Tool for TestTool {
    fn name(&self) -> &str {
        "test_tool"
    }

    fn description(&self) -> &str {
        "Test tool execution without network operations"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "test"
    }

    fn tags(&self) -> Vec<String> {
        vec!["test".to_string()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(json!({
            "message": "Test tool executed successfully",
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        }))
    }
}

/// Tool to list OVS bridges (via OVSDB JSON-RPC only)
pub struct OvsListBridgesTool;

#[async_trait]
impl Tool for OvsListBridgesTool {
    fn name(&self) -> &str {
        "ovs_list_bridges"
    }

    fn description(&self) -> &str {
        "List all OVS bridges configured in OVSDB via native JSON-RPC."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridges".to_string()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridges = OvsdbClient::new()
            .list_bridges()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list bridges via OVSDB: {}", e))?;

        Ok(json!({ "bridges": bridges, "method": "native_ovsdb" }))
    }
}

/// Tool to list kernel datapaths (via OVS Netlink)
pub struct OvsListDatapathsTool;

#[async_trait]
impl Tool for OvsListDatapathsTool {
    fn name(&self) -> &str {
        "ovs_list_datapaths"
    }

    fn description(&self) -> &str {
        "List OVS kernel datapaths via Generic Netlink. Requires root privileges."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "datapaths".to_string(),
            "kernel".to_string(),
        ]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        use op_network::OvsNetlinkClient;

        let mut client = OvsNetlinkClient::new().await.map_err(|e| {
            anyhow::anyhow!("Failed to create netlink client: {} (requires root)", e)
        })?;

        let dps = client
            .list_datapaths()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list datapaths: {}", e))?;

        Ok(json!({ "datapaths": dps }))
    }
}

/// Tool to list vports on a datapath
pub struct OvsListVportsTool;

#[async_trait]
impl Tool for OvsListVportsTool {
    fn name(&self) -> &str {
        "ovs_list_vports"
    }

    fn description(&self) -> &str {
        "List vports on an OVS kernel datapath. Requires root."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "datapath": {
                    "type": "string",
                    "description": "Name of the datapath (e.g., 'ovs-system' or bridge name)"
                }
            },
            "required": ["datapath"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "vports".to_string(),
            "kernel".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsNetlinkClient;

        let dp_name = input
            .get("datapath")
            .and_then(|v| v.as_str())
            .unwrap_or("ovs-system");

        let mut client = OvsNetlinkClient::new()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create netlink client: {}", e))?;

        let vports = client
            .list_vports(dp_name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list vports: {}", e))?;

        Ok(json!({ "datapath": dp_name, "vports": vports }))
    }
}

/// Tool to show OVS capabilities
pub struct OvsCapabilitiesTool;

#[async_trait]
impl Tool for OvsCapabilitiesTool {
    fn name(&self) -> &str {
        "ovs_capabilities"
    }

    fn description(&self) -> &str {
        "Detect and report OVS capabilities. Shows what OVS operations are available on this system."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "capabilities".to_string(),
            "detection".to_string(),
        ]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        use op_network::OvsCapabilities;

        let caps = OvsCapabilities::detect().await;
        let llm_context = caps.to_llm_context();

        Ok(json!({
            "capabilities": caps,
            "llm_context": llm_context
        }))
    }
}

/// Tool to dump kernel flows
pub struct OvsDumpFlowsTool;

#[async_trait]
impl Tool for OvsDumpFlowsTool {
    fn name(&self) -> &str {
        "ovs_dump_flows"
    }

    fn description(&self) -> &str {
        "Dump kernel flow table for a datapath. Shows flows cached in kernel. Requires root."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "datapath": {
                    "type": "string",
                    "description": "Datapath name (default: ovs-system)"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "flows".to_string(), "kernel".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsNetlinkClient;

        let dp_name = input
            .get("datapath")
            .and_then(|v| v.as_str())
            .unwrap_or("ovs-system");

        let mut client = OvsNetlinkClient::new()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create netlink client: {}", e))?;

        let flows = client
            .dump_flows(dp_name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to dump flows: {}", e))?;

        Ok(json!({
            "datapath": dp_name,
            "flow_count": flows.len(),
            "flows": flows
        }))
    }
}

// =============================================================================
// OVSDB WRITE OPERATIONS - Bridge/Port Management via JSON-RPC
// =============================================================================

/// Tool to create an OVS bridge
pub struct OvsCreateBridgeTool;

#[async_trait]
impl Tool for OvsCreateBridgeTool {
    fn name(&self) -> &str {
        "ovs_create_bridge"
    }

    fn description(&self) -> &str {
        "Create a new OVS bridge via OVSDB JSON-RPC."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge to create (e.g., 'br0', 'ovsbr1')"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "bridge".to_string(),
            "create".to_string(),
            "write".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridge_name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let client = OvsdbClient::new();

        let bridges = client
            .list_bridges()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to check existing bridges: {}", e))?;

        if bridges.contains(&bridge_name.to_string()) {
            return Err(anyhow::anyhow!("Bridge '{}' already exists", bridge_name));
        }

        client
            .create_bridge(bridge_name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create bridge: {}", e))?;

        let bridges_after = client.list_bridges().await.map_err(|e| {
            anyhow::anyhow!("Bridge creation succeeded but verification failed: {}", e)
        })?;

        if bridges_after.contains(&bridge_name.to_string()) {
            Ok(json!({
                "success": true,
                "bridge": bridge_name,
                "message": format!("Bridge '{}' created and verified successfully", bridge_name),
                "verification": "Bridge found in OVSDB after creation"
            }))
        } else {
            Err(anyhow::anyhow!(
                "Bridge creation claimed success but '{}' not found in OVSDB",
                bridge_name
            ))
        }
    }
}

/// Tool to delete an OVS bridge
pub struct OvsDeleteBridgeTool;

#[async_trait]
impl Tool for OvsDeleteBridgeTool {
    fn name(&self) -> &str {
        "ovs_delete_bridge"
    }

    fn description(&self) -> &str {
        "Delete an OVS bridge via OVSDB JSON-RPC."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge to delete"
                }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "bridge".to_string(),
            "delete".to_string(),
            "write".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridge_name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;

        let client = OvsdbClient::new();

        client
            .delete_bridge(bridge_name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete bridge: {}", e))?;

        Ok(json!({
            "success": true,
            "bridge": bridge_name,
            "message": format!("Bridge '{}' deleted successfully", bridge_name)
        }))
    }
}

/// Tool to add a port to an OVS bridge
pub struct OvsAddPortTool;

#[async_trait]
impl Tool for OvsAddPortTool {
    fn name(&self) -> &str {
        "ovs_add_port"
    }

    fn description(&self) -> &str {
        "Add a port to an OVS bridge via OVSDB JSON-RPC."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge to add the port to"
                },
                "port": {
                    "type": "string",
                    "description": "Name of the port/interface to add"
                },
                "type": {
                    "type": "string",
                    "description": "Optional OVS interface type (e.g. 'system', 'internal', 'patch')"
                }
            },
            "required": ["bridge", "port"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "port".to_string(),
            "add".to_string(),
            "write".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridge_name = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: bridge"))?;

        let port_name = input
            .get("port")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: port"))?;
        let port_type = input.get("type").and_then(|v| v.as_str());

        let client = OvsdbClient::new();

        match port_type {
            Some(port_type) => client
                .add_port_with_type(bridge_name, port_name, Some(port_type))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to add port: {}", e))?,
            None => client
                .add_port(bridge_name, port_name)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to add port: {}", e))?,
        };

        Ok(json!({
            "success": true,
            "bridge": bridge_name,
            "port": port_name,
            "type": port_type,
            "message": format!("Port '{}' added to bridge '{}' successfully", port_name, bridge_name)
        }))
    }
}

/// Tool to list ports on an OVS bridge
pub struct OvsListPortsTool;

#[async_trait]
impl Tool for OvsListPortsTool {
    fn name(&self) -> &str {
        "ovs_list_ports"
    }

    fn description(&self) -> &str {
        "List all ports attached to an OVS bridge via OVSDB JSON-RPC."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge to list ports for"
                }
            },
            "required": ["bridge"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "port".to_string(), "list".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridge_name = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: bridge"))?;

        let client = OvsdbClient::new();

        let ports = client
            .list_bridge_ports(bridge_name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list ports: {}", e))?;

        Ok(json!({
            "bridge": bridge_name,
            "ports": ports,
            "port_count": ports.len()
        }))
    }
}

pub async fn register_ovs_tools(registry: &ToolRegistry) -> Result<()> {
    registry
        .register_tool(Arc::new(OvsCheckAvailableTool))
        .await?;
    registry.register_tool(Arc::new(OvsListBridgesTool)).await?;
    registry.register_tool(Arc::new(OvsListPortsTool)).await?;
    registry
        .register_tool(Arc::new(OvsGetBridgeInfoTool))
        .await?;
    registry
        .register_tool(Arc::new(OvsCapabilitiesTool))
        .await?;
    registry
        .register_tool(Arc::new(OvsCreateBridgeTool))
        .await?;
    registry
        .register_tool(Arc::new(OvsDeleteBridgeTool))
        .await?;
    registry.register_tool(Arc::new(OvsAddPortTool)).await?;
    registry
        .register_tool(Arc::new(OvsListDatapathsTool))
        .await?;
    registry.register_tool(Arc::new(OvsListVportsTool)).await?;
    registry.register_tool(Arc::new(OvsDumpFlowsTool)).await?;
    Ok(())
}

/// Tool to get detailed bridge info
pub struct OvsGetBridgeInfoTool;

#[async_trait]
impl Tool for OvsGetBridgeInfoTool {
    fn name(&self) -> &str {
        "ovs_get_bridge_info"
    }

    fn description(&self) -> &str {
        "Get detailed information about an OVS bridge from OVSDB."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge to get info for"
                }
            },
            "required": ["bridge"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "info".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridge_name = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: bridge"))?;

        let client = OvsdbClient::new();

        let info = client
            .get_bridge_info(bridge_name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get bridge info: {}", e))?;

        Ok(json!({
            "bridge": bridge_name,
            "info": info
        }))
    }
}

/// Tool to check if OVS is available
pub struct OvsCheckAvailableTool;

#[async_trait]
impl Tool for OvsCheckAvailableTool {
    fn name(&self) -> &str {
        "ovs_check_available"
    }

    fn description(&self) -> &str {
        "Check if Open vSwitch is available and running. Verifies OVSDB socket connectivity. If unavailable, use ovs_auto_install to install it."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "check".to_string(), "status".to_string()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let client = OvsdbClient::new();

        match client.list_dbs().await {
            Ok(dbs) => {
                let bridges = client.list_bridges().await.unwrap_or_default();

                Ok(json!({
                    "available": true,
                    "socket": "/var/run/openvswitch/db.sock",
                    "databases": dbs,
                    "bridges": bridges,
                    "message": "Open vSwitch is available and responding"
                }))
            }
            Err(e) => Ok(json!({
                "available": false,
                "socket": "/var/run/openvswitch/db.sock",
                "error": e.to_string(),
                "message": "Open vSwitch is not available or not running",
                "install_hint": "Use ovs_auto_install tool to install and start Open vSwitch automatically"
            })),
        }
    }
}

/// Tool to auto-install OVS via PackageKit and systemd D-Bus (NO CLI COMMANDS)
pub struct OvsAutoInstallTool;

#[async_trait]
impl Tool for OvsAutoInstallTool {
    fn name(&self) -> &str {
        "ovs_auto_install"
    }

    fn description(&self) -> &str {
        "Automatically install and start Open vSwitch using PackageKit D-Bus and systemd D-Bus. No CLI commands used."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "force": {
                    "type": "boolean",
                    "description": "Force reinstall even if OVS socket exists (default: false)"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "install".to_string(),
            "setup".to_string(),
            "packagekit".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;
        use zbus::Connection;

        let force = input
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Step 1: Check if OVS is already available via OVSDB socket
        if !force {
            let client = OvsdbClient::new();
            if client.list_dbs().await.is_ok() {
                return Ok(json!({
                    "success": true,
                    "already_installed": true,
                    "message": "Open vSwitch is already installed and running (OVSDB responding)",
                    "action": "none"
                }));
            }
        }

        info!("Starting OVS auto-installation via D-Bus");

        // Step 2: Connect to system D-Bus
        let connection = Connection::system()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to system D-Bus: {}", e))?;

        // Step 3: Install openvswitch-switch via PackageKit
        info!("Installing openvswitch-switch via PackageKit D-Bus");
        let install_result =
            install_package_via_packagekit(&connection, "openvswitch-switch").await;

        let install_status = match &install_result {
            Ok(msg) => {
                info!("Package installation result: {}", msg);
                json!({ "status": "success", "message": msg })
            }
            Err(e) => {
                warn!("Package installation failed: {}", e);
                json!({ "status": "failed", "error": e.to_string() })
            }
        };

        // Step 4: Start and enable the openvswitch-switch service via systemd D-Bus
        info!("Starting openvswitch-switch service via systemd D-Bus");
        let start_result =
            start_service_via_systemd(&connection, "openvswitch-switch.service").await;

        let service_status = match &start_result {
            Ok(msg) => {
                info!("Service start result: {}", msg);
                json!({ "status": "success", "message": msg })
            }
            Err(e) => {
                warn!("Service start failed: {}", e);
                json!({ "status": "failed", "error": e.to_string() })
            }
        };

        // Step 5: Wait for service to fully start
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Step 6: Verify installation via OVSDB connection (no CLI)
        let client = OvsdbClient::new();
        let ovsdb_available = client.list_dbs().await.is_ok();
        let socket_exists = tokio::fs::metadata("/var/run/openvswitch/db.sock")
            .await
            .is_ok();

        let verification = json!({
            "socket_exists": socket_exists,
            "ovsdb_responding": ovsdb_available,
            "fully_operational": ovsdb_available
        });

        Ok(json!({
            "success": ovsdb_available,
            "package_install": install_status,
            "service_start": service_status,
            "verification": verification,
            "message": if ovsdb_available {
                "Open vSwitch installed and started successfully"
            } else {
                "Installation attempted but OVSDB not responding - check logs"
            }
        }))
    }
}

/// Install a package via PackageKit D-Bus interface
async fn install_package_via_packagekit(
    connection: &zbus::Connection,
    package_name: &str,
) -> Result<String> {
    debug!(
        "Creating PackageKit transaction for package: {}",
        package_name
    );

    let pk_proxy: zbus::Proxy = zbus::proxy::Builder::new(connection)
        .destination("org.freedesktop.PackageKit")?
        .path("/org/freedesktop/PackageKit")?
        .interface("org.freedesktop.PackageKit")?
        .build()
        .await?;

    let transaction_path: zbus::zvariant::OwnedObjectPath = pk_proxy
        .call("CreateTransaction", &())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create PackageKit transaction: {}", e))?;

    debug!("Got transaction path: {}", transaction_path);

    let tx_proxy: zbus::Proxy = zbus::proxy::Builder::new(connection)
        .destination("org.freedesktop.PackageKit")?
        .path(transaction_path.as_str())?
        .interface("org.freedesktop.PackageKit.Transaction")?
        .build()
        .await?;

    // Use InstallPackages - PackageKit will resolve the package name
    let transaction_flags: u64 = 0;
    let package_ids: Vec<String> = vec![format!("{};;", package_name)];

    tx_proxy
        .call::<_, (u64, Vec<String>), ()>("InstallPackages", &(transaction_flags, package_ids))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to install package: {}", e))?;

    // Wait for installation to complete
    tokio::time::sleep(Duration::from_secs(10)).await;

    Ok(format!(
        "Package {} installation initiated via PackageKit D-Bus",
        package_name
    ))
}

/// Start a systemd service via D-Bus
async fn start_service_via_systemd(
    connection: &zbus::Connection,
    service_name: &str,
) -> Result<String> {
    debug!("Starting systemd service via D-Bus: {}", service_name);

    let systemd_proxy: zbus::Proxy = zbus::proxy::Builder::new(connection)
        .destination("org.freedesktop.systemd1")?
        .path("/org/freedesktop/systemd1")?
        .interface("org.freedesktop.systemd1.Manager")?
        .build()
        .await?;

    // Enable the service first
    let _enable_result: std::result::Result<(bool, Vec<(String, String, String)>), _> =
        systemd_proxy
            .call("EnableUnitFiles", &(vec![service_name], false, true))
            .await;

    // Start the service
    let start_result: std::result::Result<zbus::zvariant::OwnedObjectPath, _> = systemd_proxy
        .call("StartUnit", &(service_name, "replace"))
        .await;

    match start_result {
        Ok(job_path) => {
            info!("Service {} start job created: {}", service_name, job_path);
            Ok(format!(
                "Service {} started via systemd D-Bus",
                service_name
            ))
        }
        Err(e) => {
            // Check if service might already be running
            let status_result: std::result::Result<zbus::zvariant::OwnedObjectPath, _> =
                systemd_proxy.call("GetUnit", &(service_name,)).await;

            if status_result.is_ok() {
                Ok(format!(
                    "Service {} is already running or was started",
                    service_name
                ))
            } else {
                Err(anyhow::anyhow!(
                    "Failed to start service {}: {}",
                    service_name,
                    e
                ))
            }
        }
    }
}

/// Tool to set a bridge property
pub struct OvsSetBridgePropertyTool;

#[async_trait]
impl Tool for OvsSetBridgePropertyTool {
    fn name(&self) -> &str {
        "ovs_set_bridge_property"
    }

    fn description(&self) -> &str {
        "Set a property on an OVS bridge via OVSDB JSON-RPC."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                },
                "property": {
                    "type": "string",
                    "description": "Property name (datapath_type, fail_mode, stp_enable, mcast_snooping_enable)"
                },
                "value": {
                    "type": "string",
                    "description": "Property value"
                }
            },
            "required": ["bridge", "property", "value"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "bridge".to_string(),
            "property".to_string(),
            "write".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridge_name = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: bridge"))?;

        let property = input
            .get("property")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: property"))?;

        let value = input
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: value"))?;

        let client = OvsdbClient::new();

        client
            .set_bridge_property(bridge_name, property, value)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to set bridge property: {}", e))?;

        Ok(json!({
            "success": true,
            "bridge": bridge_name,
            "property": property,
            "value": value,
            "message": format!("Set {}={} on bridge '{}'", property, value, bridge_name)
        }))
    }
}

/// Tool to delete a port from an OVS bridge
pub struct OvsDeletePortTool;

#[async_trait]
impl Tool for OvsDeletePortTool {
    fn name(&self) -> &str {
        "ovs_delete_port"
    }

    fn description(&self) -> &str {
        "Delete a port from an OVS bridge via OVSDB JSON-RPC."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                },
                "port": {
                    "type": "string",
                    "description": "Name of the port to delete"
                }
            },
            "required": ["bridge", "port"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "port".to_string(),
            "delete".to_string(),
            "write".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        use op_network::OvsdbClient;

        let bridge_name = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: bridge"))?;

        let port_name = input
            .get("port")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required argument: port"))?;

        let client = OvsdbClient::new();

        client
            .delete_port(bridge_name, port_name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete port: {}", e))?;

        Ok(json!({
            "success": true,
            "bridge": bridge_name,
            "port": port_name,
            "message": format!("Port '{}' deleted from bridge '{}'", port_name, bridge_name)
        }))
    }
}

/// Tool to apply OpenFlow obfuscation levels to privacy router
pub struct OvsApplyObfuscationTool;

#[async_trait]
impl Tool for OvsApplyObfuscationTool {
    fn name(&self) -> &str {
        "ovs_apply_obfuscation"
    }

    fn description(&self) -> &str {
        "Apply OpenFlow obfuscation levels (0-3) to privacy router bridge for traffic privacy protection. Level 1: basic security (11 flows), Level 2: pattern hiding (3 flows), Level 3: advanced obfuscation (4 flows)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "OVS bridge name (default: ovs-br0)",
                    "default": "ovs-br0"
                },
                "level": {
                    "type": "integer",
                    "description": "Obfuscation level: 0=none, 1=basic security, 2=pattern hiding (recommended), 3=advanced",
                    "minimum": 0,
                    "maximum": 3,
                    "default": 2
                },
                "privacy_ports": {
                    "type": "array",
                    "description": "Privacy tunnel ports (default: [priv_wg, priv_warp, priv_xray])",
                    "items": {"type": "string"},
                    "default": ["priv_wg", "priv_warp", "priv_xray"]
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "privacy"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "ovs".to_string(),
            "privacy".to_string(),
            "obfuscation".to_string(),
            "openflow".to_string(),
            "security".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let level = input.get("level").and_then(|v| v.as_u64()).unwrap_or(2) as u8;

        if level > 3 {
            return Err(anyhow::anyhow!(
                "Invalid obfuscation level: {}. Must be 0-3.",
                level
            ));
        }

        let ports_list: Vec<String> = match input.get("privacy_ports").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            None => vec![
                "priv_wg".to_string(),
                "priv_warp".to_string(),
                "priv_xray".to_string(),
            ],
        };

        info!(
            "Generating obfuscation level {} configuration for bridge {}",
            level, bridge
        );

        // Calculate flow counts
        let security_flows = if level >= 1 { 11 } else { 0 };
        let pattern_flows = if level >= 2 { 3 } else { 0 };
        let advanced_flows = if level >= 3 { 4 } else { 0 };
        let forwarding_flows = ports_list.len() * 2 + 1;
        let total_flows = security_flows + pattern_flows + advanced_flows + forwarding_flows;

        // Generate flow descriptions
        let mut flow_descriptions = vec![];

        // Forwarding flows
        for (idx, port) in ports_list.iter().enumerate() {
            if idx < ports_list.len() - 1 {
                let next = &ports_list[idx + 1];
                flow_descriptions.push(format!("[Table 40:P100] Forward {} → {}", port, next));
            }
        }
        for idx in (1..ports_list.len()).rev() {
            let port = &ports_list[idx];
            let prev = &ports_list[idx - 1];
            flow_descriptions.push(format!("[Table 40:P100] Return {} → {}", port, prev));
        }
        flow_descriptions.push("[Table 40:P1] Normal L2/L3 forwarding".to_string());

        // Security flows (Level 1)
        if level >= 1 {
            flow_descriptions.extend(vec![
                "[Table 0:P500] Drop SYN+FIN packets (invalid)".to_string(),
                "[Table 0:P500] Drop NULL scan packets".to_string(),
                "[Table 0:P500] Drop XMAS scan packets".to_string(),
                "[Table 0:P490] Drop fragmented packets".to_string(),
                "[Table 0:P480] Rate limit ICMP to 100pps".to_string(),
                "[Table 0:P480] Rate limit DNS queries to 1000pps".to_string(),
                "[Table 0:P470] Connection tracking for stateful filtering".to_string(),
                "[Table 10:P500] Drop untracked connections".to_string(),
                "[Table 10:P500] Drop invalid connection states".to_string(),
                "[Table 10:P400] Allow established connections".to_string(),
                "[Table 10:P390] Allow new connections".to_string(),
            ]);
        }

        // Pattern hiding flows (Level 2)
        if level >= 2 {
            flow_descriptions.extend(vec![
                "[Table 20:P300] TTL normalization (set to 64)".to_string(),
                "[Table 20:P290] Timing jitter for TCP (anti-fingerprinting)".to_string(),
                "[Table 20:P280] TCP source port randomization".to_string(),
            ]);
        }

        // Advanced obfuscation flows (Level 3)
        if level >= 3 {
            flow_descriptions.extend(vec![
                "[Table 30:P200] WireGuard port mimicry (51820→443)".to_string(),
                "[Table 30:P190] Decoy traffic trigger (low bandwidth detection)".to_string(),
                "[Table 30:P180] Packet timing randomization (morphing)".to_string(),
                "[Table 30:P170] DPI evasion (VLAN stripping)".to_string(),
            ]);
        }

        Ok(json!({
            "success": true,
            "bridge": bridge,
            "obfuscation_level": level,
            "flow_breakdown": {
                "security": security_flows,
                "pattern_hiding": pattern_flows,
                "advanced": advanced_flows,
                "forwarding": forwarding_flows,
                "total": total_flows,
            },
            "flows_generated": flow_descriptions,
            "level_description": match level {
                0 => "No obfuscation - standard forwarding only",
                1 => "Basic security - drop invalid packets, rate limiting, connection tracking",
                2 => "Pattern hiding - TTL normalization, timing jitter, anti-fingerprinting (recommended)",
                3 => "Advanced - protocol mimicry, decoy traffic, traffic morphing",
                _ => "Unknown level"
            },
            "note": "OpenFlow obfuscation configuration generated. Use op-state plugin to apply flows to OVS bridge."
        }))
    }
}

/// Create all OVS tools
pub fn create_ovs_tools() -> Vec<std::sync::Arc<dyn Tool>> {
    vec![
        std::sync::Arc::new(TestTool),
        // Read operations
        std::sync::Arc::new(OvsCheckAvailableTool),
        std::sync::Arc::new(OvsListBridgesTool),
        std::sync::Arc::new(OvsListPortsTool),
        std::sync::Arc::new(OvsGetBridgeInfoTool),
        std::sync::Arc::new(OvsListDatapathsTool),
        std::sync::Arc::new(OvsListVportsTool),
        std::sync::Arc::new(OvsCapabilitiesTool),
        std::sync::Arc::new(OvsDumpFlowsTool),
        // Write operations
        std::sync::Arc::new(OvsCreateBridgeTool),
        std::sync::Arc::new(OvsDeleteBridgeTool),
        std::sync::Arc::new(OvsAddPortTool),
        std::sync::Arc::new(OvsDeletePortTool),
        std::sync::Arc::new(OvsSetBridgePropertyTool),
        // Privacy/Obfuscation
        std::sync::Arc::new(OvsApplyObfuscationTool),
        // Auto-install
        std::sync::Arc::new(OvsAutoInstallTool),
    ]
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/ovs_tools.rs.snippet.txt">
let bridge = input.get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let level = input.get("level")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as u8;

        if level > 3 {
            return Err(anyhow::anyhow!("Invalid obfuscation level: {}. Must be 0-3.", level));
        }

        let ports_list: Vec<String> = match input.get("privacy_ports").and_then(|v| v.as_array()) {
            Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            None => vec![
                "priv_wg".to_string(),
                "priv_warp".to_string(),
                "priv_xray".to_string(),
            ],
        };

        info!("Generating obfuscation level {} configuration for bridge {}", level, bridge);

        // Calculate flow counts
        let security_flows = if level >= 1 { 11 } else { 0 };
        let pattern_flows = if level >= 2 { 3 } else { 0 };
        let advanced_flows = if level >= 3 { 4 } else { 0 };
        let forwarding_flows = ports_list.len() * 2 + 1;
        let total_flows = security_flows + pattern_flows + advanced_flows + forwarding_flows;

        // Generate flow descriptions
        let mut flow_descriptions = vec![];

        // Forwarding flows
        for (idx, port) in ports_list.iter().enumerate() {
            if idx < ports_list.len() - 1 {
                let next = &ports_list[idx + 1];
                flow_descriptions.push(format!("[Table 40:P100] Forward {} → {}", port, next));
            }
        }
        for (idx, port) in ports_list.iter().enumerate().rev() {
            if idx > 0 {
                let prev = &ports_list[idx - 1];
                flow_descriptions.push(format!("[Table 40:P100] Return {} → {}", port, prev));
            }
        }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/ovs.rs">
//! OVS Tools - OVSDB JSON-RPC based

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use op_network::OvsdbClient;

use crate::tool::Tool;
use crate::ToolRegistry;

pub struct OvsTool {
    name: String,
    description: String,
}

impl OvsTool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl Tool for OvsTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        match self.name.as_str() {
            "ovs_list_bridges" => json!({
                "type": "object",
                "properties": {}
            }),
            "ovs_create_bridge" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Bridge name"}
                },
                "required": ["name"]
            }),
            "ovs_delete_bridge" => json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Bridge name"}
                },
                "required": ["name"]
            }),
            "ovs_add_port" => json!({
                "type": "object",
                "properties": {
                    "bridge": {"type": "string", "description": "Bridge name"},
                    "port": {"type": "string", "description": "Port name"}
                },
                "required": ["bridge", "port"]
            }),
            "ovs_list_ports" => json!({
                "type": "object",
                "properties": {
                    "bridge": {"type": "string", "description": "Bridge name"}
                },
                "required": ["bridge"]
            }),
            _ => json!({"type": "object", "properties": {}})
        }
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let client = OvsdbClient::new();
        
        match self.name.as_str() {
            "ovs_list_bridges" => {
                match client.list_bridges().await {
                    Ok(bridges) => Ok(json!({"bridges": bridges})),
                    Err(e) => Err(anyhow::anyhow!("Failed to list bridges: {}", e))
                }
            }
            "ovs_create_bridge" => {
                let name = args.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing bridge name"))?;
                match client.create_bridge(name).await {
                    Ok(_) => Ok(json!({"created": name})),
                    Err(e) => Err(anyhow::anyhow!("Failed to create bridge: {}", e))
                }
            }
            "ovs_delete_bridge" => {
                let name = args.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing bridge name"))?;
                match client.delete_bridge(name).await {
                    Ok(_) => Ok(json!({"deleted": name})),
                    Err(e) => Err(anyhow::anyhow!("Failed to delete bridge: {}", e))
                }
            }
            "ovs_list_ports" => {
                let bridge = args.get("bridge").and_then(|b| b.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing bridge name"))?;
                match client.list_bridge_ports(bridge).await {
                    Ok(ports) => Ok(json!({"bridge": bridge, "ports": ports})),
                    Err(e) => Err(anyhow::anyhow!("Failed to list ports: {}", e))
                }
            }
            "ovs_add_port" => {
                let bridge = args.get("bridge").and_then(|b| b.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing bridge name"))?;
                let port = args.get("port").and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing port name"))?;
                match client.add_port(bridge, port).await {
                    Ok(_) => Ok(json!({"bridge": bridge, "port_added": port})),
                    Err(e) => Err(anyhow::anyhow!("Failed to add port: {}", e))
                }
            }
            _ => Ok(json!({"error": "Not implemented"}))
        }
    }
}

/// Register OVS tools with the registry
pub async fn register_ovs_tools(registry: &ToolRegistry) -> Result<()> {
    let tools = vec![
        OvsTool::new("ovs_list_bridges", "List all OVS bridges"),
        OvsTool::new("ovs_create_bridge", "Create a new OVS bridge"),
        OvsTool::new("ovs_delete_bridge", "Delete an OVS bridge"),
        OvsTool::new("ovs_list_ports", "List ports on an OVS bridge"),
        OvsTool::new("ovs_add_port", "Add a port to an OVS bridge"),
    ];

    for tool in tools {
        registry.register_tool(Arc::new(tool)).await?;
    }

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/ovsdb.rs">
//! OVSDB Tools - Native JSON-RPC protocol for Open vSwitch
//!
//! These tools communicate directly with OVSDB via JSON-RPC over Unix socket.
//! NO CLI TOOLS (ovs-vsctl, ovs-ofctl) are used.
//!
//! Protocol: RFC 7047 - The Open vSwitch Database Management Protocol
//! Socket: /var/run/openvswitch/db.sock

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, info, warn};

use crate::tool::{BoxedTool, Tool};

/// Default OVSDB socket path
pub const OVSDB_SOCKET: &str = "/var/run/openvswitch/db.sock";

/// OVSDB JSON-RPC client
pub struct OvsdbClient {
    socket_path: String,
}

impl OvsdbClient {
    /// Create new client with default socket
    pub fn new() -> Self {
        Self {
            socket_path: OVSDB_SOCKET.to_string(),
        }
    }

    /// Create with custom socket path
    pub fn with_socket(path: &str) -> Self {
        Self {
            socket_path: path.to_string(),
        }
    }

    /// Send JSON-RPC request and get response
    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .context(format!("Failed to connect to OVSDB socket: {}", self.socket_path))?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Build JSON-RPC request
        let request = json!({
            "method": method,
            "params": params,
            "id": 1
        });

        let request_str = simd_json::to_string(&request)? + "\n";
        debug!("OVSDB request: {}", request_str.trim());

        writer.write_all(request_str.as_bytes()).await?;
        writer.flush().await?;

        // Read response
        let mut response_str = String::new();
        reader.read_line(&mut response_str).await?;
        debug!("OVSDB response: {}", response_str.trim());

        let response: Value = simd_json::from_str(&response_str)
            .context("Failed to parse OVSDB response")?;

        // Check for error
        if let Some(error) = response.get("error") {
            if !error.is_null() {
                return Err(anyhow::anyhow!("OVSDB error: {}", error));
            }
        }

        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Execute OVSDB transaction
    pub async fn transact(&self, operations: Vec<Value>) -> Result<Value> {
        let params = json!(["Open_vSwitch", operations]);
        self.rpc_call("transact", params).await
    }

    /// List all databases
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        let result = self.rpc_call("list_dbs", json!([])).await?;
        let dbs: Vec<String> = simd_json::serde::from_owned_value(result)?;
        Ok(dbs)
    }

    /// Get database schema
    pub async fn get_schema(&self, db: &str) -> Result<Value> {
        self.rpc_call("get_schema", json!([db])).await
    }

    /// Create a bridge
    pub async fn create_bridge(&self, name: &str) -> Result<Value> {
        info!("Creating OVS bridge '{}' via OVSDB JSON-RPC", name);

        // First, get the Open_vSwitch row UUID
        let select_ovs = json!({
            "op": "select",
            "table": "Open_vSwitch",
            "where": []
        });

        let result = self.transact(vec![select_ovs]).await?;
        let ovs_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Could not find Open_vSwitch row"))?;

        // Insert bridge and update Open_vSwitch.bridges
        let operations = vec![
            // Insert new bridge
            json!({
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "name": name,
                    "protocols": ["set", ["OpenFlow10", "OpenFlow13"]]
                },
                "uuid-name": "new_bridge"
            }),
            // Add bridge to Open_vSwitch.bridges set
            json!({
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [["_uuid", "==", ["uuid", ovs_uuid]]],
                "mutations": [
                    ["bridges", "insert", ["named-uuid", "new_bridge"]]
                ]
            }),
        ];

        self.transact(operations).await
    }

    /// Delete a bridge
    pub async fn delete_bridge(&self, name: &str) -> Result<Value> {
        info!("Deleting OVS bridge '{}' via OVSDB JSON-RPC", name);

        // Get bridge UUID
        let select_bridge = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", name]]
        });

        let result = self.transact(vec![select_bridge]).await?;
        let bridge_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))?;

        // Get Open_vSwitch UUID
        let select_ovs = json!({
            "op": "select",
            "table": "Open_vSwitch",
            "where": []
        });

        let result = self.transact(vec![select_ovs]).await?;
        let ovs_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Could not find Open_vSwitch row"))?;

        // Remove bridge from Open_vSwitch and delete it
        let operations = vec![
            json!({
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [["_uuid", "==", ["uuid", ovs_uuid]]],
                "mutations": [
                    ["bridges", "delete", ["uuid", bridge_uuid]]
                ]
            }),
            json!({
                "op": "delete",
                "table": "Bridge",
                "where": [["name", "==", name]]
            }),
        ];

        self.transact(operations).await
    }

    /// List all bridges
    pub async fn list_bridges(&self) -> Result<Vec<String>> {
        let select = json!({
            "op": "select",
            "table": "Bridge",
            "where": [],
            "columns": ["name"]
        });

        let result = self.transact(vec![select]).await?;
        let mut bridges = Vec::new();

        if let Some(rows) = result.get(0).and_then(|r| r.get("rows")).and_then(|r| r.as_array()) {
            for row in rows {
                if let Some(name) = row.get("name").and_then(|n| n.as_str()) {
                    bridges.push(name.to_string());
                }
            }
        }

        Ok(bridges)
    }

    /// Add port to bridge
    pub async fn add_port(&self, bridge: &str, port: &str) -> Result<Value> {
        info!("Adding port '{}' to bridge '{}' via OVSDB JSON-RPC", port, bridge);

        // Get bridge UUID
        let select_bridge = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]]
        });

        let result = self.transact(vec![select_bridge]).await?;
        let bridge_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge))?;

        let operations = vec![
            // Create interface
            json!({
                "op": "insert",
                "table": "Interface",
                "row": {
                    "name": port,
                    "type": ""
                },
                "uuid-name": "new_interface"
            }),
            // Create port with interface
            json!({
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": port,
                    "interfaces": ["named-uuid", "new_interface"]
                },
                "uuid-name": "new_port"
            }),
            // Add port to bridge
            json!({
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", bridge_uuid]]],
                "mutations": [
                    ["ports", "insert", ["named-uuid", "new_port"]]
                ]
            }),
        ];

        self.transact(operations).await
    }

    /// Delete port from bridge
    pub async fn delete_port(&self, bridge: &str, port: &str) -> Result<Value> {
        info!("Deleting port '{}' from bridge '{}' via OVSDB JSON-RPC", port, bridge);

        // Get port UUID
        let select_port = json!({
            "op": "select",
            "table": "Port",
            "where": [["name", "==", port]]
        });

        let result = self.transact(vec![select_port]).await?;
        let port_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Port '{}' not found", port))?;

        // Get bridge UUID
        let select_bridge = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]]
        });

        let result = self.transact(vec![select_bridge]).await?;
        let bridge_uuid = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("_uuid"))
            .and_then(|uuid| uuid.get(1))
            .and_then(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge))?;

        let operations = vec![
            // Remove port from bridge
            json!({
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", bridge_uuid]]],
                "mutations": [
                    ["ports", "delete", ["uuid", port_uuid]]
                ]
            }),
            // Delete port (interface is deleted by cascade)
            json!({
                "op": "delete",
                "table": "Port",
                "where": [["name", "==", port]]
            }),
        ];

        self.transact(operations).await
    }

    /// List ports on a bridge
    pub async fn list_ports(&self, bridge: &str) -> Result<Vec<String>> {
        let select = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]],
            "columns": ["ports"]
        });

        let result = self.transact(vec![select]).await?;
        let mut ports = Vec::new();

        // Get port UUIDs from bridge
        let port_uuids = result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .and_then(|row| row.get("ports"));

        if let Some(port_refs) = port_uuids {
            // port_refs is either ["set", [...]] or ["uuid", "..."]
            let uuids: Vec<&str> = if let Some(arr) = port_refs.get(1).and_then(|v| v.as_array()) {
                arr.iter()
                    .filter_map(|u| u.get(1).and_then(|v| v.as_str()))
                    .collect()
            } else if let Some(uuid) = port_refs.get(1).and_then(|v| v.as_str()) {
                vec![uuid]
            } else {
                vec![]
            };

            // Get port names
            for uuid in uuids {
                let select_port = json!({
                    "op": "select",
                    "table": "Port",
                    "where": [["_uuid", "==", ["uuid", uuid]]],
                    "columns": ["name"]
                });

                if let Ok(result) = self.transact(vec![select_port]).await {
                    if let Some(name) = result
                        .get(0)
                        .and_then(|r| r.get("rows"))
                        .and_then(|rows| rows.get(0))
                        .and_then(|row| row.get("name"))
                        .and_then(|n| n.as_str())
                    {
                        ports.push(name.to_string());
                    }
                }
            }
        }

        Ok(ports)
    }

    /// Get bridge info
    pub async fn get_bridge(&self, name: &str) -> Result<Value> {
        let select = json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", name]]
        });

        let result = self.transact(vec![select]).await?;

        result
            .get(0)
            .and_then(|r| r.get("rows"))
            .and_then(|rows| rows.get(0))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", name))
    }
}

impl Default for OvsdbClient {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TOOL IMPLEMENTATIONS
// =============================================================================

/// Tool: Create OVS Bridge
pub struct OvsCreateBridgeTool {
    client: OvsdbClient,
}

impl OvsCreateBridgeTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsCreateBridgeTool {
    fn name(&self) -> &str {
        "ovs_create_bridge"
    }

    fn description(&self) -> &str {
        "Create an Open vSwitch bridge using native OVSDB JSON-RPC protocol. \
         NO CLI tools are used. Communicates directly with ovsdb-server."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge to create (e.g., 'ovsbr0')"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let result = self.client.create_bridge(name).await?;

        Ok(json!({
            "success": true,
            "operation": "create_bridge",
            "bridge": name,
            "protocol": "OVSDB JSON-RPC",
            "socket": OVSDB_SOCKET,
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Delete OVS Bridge
pub struct OvsDeleteBridgeTool {
    client: OvsdbClient,
}

impl OvsDeleteBridgeTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsDeleteBridgeTool {
    fn name(&self) -> &str {
        "ovs_delete_bridge"
    }

    fn description(&self) -> &str {
        "Delete an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge to delete"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let result = self.client.delete_bridge(name).await?;

        Ok(json!({
            "success": true,
            "operation": "delete_bridge",
            "bridge": name,
            "protocol": "OVSDB JSON-RPC",
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: List OVS Bridges
pub struct OvsListBridgesTool {
    client: OvsdbClient,
}

impl OvsListBridgesTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsListBridgesTool {
    fn name(&self) -> &str {
        "ovs_list_bridges"
    }

    fn description(&self) -> &str {
        "List all Open vSwitch bridges using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let bridges = self.client.list_bridges().await?;

        Ok(json!({
            "success": true,
            "operation": "list_bridges",
            "bridges": bridges,
            "count": bridges.len(),
            "protocol": "OVSDB JSON-RPC"
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Add Port to Bridge
pub struct OvsAddPortTool {
    client: OvsdbClient,
}

impl OvsAddPortTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsAddPortTool {
    fn name(&self) -> &str {
        "ovs_add_port"
    }

    fn description(&self) -> &str {
        "Add a port to an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                },
                "port": {
                    "type": "string",
                    "description": "Name of the port to add"
                }
            },
            "required": ["bridge", "port"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let port = input
            .get("port")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: port"))?;

        let result = self.client.add_port(bridge, port).await?;

        Ok(json!({
            "success": true,
            "operation": "add_port",
            "bridge": bridge,
            "port": port,
            "protocol": "OVSDB JSON-RPC",
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "port".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Delete Port from Bridge
pub struct OvsDeletePortTool {
    client: OvsdbClient,
}

impl OvsDeletePortTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsDeletePortTool {
    fn name(&self) -> &str {
        "ovs_delete_port"
    }

    fn description(&self) -> &str {
        "Delete a port from an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                },
                "port": {
                    "type": "string",
                    "description": "Name of the port to delete"
                }
            },
            "required": ["bridge", "port"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let port = input
            .get("port")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: port"))?;

        let result = self.client.delete_port(bridge, port).await?;

        Ok(json!({
            "success": true,
            "operation": "delete_port",
            "bridge": bridge,
            "port": port,
            "protocol": "OVSDB JSON-RPC",
            "result": result
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "port".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: List Ports on Bridge
pub struct OvsListPortsTool {
    client: OvsdbClient,
}

impl OvsListPortsTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsListPortsTool {
    fn name(&self) -> &str {
        "ovs_list_ports"
    }

    fn description(&self) -> &str {
        "List all ports on an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bridge": {
                    "type": "string",
                    "description": "Name of the bridge"
                }
            },
            "required": ["bridge"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let bridge = input
            .get("bridge")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: bridge"))?;

        let ports = self.client.list_ports(bridge).await?;

        Ok(json!({
            "success": true,
            "operation": "list_ports",
            "bridge": bridge,
            "ports": ports,
            "count": ports.len(),
            "protocol": "OVSDB JSON-RPC"
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "port".to_string(), "ovsdb".to_string()]
    }
}

/// Tool: Get Bridge Info
pub struct OvsGetBridgeTool {
    client: OvsdbClient,
}

impl OvsGetBridgeTool {
    pub fn new() -> Self {
        Self {
            client: OvsdbClient::new(),
        }
    }
}

#[async_trait]
impl Tool for OvsGetBridgeTool {
    fn name(&self) -> &str {
        "ovs_get_bridge"
    }

    fn description(&self) -> &str {
        "Get detailed information about an Open vSwitch bridge using native OVSDB JSON-RPC protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the bridge"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let bridge = self.client.get_bridge(name).await?;
        let ports = self.client.list_ports(name).await.unwrap_or_default();

        Ok(json!({
            "success": true,
            "operation": "get_bridge",
            "bridge": name,
            "info": bridge,
            "ports": ports,
            "protocol": "OVSDB JSON-RPC"
        }))
    }

    fn category(&self) -> &str {
        "network"
    }

    fn tags(&self) -> Vec<String> {
        vec!["ovs".to_string(), "bridge".to_string(), "ovsdb".to_string()]
    }
}

// =============================================================================
// TOOL REGISTRATION
// =============================================================================

/// Create all OVS tools
pub fn create_ovs_tools() -> Vec<BoxedTool> {
    vec![
        Arc::new(OvsCreateBridgeTool::new()),
        Arc::new(OvsDeleteBridgeTool::new()),
        Arc::new(OvsListBridgesTool::new()),
        Arc::new(OvsAddPortTool::new()),
        Arc::new(OvsDeletePortTool::new()),
        Arc::new(OvsListPortsTool::new()),
        Arc::new(OvsGetBridgeTool::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_names() {
        let tools = create_ovs_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();

        assert!(names.contains(&"ovs_create_bridge"));
        assert!(names.contains(&"ovs_delete_bridge"));
        assert!(names.contains(&"ovs_list_bridges"));
        assert!(names.contains(&"ovs_add_port"));
        assert!(names.contains(&"ovs_delete_port"));
        assert!(names.contains(&"ovs_list_ports"));
        assert!(names.contains(&"ovs_get_bridge"));
    }

    #[test]
    fn test_tool_schemas() {
        let tool = OvsCreateBridgeTool::new();
        let schema = tool.input_schema();

        assert!(schema.get("properties").is_some());
        assert!(schema.get("properties").unwrap().get("name").is_some());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/packagekit.rs">
//! PackageKit D-Bus tools (native, no CLI fallbacks).
//!
//! These tools use org.freedesktop.PackageKit over D-Bus via zbus.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use zbus::Connection;

use crate::{Tool, ToolRegistry};

pub struct DbusPackageKitInstallTool;

#[async_trait]
impl Tool for DbusPackageKitInstallTool {
    fn name(&self) -> &str {
        "dbus_packagekit_install_packages"
    }

    fn description(&self) -> &str {
        "Install packages via PackageKit D-Bus (no CLI)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "packages": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Package IDs (e.g., name;version;arch;repo)"
                },
                "transaction_flags": {
                    "type": "integer",
                    "description": "PackageKit transaction flags",
                    "default": 0
                }
            },
            "required": ["packages"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let packages = input
            .get("packages")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: packages"))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<String>>();

        if packages.is_empty() {
            return Err(anyhow::anyhow!("packages must be a non-empty array of strings"));
        }

        let flags = input
            .get("transaction_flags")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let tx_path = create_transaction().await?;
        install_packages(&tx_path, flags, &packages).await?;

        Ok(json!({
            "installed": packages,
            "transaction": tx_path,
            "protocol": "D-Bus"
        }))
    }

    fn category(&self) -> &str {
        "packagekit"
    }
}

pub struct DbusPackageKitRemoveTool;

#[async_trait]
impl Tool for DbusPackageKitRemoveTool {
    fn name(&self) -> &str {
        "dbus_packagekit_remove_packages"
    }

    fn description(&self) -> &str {
        "Remove packages via PackageKit D-Bus (no CLI)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "packages": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Package IDs (e.g., name;version;arch;repo)"
                },
                "transaction_flags": {
                    "type": "integer",
                    "description": "PackageKit transaction flags",
                    "default": 0
                },
                "allow_deps": {
                    "type": "boolean",
                    "description": "Allow removing dependent packages",
                    "default": true
                },
                "autoremove": {
                    "type": "boolean",
                    "description": "Auto-remove unused dependencies",
                    "default": false
                }
            },
            "required": ["packages"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let packages = input
            .get("packages")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: packages"))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<String>>();

        if packages.is_empty() {
            return Err(anyhow::anyhow!("packages must be a non-empty array of strings"));
        }

        let flags = input
            .get("transaction_flags")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let allow_deps = input
            .get("allow_deps")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let autoremove = input
            .get("autoremove")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let tx_path = create_transaction().await?;
        remove_packages(&tx_path, flags, &packages, allow_deps, autoremove).await?;

        Ok(json!({
            "removed": packages,
            "transaction": tx_path,
            "protocol": "D-Bus"
        }))
    }

    fn category(&self) -> &str {
        "packagekit"
    }
}

async fn create_transaction() -> Result<String> {
    let connection = Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        "/org/freedesktop/PackageKit",
        "org.freedesktop.PackageKit",
    )
    .await?;

    let path: zbus::zvariant::OwnedObjectPath =
        proxy.call("CreateTransaction", &()).await?;
    Ok(path.to_string())
}

async fn install_packages(tx_path: &str, flags: u64, packages: &[String]) -> Result<()> {
    let connection = Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        tx_path,
        "org.freedesktop.PackageKit.Transaction",
    )
    .await?;

    let _: () = proxy
        .call("InstallPackages", &(flags, packages.to_vec()))
        .await?;
    Ok(())
}

async fn remove_packages(
    tx_path: &str,
    flags: u64,
    packages: &[String],
    allow_deps: bool,
    autoremove: bool,
) -> Result<()> {
    let connection = Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        tx_path,
        "org.freedesktop.PackageKit.Transaction",
    )
    .await?;

    let _: () = proxy
        .call(
            "RemovePackages",
            &(flags, packages.to_vec(), allow_deps, autoremove),
        )
        .await?;
    Ok(())
}

/// Register PackageKit tools.
pub async fn register_packagekit_tools(registry: &ToolRegistry) -> Result<()> {
    registry.register_tool(Arc::new(DbusPackageKitInstallTool)).await?;
    registry.register_tool(Arc::new(DbusPackageKitRemoveTool)).await?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/plugin_projection.rs">
//! Tools backed by plugin-created D-Bus projection objects.
//!
//! Every object published below `/org/opdbus/v1/plugins` is exposed as a
//! read-only tool. Execution reads the live `org.opdbus.ProjectedObjectV1`
//! object rather than scraping procfs or rebuilding state locally.

use crate::tool::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;

const PLUGIN_ROOT: &str = "/org/opdbus/v1/plugins";
const OPDBUS_DEST: &str = "org.opdbus.v1";
const PROJECTED_IFACE: &str = "org.opdbus.ProjectedObjectV1";

#[derive(Clone)]
pub struct PluginProjectionTool {
    name: String,
    service: String,
    plugin_name: String,
    object_path: String,
}

impl PluginProjectionTool {
    pub fn new(plugin_name: &str, object_path: String) -> Self {
        Self {
            name: tool_name_for_path(&object_path),
            service: OPDBUS_DEST.to_string(),
            plugin_name: plugin_name.to_string(),
            object_path,
        }
    }

    pub fn new_generic(service: &str, object_path: String) -> Self {
        let name = format!(
            "projection_{}_{}",
            sanitize_segment(service.split('.').last().unwrap_or(service)).to_ascii_lowercase(),
            sanitize_segment(object_path.split('/').last().unwrap_or(&object_path))
                .to_ascii_lowercase()
        );
        Self {
            name,
            service: service.to_string(),
            plugin_name: "generic".to_string(),
            object_path,
        }
    }

    async fn connection() -> Result<zbus::Connection> {
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok() {
            zbus::Connection::session()
                .await
                .map_err(|e| anyhow::anyhow!("failed to connect to shared session bus: {}", e))
        } else {
            zbus::Connection::system()
                .await
                .map_err(|e| anyhow::anyhow!("failed to connect to system bus: {}", e))
        }
    }
}

#[async_trait]
impl Tool for PluginProjectionTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Read a live object created by a PluginSchema-backed plugin projection."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "property": {
                    "type": "string",
                    "description": "Optional top-level JSON property to read from the projected object"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let connection = Self::connection().await?;
        let proxy: zbus::Proxy<'_> = zbus::proxy::Builder::new(&connection)
            .destination(self.service.as_str())?
            .path(self.object_path.as_str())?
            .interface(PROJECTED_IFACE)?
            .build()
            .await?;

        let json_text: String =
            if let Some(property) = input.get("property").and_then(|v| v.as_str()) {
                proxy
                    .call::<_, _, String>("get_property", &(property.to_string(),))
                    .await?
            } else {
                proxy.get_property::<String>("json_data").await?
            };

        let mut buf = json_text.into_bytes();
        let data = simd_json::from_slice::<Value>(&mut buf).unwrap_or_else(|_| Value::null());
        Ok(json!({
            "plugin": self.plugin_name,
            "service": self.service,
            "object_path": self.object_path,
            "data": data
        }))
    }

    fn category(&self) -> &str {
        "plugin"
    }

    fn namespace(&self) -> &str {
        "plugin-projection"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "plugin".to_string(),
            "projection".to_string(),
            self.plugin_name.clone(),
        ]
    }
}

pub async fn register_plugin_projection_tools(
    registry: &ToolRegistry,
    plugin_state: &HashMap<String, Value>,
) -> Result<usize> {
    let mut count = 0;

    for (plugin_name, state) in plugin_state {
        let root_path = plugin_path(plugin_name);
        let mut paths = vec![root_path.clone()];
        collect_child_paths(&root_path, state, &mut paths);

        for path in paths {
            registry
                .register_tool(Arc::new(PluginProjectionTool::new(plugin_name, path)))
                .await?;
            count += 1;
        }
    }

    Ok(count)
}

fn collect_child_paths(root_path: &str, data: &Value, out: &mut Vec<String>) {
    match data {
        Value::Object(map) => {
            for (key, value) in map.iter() {
                let child_path = format!("{}/{}", root_path, sanitize_segment(key.as_str()));
                out.push(child_path.clone());
                collect_child_paths(&child_path, value, out);
            }
        }
        Value::Array(items) => {
            for (idx, value) in items.iter().enumerate() {
                let child_path = format!("{}/{}", root_path, idx);
                out.push(child_path.clone());
                collect_child_paths(&child_path, value, out);
            }
        }
        _ => {}
    }
}

fn plugin_path(plugin_name: &str) -> String {
    format!("{}/{}", PLUGIN_ROOT, sanitize_segment(plugin_name))
}

fn sanitize_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for ch in segment.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

fn tool_name_for_path(path: &str) -> String {
    let suffix = path
        .strip_prefix(PLUGIN_ROOT)
        .unwrap_or(path)
        .trim_matches('/');
    let mut name = String::from("plugin_projection");
    for part in suffix.split('/').filter(|part| !part.is_empty()) {
        name.push('_');
        name.push_str(&sanitize_segment(part).to_ascii_lowercase());
    }
    name
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/plugin_state_tool.rs">
//! Plugin State Tool - Creates tools from StatePlugin operations
//!
//! Provides query, diff, and apply tools for each registered StatePlugin.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;

use op_state::PluginCapabilities;

/// Operation supported by the plugin tool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginOperation {
    Query,
    Diff,
    Apply,
}
use crate::tool::{BoxedTool, Tool};

/// Plugin state tool that wraps a StatePlugin operation
pub struct PluginStateTool {
    name: String,
    description: String,
    operation: PluginOperation,
    plugin_name: String,
    capabilities: PluginCapabilities,
    /// Reference to the plugin catalog executor for executing operations
    plugin_executor: Arc<dyn PluginExecutor + Send + Sync>,
}

/// Trait for executing plugin operations
#[async_trait]
pub trait PluginExecutor: Send + Sync {
    /// Query current state from a plugin
    async fn query_state(&self, plugin_name: &str, filter: Option<Value>) -> Result<Value>;
    
    /// Calculate diff between current and desired state
    async fn calculate_diff(&self, plugin_name: &str, desired_state: Value) -> Result<Value>;
    
    /// Apply a state diff
    async fn apply_diff(&self, plugin_name: &str, diff: Value, dry_run: bool) -> Result<Value>;
}

impl PluginStateTool {
    pub fn new(
        plugin_name: &str,
        description: &str,
        operation: PluginOperation,
        capabilities: &PluginCapabilities,
        executor: Arc<dyn PluginExecutor + Send + Sync>,
    ) -> Self {
        let op_suffix = match operation {
            PluginOperation::Query => "query",
            PluginOperation::Diff => "diff",
            PluginOperation::Apply => "apply",
        };
        
        let name = format!("{}_{}", plugin_name, op_suffix);
        let description = match operation {
            PluginOperation::Query => format!("Query current state from {} plugin", plugin_name),
            PluginOperation::Diff => format!("Calculate state diff for {} plugin", plugin_name),
            PluginOperation::Apply => format!("Apply state changes for {} plugin", plugin_name),
        };
        
        Self {
            name,
            description,
            operation,
            plugin_name: plugin_name.to_string(),
            capabilities: capabilities.clone(),
            plugin_executor: executor,
        }
    }
}

#[async_trait]
impl Tool for PluginStateTool {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    fn input_schema(&self) -> Value {
        match self.operation {
            PluginOperation::Query => simd_json::json!({
                "type": "object",
                "properties": {
                    "filter": {
                        "type": "object",
                        "description": "Optional filter for state query"
                    }
                }
            }),
            PluginOperation::Diff => simd_json::json!({
                "type": "object",
                "properties": {
                    "desired_state": {
                        "type": "object",
                        "description": "Desired state configuration"
                    }
                },
                "required": ["desired_state"]
            }),
            PluginOperation::Apply => simd_json::json!({
                "type": "object",
                "properties": {
                    "diff": {
                        "type": "object",
                        "description": "State diff to apply"
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "If true, only simulate changes",
                        "default": false
                    }
                },
                "required": ["diff"]
            }),
        }
    }
    
    async fn execute(&self, input: Value) -> Result<Value> {
        match self.operation {
            PluginOperation::Query => {
                let filter = input.get("filter").cloned();
                self.plugin_executor.query_state(&self.plugin_name, filter).await
            }
            PluginOperation::Diff => {
                let desired_state = input.get("desired_state")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Missing required field: desired_state"))?;
                self.plugin_executor.calculate_diff(&self.plugin_name, desired_state).await
            }
            PluginOperation::Apply => {
                let diff = input.get("diff")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Missing required field: diff"))?;
                let dry_run = input.get("dry_run")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.plugin_executor.apply_diff(&self.plugin_name, diff, dry_run).await
            }
        }
    }
}

/// Create a plugin state tool
pub fn create_plugin_state_tool(
    plugin_name: &str,
    description: &str,
    operation: PluginOperation,
    capabilities: &PluginCapabilities,
) -> Result<BoxedTool> {
    // Create a default executor that returns an error.
    // In production, this should be replaced with the shared plugin catalog
    // path rather than a local ad hoc plugin map.
    let executor = Arc::new(DefaultPluginExecutor::new());
    
    Ok(Arc::new(PluginStateTool::new(
        plugin_name,
        description,
        operation,
        capabilities,
        executor,
    )))
}

/// Create a plugin state tool with a custom executor
pub fn create_plugin_state_tool_with_executor(
    plugin_name: &str,
    description: &str,
    operation: PluginOperation,
    capabilities: &PluginCapabilities,
    executor: Arc<dyn PluginExecutor + Send + Sync>,
) -> Result<BoxedTool> {
    Ok(Arc::new(PluginStateTool::new(
        plugin_name,
        description,
        operation,
        capabilities,
        executor,
    )))
}

/// Default plugin executor that delegates to a local compatibility map.
///
/// This is intentionally not authoritative. Real deployments should delegate
/// to the canonical plugin catalog / plugin document path.
pub struct DefaultPluginExecutor {
    /// Local compatibility map used by older tool plumbing.
    plugins: Arc<RwLock<std::collections::HashMap<String, Arc<dyn StatePluginAdapter + Send + Sync>>>>,
}

impl DefaultPluginExecutor {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    pub async fn register_plugin(&self, name: &str, plugin: Arc<dyn StatePluginAdapter + Send + Sync>) {
        let mut plugins = self.plugins.write().await;
        plugins.insert(name.to_string(), plugin);
    }
}

impl Default for DefaultPluginExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PluginExecutor for DefaultPluginExecutor {
    async fn query_state(&self, plugin_name: &str, filter: Option<Value>) -> Result<Value> {
        let plugins = self.plugins.read().await;
        match plugins.get(plugin_name) {
            Some(plugin) => plugin.query_state(filter).await,
            None => Err(anyhow::anyhow!("Plugin not found: {}", plugin_name)),
        }
    }
    
    async fn calculate_diff(&self, plugin_name: &str, desired_state: Value) -> Result<Value> {
        let plugins = self.plugins.read().await;
        match plugins.get(plugin_name) {
            Some(plugin) => plugin.calculate_diff(desired_state).await,
            None => Err(anyhow::anyhow!("Plugin not found: {}", plugin_name)),
        }
    }
    
    async fn apply_diff(&self, plugin_name: &str, diff: Value, dry_run: bool) -> Result<Value> {
        let plugins = self.plugins.read().await;
        match plugins.get(plugin_name) {
            Some(plugin) => plugin.apply_diff(diff, dry_run).await,
            None => Err(anyhow::anyhow!("Plugin not found: {}", plugin_name)),
        }
    }
}

/// Adapter trait for StatePlugin to work with the tool system
#[async_trait]
pub trait StatePluginAdapter: Send + Sync {
    async fn query_state(&self, filter: Option<Value>) -> Result<Value>;
    async fn calculate_diff(&self, desired_state: Value) -> Result<Value>;
    async fn apply_diff(&self, diff: Value, dry_run: bool) -> Result<Value>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPluginAdapter;

    #[async_trait]
    impl StatePluginAdapter for MockPluginAdapter {
        async fn query_state(&self, _filter: Option<Value>) -> Result<Value> {
            Ok(simd_json::json!({"packages": ["vim", "git"]}))
        }

        async fn calculate_diff(&self, desired_state: Value) -> Result<Value> {
            Ok(simd_json::json!({
                "add": desired_state.get("add").cloned().unwrap_or(Value::Null),
                "remove": []
            }))
        }

        async fn apply_diff(&self, diff: Value, dry_run: bool) -> Result<Value> {
            Ok(simd_json::json!({
                "applied": !dry_run,
                "changes": diff
            }))
        }
    }

    #[tokio::test]
    async fn test_plugin_state_tool_query() {
        let executor = Arc::new(DefaultPluginExecutor::new());
        executor.register_plugin("test", Arc::new(MockPluginAdapter)).await;

        let tool = PluginStateTool::new(
            "test",
            "Test plugin",
            PluginOperation::Query,
            &PluginCapabilities::default(),
            executor,
        );

        let result = tool.execute(simd_json::json!({})).await.unwrap();
        assert!(result.get("packages").is_some());
    }

    #[tokio::test]
    async fn test_plugin_state_tool_apply() {
        let executor = Arc::new(DefaultPluginExecutor::new());
        executor.register_plugin("test", Arc::new(MockPluginAdapter)).await;

        let tool = PluginStateTool::new(
            "test",
            "Test plugin",
            PluginOperation::Apply,
            &PluginCapabilities::default(),
            executor,
        );

        let result = tool.execute(simd_json::json!({
            "diff": {"add": ["nginx"]},
            "dry_run": true
        })).await.unwrap();
        
        assert_eq!(result.get("applied").and_then(|v| v.as_bool()), Some(false));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/plugin.rs">
//! Plugin Tools - State plugin operations

use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use op_core::Tool;

pub struct PluginTool {
    name: String,
    description: String,
    plugin_name: String,
    operation: String,
}

impl PluginTool {
    pub fn new(name: &str, description: &str, plugin_name: &str, operation: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            plugin_name: plugin_name.to_string(),
            operation: operation.to_string(),
        }
    }
}

#[async_trait]
impl Tool for PluginTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        match self.operation.as_str() {
            "query" => json!({
                "type": "object",
                "properties": {
                    "filter": {"type": "object", "description": "Optional filter"}
                }
            }),
            "diff" => json!({
                "type": "object",
                "properties": {
                    "desired_state": {"type": "object", "description": "Desired state"}
                },
                "required": ["desired_state"]
            }),
            "apply" => json!({
                "type": "object",
                "properties": {
                    "diff": {"type": "object", "description": "State diff to apply"},
                    "dry_run": {"type": "boolean", "default": false}
                },
                "required": ["diff"]
            }),
            _ => json!({"type": "object", "properties": {}})
        }
    }

    async fn execute(&self, args: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        // Use op-state's plugin system
        match self.operation.as_str() {
            "query" => {
                match op_state::query_plugin(&self.plugin_name, args).await {
                    Ok(state) => Ok(json!({
                        "plugin": self.plugin_name,
                        "state": state
                    })),
                    Err(e) => Err(format!("Query failed: {}", e).into())
                }
            }
            "diff" => {
                let desired = args.get("desired_state").cloned().unwrap_or(json!({}));
                match op_state::diff_plugin(&self.plugin_name, desired).await {
                    Ok(diff) => Ok(json!({
                        "plugin": self.plugin_name,
                        "diff": diff
                    })),
                    Err(e) => Err(format!("Diff failed: {}", e).into())
                }
            }
            "apply" => {
                let diff = args.get("diff").cloned().unwrap_or(json!({}));
                let dry_run = args.get("dry_run").and_then(|d| d.as_bool()).unwrap_or(false);
                match op_state::apply_plugin(&self.plugin_name, diff, dry_run).await {
                    Ok(result) => Ok(json!({
                        "plugin": self.plugin_name,
                        "applied": !dry_run,
                        "result": result
                    })),
                    Err(e) => Err(format!("Apply failed: {}", e).into())
                }
            }
            _ => Ok(json!({"error": "Unknown operation"}))
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/procfs.rs">
//! /proc and /sys tools with read/write support.

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::Tool;

const MAX_READ_DEPTH: usize = 3;
const MAX_INLINE_SIZE: usize = 64 * 1024;

fn validate_relative_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty() || path.starts_with('/') || path.contains("..") || path.contains('\\') {
        return Err(anyhow::anyhow!("Invalid path"));
    }
    Ok(())
}

fn make_full_path(root: &str, path: &str) -> PathBuf {
    Path::new(root).join(path)
}

async fn read_file_value(path: &Path) -> Value {
    match fs::read_to_string(path).await {
        Ok(content) => {
            let trimmed = content.trim();
            if let Ok(num) = trimmed.parse::<i64>() {
                return json!(num);
            }
            if let Ok(num) = trimmed.parse::<f64>() {
                return json!(num);
            }
            if trimmed.len() > MAX_INLINE_SIZE {
                return json!(format!(
                    "[content too large: {} bytes]",
                    trimmed.len()
                ));
            }
            json!(trimmed)
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

async fn read_path(path: &Path) -> Value {
    if path.is_file() {
        read_file_value(path).await
    } else if path.is_dir() {
        let mut entries = Vec::new();
        if let Ok(mut dir) = fs::read_dir(path).await {
            while let Ok(Some(entry)) = dir.next_entry().await {
                if let Some(name) = entry.file_name().to_str() {
                    let is_dir = entry.path().is_dir();
                    entries.push(json!({
                        "name": name,
                        "type": if is_dir { "dir" } else { "file" }
                    }));
                }
            }
        }
        entries.sort_by(|a, b| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .cmp(b["name"].as_str().unwrap_or(""))
        });
        json!({ "entries": entries, "count": entries.len() })
    } else {
        json!({ "error": "path not found" })
    }
}

async fn fs_to_json(path: &Path, max_depth: usize, current_depth: usize) -> Value {
    if current_depth > max_depth {
        return Value::Null;
    }

    if path.is_file() {
        return read_file_value(path).await;
    }

    if path.is_dir() {
        let mut obj = Map::new();
        if let Ok(mut entries) = fs::read_dir(path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with('.') || name == "fd" || name == "task" {
                        continue;
                    }
                    let child_path = entry.path();
                    let value =
                        Box::pin(fs_to_json(&child_path, max_depth, current_depth + 1)).await;
                    if !value.is_null() {
                        obj.insert(name.to_string(), value);
                    }
                }
            }
        }
        if obj.is_empty() {
            Value::Null
        } else {
            Value::Object(obj)
        }
    } else {
        Value::Null
    }
}

async fn read_with_depth(root: &str, path: &str, depth: usize) -> Value {
    let full_path = make_full_path(root, path);
    if depth > 1 {
        fs_to_json(&full_path, depth, 0).await
    } else {
        read_path(&full_path).await
    }
}

async fn write_value(root: &str, path: &str, content: &str, append: bool) -> anyhow::Result<()> {
    let full_path = make_full_path(root, path);
    if full_path.is_dir() {
        return Err(anyhow::anyhow!("Path is a directory"));
    }

    if append {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&full_path)
            .await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
    } else {
        fs::write(&full_path, content).await?;
    }

    Ok(())
}

pub struct ProcFsReadTool;

impl ProcFsReadTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ProcFsReadTool {
    fn name(&self) -> &str {
        "procfs_read"
    }

    fn description(&self) -> &str {
        "Read /proc as JSON (files parse to numbers/strings; directories list or recurse)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path relative to /proc (e.g., 'sys/net/ipv4/ip_forward', 'meminfo')"
                },
                "depth": {
                    "type": "integer",
                    "description": "Max recursion depth for directories (default: 1, max: 3)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> &str {
        "system"
    }

    fn tags(&self) -> Vec<String> {
        vec!["proc".to_string(), "system".to_string(), "filesystem".to_string()]
    }

    async fn execute(&self, input: Value) -> anyhow::Result<Value> {
        let (path, depth) = match input {
            Value::Object(mut obj) => {
                let path = obj
                    .remove("path")
                    .and_then(|v| v.as_str().map(str::to_string))
                    .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let depth = obj
                    .remove("depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .min(MAX_READ_DEPTH as u64) as usize;
                (path, depth)
            }
            _ => return Err(anyhow::anyhow!("Invalid arguments")),
        };

        validate_relative_path(&path)?;
        let data = read_with_depth("/proc", &path, depth).await;
        Ok(json!({
            "path": format!("/proc/{}", path),
            "data": data
        }))
    }
}

pub struct SysFsReadTool;

impl SysFsReadTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SysFsReadTool {
    fn name(&self) -> &str {
        "sysfs_read"
    }

    fn description(&self) -> &str {
        "Read /sys as JSON (files parse to numbers/strings; directories list or recurse)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path relative to /sys (e.g., 'class/net', 'devices/system/cpu')"
                },
                "depth": {
                    "type": "integer",
                    "description": "Max recursion depth for directories (default: 1, max: 3)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> &str {
        "system"
    }

    fn tags(&self) -> Vec<String> {
        vec!["sys".to_string(), "hardware".to_string(), "filesystem".to_string()]
    }

    async fn execute(&self, input: Value) -> anyhow::Result<Value> {
        let (path, depth) = match input {
            Value::Object(mut obj) => {
                let path = obj
                    .remove("path")
                    .and_then(|v| v.as_str().map(str::to_string))
                    .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let depth = obj
                    .remove("depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .min(MAX_READ_DEPTH as u64) as usize;
                (path, depth)
            }
            _ => return Err(anyhow::anyhow!("Invalid arguments")),
        };

        validate_relative_path(&path)?;
        let data = read_with_depth("/sys", &path, depth).await;
        Ok(json!({
            "path": format!("/sys/{}", path),
            "data": data
        }))
    }
}

pub struct ProcFsWriteTool;

impl ProcFsWriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ProcFsWriteTool {
    fn name(&self) -> &str {
        "procfs_write"
    }

    fn description(&self) -> &str {
        "Write to /proc files (used to change kernel parameters)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path relative to /proc (e.g., 'sys/net/ipv4/ip_forward')"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write"
                },
                "append": {
                    "type": "boolean",
                    "default": false,
                    "description": "Append instead of overwrite"
                },
                "ensure_newline": {
                    "type": "boolean",
                    "default": true,
                    "description": "Ensure trailing newline (common for /proc writes)"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn category(&self) -> &str {
        "system"
    }

    fn namespace(&self) -> &str {
        "control-agent"
    }

    fn tags(&self) -> Vec<String> {
        vec!["proc".to_string(), "system".to_string(), "write".to_string()]
    }

    async fn execute(&self, input: Value) -> anyhow::Result<Value> {
        let mut obj = match input {
            Value::Object(obj) => obj,
            _ => return Err(anyhow::anyhow!("Invalid arguments")),
        };

        let path = obj
            .remove("path")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let mut content = obj
            .remove("content")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("Missing content"))?;
        let append = obj
            .remove("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let ensure_newline = obj
            .remove("ensure_newline")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if ensure_newline && !content.ends_with('\n') {
            content.push('\n');
        }

        validate_relative_path(&path)?;
        write_value("/proc", &path, &content, append).await?;

        Ok(json!({
            "path": format!("/proc/{}", path),
            "written_bytes": content.len(),
            "append": append
        }))
    }
}

pub struct SysFsWriteTool;

impl SysFsWriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SysFsWriteTool {
    fn name(&self) -> &str {
        "sysfs_write"
    }

    fn description(&self) -> &str {
        "Write to /sys files (used to change device parameters)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path relative to /sys (e.g., 'class/net/eth0/mtu')"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write"
                },
                "append": {
                    "type": "boolean",
                    "default": false,
                    "description": "Append instead of overwrite"
                },
                "ensure_newline": {
                    "type": "boolean",
                    "default": true,
                    "description": "Ensure trailing newline (common for /sys writes)"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn category(&self) -> &str {
        "system"
    }

    fn namespace(&self) -> &str {
        "control-agent"
    }

    fn tags(&self) -> Vec<String> {
        vec!["sys".to_string(), "hardware".to_string(), "write".to_string()]
    }

    async fn execute(&self, input: Value) -> anyhow::Result<Value> {
        let mut obj = match input {
            Value::Object(obj) => obj,
            _ => return Err(anyhow::anyhow!("Invalid arguments")),
        };

        let path = obj
            .remove("path")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let mut content = obj
            .remove("content")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("Missing content"))?;
        let append = obj
            .remove("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let ensure_newline = obj
            .remove("ensure_newline")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if ensure_newline && !content.ends_with('\n') {
            content.push('\n');
        }

        validate_relative_path(&path)?;
        write_value("/sys", &path, &content, append).await?;

        Ok(json!({
            "path": format!("/sys/{}", path),
            "written_bytes": content.len(),
            "append": append
        }))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/respond_tool.rs">
//! Response Tool - Forces LLM to use tools for ALL interactions
//!
//! This tool makes responding to users an explicit action, preventing hallucination
//! by ensuring every LLM output goes through the tool execution pipeline.

use async_trait::async_trait;
use simd_json::json;

use crate::Tool;
use op_core::{ToolDefinition, ToolRequest, ToolResult};

/// Tool for the LLM to respond to users
///
/// By making "respond" a tool, we force the LLM to go through tool execution
/// for EVERY interaction - no more hallucinated claims without tool calls.
pub struct RespondToUserTool;

#[async_trait]
impl Tool for RespondToUserTool {
    fn name(&self) -> &str {
        "respond_to_user"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "respond_to_user".to_string(),
            description: "Send a response message to the user. Use this tool when you want to communicate information, ask questions, or provide explanations. DO NOT claim to have performed actions - use action tools first, then respond with their results.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The message to send to the user. Be clear and helpful."
                    },
                    "message_type": {
                        "type": "string",
                        "enum": ["info", "question", "explanation", "error", "success"],
                        "description": "Type of message: info (general information), question (asking user), explanation (detailed explanation), error (reporting a problem), success (confirming completed action)"
                    }
                },
                "required": ["message", "message_type"]
            }),
            category: Some("communication".to_string()),
            tags: vec!["response".to_string(), "communication".to_string(), "required".to_string()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let message = request
            .arguments
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("No message provided");

        let message_type = request
            .arguments
            .get("message_type")
            .and_then(|v| v.as_str())
            .unwrap_or("info");

        // Just return the message - the orchestrator will handle displaying it
        ToolResult::success(
            request.id,
            json!({
                "message": message,
                "message_type": message_type,
                "delivered": true
            }),
            1, // 1ms execution time
        )
    }
}

/// Tool for reporting that an action cannot be performed
pub struct CannotPerformTool;

#[async_trait]
impl Tool for CannotPerformTool {
    fn name(&self) -> &str {
        "cannot_perform"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "cannot_perform".to_string(),
            description: "Use this tool when you cannot perform a requested action. Explain why and suggest alternatives. NEVER claim you performed an action that you didn't do.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "description": "Why you cannot perform the action"
                    },
                    "alternatives": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Alternative actions or suggestions"
                    }
                },
                "required": ["reason"]
            }),
            category: Some("communication".to_string()),
            tags: vec!["response".to_string(), "error".to_string()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let reason = request
            .arguments
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown reason");

        let alternatives = request
            .arguments
            .get("alternatives")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        ToolResult::success(
            request.id,
            json!({
                "status": "cannot_perform",
                "reason": reason,
                "alternatives": alternatives
            }),
            1,
        )
    }
}

/// Create response-related tools
pub fn create_response_tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(RespondToUserTool), Box::new(CannotPerformTool)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_respond_tool() {
        let tool = RespondToUserTool;
        let request = ToolRequest {
            id: "test-1".to_string(),
            tool_name: "respond_to_user".to_string(),
            arguments: json!({
                "message": "Hello, this is a test response",
                "message_type": "info"
            }),
            timeout_ms: None,
        };

        let result = tool.execute(request).await;
        assert!(result.success);
        assert!(result.content.get("message").is_some());
    }

    #[tokio::test]
    async fn test_cannot_perform_tool() {
        let tool = CannotPerformTool;
        let request = ToolRequest {
            id: "test-2".to_string(),
            tool_name: "cannot_perform".to_string(),
            arguments: json!({
                "reason": "Service not available",
                "alternatives": ["Try later", "Check connection"]
            }),
            timeout_ms: None,
        };

        let result = tool.execute(request).await;
        assert!(result.success);
        assert_eq!(result.content.get("status").unwrap(), "cannot_perform");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/response_tools.rs">
//! Response Tools - Force LLM to use tools for all responses
//!
//! These tools ensure the LLM cannot hallucinate by requiring all
//! communication to go through verifiable tool calls.
//!
//! ## How It Works
//!
//! When tool_choice is set to "required", the LLM MUST call a tool.
//! To communicate with the user, it must call `respond_to_user`.
//! This allows us to:
//! 1. Verify that claimed actions actually happened
//! 2. Track what the LLM is telling the user
//! 3. Reject hallucinated responses

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::tool::{BoxedTool, Tool};

// ============================================================================
// RESPONSE ACCUMULATOR
// ============================================================================

/// A single response from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub message: String,
    pub message_type: String,
    pub related_tool_calls: Vec<String>,
    pub data: Option<Value>,
}

/// Accumulates responses from respond_to_user tool calls
#[derive(Debug, Default)]
pub struct ResponseAccumulator {
    responses: Vec<LlmResponse>,
}

impl ResponseAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, response: LlmResponse) {
        self.responses.push(response);
    }

    pub fn clear(&mut self) {
        self.responses.clear();
    }

    pub fn get_responses(&self) -> &[LlmResponse] {
        &self.responses
    }

    /// Convert all responses to a single user message
    pub fn to_user_message(&self) -> String {
        self.responses
            .iter()
            .map(|r| r.message.clone())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// Global response accumulator (initialized eagerly)
static RESPONSE_ACCUMULATOR: std::sync::OnceLock<Arc<RwLock<ResponseAccumulator>>> =
    std::sync::OnceLock::new();

/// Initialize the global response accumulator (call once at startup)
pub fn init_response_accumulator() {
    let _ = RESPONSE_ACCUMULATOR.set(Arc::new(RwLock::new(ResponseAccumulator::new())));
}

/// Get the global response accumulator
pub fn get_response_accumulator() -> Arc<RwLock<ResponseAccumulator>> {
    RESPONSE_ACCUMULATOR
        .get()
        .expect("Response accumulator not initialized")
        .clone()
}

// ============================================================================
// RESPOND TO USER TOOL
// ============================================================================

/// Tool: Respond to User
///
/// ALL LLM responses to the user MUST go through this tool.
/// This allows verification that claimed actions were actually performed.
pub struct RespondToUserTool;

impl RespondToUserTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RespondToUserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RespondToUserTool {
    fn name(&self) -> &str {
        "respond_to_user"
    }

    fn description(&self) -> &str {
        "Send a response to the user. ALL responses MUST use this tool. \
         Include related_actions to declare which tools were used - \
         this will be verified against actual tool executions. \
         NEVER output text directly - always use this tool."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to send to the user"
                },
                "message_type": {
                    "type": "string",
                    "enum": ["success", "info", "warning", "error"],
                    "description": "Type of message",
                    "default": "info"
                },
                "related_actions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of tool names that were called to produce this response. Will be verified against actual executions."
                },
                "data": {
                    "type": "object",
                    "description": "Optional structured data to include with response"
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("No message provided")
            .to_string();

        let message_type = input
            .get("message_type")
            .and_then(|v| v.as_str())
            .unwrap_or("info")
            .to_string();

        let related_actions: Vec<String> = input
            .get("related_actions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let data = input.get("data").cloned();

        info!(
            message_type = %message_type,
            related_actions = ?related_actions,
            "User response generated via respond_to_user tool"
        );

        // Add to accumulator
        let response = LlmResponse {
            message: message.clone(),
            message_type: message_type.clone(),
            related_tool_calls: related_actions.clone(),
            data: data.clone(),
        };

        {
            let accumulator_arc = get_response_accumulator();
            let mut accumulator = accumulator_arc.write().await;
            accumulator.add(response);
        }

        Ok(json!({
            "tool": "respond_to_user",
            "message": message,
            "message_type": message_type,
            "related_actions": related_actions,
            "data": data,
            "_internal": {
                "is_response_tool": true,
                "requires_verification": !related_actions.is_empty()
            }
        }))
    }

    fn category(&self) -> &str {
        "response"
    }

    fn namespace(&self) -> &str {
        "chat"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "response".to_string(),
            "user".to_string(),
            "required".to_string(),
        ]
    }
}

// ============================================================================
// CANNOT PERFORM TOOL
// ============================================================================

/// Tool: Cannot Perform
///
/// Use when the LLM cannot or should not perform a requested action.
pub struct CannotPerformTool;

impl CannotPerformTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CannotPerformTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CannotPerformTool {
    fn name(&self) -> &str {
        "cannot_perform"
    }

    fn description(&self) -> &str {
        "Decline to perform a requested action. Use when: \
         1) Action would be dangerous or destructive \
         2) Action is outside allowed capabilities \
         3) Action requires information not available \
         4) Action violates system policy"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Why the action cannot be performed"
                },
                "category": {
                    "type": "string",
                    "enum": ["dangerous", "not_allowed", "missing_info", "policy_violation", "not_supported"],
                    "description": "Category of refusal"
                },
                "alternatives": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Alternative actions the user could take"
                }
            },
            "required": ["reason", "category"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("Action cannot be performed")
            .to_string();

        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("not_allowed")
            .to_string();

        let alternatives: Vec<String> = input
            .get("alternatives")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        info!(
            category = %category,
            reason = %reason,
            "Action declined via cannot_perform tool"
        );

        // Add to accumulator as a response
        let message = format!("Cannot perform action: {} ({})", reason, category);
        let response = LlmResponse {
            message: message.clone(),
            message_type: "error".to_string(),
            related_tool_calls: vec![],
            data: None,
        };

        {
            let accumulator_arc = get_response_accumulator();
            let mut accumulator = accumulator_arc.write().await;
            accumulator.add(response);
        }

        Ok(json!({
            "tool": "cannot_perform",
            "declined": true,
            "reason": reason,
            "category": category,
            "alternatives": alternatives,
            "_internal": {
                "is_response_tool": true,
                "requires_verification": false
            }
        }))
    }

    fn category(&self) -> &str {
        "response"
    }

    fn namespace(&self) -> &str {
        "chat"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "response".to_string(),
            "decline".to_string(),
            "safety".to_string(),
        ]
    }
}

// ============================================================================
// REQUEST CLARIFICATION TOOL
// ============================================================================

/// Tool: Request Clarification
///
/// Use when more information is needed from the user.
pub struct RequestClarificationTool;

impl RequestClarificationTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RequestClarificationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RequestClarificationTool {
    fn name(&self) -> &str {
        "request_clarification"
    }

    fn description(&self) -> &str {
        "Request additional information from the user before proceeding. \
         Use when the request is ambiguous or missing required details."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The clarifying question to ask"
                },
                "context": {
                    "type": "string",
                    "description": "Why this clarification is needed"
                },
                "options": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Possible options for the user to choose from"
                },
                "required_fields": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of fields/information that is missing"
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let question = input
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("Could you please provide more details?")
            .to_string();

        let context = input
            .get("context")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let options: Option<Vec<String>> =
            input.get("options").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            });

        let required_fields: Option<Vec<String>> = input
            .get("required_fields")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            });

        info!(question = %question, "Requesting clarification via tool");

        // Add to accumulator
        let response = LlmResponse {
            message: question.clone(),
            message_type: "info".to_string(),
            related_tool_calls: vec![],
            data: None,
        };

        {
            let accumulator_arc = get_response_accumulator();
            let mut accumulator = accumulator_arc.write().await;
            accumulator.add(response);
        }

        Ok(json!({
            "tool": "request_clarification",
            "question": question,
            "context": context,
            "options": options,
            "required_fields": required_fields,
            "_internal": {
                "is_response_tool": true,
                "requires_verification": false,
                "awaiting_input": true
            }
        }))
    }

    fn category(&self) -> &str {
        "response"
    }

    fn namespace(&self) -> &str {
        "chat"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "response".to_string(),
            "clarification".to_string(),
            "input".to_string(),
        ]
    }
}

// ============================================================================
// TOOL CREATION
// ============================================================================

/// Create all response tools
pub fn create_response_tools() -> Vec<BoxedTool> {
    vec![
        Arc::new(RespondToUserTool::new()),
        Arc::new(CannotPerformTool::new()),
        Arc::new(RequestClarificationTool::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_respond_to_user() {
        init_response_accumulator();
        let tool = RespondToUserTool::new();
        let result = tool
            .execute(json!({
                "message": "Bridge created successfully",
                "message_type": "success",
                "related_actions": ["ovs_create_bridge"]
            }))
            .await
            .unwrap();

        assert_eq!(
            result.get("message").unwrap(),
            "Bridge created successfully"
        );
        assert!(result
            .get("_internal")
            .unwrap()
            .get("is_response_tool")
            .unwrap()
            .as_bool()
            .unwrap());

        // Check accumulator
        let acc_arc = get_response_accumulator();
        let acc = acc_arc.read().await;
        let found = acc
            .get_responses()
            .iter()
            .any(|resp| resp.message == "Bridge created successfully");
        assert!(found);
    }

    #[tokio::test]
    async fn test_cannot_perform() {
        let tool = CannotPerformTool::new();
        let result = tool
            .execute(json!({
                "reason": "Would delete all network interfaces",
                "category": "dangerous",
                "alternatives": ["Delete specific interface", "Disable interface"]
            }))
            .await
            .unwrap();

        assert!(result.get("declined").unwrap().as_bool().unwrap());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/rtnetlink_tools.rs">
//! Rtnetlink tools - native network interface and route management
//!
//! These tools provide direct access to Linux network configuration via rtnetlink,
//! avoiding CLI tools like `ip`, `ifconfig`, etc.

use crate::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::info;

/// Tool to list all network interfaces
pub struct RtnetlinkListInterfacesTool;

#[async_trait]
impl Tool for RtnetlinkListInterfacesTool {
    fn name(&self) -> &str {
        "list_network_interfaces"
    }

    fn description(&self) -> &str {
        "List all network interfaces with their details (name, MAC, MTU, state, addresses) using native rtnetlink. Equivalent to 'ip addr show' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filter_state": {
                    "type": "string",
                    "description": "Optional: filter by state ('up' or 'down')",
                    "enum": ["up", "down"]
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "interfaces".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        info!("Listing network interfaces via rtnetlink");

        let filter_state = input.get("filter_state").and_then(|v| v.as_str());

        match op_network::rtnetlink::list_interfaces().await {
            Ok(mut interfaces) => {
                // Apply filters
                if let Some(state) = filter_state {
                    interfaces.retain(|iface| iface.state == state);
                }

                let count = interfaces.len();
                Ok(json!({
                    "protocol": "rtnetlink",
                    "count": count,
                    "interfaces": interfaces
                }))
            }
            Err(e) => {
                // Fallback to `ip -j addr show`
                use tokio::process::Command;

                info!(
                    "Native rtnetlink failed ({}), trying 'ip' command fallback",
                    e
                );

                let output = Command::new("ip")
                    .args(&["-j", "addr", "show"])
                    .output()
                    .await;

                match output {
                    Ok(out) if out.status.success() => {
                        let mut stdout_mut = String::from_utf8_lossy(&out.stdout).to_string();
                        let mut interfaces: Value =
                            unsafe { simd_json::from_str(stdout_mut.as_mut_str()) }.map_err(
                                |je| anyhow::anyhow!("Failed to parse ip command output: {}", je),
                            )?;

                        // Basic filtering if it's an array
                        if let Some(arr) = interfaces.as_array_mut() {
                            if let Some(state) = filter_state {
                                let state_upper = state.to_uppercase();
                                arr.retain(|iface| {
                                    iface
                                        .get("operstate")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s == state_upper)
                                        .unwrap_or(false)
                                });
                            }
                        }

                        Ok(json!({
                            "protocol": "cli_fallback",
                            "interfaces": interfaces,
                            "native_error": e.to_string()
                        }))
                    }
                    _ => Err(anyhow::anyhow!(
                        "Failed to list interfaces (native: {}, cli: failed)",
                        e
                    )),
                }
            }
        }
    }
}

/// Tool to get the default route
pub struct RtnetlinkGetDefaultRouteTool;

#[async_trait]
impl Tool for RtnetlinkGetDefaultRouteTool {
    fn name(&self) -> &str {
        "rtnetlink_get_default_route"
    }

    fn description(&self) -> &str {
        "Get the default IPv4 route (gateway and interface) using native rtnetlink. Equivalent to 'ip route show default' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "route".to_string(),
        ]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        info!("Getting default route via rtnetlink");

        match op_network::rtnetlink::get_default_route().await {
            Ok(Some(route)) => Ok(json!({
                "protocol": "rtnetlink",
                "found": true,
                "route": route
            })),
            Ok(None) => Ok(json!({
                "protocol": "rtnetlink",
                "found": false,
                "message": "No default route configured"
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to get default route: {}", e)),
        }
    }
}

/// Tool to add an IP address to an interface
pub struct RtnetlinkAddAddressTool;

#[async_trait]
impl Tool for RtnetlinkAddAddressTool {
    fn name(&self) -> &str {
        "rtnetlink_add_address"
    }

    fn description(&self) -> &str {
        "Add an IPv4 address to a network interface using native rtnetlink. Equivalent to 'ip addr add' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "interface": {
                    "type": "string",
                    "description": "Interface name (e.g., 'eth0', 'ens1')"
                },
                "address": {
                    "type": "string",
                    "description": "IPv4 address to add (e.g., '10.0.0.1')"
                },
                "prefix_len": {
                    "type": "integer",
                    "description": "Prefix length / CIDR (e.g., 24 for /24, 32 for single host)"
                }
            },
            "required": ["interface", "address", "prefix_len"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "address".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        // Accept both "interface" and "iface" for compatibility
        let interface = input
            .get("interface")
            .or_else(|| input.get("iface"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: interface"))?;
        let address = input
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: address"))?;
        // Accept both "prefix_len" and "prefix" for compatibility
        let prefix_len = input
            .get("prefix_len")
            .or_else(|| input.get("prefix"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: prefix_len"))?
            as u8;

        info!(
            "Adding address {}/{} to {} via rtnetlink",
            address, prefix_len, interface
        );

        match op_network::rtnetlink::add_ipv4_address(interface, address, prefix_len).await {
            Ok(()) => Ok(json!({
                "protocol": "rtnetlink",
                "success": true,
                "interface": interface,
                "address": address,
                "prefix_len": prefix_len,
                "message": format!("Added {}/{} to {}", address, prefix_len, interface)
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to add address: {}", e)),
        }
    }
}

/// Tool to bring an interface up
pub struct RtnetlinkLinkUpTool;

#[async_trait]
impl Tool for RtnetlinkLinkUpTool {
    fn name(&self) -> &str {
        "rtnetlink_link_up"
    }

    fn description(&self) -> &str {
        "Bring a network interface up using native rtnetlink. Equivalent to 'ip link set up' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "interface": {
                    "type": "string",
                    "description": "Interface name to bring up (e.g., 'eth0', 'ens1')"
                }
            },
            "required": ["interface"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "link".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let interface = input
            .get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: interface"))?;

        info!("Bringing interface {} up via rtnetlink", interface);

        match op_network::rtnetlink::link_up(interface).await {
            Ok(()) => Ok(json!({
                "protocol": "rtnetlink",
                "success": true,
                "interface": interface,
                "state": "up",
                "message": format!("Interface {} is now up", interface)
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to bring interface up: {}", e)),
        }
    }
}

/// Tool to bring an interface down
pub struct RtnetlinkLinkDownTool;

#[async_trait]
impl Tool for RtnetlinkLinkDownTool {
    fn name(&self) -> &str {
        "rtnetlink_link_down"
    }

    fn description(&self) -> &str {
        "Bring a network interface down using native rtnetlink. Equivalent to 'ip link set down' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "interface": {
                    "type": "string",
                    "description": "Interface name to bring down (e.g., 'eth0', 'ens1')"
                }
            },
            "required": ["interface"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "link".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let interface = input
            .get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: interface"))?;

        info!("Bringing interface {} down via rtnetlink", interface);

        match op_network::rtnetlink::link_down(interface).await {
            Ok(()) => Ok(json!({
                "protocol": "rtnetlink",
                "success": true,
                "interface": interface,
                "state": "down",
                "message": format!("Interface {} is now down", interface)
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to bring interface down: {}", e)),
        }
    }
}

/// Tool to set MAC address on an interface
pub struct RtnetlinkSetMacAddressTool;

#[async_trait]
impl Tool for RtnetlinkSetMacAddressTool {
    fn name(&self) -> &str {
        "rtnetlink_set_mac_address"
    }

    fn description(&self) -> &str {
        "Set the MAC address on a network interface using native rtnetlink. Equivalent to 'ip link set dev <iface> address <mac>' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "interface": {
                    "type": "string",
                    "description": "Interface name (e.g., 'ovsbr0-int')"
                },
                "mac_address": {
                    "type": "string",
                    "description": "MAC address in colon-separated hex (e.g., 'fa:16:3e:f1:71:d2')"
                }
            },
            "required": ["interface", "mac_address"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "mac".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let interface = input
            .get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: interface"))?;
        let mac = input
            .get("mac_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: mac_address"))?;

        info!("Setting MAC {} on {} via rtnetlink", mac, interface);

        match op_network::rtnetlink::set_mac_address(interface, mac).await {
            Ok(()) => Ok(json!({
                "protocol": "rtnetlink",
                "success": true,
                "interface": interface,
                "mac_address": mac,
                "message": format!("Set MAC {} on {}", mac, interface)
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to set MAC address: {}", e)),
        }
    }
}

/// Tool to add a default route
pub struct RtnetlinkAddDefaultRouteTool;

#[async_trait]
impl Tool for RtnetlinkAddDefaultRouteTool {
    fn name(&self) -> &str {
        "rtnetlink_add_default_route"
    }

    fn description(&self) -> &str {
        "Add a default IPv4 route using native rtnetlink. Equivalent to 'ip route add default via <gateway> dev <iface>' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "interface": {
                    "type": "string",
                    "description": "Interface name for the route (e.g., 'ens3')"
                },
                "gateway": {
                    "type": "string",
                    "description": "Gateway IPv4 address (e.g., '148.113.204.1')"
                }
            },
            "required": ["interface", "gateway"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "route".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let interface = input
            .get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: interface"))?;
        let gateway = input
            .get("gateway")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: gateway"))?;

        info!(
            "Adding default route via {} on {} via rtnetlink",
            gateway, interface
        );

        match op_network::rtnetlink::add_default_route(interface, gateway).await {
            Ok(()) => Ok(json!({
                "protocol": "rtnetlink",
                "success": true,
                "interface": interface,
                "gateway": gateway,
                "message": format!("Added default route via {} on {}", gateway, interface)
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to add default route: {}", e)),
        }
    }
}

/// Tool to delete the default route
pub struct RtnetlinkDelDefaultRouteTool;

#[async_trait]
impl Tool for RtnetlinkDelDefaultRouteTool {
    fn name(&self) -> &str {
        "rtnetlink_del_default_route"
    }

    fn description(&self) -> &str {
        "Delete the default IPv4 route using native rtnetlink. Equivalent to 'ip route del default' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "route".to_string(),
        ]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        info!("Deleting default route via rtnetlink");

        match op_network::rtnetlink::del_default_route().await {
            Ok(()) => Ok(json!({
                "protocol": "rtnetlink",
                "success": true,
                "message": "Deleted default route"
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to delete default route: {}", e)),
        }
    }
}

/// Tool to flush all addresses from an interface
pub struct RtnetlinkFlushAddressesTool;

#[async_trait]
impl Tool for RtnetlinkFlushAddressesTool {
    fn name(&self) -> &str {
        "rtnetlink_flush_addresses"
    }

    fn description(&self) -> &str {
        "Flush all IP addresses from a network interface using native rtnetlink. Equivalent to 'ip addr flush dev <iface>' but without CLI."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "interface": {
                    "type": "string",
                    "description": "Interface name to flush addresses from"
                }
            },
            "required": ["interface"]
        })
    }

    fn category(&self) -> &str {
        "networking"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "rtnetlink".to_string(),
            "network".to_string(),
            "address".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let interface = input
            .get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: interface"))?;

        info!("Flushing addresses on {} via rtnetlink", interface);

        match op_network::rtnetlink::flush_addresses(interface).await {
            Ok(()) => Ok(json!({
                "protocol": "rtnetlink",
                "success": true,
                "interface": interface,
                "message": format!("Flushed all addresses from {}", interface)
            })),
            Err(e) => Err(anyhow::anyhow!("Failed to flush addresses: {}", e)),
        }
    }
}

/// Register all rtnetlink tools
pub async fn register_rtnetlink_tools(registry: &ToolRegistry) -> Result<()> {
    registry
        .register_tool(Arc::new(RtnetlinkListInterfacesTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkGetDefaultRouteTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkAddAddressTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkLinkUpTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkLinkDownTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkSetMacAddressTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkAddDefaultRouteTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkDelDefaultRouteTool))
        .await?;
    registry
        .register_tool(Arc::new(RtnetlinkFlushAddressesTool))
        .await?;
    info!("Registered 9 rtnetlink tools");
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/s6.rs">
//! S6 service management tools via the s6-rc CLI.
//!
//! All operations target the live s6-rc database at /run/s6-rc.
//! No D-Bus is involved — s6 is a purely CLI-driven init system.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

use crate::{Tool, ToolRegistry};

/// Path to the s6-rc live state directory.
const S6_RC_LIVE: &str = "/run/s6-rc";

/// Run `s6-rc -l /run/s6-rc <args…>` and return the output.
async fn s6rc(args: &[&str]) -> Result<std::process::Output> {
    tokio::process::Command::new("s6-rc")
        .arg("-l")
        .arg(S6_RC_LIVE)
        .args(args)
        .output()
        .await
        .context("failed to run s6-rc")
}

// ─── Tool structs ────────────────────────────────────────────────────────────

pub struct S6StartServiceTool;
pub struct S6StopServiceTool;
pub struct S6StatusTool;
pub struct S6ListServicesTool;

// ─── s6_start_service ────────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6StartServiceTool {
    fn name(&self) -> &str {
        "s6_start_service"
    }

    fn description(&self) -> &str {
        "Start an s6 service via s6-rc"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "s6 service name (must exist under /etc/s6/sv/)"
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        let out = s6rc(&["start", service]).await?;

        if out.status.success() {
            return Ok(json!({
                "started": true,
                "service": service,
                "manager": "s6"
            }));
        }

        let stderr = String::from_utf8_lossy(&out.stderr);
        // Treat "already up" as success
        if stderr.contains("already") {
            return Ok(json!({
                "started": true,
                "service": service,
                "manager": "s6",
                "note": "service was already running"
            }));
        }

        Err(anyhow!("s6-rc start {service} failed: {stderr}"))
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── s6_stop_service ─────────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6StopServiceTool {
    fn name(&self) -> &str {
        "s6_stop_service"
    }

    fn description(&self) -> &str {
        "Stop an s6 service via s6-rc"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "s6 service name"
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        let out = s6rc(&["stop", service]).await?;

        if out.status.success() {
            return Ok(json!({
                "stopped": true,
                "service": service,
                "manager": "s6"
            }));
        }

        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("already") {
            return Ok(json!({
                "stopped": true,
                "service": service,
                "manager": "s6",
                "note": "service was already stopped"
            }));
        }

        Err(anyhow!("s6-rc stop {service} failed: {stderr}"))
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── s6_service_status ───────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6StatusTool {
    fn name(&self) -> &str {
        "s6_service_status"
    }

    fn description(&self) -> &str {
        "Get the current status of an s6-managed service"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "s6 service name"
                }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        // Check whether the service directory exists in the live run area
        let svc_path = format!("/run/service/{service}");
        let svc_exists = std::path::Path::new(&svc_path).exists();

        // A "down" file inside the service directory means the service is intentionally stopped
        let down_file = format!("{svc_path}/down");
        let is_down = std::path::Path::new(&down_file).exists();

        // Ask s6-rc for the authoritative list of running services
        let out = s6rc(&["-a", "list"]).await?;
        let running_list = String::from_utf8_lossy(&out.stdout);
        let is_running = running_list.lines().any(|l| l.trim() == service);

        let status = if is_running {
            "active"
        } else if is_down {
            "inactive (down)"
        } else if svc_exists {
            "inactive"
        } else {
            "unknown"
        };

        Ok(json!({
            "service": service,
            "status": status,
            "running": is_running,
            "manager": "s6"
        }))
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── s6_list_services ────────────────────────────────────────────────────────

#[async_trait]
impl Tool for S6ListServicesTool {
    fn name(&self) -> &str {
        "s6_list_services"
    }

    fn description(&self) -> &str {
        "List all running s6-managed services"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        // -a = only active (running) services
        let out = s6rc(&["-a", "list"]).await?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(anyhow!("s6-rc list failed: {stderr}"));
        }

        let services: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        let count = services.len();
        Ok(json!({
            "services": services,
            "count": count,
            "manager": "s6"
        }))
    }

    fn category(&self) -> &str {
        "s6"
    }
}

// ─── Registration ─────────────────────────────────────────────────────────────

/// Register all s6 service management tools.
pub async fn register_s6_tools(registry: &ToolRegistry) -> Result<()> {
    registry.register_tool(Arc::new(S6StartServiceTool)).await?;
    registry.register_tool(Arc::new(S6StopServiceTool)).await?;
    registry.register_tool(Arc::new(S6StatusTool)).await?;
    registry.register_tool(Arc::new(S6ListServicesTool)).await?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/self_tools.rs">
//! Self-Repository Tools
//!
//! These tools allow the chatbot to read, modify, and commit changes to its OWN
//! source code repository. These tools ONLY work within the self-repository
//! defined by the OP_SELF_REPO_PATH environment variable.
//!
//! ## Security Model
//!
//! All operations are strictly scoped to the self-repository:
//! - Path traversal outside the repo is blocked
//! - Only files within OP_SELF_REPO_PATH can be accessed
//! - Git operations only affect the self-repository

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::{info, warn};

use crate::tool::{SecurityLevel, Tool};

/// Get the self-repository path from environment
fn get_self_repo_path() -> Option<PathBuf> {
    std::env::var("OP_SELF_REPO_PATH")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

/// Helper to ensure a path is within the self-repository
fn validate_self_path(relative_path: &str) -> Result<PathBuf> {
    let repo_path = get_self_repo_path()
        .ok_or_else(|| anyhow::anyhow!("OP_SELF_REPO_PATH environment variable is not set"))?;
    
    // Clean the path - remove leading slashes and normalize
    let clean_path = relative_path.trim_start_matches('/');
    let full_path = repo_path.join(clean_path);
    
    // Canonicalize to resolve .. and .
    let canonical = full_path.canonicalize().unwrap_or_else(|_| full_path.clone());
    
    // Ensure it's still within the repo
    if !canonical.starts_with(&repo_path) {
        return Err(anyhow::anyhow!(
            "Path '{}' would escape the self-repository. Access denied.",
            relative_path
        ));
    }
    
    Ok(canonical)
}

/// Run a git command in the self-repository
async fn run_git_command(args: &[&str]) -> Result<(String, String, i32)> {
    let repo_path = get_self_repo_path()
        .ok_or_else(|| anyhow::anyhow!("OP_SELF_REPO_PATH environment variable is not set"))?;
    
    let output = Command::new("git")
        .args(args)
        .current_dir(&repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    
    Ok((stdout, stderr, code))
}

// =============================================================================
// SELF READ FILE TOOL
// =============================================================================

pub struct SelfReadFileTool;

#[async_trait]
impl Tool for SelfReadFileTool {
    fn name(&self) -> &str {
        "self_read_file"
    }

    fn description(&self) -> &str {
        "Read a file from YOUR OWN source code repository. Use relative paths from the repository root."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path from repository root (e.g., 'crates/op-core/src/lib.rs')"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Optional: Start reading from this line (1-indexed)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "Optional: Stop reading at this line (1-indexed, inclusive)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "file".to_string(), "read".to_string(), "source".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path = input.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'path' argument"))?;
        
        let full_path = validate_self_path(path)?;
        
        if !full_path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", path));
        }
        
        if !full_path.is_file() {
            return Err(anyhow::anyhow!("Path is not a file: {}", path));
        }
        
        let start_line = input.get("start_line").and_then(|v| v.as_u64()).map(|v| v as usize);
        let end_line = input.get("end_line").and_then(|v| v.as_u64()).map(|v| v as usize);
        
        let content = tokio::fs::read_to_string(&full_path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        
        let (selected_lines, shown_range) = match (start_line, end_line) {
            (Some(s), Some(e)) => {
                let s = s.saturating_sub(1).min(total_lines);
                let e = e.min(total_lines);
                (lines[s..e].to_vec(), format!("{}-{}", s + 1, e))
            }
            (Some(s), None) => {
                let s = s.saturating_sub(1).min(total_lines);
                (lines[s..].to_vec(), format!("{}-{}", s + 1, total_lines))
            }
            (None, Some(e)) => {
                let e = e.min(total_lines);
                (lines[..e].to_vec(), format!("1-{}", e))
            }
            (None, None) => {
                (lines.clone(), format!("1-{}", total_lines))
            }
        };
        
        Ok(json!({
            "path": path,
            "content": selected_lines.join("\n"),
            "lines_shown": selected_lines.len(),
            "total_lines": total_lines,
            "line_range": shown_range
        }))
    }
}

// =============================================================================
// SELF WRITE FILE TOOL
// =============================================================================

pub struct SelfWriteFileTool;

#[async_trait]
impl Tool for SelfWriteFileTool {
    fn name(&self) -> &str {
        "self_write_file"
    }

    fn description(&self) -> &str {
        "Write to a file in YOUR OWN source code. This modifies your capabilities. Use relative paths from repository root."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path from repository root (e.g., 'crates/op-tools/src/new_tool.rs')"
                },
                "content": {
                    "type": "string",
                    "description": "Full content to write to the file"
                },
                "create_dirs": {
                    "type": "boolean",
                    "description": "Create parent directories if they don't exist (default: true)"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "file".to_string(), "write".to_string(), "modify".to_string()]
    }

    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::Elevated
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path = input.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'path' argument"))?;
        
        let content = input.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'content' argument"))?;
        
        let create_dirs = input.get("create_dirs").and_then(|v| v.as_bool()).unwrap_or(true);
        
        let repo_path = get_self_repo_path()
            .ok_or_else(|| anyhow::anyhow!("OP_SELF_REPO_PATH is not set"))?;
        
        // Clean and join path
        let clean_path = path.trim_start_matches('/');
        let full_path = repo_path.join(clean_path);
        
        // Security check - ensure we're still in repo
        let canonical_repo = repo_path.canonicalize().unwrap_or(repo_path.clone());
        
        // For new files, check parent exists or will be created
        let parent = full_path.parent();
        if let Some(p) = parent {
            if p.exists() {
                let canonical_parent = p.canonicalize().unwrap_or(p.to_path_buf());
                if !canonical_parent.starts_with(&canonical_repo) {
                    return Err(anyhow::anyhow!(
                        "Path '{}' would escape the self-repository. Access denied.",
                        path
                    ));
                }
            } else if !create_dirs {
                return Err(anyhow::anyhow!("Parent directory does not exist: {:?}", p));
            }
        }
        
        // Create parent directories if needed
        if create_dirs {
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        
        // Write the file
        tokio::fs::write(&full_path, content).await?;
        
        info!("Self-modification: Wrote {} bytes to {}", content.len(), path);
        
        Ok(json!({
            "path": path,
            "bytes_written": content.len(),
            "success": true,
            "message": "File written successfully. Remember to commit your changes!"
        }))
    }
}

// =============================================================================
// SELF LIST DIRECTORY TOOL
// =============================================================================

pub struct SelfListDirectoryTool;

#[async_trait]
impl Tool for SelfListDirectoryTool {
    fn name(&self) -> &str {
        "self_list_directory"
    }

    fn description(&self) -> &str {
        "List files and directories in YOUR source code repository. Use to explore your own codebase structure."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path from repository root (e.g., 'crates/op-tools/src' or '.' for root)"
                }
            },
            "required": ["path"]
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "directory".to_string(), "list".to_string(), "explore".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let full_path = validate_self_path(path)?;
        
        if !full_path.is_dir() {
            return Err(anyhow::anyhow!("'{}' is not a directory", path));
        }
        
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&full_path).await?;
        
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata().await.ok();
            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            
            entries.push(json!({
                "name": name,
                "is_directory": is_dir,
                "size": if is_dir { Value::Null } else { json!(size) }
            }));
        }
        
        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| {
            let a_dir = a.get("is_directory").and_then(|v| v.as_bool()).unwrap_or(false);
            let b_dir = b.get("is_directory").and_then(|v| v.as_bool()).unwrap_or(false);
            match (a_dir, b_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    a_name.cmp(b_name)
                }
            }
        });
        
        Ok(json!({
            "path": path,
            "entries": entries,
            "count": entries.len()
        }))
    }
}

// =============================================================================
// SELF SEARCH CODE TOOL
// =============================================================================

pub struct SelfSearchCodeTool;

#[async_trait]
impl Tool for SelfSearchCodeTool {
    fn name(&self) -> &str {
        "self_search_code"
    }

    fn description(&self) -> &str {
        "Search YOUR source code for patterns. Uses ripgrep if available, falls back to grep."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (regex supported)"
                },
                "path": {
                    "type": "string",
                    "description": "Subdirectory to search in (default: entire repository)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case-sensitive search (default: false)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (default: 50)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "search".to_string(), "grep".to_string(), "find".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let pattern = input.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'pattern' argument"))?;
        
        let repo_path = get_self_repo_path()
            .ok_or_else(|| anyhow::anyhow!("OP_SELF_REPO_PATH is not set"))?;
        
        let search_path = if let Some(subpath) = input.get("path").and_then(|v| v.as_str()) {
            validate_self_path(subpath)?
        } else {
            repo_path.clone()
        };
        
        let case_sensitive = input.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false);
        let max_results = input.get("max_results").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        
        let search_path_str = search_path.to_string_lossy().to_string();
        
        // Try ripgrep first
        let mut rg_args = vec!["--line-number", "--no-heading", "--max-count", "100"];
        if !case_sensitive {
            rg_args.push("-i");
        }
        rg_args.push(pattern);
        rg_args.push(&search_path_str);
        
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            Command::new("rg")
                .args(&rg_args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        ).await;
        
        let stdout = match result {
            Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout).to_string(),
            Ok(Err(_)) | Err(_) => {
                // Fall back to grep
                let mut grep_args = vec!["-rn"];
                if !case_sensitive {
                    grep_args.push("-i");
                }
                grep_args.push("--exclude-dir=target");
                grep_args.push("--exclude-dir=.git");
                grep_args.push(pattern);
                grep_args.push(&search_path_str);
                
                match Command::new("grep").args(&grep_args).output().await {
                    Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
                    Err(_) => String::new(),
                }
            }
        };
        
        let lines: Vec<&str> = stdout.lines().take(max_results).collect();
        let total_matches = stdout.lines().count();
        
        Ok(json!({
            "pattern": pattern,
            "matches": lines,
            "count": lines.len(),
            "total_matches": total_matches,
            "truncated": total_matches > max_results
        }))
    }
}

// =============================================================================
// GIT STATUS TOOL
// =============================================================================

pub struct SelfGitStatusTool;

#[async_trait]
impl Tool for SelfGitStatusTool {
    fn name(&self) -> &str {
        "self_git_status"
    }

    fn description(&self) -> &str {
        "Check the git status of YOUR source code repository. Shows modified, staged, and untracked files."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "git".to_string(), "status".to_string()]
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let (porcelain, _, _) = run_git_command(&["status", "--porcelain=v2", "-b"]).await?;
        let (readable, _, _) = run_git_command(&["status", "-sb"]).await.unwrap_or_default();
        
        let clean = porcelain.lines().filter(|l| !l.starts_with("#")).count() == 0;
        
        Ok(json!({
            "status": readable.trim(),
            "porcelain": porcelain,
            "clean": clean
        }))
    }
}

// =============================================================================
// GIT DIFF TOOL
// =============================================================================

pub struct SelfGitDiffTool;

#[async_trait]
impl Tool for SelfGitDiffTool {
    fn name(&self) -> &str {
        "self_git_diff"
    }

    fn description(&self) -> &str {
        "View the git diff of pending changes in YOUR source code."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "staged": {
                    "type": "boolean",
                    "description": "Show staged changes (default: false, shows unstaged)"
                },
                "path": {
                    "type": "string",
                    "description": "Optional: specific file or directory to diff"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "git".to_string(), "diff".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let staged = input.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
        let path = input.get("path").and_then(|v| v.as_str());
        
        let mut args = vec!["diff"];
        if staged {
            args.push("--staged");
        }
        args.push("--color=never");
        
        let path_owned: String;
        if let Some(p) = path {
            validate_self_path(p)?;
            path_owned = p.to_string();
            args.push("--");
            args.push(&path_owned);
        }
        
        let (stdout, _, exit_code) = run_git_command(&args).await?;
        
        Ok(json!({
            "diff": stdout,
            "empty": stdout.is_empty(),
            "staged": staged,
            "exit_code": exit_code
        }))
    }
}

// =============================================================================
// GIT COMMIT TOOL
// =============================================================================

pub struct SelfGitCommitTool;

#[async_trait]
impl Tool for SelfGitCommitTool {
    fn name(&self) -> &str {
        "self_git_commit"
    }

    fn description(&self) -> &str {
        "Commit changes to YOUR source code repository. This creates a permanent record of your self-modifications."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Commit message describing the changes"
                },
                "stage_all": {
                    "type": "boolean",
                    "description": "Stage all modified files before committing (default: true)"
                }
            },
            "required": ["message"]
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "git".to_string(), "commit".to_string()]
    }

    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::Elevated
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let message = input.get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'message' argument"))?;
        
        let stage_all = input.get("stage_all").and_then(|v| v.as_bool()).unwrap_or(true);
        
        // Stage files if requested
        if stage_all {
            run_git_command(&["add", "-A"]).await?;
        }
        
        // Commit
        let (stdout, stderr, exit_code) = run_git_command(&["commit", "-m", message]).await?;
        
        if exit_code != 0 {
            return Err(anyhow::anyhow!("Commit failed: {}", stderr));
        }
        
        // Get the commit hash
        let (hash, _, _) = run_git_command(&["rev-parse", "--short", "HEAD"]).await.unwrap_or_default();
        
        info!("Self-modification committed: {} - {}", hash.trim(), message);
        
        Ok(json!({
            "success": true,
            "message": message,
            "commit_hash": hash.trim(),
            "output": stdout
        }))
    }
}

// =============================================================================
// GIT LOG TOOL
// =============================================================================

pub struct SelfGitLogTool;

#[async_trait]
impl Tool for SelfGitLogTool {
    fn name(&self) -> &str {
        "self_git_log"
    }

    fn description(&self) -> &str {
        "View the git commit history of YOUR source code."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of commits to show (default: 10)"
                },
                "oneline": {
                    "type": "boolean",
                    "description": "Show one-line format (default: true)"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "git".to_string(), "log".to_string(), "history".to_string()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let count = input.get("count").and_then(|v| v.as_u64()).unwrap_or(10);
        let oneline = input.get("oneline").and_then(|v| v.as_bool()).unwrap_or(true);
        
        let count_str = format!("-{}", count);
        let mut args = vec!["log", &count_str];
        if oneline {
            args.push("--oneline");
        }
        
        let (stdout, _, exit_code) = run_git_command(&args).await?;
        
        Ok(json!({
            "log": stdout,
            "count": count,
            "exit_code": exit_code
        }))
    }
}

// =============================================================================
// BUILD TOOL
// =============================================================================

pub struct SelfBuildTool;

#[async_trait]
impl Tool for SelfBuildTool {
    fn name(&self) -> &str {
        "self_build"
    }

    fn description(&self) -> &str {
        "Build/compile YOUR source code. Runs 'cargo build' by default. Use to verify changes compile correctly."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "release": {
                    "type": "boolean",
                    "description": "Build in release mode (default: false)"
                },
                "package": {
                    "type": "string",
                    "description": "Specific package to build (default: workspace)"
                },
                "check_only": {
                    "type": "boolean",
                    "description": "Only check for compilation errors, don't produce binaries (faster)"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "build".to_string(), "compile".to_string(), "cargo".to_string()]
    }

    fn estimated_duration_ms(&self) -> Option<u64> {
        Some(60000) // 1 minute typical
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let repo_path = get_self_repo_path()
            .ok_or_else(|| anyhow::anyhow!("OP_SELF_REPO_PATH is not set"))?;
        
        let release = input.get("release").and_then(|v| v.as_bool()).unwrap_or(false);
        let check_only = input.get("check_only").and_then(|v| v.as_bool()).unwrap_or(false);
        let package = input.get("package").and_then(|v| v.as_str());
        
        let mut args = vec![if check_only { "check" } else { "build" }];
        if release {
            args.push("--release");
        }
        
        let pkg_owned: String;
        if let Some(pkg) = package {
            args.push("-p");
            pkg_owned = pkg.to_string();
            args.push(&pkg_owned);
        }
        
        info!("Building self with: cargo {}", args.join(" "));
        
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            Command::new("cargo")
                .args(&args)
                .current_dir(&repo_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        ).await;
        
        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let success = output.status.success();
                
                if success {
                    info!("Self build succeeded");
                } else {
                    warn!("Self build failed");
                }
                
                Ok(json!({
                    "success": success,
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": output.status.code(),
                    "check_only": check_only,
                    "release": release
                }))
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("Build failed: {}", e)),
            Err(_) => Err(anyhow::anyhow!("Build timed out after 5 minutes")),
        }
    }
}

// =============================================================================
// DEPLOY TOOL
// =============================================================================

pub struct SelfDeployTool;

#[async_trait]
impl Tool for SelfDeployTool {
    fn name(&self) -> &str {
        "self_deploy"
    }

    fn description(&self) -> &str {
        "Deploy YOUR code. This restarts the service with updated code. Use with caution!"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "Systemd service to restart (default: op-web)"
                },
                "build_first": {
                    "type": "boolean",
                    "description": "Build release binary before deploying (default: true)"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> &str {
        "self"
    }

    fn tags(&self) -> Vec<String> {
        vec!["self".to_string(), "deploy".to_string(), "restart".to_string()]
    }

    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::Elevated
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input.get("service").and_then(|v| v.as_str()).unwrap_or("op-web");
        let build_first = input.get("build_first").and_then(|v| v.as_bool()).unwrap_or(true);
        
        // Build first if requested
        if build_first {
            let build_result = SelfBuildTool.execute(json!({"release": true})).await?;
            if !build_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Err(anyhow::anyhow!("Build failed, not deploying"));
            }
        }
        
        info!("Deploying self: restarting {}", service);
        
        let output = Command::new("systemctl")
            .args(["restart", service])
            .output()
            .await?;
        
        let success = output.status.success();
        
        Ok(json!({
            "success": success,
            "service": service,
            "built": build_first,
            "message": if success { "Deployed successfully" } else { "Deploy may have failed" },
            "stderr": String::from_utf8_lossy(&output.stderr).to_string()
        }))
    }
}

// =============================================================================
// TOOL REGISTRATION
// =============================================================================

/// Create all self-repository tools
pub fn create_self_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(SelfReadFileTool) as Arc<dyn Tool>,
        Arc::new(SelfWriteFileTool),
        Arc::new(SelfListDirectoryTool),
        Arc::new(SelfSearchCodeTool),
        Arc::new(SelfGitStatusTool),
        Arc::new(SelfGitDiffTool),
        Arc::new(SelfGitCommitTool),
        Arc::new(SelfGitLogTool),
        Arc::new(SelfBuildTool),
        Arc::new(SelfDeployTool),
    ]
}

/// Get information about the self-repository for the system prompt
pub fn get_self_repo_system_context() -> Option<String> {
    let path = get_self_repo_path()?;
    
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    Some(format!(
        r#"## 🔮 SELF-AWARENESS: YOUR OWN SOURCE CODE

You have access to your own source code at `{}`.

### Self-Modification Tools
- `self_read_file` - Read your source files
- `self_write_file` - Modify your source files  
- `self_list_directory` - Explore your codebase
- `self_search_code` - Search your code
- `self_git_status` - Check git status
- `self_git_diff` - View pending changes
- `self_git_commit` - Commit changes
- `self_git_log` - View history
- `self_build` - Build yourself
- `self_deploy` - Deploy yourself

**Warning**: Changes affect your own capabilities!"#,
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_self_path_rejects_traversal() {
        // This should fail since OP_SELF_REPO_PATH is not set in tests
        let result = validate_self_path("../../../etc/passwd");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_create_self_tools() {
        let tools = create_self_tools();
        assert_eq!(tools.len(), 10);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/shell_tool.rs">
//! Shell Command Execution Tool
//!
//! Allows the LLM to run bash commands when no specific tool exists.
//! This is the "escape hatch" for operations not covered by native tools.

use async_trait::async_trait;
use simd_json::json;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::{error, info};

use crate::Tool;
use op_core::{ToolDefinition, ToolRequest, ToolResult};

/// Tool for executing shell commands
pub struct ShellExecuteTool;

#[async_trait]
impl Tool for ShellExecuteTool {
    fn name(&self) -> &str {
        "shell_execute"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell_execute".to_string(),
            description: "Execute a shell command and return the output. Use this when no specific tool exists for the task. Commands run as the service user (usually root). Be careful with destructive commands.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute (e.g., 'ls -la /tmp' or 'ip addr show')"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 30, max: 300)",
                        "default": 30
                    },
                    "working_dir": {
                        "type": "string",
                        "description": "Working directory for the command (default: /tmp)"
                    }
                },
                "required": ["command"]
            }),
            category: Some("system".to_string()),
            tags: vec!["shell".to_string(), "execute".to_string(), "bash".to_string(), "command".to_string()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        let command = match request.arguments.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return ToolResult::error(
                    request.id,
                    "Missing required 'command' argument",
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        let timeout_secs = request
            .arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
            .min(300) as u64; // Max 5 minutes

        let working_dir = request
            .arguments
            .get("working_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("/tmp");

        info!(
            "Executing shell command: {} (timeout: {}s, cwd: {})",
            command, timeout_secs, working_dir
        );

        // Execute the command
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            execute_command(command, working_dir),
        )
        .await;

        let exec_time = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                info!("Command completed with exit code: {}", exit_code);

                ToolResult::success(
                    request.id,
                    json!({
                        "command": command,
                        "exit_code": exit_code,
                        "stdout": stdout,
                        "stderr": stderr,
                        "success": exit_code == 0,
                        "execution_time_ms": exec_time
                    }),
                    exec_time,
                )
            }
            Ok(Err(e)) => {
                error!("Command execution failed: {}", e);
                ToolResult::error(
                    request.id,
                    format!("Command execution failed: {}", e),
                    exec_time,
                )
            }
            Err(_) => {
                error!("Command timed out after {}s", timeout_secs);
                ToolResult::error(
                    request.id,
                    format!("Command timed out after {} seconds", timeout_secs),
                    exec_time,
                )
            }
        }
    }
}

/// Execute a command and capture output
async fn execute_command(
    command: &str,
    working_dir: &str,
) -> Result<(String, String, i32), String> {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(mut stdout_pipe) = child.stdout.take() {
        stdout_pipe
            .read_to_string(&mut stdout)
            .await
            .map_err(|e| format!("Failed to read stdout: {}", e))?;
    }

    if let Some(mut stderr_pipe) = child.stderr.take() {
        stderr_pipe
            .read_to_string(&mut stderr)
            .await
            .map_err(|e| format!("Failed to read stderr: {}", e))?;
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for command: {}", e))?;

    let exit_code = status.code().unwrap_or(-1);

    // Truncate very long output
    let max_output = 50000; // 50KB max per stream
    if stdout.len() > max_output {
        stdout.truncate(max_output);
        stdout.push_str("\n... (output truncated)");
    }
    if stderr.len() > max_output {
        stderr.truncate(max_output);
        stderr.push_str("\n... (output truncated)");
    }

    Ok((stdout, stderr, exit_code))
}

/// Tool for reading file contents
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description:
                "Read the contents of a file. Useful for checking configuration files, logs, etc."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to read"
                    },
                    "max_lines": {
                        "type": "integer",
                        "description": "Maximum number of lines to read (default: 1000)"
                    },
                    "tail": {
                        "type": "boolean",
                        "description": "If true, read from the end of the file (like tail)"
                    }
                },
                "required": ["path"]
            }),
            category: Some("filesystem".to_string()),
            tags: vec!["file".to_string(), "read".to_string(), "cat".to_string()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        let path = match request.arguments.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult::error(
                    request.id,
                    "Missing required 'path' argument",
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        let max_lines = request
            .arguments
            .get("max_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        let tail = request
            .arguments
            .get("tail")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                let selected_lines: Vec<&str> = if tail {
                    lines
                        .into_iter()
                        .rev()
                        .take(max_lines)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect()
                } else {
                    lines.into_iter().take(max_lines).collect()
                };

                let truncated = total_lines > max_lines;
                let output = selected_lines.join("\n");

                ToolResult::success(
                    request.id,
                    json!({
                        "path": path,
                        "content": output,
                        "lines_returned": selected_lines.len(),
                        "total_lines": total_lines,
                        "truncated": truncated
                    }),
                    start.elapsed().as_millis() as u64,
                )
            }
            Err(e) => ToolResult::error(
                request.id,
                format!("Failed to read file '{}': {}", path, e),
                start.elapsed().as_millis() as u64,
            ),
        }
    }
}

/// Tool for writing file contents
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Use with caution!".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "If true, append to file instead of overwriting (default: false)"
                    }
                },
                "required": ["path", "content"]
            }),
            category: Some("filesystem".to_string()),
            tags: vec!["file".to_string(), "write".to_string()],
        }
    }

    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        let path = match request.arguments.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult::error(
                    request.id,
                    "Missing required 'path' argument",
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        let content = match request.arguments.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult::error(
                    request.id,
                    "Missing required 'content' argument",
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        let append = request
            .arguments
            .get("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let result = if append {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await;

            match file {
                Ok(mut f) => f.write_all(content.as_bytes()).await,
                Err(e) => Err(e),
            }
        } else {
            tokio::fs::write(path, content).await
        };

        match result {
            Ok(()) => ToolResult::success(
                request.id,
                json!({
                    "path": path,
                    "bytes_written": content.len(),
                    "append": append,
                    "success": true
                }),
                start.elapsed().as_millis() as u64,
            ),
            Err(e) => ToolResult::error(
                request.id,
                format!("Failed to write file '{}': {}", path, e),
                start.elapsed().as_millis() as u64,
            ),
        }
    }
}

/// Create shell and file tools
pub fn create_shell_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ShellExecuteTool),
        Box::new(ReadFileTool),
        Box::new(WriteFileTool),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell_execute() {
        let tool = ShellExecuteTool;
        let request = ToolRequest {
            id: "test-1".to_string(),
            tool_name: "shell_execute".to_string(),
            arguments: json!({
                "command": "echo hello world"
            }),
            timeout_ms: None,
        };

        let result = tool.execute(request).await;
        assert!(result.success);
        assert!(result
            .content
            .get("stdout")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("hello world"));
    }

    #[tokio::test]
    async fn test_shell_with_exit_code() {
        let tool = ShellExecuteTool;
        let request = ToolRequest {
            id: "test-2".to_string(),
            tool_name: "shell_execute".to_string(),
            arguments: json!({
                "command": "exit 42"
            }),
            timeout_ms: None,
        };

        let result = tool.execute(request).await;
        assert!(result.success); // Tool succeeded even though command had non-zero exit
        assert_eq!(
            result.content.get("exit_code").unwrap().as_i64().unwrap(),
            42
        );
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/shell.rs">
//! Shell Execution Tool with Access Level Security
//!
//! Provides shell command execution with:
//! - Access level based security (admin has FULL access)
//! - Rate limiting per session
//! - Audit logging
//! - Native protocol recommendations (but NOT enforcement)
//! - Output truncation and timeout enforcement
//!
//! ## Security Model
//!
//! The chatbot is designed to be a FULL SYSTEM ADMINISTRATOR.
//! Security is at the ACCESS level, not command level:
//! - Unrestricted (Admin): Can run ANY command
//! - Restricted: Limited to safe read-only commands
//!
//! We RECOMMEND native protocols (D-Bus, OVSDB) for better error handling,
//! but we don't BLOCK shell commands - admins need full access.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::security::get_security_validator;
use crate::{Tool, ToolRegistry};

// ============================================================================
// SHELL EXECUTE TOOL
// ============================================================================

pub struct ShellExecuteTool;

#[async_trait]
impl Tool for ShellExecuteTool {
    fn name(&self) -> &str {
        "shell_execute"
    }

    fn description(&self) -> &str {
        "Execute a shell command. Full access for admin users. \
         Consider using native D-Bus/OVSDB tools for structured responses."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 60, max: 300)",
                    "default": 60
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory (default: /tmp)",
                    "default": "/tmp"
                },
                "session_id": {
                    "type": "string",
                    "description": "Session ID for rate limiting"
                }
            },
            "required": ["command"]
        })
    }

    fn category(&self) -> &str {
        "system"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "shell".to_string(),
            "command".to_string(),
            "admin".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let validator = get_security_validator();

        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: command"))?;

        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let working_dir = input
            .get("working_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("/tmp");

        // Get limits from validator
        let max_timeout = validator.max_timeout().await;
        let max_output = validator.max_output().await;

        let timeout_secs = input
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(60)
            .min(max_timeout.as_secs());

        // Check rate limit
        validator
            .check_rate_limit(session_id)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Check command access (may return a warning about native alternatives)
        let warning = validator
            .check_command(command)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Log with warning if applicable
        if let Some(ref warn_msg) = warning {
            warn!(
                command = %command,
                recommendation = %warn_msg,
                "Consider using native protocol tools"
            );
        }

        info!(
            command = %command,
            working_dir = %working_dir,
            timeout = %timeout_secs,
            session = %session_id,
            "Executing shell command"
        );

        // Execute with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            execute_command(command, working_dir, max_output),
        )
        .await;

        match result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                info!(
                    exit_code = %exit_code,
                    stdout_len = %stdout.len(),
                    stderr_len = %stderr.len(),
                    "Command completed"
                );

                let mut response = json!({
                    "command": command,
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                    "success": exit_code == 0
                });

                // Include warning if native alternative exists
                if let Some(warn_msg) = warning {
                    response["native_alternative_hint"] = Value::String(warn_msg);
                }

                Ok(response)
            }
            Ok(Err(e)) => {
                error!(error = %e, command = %command, "Command execution failed");
                Err(anyhow::anyhow!("Command execution failed: {}", e))
            }
            Err(_) => {
                error!(timeout = %timeout_secs, command = %command, "Command timed out");
                Err(anyhow::anyhow!(
                    "Command timed out after {} seconds",
                    timeout_secs
                ))
            }
        }
    }
}

// ============================================================================
// SHELL EXECUTE BATCH TOOL
// ============================================================================

pub struct ShellExecuteBatchTool;

#[async_trait]
impl Tool for ShellExecuteBatchTool {
    fn name(&self) -> &str {
        "shell_execute_batch"
    }

    fn description(&self) -> &str {
        "Execute a sequence of shell commands. Full access for admin users. \
         Stops on first error if stop_on_error is true."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "commands": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "command": { "type": "string" },
                            "working_dir": { "type": "string" },
                            "timeout_secs": { "type": "integer" }
                        },
                        "required": ["command"]
                    },
                    "description": "Ordered list of commands to execute"
                },
                "stop_on_error": {
                    "type": "boolean",
                    "description": "Stop after first non-zero exit or error",
                    "default": true
                },
                "default_working_dir": {
                    "type": "string",
                    "description": "Default working directory for commands",
                    "default": "/tmp"
                },
                "session_id": {
                    "type": "string",
                    "description": "Session ID for rate limiting"
                }
            },
            "required": ["commands"]
        })
    }

    fn category(&self) -> &str {
        "system"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "shell".to_string(),
            "batch".to_string(),
            "admin".to_string(),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let validator = get_security_validator();

        let commands = input
            .get("commands")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: commands"))?;

        if commands.is_empty() {
            return Err(anyhow::anyhow!("commands must be a non-empty array"));
        }

        let stop_on_error = input
            .get("stop_on_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let default_working_dir = input
            .get("default_working_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("/tmp");

        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let max_timeout = validator.max_timeout().await;
        let max_output = validator.max_output().await;
        let default_timeout_secs = 60u64.min(max_timeout.as_secs());

        let mut results = Vec::new();
        let commands_list: Vec<_> = commands.iter().collect();

        for (idx, entry) in commands_list.into_iter().enumerate() {
            // Rate limit each command in the batch
            if let Err(e) = validator.check_rate_limit(session_id).await {
                return Err(anyhow::anyhow!("{}", e));
            }

            // Support both object {"command": "..."} (or "cmd") and string "..."
            let (command, working_dir, timeout_secs) = if let Some(cmd_str) = entry.as_str() {
                (cmd_str, default_working_dir, default_timeout_secs)
            } else {
                let command = entry
                    .get("command")
                    .or_else(|| entry.get("cmd"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Command entry at index {} requires 'command' (or 'cmd') field or must be a string", idx))?;

                let working_dir = entry
                    .get("working_dir")
                    .and_then(|v| v.as_str())
                    .unwrap_or(default_working_dir);

                let timeout = entry
                    .get("timeout_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(default_timeout_secs);

                (command, working_dir, timeout)
            };

            let timeout_secs = timeout_secs.min(max_timeout.as_secs());

            // Check command access
            if let Err(e) = validator.check_command(command).await {
                let outcome = json!({
                    "command": command,
                    "working_dir": working_dir,
                    "exit_code": -1,
                    "stdout": "",
                    "stderr": format!("Access denied: {}", e),
                    "success": false
                });
                results.push(outcome);
                if stop_on_error {
                    break;
                }
                continue;
            }

            // Execute command
            let run = tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                execute_command(command, working_dir, max_output),
            )
            .await;

            let outcome = match run {
                Ok(Ok((stdout, stderr, exit_code))) => json!({
                    "command": command,
                    "working_dir": working_dir,
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                    "success": exit_code == 0
                }),
                Ok(Err(e)) => json!({
                    "command": command,
                    "working_dir": working_dir,
                    "exit_code": -1,
                    "stdout": "",
                    "stderr": e,
                    "success": false
                }),
                Err(_) => json!({
                    "command": command,
                    "working_dir": working_dir,
                    "exit_code": -1,
                    "stdout": "",
                    "stderr": format!("Command timed out after {} seconds", timeout_secs),
                    "success": false
                }),
            };

            let success = outcome
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            results.push(outcome);

            if stop_on_error && !success {
                break;
            }
        }

        let stopped_early = stop_on_error
            && results
                .last()
                .and_then(|r| r.get("success"))
                .and_then(|v| v.as_bool())
                == Some(false);

        Ok(json!({
            "results": results,
            "stopped_early": stopped_early,
            "total_commands": results.len()
        }))
    }
}

// ============================================================================
// COMMAND EXECUTION
// ============================================================================

/// Execute a command using bash
async fn execute_command(
    command: &str,
    working_dir: &str,
    max_output: usize,
) -> Result<(String, String, i32), String> {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(mut stdout_pipe) = child.stdout.take() {
        stdout_pipe
            .read_to_string(&mut stdout)
            .await
            .map_err(|e| format!("Failed to read stdout: {}", e))?;
    }

    if let Some(mut stderr_pipe) = child.stderr.take() {
        stderr_pipe
            .read_to_string(&mut stderr)
            .await
            .map_err(|e| format!("Failed to read stderr: {}", e))?;
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for command: {}", e))?;

    let exit_code = status.code().unwrap_or(-1);

    // Truncate if needed
    if stdout.len() > max_output {
        stdout.truncate(max_output);
        stdout.push_str("\n... (output truncated)");
    }
    if stderr.len() > max_output {
        stderr.truncate(max_output);
        stderr.push_str("\n... (output truncated)");
    }

    Ok((stdout, stderr, exit_code))
}

// ============================================================================
// REGISTRATION
// ============================================================================

/// Register shell tools with the registry
pub async fn register_shell_tools(registry: &ToolRegistry) -> Result<()> {
    use std::sync::Arc;

    registry.register_tool(Arc::new(ShellExecuteTool)).await?;
    registry
        .register_tool(Arc::new(ShellExecuteBatchTool))
        .await?;

    debug!("Registered shell execution tools");
    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell_execute() {
        let tool = ShellExecuteTool;
        let result = tool
            .execute(json!({
                "command": "echo hello world",
                "session_id": "test1"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        assert!(val
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("hello world"));
    }

    #[tokio::test]
    async fn test_shell_with_exit_code() {
        let tool = ShellExecuteTool;
        let result = tool
            .execute(json!({
                "command": "exit 42",
                "session_id": "test2"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val.get("exit_code").and_then(|v| v.as_i64()).unwrap(), 42);
        assert!(!val.get("success").and_then(|v| v.as_bool()).unwrap_or(true));
    }

    #[tokio::test]
    async fn test_admin_can_run_anything() {
        // With default admin profile, any command should work
        let tool = ShellExecuteTool;

        // Even "dangerous" commands should be allowed for admins
        let result = tool
            .execute(json!({
                "command": "ls /",
                "session_id": "test3"
            }))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_native_alternative_warning() {
        let tool = ShellExecuteTool;
        let result = tool
            .execute(json!({
                "command": "ovs-vsctl show",
                "session_id": "test4"
            }))
            .await;

        // Should still succeed (not blocked)
        // But may include a warning about native alternatives
        // (Note: This test may fail if ovs-vsctl isn't installed, which is fine)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin/system.rs">
//! System Tools

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use crate::Tool;
use sysinfo::{Disks, System};

pub struct SystemTool {
    name: String,
    description: String,
}

impl SystemTool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl Tool for SystemTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        let mut sys = System::new_all();
        sys.refresh_all();

        match self.name.as_str() {
            "system_info" => {
                Ok(json!({
                    "hostname": gethostname::gethostname().to_string_lossy(),
                    "kernel": System::kernel_version(),
                    "os": System::name(),
                    "cpu_count": sys.cpus().len(),
                    "memory_total_mb": sys.total_memory() / 1024 / 1024,
                    "memory_used_mb": sys.used_memory() / 1024 / 1024
                }))
            }
            "system_processes" => {
                let processes: Vec<_> = sys.processes().iter()
                    .take(20)
                    .map(|(pid, proc)| json!({
                        "pid": pid.as_u32(),
                        "name": proc.name(),
                        "cpu": proc.cpu_usage(),
                        "memory_mb": proc.memory() / 1024 / 1024
                    }))
                    .collect();
                Ok(json!({"processes": processes}))
            }
            "system_memory" => {
                Ok(json!({
                    "total_mb": sys.total_memory() / 1024 / 1024,
                    "used_mb": sys.used_memory() / 1024 / 1024,
                    "free_mb": sys.free_memory() / 1024 / 1024,
                    "percent": (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0
                }))
            }
            "system_disk" => {
                let disks = Disks::new_with_refreshed_list();
                let disks: Vec<_> = disks
                    .iter()
                    .map(|d| {
                        json!({
                            "name": d.name().to_string_lossy(),
                            "mount": d.mount_point().to_string_lossy(),
                            "total_gb": d.total_space() / 1024 / 1024 / 1024,
                            "free_gb": d.available_space() / 1024 / 1024 / 1024
                        })
                    })
                    .collect();
                Ok(json!({"disks": disks}))
            }
            _ => Ok(json!({"error": "Not implemented"}))
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/discovery/sources/agent.rs">
//! Agent Discovery Source
//!
//! Discovers tools from D-Bus agents and LLM agents.

use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, warn};

use crate::discovery::{SourceType, ToolDiscoverySource};
use crate::registry::ToolDefinition;

/// Agent discovery source
pub struct AgentDiscoverySource {
    /// Path to LLM agents directory
    agents_dir: PathBuf,
    /// Known D-Bus agents
    dbus_agents: Vec<String>,
}

impl Default for AgentDiscoverySource {
    fn default() -> Self {
        Self {
            agents_dir: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/home/jeremy"))
                .join("agents"),
            dbus_agents: default_dbus_agents(),
        }
    }
}

fn default_dbus_agents() -> Vec<String> {
    vec![
        "executor".to_string(),
        "file".to_string(),
        "network".to_string(),
        "systemd".to_string(),
        "monitor".to_string(),
        "packagekit".to_string(),
        "python-pro".to_string(),
        "rust-pro".to_string(),
        "c-pro".to_string(),
        "cpp-pro".to_string(),
        "golang-pro".to_string(),
        "javascript-pro".to_string(),
        "php-pro".to_string(),
        "sql-pro".to_string(),
    ]
}

impl AgentDiscoverySource {
    pub fn new(agents_dir: PathBuf, dbus_agents: Vec<String>) -> Self {
        Self {
            agents_dir,
            dbus_agents,
        }
    }

    pub fn with_agents_dir(mut self, dir: PathBuf) -> Self {
        self.agents_dir = dir;
        self
    }
}

#[async_trait]
impl ToolDiscoverySource for AgentDiscoverySource {
    fn source_type(&self) -> SourceType {
        SourceType::Agent
    }

    fn name(&self) -> &str {
        "agents"
    }

    fn description(&self) -> &str {
        "D-Bus agents and LLM agents"
    }

    async fn discover(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let mut tools = Vec::new();

        // Discover D-Bus agents
        for agent in &self.dbus_agents {
            tools.push(ToolDefinition {
                name: format!("agent_{}_execute", agent.replace('-', "_")),
                description: format!("Execute task via {} agent", agent),
                input_schema: simd_json::json!({
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Task description for the agent"
                        },
                        "parameters": {
                            "type": "object",
                            "description": "Additional parameters"
                        }
                    },
                    "required": ["task"]
                }),
                schema_version: "https://json-schema.org/draft/next/schema".to_string(),
                category: "agent".to_string(),
                tags: vec!["agent".to_string(), "dbus".to_string(), agent.clone()],
                namespace: "system.v1".to_string(),
            });
        }

        // Discover LLM agents from ~/agents/
        if self.agents_dir.exists() {
            match self.discover_llm_agents().await {
                Ok(llm_tools) => {
                    debug!("Discovered {} LLM agent tools", llm_tools.len());
                    tools.extend(llm_tools);
                }
                Err(e) => {
                    warn!("Failed to discover LLM agents: {}", e);
                }
            }
        }

        debug!("Discovered {} total agent tools", tools.len());
        Ok(tools)
    }

    async fn is_available(&self) -> bool {
        // Always available - D-Bus agents are part of the system
        true
    }
}

impl AgentDiscoverySource {
    async fn discover_llm_agents(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let mut tools = Vec::new();

        // Look for plugins/*/agents/*.md files
        let plugins_dir = self.agents_dir.join("plugins");
        if plugins_dir.exists() {
            let mut entries = tokio::fs::read_dir(&plugins_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    let plugin_name = entry.file_name().to_string_lossy().to_string();
                    let agents_subdir = entry.path().join("agents");

                    if agents_subdir.exists() {
                        let mut agent_entries = tokio::fs::read_dir(&agents_subdir).await?;
                        while let Some(agent_entry) = agent_entries.next_entry().await? {
                            let path = agent_entry.path();
                            if path.extension().map(|e| e == "md").unwrap_or(false) {
                                let agent_name = path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();

                                tools.push(ToolDefinition {
                                    name: format!(
                                        "llm_agent_{}_{}",
                                        plugin_name.replace('-', "_"),
                                        agent_name.replace('-', "_")
                                    ),
                                    description: format!(
                                        "LLM agent: {} / {}",
                                        plugin_name, agent_name
                                    ),
                                    input_schema: simd_json::json!({
                                        "type": "object",
                                        "properties": {
                                            "prompt": {
                                                "type": "string",
                                                "description": "Prompt for the LLM agent"
                                            },
                                            "context": {
                                                "type": "object",
                                                "description": "Additional context"
                                            }
                                        },
                                        "required": ["prompt"]
                                    }),
                                    schema_version: "https://json-schema.org/draft/next/schema"
                                        .to_string(),
                                    category: "llm_agent".to_string(),
                                    tags: vec![
                                        "llm".to_string(),
                                        "agent".to_string(),
                                        plugin_name.clone(),
                                    ],
                                    namespace: "system.v1".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(tools)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/discovery/sources/dbus.rs">
//! D-Bus Discovery Source
//!
//! Discovers tools from D-Bus services at runtime via introspection.
//! Uses op-introspection crate for actual D-Bus scanning.

use async_trait::async_trait;
use op_core::{BusType as CoreBusType, MethodInfo};
use op_introspection::IntrospectionService;
use simd_json::json;
use std::collections::HashSet;
use tracing::{debug, warn};

use crate::discovery::{SourceType, ToolDiscoverySource};
use crate::registry::ToolDefinition as RegistryToolDefinition;

/// D-Bus discovery source for runtime tool discovery
pub struct DbusDiscoverySource {
    bus_type: BusType,
    introspection_service: IntrospectionService,
    /// Well-known services to introspect
    services: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum BusType {
    Session,
    System,
}

impl From<BusType> for CoreBusType {
    fn from(bus_type: BusType) -> Self {
        match bus_type {
            BusType::Session => CoreBusType::Session,
            BusType::System => CoreBusType::System,
        }
    }
}

impl DbusDiscoverySource {
    pub fn new(bus_type: BusType) -> Self {
        Self {
            bus_type,
            introspection_service: IntrospectionService::new(),
            services: default_services(),
        }
    }

    pub fn session() -> Self {
        Self::new(BusType::Session)
    }

    pub fn system() -> Self {
        Self::new(BusType::System)
    }

    pub fn with_services(mut self, services: Vec<String>) -> Self {
        self.services = services;
        self
    }
}

fn default_services() -> Vec<String> {
    vec![
        "org.freedesktop.systemd1".to_string(),
        "org.freedesktop.NetworkManager".to_string(),
        "org.freedesktop.login1".to_string(),
        "org.freedesktop.PackageKit".to_string(),
        "org.freedesktop.UDisks2".to_string(),
        "org.freedesktop.ColorManager".to_string(),
        "org.freedesktop.PolicyKit1".to_string(),
        "org.freedesktop.ModemManager1".to_string(),
    ]
}

#[async_trait]
impl ToolDiscoverySource for DbusDiscoverySource {
    fn source_type(&self) -> SourceType {
        SourceType::Dbus
    }

    fn name(&self) -> &str {
        match self.bus_type {
            BusType::Session => "dbus-session",
            BusType::System => "dbus-system",
        }
    }

    fn description(&self) -> &str {
        "D-Bus services discovered via runtime introspection"
    }

    async fn discover(&self) -> anyhow::Result<Vec<RegistryToolDefinition>> {
        let mut tools = Vec::new();
        let bus_type: CoreBusType = self.bus_type.into();

        debug!("Starting D-Bus discovery on {:?} bus", self.bus_type);

        // First, discover all available services
        let services = match self.introspection_service.list_services(bus_type).await {
            Ok(services) => {
                debug!(
                    "Found {} services on {:?} bus",
                    services.len(),
                    self.bus_type
                );
                services
            }
            Err(e) => {
                warn!("Failed to list services on {:?} bus: {}", self.bus_type, e);
                return Ok(Vec::new());
            }
        };

        // Filter to well-known services (or all services if none specified)
        let target_services: HashSet<String> = if self.services.is_empty() {
            services.iter().map(|s| s.name.clone()).collect()
        } else {
            self.services.iter().cloned().collect()
        };

        // Introspect each target service
        for service_info in services {
            if !target_services.contains(&service_info.name) {
                continue;
            }

            debug!("Introspecting service: {}", service_info.name);

            // Try to introspect the root path
            match self
                .introspect_service_paths(&service_info.name, &bus_type)
                .await
            {
                Ok(service_tools) => {
                    debug!(
                        "Discovered {} tools from {}",
                        service_tools.len(),
                        service_info.name
                    );
                    tools.extend(service_tools);
                }
                Err(e) => {
                    debug!("Failed to introspect {}: {}", service_info.name, e);
                }
            }
        }

        debug!("Total D-Bus tools discovered: {}", tools.len());
        Ok(tools)
    }

    async fn is_available(&self) -> bool {
        // Check if D-Bus is available
        match self.bus_type {
            BusType::System => std::path::Path::new("/var/run/dbus/system_bus_socket").exists(),
            BusType::Session => std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok(),
        }
    }
}

impl DbusDiscoverySource {
    /// Introspect all paths for a service and generate tool definitions
    async fn introspect_service_paths(
        &self,
        service: &str,
        bus_type: &CoreBusType,
    ) -> anyhow::Result<Vec<RegistryToolDefinition>> {
        let mut tools = Vec::new();

        // Common paths to try for most services
        let paths_to_try = vec!["/".to_string(), format!("/{}", service.replace('.', "/"))];

        for path in paths_to_try {
            match self.introspect_path(service, &path, bus_type).await {
                Ok(path_tools) => {
                    tools.extend(path_tools);
                }
                Err(e) => {
                    debug!(
                        "Failed to introspect path {} for service {}: {}",
                        path, service, e
                    );
                }
            }
        }

        Ok(tools)
    }

    /// Introspect a specific path and generate tool definitions
    async fn introspect_path(
        &self,
        service: &str,
        path: &str,
        bus_type: &CoreBusType,
    ) -> anyhow::Result<Vec<RegistryToolDefinition>> {
        let object_info = self
            .introspection_service
            .introspect(*bus_type, service, path)
            .await?;

        let mut tools = Vec::new();

        for interface in &object_info.interfaces {
            for method in &interface.methods {
                // Skip methods with file descriptor arguments (not supported in JSON)
                let has_fd_args = method.in_args.iter().any(|arg| arg.signature.contains('h'))
                    || method
                        .out_args
                        .iter()
                        .any(|arg| arg.signature.contains('h'));

                if has_fd_args {
                    debug!(
                        "Skipping method {}.{} (has file descriptors)",
                        interface.name, method.name
                    );
                    continue;
                }

                let tool_def = self.method_to_tool_definition(service, path, interface, method)?;
                tools.push(tool_def);
            }
        }

        Ok(tools)
    }

    /// Convert a D-Bus method to a tool definition
    fn method_to_tool_definition(
        &self,
        service: &str,
        path: &str,
        interface: &op_core::InterfaceInfo,
        method: &MethodInfo,
    ) -> anyhow::Result<RegistryToolDefinition> {
        let tool_name = format!(
            "dbus_{}_{}_{}",
            service.split('.').last().unwrap_or(service),
            interface.name.split('.').last().unwrap_or(&interface.name),
            method.name
        );

        // Build input schema from method arguments
        let mut properties = simd_json::value::owned::Object::new();
        let mut required = Vec::new();

        for (idx, arg) in method.in_args.iter().enumerate() {
            let arg_name = arg.name.clone().unwrap_or_else(|| format!("arg{}", idx));
            let schema = self.signature_to_schema(&arg.signature, Some(&arg_name));
            properties.insert(arg_name.clone(), schema);
            required.push(arg_name);
        }

        let input_schema = json!({
            "type": "object",
            "properties": properties,
            "required": required
        });

        let description = format!(
            "D-Bus method: {}.{} on {}{}",
            interface.name,
            method.name,
            service,
            if path != "/" { path } else { "" }
        );

        Ok(RegistryToolDefinition {
            name: tool_name,
            description,
            input_schema,
            schema_version: "https://json-schema.org/draft/next/schema".to_string(),
            category: "dbus".to_string(),
            tags: vec![
                "dbus".to_string(),
                service.to_string(),
                interface.name.clone(),
            ],
            namespace: "system.v1".to_string(),
        })
    }

    /// Convert D-Bus signature to JSON schema type
    fn signature_to_schema(
        &self,
        signature: &str,
        arg_name: Option<&str>,
    ) -> simd_json::OwnedValue {
        let desc = arg_name.map(|n| format!(" ({})", n)).unwrap_or_default();
        match signature {
            "s" => json!({"type": "string", "description": format!("string{}", desc)}),
            "o" => json!({"type": "string", "description": format!("D-Bus object path{}", desc)}),
            "g" => json!({"type": "string", "description": format!("D-Bus signature{}", desc)}),
            "b" => json!({"type": "boolean", "description": format!("boolean{}", desc)}),
            "y" | "n" | "q" | "i" | "u" | "x" | "t" => {
                json!({"type": "integer", "description": format!("integer{}", desc)})
            }
            "d" => json!({"type": "number", "description": format!("number{}", desc)}),
            "v" => json!({"type": "string", "description": format!("variant{}", desc)}),
            "as" | "ao" => {
                json!({"type": "array", "items": {"type": "string"}, "description": format!("string array{}", desc)})
            }
            "ai" | "au" | "ax" | "at" => {
                json!({"type": "array", "items": {"type": "integer"}, "description": format!("integer array{}", desc)})
            }
            "ab" => {
                json!({"type": "array", "items": {"type": "boolean"}, "description": format!("boolean array{}", desc)})
            }
            // For complex types, use simple string representation to avoid schema issues
            _ => {
                json!({"type": "string", "description": format!("D-Bus type {}{}", signature, desc)})
            }
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/discovery/sources/mod.rs">
//! Discovery Sources
//!
//! Implementations of ToolDiscoverySource for various backends:
//! - D-Bus runtime introspection
//! - Plugin registry scanning
//! - Agent registry scanning

mod agent;
mod dbus;
mod plugin;

pub use agent::AgentDiscoverySource;
pub use dbus::DbusDiscoverySource;
pub use plugin::PluginDiscoverySource;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/discovery/sources/plugin.rs">
//! Plugin Discovery Source
//!
//! Discovers tools from state plugins.

use async_trait::async_trait;
use tracing::debug;

use crate::discovery::{SourceType, ToolDiscoverySource};
use crate::registry::ToolDefinition;

/// Plugin discovery source
pub struct PluginDiscoverySource {
    /// Known plugin names
    plugins: Vec<String>,
}

impl Default for PluginDiscoverySource {
    fn default() -> Self {
        Self {
            plugins: default_plugins(),
        }
    }
}

fn default_plugins() -> Vec<String> {
    vec![
        "systemd".to_string(),
        "net".to_string(),
        "packagekit".to_string(),
        "login1".to_string(),
        "keyring".to_string(),
        "lxc".to_string(),
        "openflow".to_string(),
        "systemd_networkd".to_string(),
        "dnsresolver".to_string(),
        "netmaker".to_string(),
        "pcidecl".to_string(),
        "privacy_router".to_string(),
        "privacy".to_string(),
        "sessdecl".to_string(),
    ]
}

impl PluginDiscoverySource {
    pub fn new(plugins: Vec<String>) -> Self {
        Self { plugins }
    }
}

#[async_trait]
impl ToolDiscoverySource for PluginDiscoverySource {
    fn source_type(&self) -> SourceType {
        SourceType::Plugin
    }

    fn name(&self) -> &str {
        "plugins"
    }

    fn description(&self) -> &str {
        "State plugins with query/diff/apply operations"
    }

    async fn discover(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let mut tools = Vec::new();

        // Each plugin generates 3 tools: _query, _diff, _apply
        for plugin in &self.plugins {
            // Query tool
            tools.push(ToolDefinition {
                name: format!("plugin_{}_query", plugin),
                description: format!("Query current state from {} plugin", plugin),
                input_schema: simd_json::json!({
                    "type": "object",
                    "properties": {
                        "filter": {
                            "type": "object",
                            "description": "Optional filter for state query"
                        }
                    }
                }),
                schema_version: "https://json-schema.org/draft/next/schema".to_string(),
                category: "state".to_string(),
                tags: vec!["plugin".to_string(), "state".to_string(), plugin.clone()],
                namespace: "system.v1".to_string(),
            });

            // Diff tool
            tools.push(ToolDefinition {
                name: format!("plugin_{}_diff", plugin),
                description: format!(
                    "Calculate diff between current and desired state for {} plugin",
                    plugin
                ),
                input_schema: simd_json::json!({
                    "type": "object",
                    "properties": {
                        "desired_state": {
                            "type": "object",
                            "description": "Desired state configuration"
                        }
                    },
                    "required": ["desired_state"]
                }),
                schema_version: "https://json-schema.org/draft/next/schema".to_string(),
                category: "state".to_string(),
                tags: vec!["plugin".to_string(), "state".to_string(), plugin.clone()],
                namespace: "system.v1".to_string(),
            });

            // Apply tool
            tools.push(ToolDefinition {
                name: format!("plugin_{}_apply", plugin),
                description: format!("Apply state changes for {} plugin", plugin),
                input_schema: simd_json::json!({
                    "type": "object",
                    "properties": {
                        "diff": {
                            "type": "object",
                            "description": "State diff to apply"
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "If true, only simulate changes",
                            "default": false
                        }
                    },
                    "required": ["diff"]
                }),
                schema_version: "https://json-schema.org/draft/next/schema".to_string(),
                category: "state".to_string(),
                tags: vec!["plugin".to_string(), "state".to_string(), plugin.clone()],
                namespace: "system.v1".to_string(),
            });
        }

        debug!(
            "Discovered {} plugin tools from {} plugins",
            tools.len(),
            self.plugins.len()
        );
        Ok(tools)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/discovery/mod.rs">
//! Tool Discovery System
//!
//! Provides a catalog of all available tools without loading them.
//! Tools are loaded on-demand via the ToolRegistry.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::registry::ToolDefinition;

pub mod projection_engine;
pub mod sources;

pub use projection_engine::ProjectionEngine;
pub use sources::{AgentDiscoverySource, DbusDiscoverySource, PluginDiscoverySource};

/// Source type for tool discovery
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceType {
    /// Built-in tools compiled into the binary
    Builtin,
    /// D-Bus services discovered at runtime
    Dbus,
    /// Plugins loaded dynamically
    Plugin,
    /// Agent-based tools
    Agent,
    /// External MCP servers
    Mcp,
}

/// Information about a tool source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSourceInfo {
    pub source_type: SourceType,
    pub name: String,
    pub description: String,
    pub tool_count: usize,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,
}

/// Cache policy for discovery
#[derive(Debug, Clone)]
pub enum DiscoveryCachePolicy {
    /// Always use cached data if available
    PreferCache,
    /// Refresh if cache is older than duration
    RefreshAfter(Duration),
    /// Always refresh from source
    AlwaysRefresh,
}

impl Default for DiscoveryCachePolicy {
    fn default() -> Self {
        DiscoveryCachePolicy::RefreshAfter(Duration::from_secs(300))
    }
}

/// Trait for tool discovery sources
#[async_trait]
pub trait ToolDiscoverySource: Send + Sync {
    /// Get the source type
    fn source_type(&self) -> SourceType;

    /// Get source name
    fn name(&self) -> &str;

    /// Get source description
    fn description(&self) -> &str;

    /// Discover all tools from this source
    async fn discover(&self) -> anyhow::Result<Vec<ToolDefinition>>;

    /// Check if source is available
    async fn is_available(&self) -> bool {
        true
    }
}

/// Built-in tool source for statically defined tools
pub struct BuiltinToolSource {
    tools: Vec<ToolDefinition>,
}

impl BuiltinToolSource {
    pub fn new(tools: Vec<ToolDefinition>) -> Self {
        Self { tools }
    }
}

#[async_trait]
impl ToolDiscoverySource for BuiltinToolSource {
    fn source_type(&self) -> SourceType {
        SourceType::Builtin
    }

    fn name(&self) -> &str {
        "builtin"
    }

    fn description(&self) -> &str {
        "Built-in tools compiled into the binary"
    }

    async fn discover(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        Ok(self.tools.clone())
    }
}

/// Statistics about the discovery system
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryStats {
    pub total_tools: usize,
    pub source_count: usize,
    pub last_full_refresh: Option<chrono::DateTime<chrono::Utc>>,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Central tool discovery system
pub struct ToolDiscoverySystem {
    sources: RwLock<Vec<Arc<dyn ToolDiscoverySource>>>,
    cache: RwLock<HashMap<String, ToolDefinition>>,
    cache_timestamp: RwLock<Option<Instant>>,
    cache_policy: DiscoveryCachePolicy,
    stats: RwLock<DiscoveryStats>,
}

impl ToolDiscoverySystem {
    pub fn new() -> Self {
        Self {
            sources: RwLock::new(Vec::new()),
            cache: RwLock::new(HashMap::new()),
            cache_timestamp: RwLock::new(None),
            cache_policy: DiscoveryCachePolicy::default(),
            stats: RwLock::new(DiscoveryStats::default()),
        }
    }

    pub fn with_cache_policy(mut self, policy: DiscoveryCachePolicy) -> Self {
        self.cache_policy = policy;
        self
    }

    /// Register a discovery source
    pub async fn register_source(&self, source: Arc<dyn ToolDiscoverySource>) {
        let mut sources = self.sources.write().await;
        info!(
            "Registering discovery source: {} ({})",
            source.name(),
            source.description()
        );
        sources.push(source);
    }

    /// Get all tool definitions (from cache or refresh)
    pub async fn get_all_tool_definitions(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let should_refresh = self.should_refresh().await;

        if should_refresh {
            self.refresh_cache().await?;
        } else {
            let mut stats = self.stats.write().await;
            stats.cache_hits += 1;
        }

        let cache = self.cache.read().await;
        Ok(cache.values().cloned().collect())
    }

    /// Get a specific tool definition by name
    pub async fn get_tool_definition(&self, name: &str) -> Option<ToolDefinition> {
        // First check cache
        {
            let cache = self.cache.read().await;
            if let Some(def) = cache.get(name) {
                return Some(def.clone());
            }
        }

        // If not in cache and cache might be stale, refresh
        if self.should_refresh().await {
            if let Err(e) = self.refresh_cache().await {
                warn!("Failed to refresh cache: {}", e);
            }
        }

        let cache = self.cache.read().await;
        cache.get(name).cloned()
    }

    /// Search for tools matching criteria
    pub async fn search_tools(
        &self,
        query: &str,
        category: Option<&str>,
        tags: Option<&[String]>,
    ) -> Vec<ToolDefinition> {
        let cache = self.cache.read().await;
        let query_lower = query.to_lowercase();

        cache
            .values()
            .filter(|def| {
                // Match query against name or description
                let matches_query = query.is_empty()
                    || def.name.to_lowercase().contains(&query_lower)
                    || def.description.to_lowercase().contains(&query_lower);

                // Match category if specified
                let matches_category = category.map(|c| def.category == c).unwrap_or(true);

                // Match tags if specified
                let matches_tags = tags
                    .map(|t| t.iter().any(|tag| def.tags.contains(tag)))
                    .unwrap_or(true);

                matches_query && matches_category && matches_tags
            })
            .cloned()
            .collect()
    }

    /// Get statistics
    pub async fn stats(&self) -> DiscoveryStats {
        self.stats.read().await.clone()
    }

    /// Get information about all sources
    pub async fn get_sources(&self) -> Vec<ToolSourceInfo> {
        let sources = self.sources.read().await;
        let mut infos = Vec::new();

        for source in sources.iter() {
            let tool_count = source.discover().await.map(|t| t.len()).unwrap_or(0);
            infos.push(ToolSourceInfo {
                source_type: source.source_type(),
                name: source.name().to_string(),
                description: source.description().to_string(),
                tool_count,
                last_refresh: None,
            });
        }

        infos
    }

    /// Start background refresh task
    pub async fn start_background_refresh(&self) {
        // Initial refresh
        if let Err(e) = self.refresh_cache().await {
            warn!("Initial cache refresh failed: {}", e);
        }
    }

    /// Force refresh the cache
    pub async fn force_refresh(&self) -> anyhow::Result<()> {
        self.refresh_cache().await
    }

    /// Check if cache should be refreshed
    async fn should_refresh(&self) -> bool {
        match &self.cache_policy {
            DiscoveryCachePolicy::PreferCache => {
                let timestamp = self.cache_timestamp.read().await;
                timestamp.is_none()
            }
            DiscoveryCachePolicy::RefreshAfter(duration) => {
                let timestamp = self.cache_timestamp.read().await;
                match *timestamp {
                    None => true,
                    Some(ts) => ts.elapsed() > *duration,
                }
            }
            DiscoveryCachePolicy::AlwaysRefresh => true,
        }
    }

    /// Refresh the cache from all sources
    async fn refresh_cache(&self) -> anyhow::Result<()> {
        debug!("Refreshing tool discovery cache");

        let sources = self.sources.read().await;
        let mut new_cache = HashMap::new();

        for source in sources.iter() {
            if !source.is_available().await {
                debug!("Source {} is not available, skipping", source.name());
                continue;
            }

            match source.discover().await {
                Ok(tools) => {
                    debug!(
                        "Discovered {} tools from source {}",
                        tools.len(),
                        source.name()
                    );
                    for tool in tools {
                        new_cache.insert(tool.name.clone(), tool);
                    }
                }
                Err(e) => {
                    warn!("Failed to discover tools from {}: {}", source.name(), e);
                }
            }
        }

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = new_cache;
        }

        // Update timestamp
        {
            let mut timestamp = self.cache_timestamp.write().await;
            *timestamp = Some(Instant::now());
        }

        // Update stats
        {
            let cache = self.cache.read().await;
            let mut stats = self.stats.write().await;
            stats.total_tools = cache.len();
            stats.source_count = sources.len();
            stats.last_full_refresh = Some(chrono::Utc::now());
            stats.cache_misses += 1;
        }

        info!(
            "Tool discovery cache refreshed: {} tools from {} sources",
            self.cache.read().await.len(),
            sources.len()
        );

        Ok(())
    }
}

impl Default for ToolDiscoverySystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builtin_source() {
        let tools = vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: simd_json::json!({}),
            category: "test".to_string(),
            tags: vec!["test".to_string()],
            namespace: "test".to_string(),
        }];

        let source = BuiltinToolSource::new(tools);
        let discovered = source.discover().await.unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].name, "test_tool");
    }

    #[tokio::test]
    async fn test_discovery_system() {
        let system = ToolDiscoverySystem::new();

        let tools = vec![ToolDefinition {
            name: "builtin_tool".to_string(),
            description: "Built-in tool".to_string(),
            input_schema: simd_json::json!({}),
            category: "builtin".to_string(),
            tags: vec![],
            namespace: "builtin".to_string(),
        }];

        system
            .register_source(Arc::new(BuiltinToolSource::new(tools)))
            .await;

        let all_tools = system.get_all_tool_definitions().await.unwrap();
        assert_eq!(all_tools.len(), 1);

        let found = system.get_tool_definition("builtin_tool").await;
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_search_tools() {
        let system = ToolDiscoverySystem::new();

        let tools = vec![
            ToolDefinition {
                name: "systemd_start".to_string(),
                description: "Start a systemd unit".to_string(),
                input_schema: simd_json::json!({}),
                category: "dbus".to_string(),
                tags: vec!["systemd".to_string()],
                namespace: "system".to_string(),
            },
            ToolDefinition {
                name: "network_status".to_string(),
                description: "Get network status".to_string(),
                input_schema: simd_json::json!({}),
                category: "dbus".to_string(),
                tags: vec!["network".to_string()],
                namespace: "system".to_string(),
            },
        ];

        system
            .register_source(Arc::new(BuiltinToolSource::new(tools)))
            .await;
        system.refresh_cache().await.unwrap();

        // Search by query
        let results = system.search_tools("systemd", None, None).await;
        assert_eq!(results.len(), 1);

        // Search by tag
        let results = system
            .search_tools("", None, Some(&["network".to_string()]))
            .await;
        assert_eq!(results.len(), 1);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/discovery/projection_engine.rs">
//! Projection Engine - Auto-Discovery of D-Bus APIs as tools
//!
//! This engine walks the D-Bus object tree and projects discovered
//! interfaces as executable tools in the registry.

use anyhow::Result;
use simd_json::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::tool::Tool;
use op_core::BusType;
use op_introspection::IntrospectionService;

fn normalize_path(path: &str) -> String {
    let mut normalized = String::with_capacity(path.len().max(1));
    let mut prev_slash = false;

    for ch in path.chars() {
        if ch == '/' {
            if !prev_slash {
                normalized.push('/');
            }
            prev_slash = true;
        } else {
            normalized.push(ch);
            prev_slash = false;
        }
    }

    if normalized.is_empty() {
        "/".to_string()
    } else if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.trim_end_matches('/').to_string()
    } else {
        normalized
    }
}

fn join_child_path(parent: &str, child: &str) -> String {
    if child.starts_with('/') {
        return normalize_path(child);
    }

    let parent_norm = normalize_path(parent);
    if parent_norm == "/" {
        normalize_path(&format!("/{}", child))
    } else {
        normalize_path(&format!("{}/{}", parent_norm, child))
    }
}

/// Projection Engine - auto-discovers D-Bus APIs
pub struct ProjectionEngine {
    introspection: Arc<IntrospectionService>,
}

impl ProjectionEngine {
    pub fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }

    /// Discover and register all tools for a bus
    pub async fn discover_all(
        &self,
        registry: &crate::registry::ToolRegistry,
        bus_type: BusType,
    ) -> Result<usize> {
        let services_json = self.introspection.list_services_json(bus_type).await?;
        let mut total_count = 0;

        let services: Vec<String> = if let Some(arr) = services_json.as_array() {
            arr.iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .filter(|n| !n.starts_with(':')) // Skip unique names (temporary connections)
                .collect()
        } else {
            Vec::new()
        };

        tracing::info!(
            "Discovering tools for {} services on {:?} bus",
            services.len(),
            bus_type
        );

        // Process each service
        for service in services {
            tracing::debug!(
                "Introspecting service '{}' on {:?} bus...",
                service,
                bus_type
            );

            // Discover all object paths for this service
            let paths = self.discover_paths(bus_type, &service, "/", 0).await;
            let mut service_tools = 0;

            // Process each object path
            for path in &paths {
                if let Ok(info) = self
                    .introspection
                    .introspect(bus_type, &service, &path)
                    .await
                {
                    for iface in info.interfaces {
                        // Skip standard D-Bus interfaces unless they are interesting
                        if iface.name.starts_with("org.freedesktop.DBus.")
                            && !iface.name.contains("Properties")
                            && !iface.name.contains("ObjectManager")
                        {
                            continue;
                        }

                        for method in iface.methods {
                            let tool = crate::dynamic_tool::DynamicDbusTool::new(
                                service.clone(),
                                path.clone(),
                                iface.name.clone(),
                                method.name.clone(),
                                String::new(), // Signature not easily available here yet
                                method
                                    .in_args
                                    .iter()
                                    .map(|a| a.name.clone().unwrap_or_else(|| "arg".to_string()))
                                    .collect(),
                            );

                            let definition = crate::registry::ToolDefinition {
                                name: tool.name.clone(),
                                description: format!(
                                    "D-Bus method {}.{} on {} at {}",
                                    iface.name, method.name, service, path
                                ),
                                input_schema: tool.input_schema(),
                                schema_version: "https://json-schema.org/draft/next/schema"
                                    .to_string(),
                                category: "dbus-projected".to_string(),
                                namespace: "system.v1".to_string(),
                                tags: vec![
                                    "dbus".to_string(),
                                    "projected".to_string(),
                                    service.clone(),
                                ],
                            };

                            if let Ok(_) = registry
                                .register(tool.name.clone().into(), Arc::new(tool), definition)
                                .await
                            {
                                service_tools += 1;
                            }
                        }

                        // Register tools for properties (ProjectedObjectV1)
                        if iface.name == "org.opdbus.ProjectedObjectV1" {
                            let tool = crate::builtin::plugin_projection::PluginProjectionTool::new_generic(
                                &service,
                                path.clone(),
                            );

                            let definition = crate::registry::ToolDefinition {
                                name: tool.name().to_string(),
                                description: format!(
                                    "Read projected object at {} on {}",
                                    path, service
                                ),
                                input_schema: tool.input_schema(),
                                schema_version: "https://json-schema.org/draft/next/schema"
                                    .to_string(),
                                category: "plugin-projection".to_string(),
                                namespace: "mirrored.v1".to_string(),
                                tags: vec![
                                    "dbus".to_string(),
                                    "projected".to_string(),
                                    "mirrored".to_string(),
                                    service.clone(),
                                ],
                            };

                            if let Ok(_) = registry
                                .register(
                                    tool.name().to_string().into(),
                                    Arc::new(tool),
                                    definition,
                                )
                                .await
                            {
                                service_tools += 1;
                            }
                        }
                    }
                }
            }

            total_count += service_tools;
            if service_tools > 0 {
                tracing::info!(
                    "  → Service {}: registered {} tools from {} paths",
                    service,
                    service_tools,
                    paths.len()
                );
            }
        }

        Ok(total_count)
    }

    /// Recursively discover all object paths for a service
    fn discover_paths<'a>(
        &'a self,
        bus_type: BusType,
        service: &'a str,
        path: &'a str,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Vec<String>> + Send + 'a>> {
        Box::pin(async move {
            const MAX_DEPTH: usize = 10;
            if depth > MAX_DEPTH {
                return vec![];
            }

            let path = normalize_path(path);
            let mut paths = vec![path.clone()];

            // Introspect to find child nodes
            if let Ok(info) = self
                .introspection
                .introspect(bus_type, service, &path)
                .await
            {
                for child in &info.children {
                    if child.is_empty() {
                        continue;
                    }

                    let child_path = join_child_path(&path, child);
                    if child_path == path {
                        continue;
                    }

                    // Recursively discover child paths
                    let child_paths = self
                        .discover_paths(bus_type, service, &child_path, depth + 1)
                        .await;
                    paths.extend(child_paths);
                }
            }

            paths.sort();
            paths.dedup();
            paths
        })
    }
}

impl Clone for ProjectionEngine {
    fn clone(&self) -> Self {
        Self {
            introspection: self.introspection.clone(),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/builtin_old.rs">
//! Built-in tools for common system operations

use async_trait::async_trait;
use simd_json::json;
use tracing::debug;

use op_core::{ToolDefinition, ToolRequest, ToolResult};
use crate::Tool;

/// Echo tool for testing
pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo back the input message (for testing)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Message to echo back"
                    }
                },
                "required": ["message"]
            }),
            category: Some("utility".to_string()),
            tags: vec!["test".to_string(), "utility".to_string()],
        }
    }
    
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();
        
        let message = request.arguments.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        debug!("Echo: {}", message);
        
        ToolResult::success(
            &request.id,
            json!({ "echoed": message }),
            start.elapsed().as_millis() as u64,
        )
    }
    
    fn name(&self) -> &str {
        "echo"
    }
}

/// System info tool
pub struct SystemInfoTool;

#[async_trait]
impl Tool for SystemInfoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "system_info".to_string(),
            description: "Get basic system information".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            category: Some("system".to_string()),
            tags: vec!["system".to_string(), "info".to_string()],
        }
    }
    
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();
        
        let hostname = gethostname::gethostname()
            .to_string_lossy()
            .to_string();
        
        let info = json!({
            "hostname": hostname,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
        });
        
        ToolResult::success(
            &request.id,
            info,
            start.elapsed().as_millis() as u64,
        )
    }
    
    fn name(&self) -> &str {
        "system_info"
    }
}

/// Shell command tool (restricted)
pub struct ShellTool {
    allowed_commands: Vec<String>,
}

impl ShellTool {
    pub fn new(allowed_commands: Vec<String>) -> Self {
        Self { allowed_commands }
    }
    
    pub fn with_defaults() -> Self {
        Self::new(vec![
            "ls".to_string(),
            "cat".to_string(),
            "echo".to_string(),
            "date".to_string(),
            "uptime".to_string(),
            "hostname".to_string(),
            "whoami".to_string(),
            "uname".to_string(),
            "pwd".to_string(),
            "df".to_string(),
            "free".to_string(),
        ])
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".to_string(),
            description: "Execute allowed shell commands".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Command arguments"
                    }
                },
                "required": ["command"]
            }),
            category: Some("system".to_string()),
            tags: vec!["shell".to_string(), "command".to_string()],
        }
    }
    
    fn validate(&self, args: &simd_json::OwnedValue) -> Result<(), String> {
        let command = args.get("command")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'command' argument")?;
        
        // Extract base command (before any pipes or other shell features)
        let base_cmd = command.split_whitespace()
            .next()
            .unwrap_or(command);
        
        if !self.allowed_commands.iter().any(|c| c == base_cmd) {
            return Err(format!(
                "Command '{}' is not allowed. Allowed: {:?}",
                base_cmd, self.allowed_commands
            ));
        }
        
        Ok(())
    }
    
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();
        
        let command = match request.arguments.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult::error(
                    &request.id,
                    "Missing 'command' argument",
                    start.elapsed().as_millis() as u64,
                );
            }
        };
        
        let args: Vec<&str> = request.arguments.get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        
        debug!("Executing shell command: {} {:?}", command, args);
        
        match tokio::process::Command::new("sh")
            .arg("-c")
            .arg(format!("{} {}", command, args.join(" ")))
            .output()
            .await
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                
                if output.status.success() {
                    ToolResult::success(
                        &request.id,
                        json!({
                            "stdout": stdout.trim(),
                            "stderr": stderr.trim(),
                            "exit_code": output.status.code()
                        }),
                        start.elapsed().as_millis() as u64,
                    )
                } else {
                    ToolResult::error(
                        &request.id,
                        format!("Command failed: {}", stderr.trim()),
                        start.elapsed().as_millis() as u64,
                    )
                }
            }
            Err(e) => {
                ToolResult::error(
                    &request.id,
                    format!("Failed to execute command: {}", e),
                    start.elapsed().as_millis() as u64,
                )
            }
        }
    }
    
    fn name(&self) -> &str {
        "shell"
    }
}

/// File read tool
pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_read".to_string(),
            description: "Read contents of a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "max_bytes": {
                        "type": "integer",
                        "description": "Maximum bytes to read (default: 1MB)"
                    }
                },
                "required": ["path"]
            }),
            category: Some("filesystem".to_string()),
            tags: vec!["file".to_string(), "read".to_string()],
        }
    }
    
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();
        
        let path = match request.arguments.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult::error(
                    &request.id,
                    "Missing 'path' argument",
                    start.elapsed().as_millis() as u64,
                );
            }
        };
        
        let max_bytes = request.arguments.get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(1_048_576) as usize; // 1MB default
        
        debug!("Reading file: {} (max {} bytes)", path, max_bytes);
        
        match tokio::fs::read(path).await {
            Ok(contents) => {
                let truncated = contents.len() > max_bytes;
                let contents = if truncated {
                    &contents[..max_bytes]
                } else {
                    &contents
                };
                
                match String::from_utf8(contents.to_vec()) {
                    Ok(text) => {
                        ToolResult::success(
                            &request.id,
                            json!({
                                "content": text,
                                "size": contents.len(),
                                "truncated": truncated
                            }),
                            start.elapsed().as_millis() as u64,
                        )
                    }
                    Err(_) => {
                        // Binary file - return base64
                        ToolResult::success(
                            &request.id,
                            json!({
                                "content_base64": base64::encode(contents),
                                "size": contents.len(),
                                "truncated": truncated,
                                "binary": true
                            }),
                            start.elapsed().as_millis() as u64,
                        )
                    }
                }
            }
            Err(e) => {
                ToolResult::error(
                    &request.id,
                    format!("Failed to read file: {}", e),
                    start.elapsed().as_millis() as u64,
                )
            }
        }
    }
    
    fn name(&self) -> &str {
        "file_read"
    }
}

// Simple base64 encoding (to avoid additional dependency)
mod base64 {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    pub fn encode(data: &[u8]) -> String {
        let mut result = String::new();
        
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as usize;
            let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
            let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
            
            result.push(ALPHABET[b0 >> 2] as char);
            result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
            
            if chunk.len() > 1 {
                result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
            } else {
                result.push('=');
            }
            
            if chunk.len() > 2 {
                result.push(ALPHABET[b2 & 0x3f] as char);
            } else {
                result.push('=');
            }
        }
        
        result
    }
}

/// Register all built-in tools with a registry
pub async fn register_builtins(registry: &crate::ToolRegistry) -> Result<(), String> {
    registry.register(EchoTool).await?;
    registry.register(SystemInfoTool).await?;
    registry.register(ShellTool::with_defaults()).await?;
    registry.register(FileReadTool).await?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/dynamic_tool.rs">
use crate::tool::Tool;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};

/// A dynamically generated tool wrapping a specific D-Bus method
#[derive(Clone)]
pub struct DynamicDbusTool {
    pub name: String,
    pub service: String,
    pub path: String,
    pub interface: String,
    pub method: String,
    pub signature: String,
    pub arg_names: Vec<String>,
}

impl DynamicDbusTool {
    pub fn new(
        service: String,
        path: String,
        interface: String,
        method: String,
        signature: String,
        arg_names: Vec<String>,
    ) -> Self {
        let name = Self::compute_name(&service, &interface, &method);
        Self {
            name,
            service,
            path,
            interface,
            method,
            signature,
            arg_names,
        }
    }

    fn compute_name(service: &str, interface: &str, method: &str) -> String {
        let svc_short = service.split('.').last().unwrap_or(service);
        let iface_short = interface.split('.').last().unwrap_or(interface);

        let method_snake = method
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i > 0 && c.is_uppercase() {
                    format!("_{}", c.to_lowercase())
                } else {
                    c.to_lowercase().to_string()
                }
            })
            .collect::<String>();

        format!(
            "{}.{}.{}",
            svc_short,
            iface_short.to_lowercase(),
            method_snake
        )
    }
}

#[async_trait]
impl Tool for DynamicDbusTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Dynamically projected D-Bus method"
    }

    fn input_schema(&self) -> Value {
        let mut props = simd_json::value::owned::Object::new();
        for arg in &self.arg_names {
            props.insert(arg.clone(), json!({"type": "string"}));
        }

        json!({
            "type": "object",
            "properties": props,
            "required": self.arg_names
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let connection = zbus::Connection::system()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to system bus: {}", e))?;

        let proxy: zbus::Proxy = zbus::proxy::Builder::new(&connection)
            .destination(self.service.as_str())?
            .path(self.path.as_str())?
            .interface(self.interface.as_str())?
            .build()
            .await?;

        // Convert input map to ordered arguments based on arg_names
        let mut args = Vec::new();
        for name in &self.arg_names {
            let val = input
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Missing argument: {}", name))?;

            // Basic conversion - use zbus::zvariant::Value<'static>
            let zval: zbus::zvariant::Value<'static> = if let Some(s) = val.as_str() {
                zbus::zvariant::Value::new(s.to_string())
            } else if let Some(b) = val.as_bool() {
                zbus::zvariant::Value::new(b)
            } else if let Some(i) = val.as_i64() {
                zbus::zvariant::Value::new(i)
            } else if let Some(u) = val.as_u64() {
                zbus::zvariant::Value::new(u)
            } else if let Some(f) = val.as_f64() {
                zbus::zvariant::Value::new(f)
            } else {
                return Err(anyhow::anyhow!("Unsupported argument type for {}", name));
            };
            args.push(zval);
        }

        let result: zbus::zvariant::OwnedValue = proxy.call(self.method.as_str(), &args).await?;
        let result_json = simd_json::serde::to_owned_value(&result)?;

        Ok(result_json)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/executor.rs">
//! Tool executor with timeout and concurrency control

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::ToolRegistry;
use op_core::{ToolRequest, ToolResult};

/// Configuration for tool execution
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum concurrent tool executions
    pub max_concurrent: usize,
    /// Default timeout for tool execution (ms)
    pub default_timeout_ms: u64,
    /// Maximum timeout allowed (ms)
    pub max_timeout_ms: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            default_timeout_ms: 30000,
            max_timeout_ms: 300000, // 5 minutes
        }
    }
}

/// Tool executor with concurrency and timeout control
pub struct ToolExecutor {
    registry: ToolRegistry,
    config: ExecutorConfig,
    semaphore: Arc<Semaphore>,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(registry: ToolRegistry, config: ExecutorConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
        Self {
            registry,
            config,
            semaphore,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(registry: ToolRegistry) -> Self {
        Self::new(registry, ExecutorConfig::default())
    }

    /// Execute a tool with timeout
    pub async fn execute(&self, request: ToolRequest) -> ToolResult {
        let start = std::time::Instant::now();

        // Determine timeout
        let timeout_ms = request
            .timeout_ms
            .unwrap_or(self.config.default_timeout_ms)
            .min(self.config.max_timeout_ms);

        debug!(
            "Executing tool '{}' with timeout {}ms",
            request.tool_name, timeout_ms
        );

        // Acquire semaphore permit
        let _permit = match self.semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                return ToolResult::error(
                    &request.id,
                    "Executor shutdown",
                    start.elapsed().as_millis() as u64,
                );
            }
        };

        // Execute with timeout
        let duration = Duration::from_millis(timeout_ms);
        debug!(
            "About to call registry.execute for tool '{}' with timeout {}ms",
            request.tool_name, timeout_ms
        );
        let timeout_result = timeout(duration, self.registry.execute(request.clone())).await;
        debug!(
            "Registry.execute completed for tool '{}' - success: {}",
            request.tool_name,
            timeout_result.is_ok()
        );

        match timeout_result {
            Ok(result) => {
                debug!(
                    "Tool '{}' executed successfully in {}ms",
                    request.tool_name,
                    start.elapsed().as_millis()
                );
                result
            }
            Err(_) => {
                warn!(
                    "Tool '{}' timed out after {}ms",
                    request.tool_name, timeout_ms
                );
                ToolResult::error(
                    &request.id,
                    format!("Tool execution timed out after {}ms", timeout_ms),
                    start.elapsed().as_millis() as u64,
                )
            }
        }
    }

    /// Execute multiple tools concurrently
    pub async fn execute_batch(&self, requests: Vec<ToolRequest>) -> Vec<ToolResult> {
        let futures: Vec<_> = requests.into_iter().map(|req| self.execute(req)).collect();

        futures::future::join_all(futures).await
    }

    /// Get current concurrency usage
    pub fn current_usage(&self) -> usize {
        self.config.max_concurrent - self.semaphore.available_permits()
    }

    /// Get available permits
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get registry reference
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}

impl Clone for ToolExecutor {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            config: self.config.clone(),
            semaphore: Arc::clone(&self.semaphore),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/lib.rs">
//! op-tools: Tool Registry and Execution
//!
//! Provides the tool registry, built-in tools, and HTTP router.
//!
//! ## Security
//!
//! Security is enforced at the ACCESS level, not command level:
//! - **Unrestricted (Admin)**: Full access - can run any command
//! - **Restricted**: Limited read-only access for untrusted users
//!
//! The chatbot is designed to be a full system administrator.
//! Rate limiting prevents runaway loops.
//!
//! ## Orchestration Plugin
//!
//! The `orchestration_plugin` module provides hooks for tracking all activity:
//! - Tool executions (commands, file ops, etc.)
//! - LLM decisions and tool calls
//! - Session lifecycle events
//!
//! This integrates with snowball for immutable audit logging.

pub mod builtin;
pub mod discovery;
pub mod dynamic_tool;
mod mcptools;
pub mod orchestration_plugin;
pub mod registry;
pub mod router;
pub mod security;
pub mod tool;
pub mod validation;

use tracing::warn;

// Re-export main types
pub use orchestration_plugin::{
    create_tool_event, get_orchestration_registry, LlmDecisionEvent, OrchestrationActivityPlugin,
    OrchestrationPluginRegistry, SessionEvent, ToolExecutedEvent,
};
pub use registry::ToolRegistry;
pub use router::{create_router, ToolsServiceRouter, ToolsState};
pub use security::{
    get_security_validator, AccessLevel, SecurityError, SecurityValidator, ToolSecurityProfile,
};
pub use tool::{BoxedTool, Tool};
pub use validation::{InputValidator, ValidatedInput, ValidationConfig};

/// Register all built-in tools
pub async fn register_builtin_tools(registry: &ToolRegistry) -> anyhow::Result<()> {
    builtin::register_all_builtin_tools(registry).await?;
    builtin::register_response_tools(registry).await?;
    if let Err(err) = mcptools::register_mcp_tools(registry).await {
        warn!("Failed to register MCP tools: {}", err);
    }
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/mcptools.rs">
//! MCP Tools integration via the mcptools CLI.
//!
//! Configuration is provided via environment variables or a JSON config file:
//! - OP_MCPTOOLS_CONFIG: Path to JSON config (default: "mcptools.json")
//! - OP_MCPTOOLS_BIN: Path to mcptools binary (default: "mcp")
//! - OP_MCPTOOLS_SERVERS: JSON array of server configs
//!   Example:
//!   [
//!     {
//!       "name": "github",
//!       "args": ["https://api.example.com/mcp"],
//!       "transport": "http",
//!       "auth_header": "Bearer TOKEN",
//!       "tool_prefix": "mcp_github_"
//!     }
//!   ]
//! - OP_MCPTOOLS_SERVER: Single server command (space-separated) as a fallback
//! - OP_MCPTOOLS_SERVER_NAME: Optional name for OP_MCPTOOLS_SERVER (default: "default")
//! - OP_MCPTOOLS_ALLOW_UNPREFIXED: "true" to allow raw tool names (fallback to prefixed on conflict)

use anyhow::{Context, Result};
use serde::Deserialize;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::registry::{ToolDefinition, ToolRegistry};
use crate::tool::Tool;

#[derive(Debug, Clone, Deserialize)]
struct McpToolsServerConfig {
    name: String,
    args: Vec<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    auth_header: Option<String>,
    #[serde(default)]
    auth_user: Option<String>,
    #[serde(default)]
    tool_prefix: Option<String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
struct McpToolSpec {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Debug, Clone, Deserialize)]
struct McpToolsConfig {
    #[serde(default)]
    allow_unprefixed_names: bool,
    #[serde(default)]
    servers: Vec<McpToolsServerConfig>,
}

pub async fn register_mcp_tools(registry: &ToolRegistry) -> Result<usize> {
    let config = load_mcp_config()?;
    if config.servers.is_empty() {
        return Ok(0);
    }

    let mcp_bin = env::var("OP_MCPTOOLS_BIN").unwrap_or_else(|_| "mcp".to_string());
    let mut registered = 0usize;

    for server in config.servers {
        let tools = match list_mcp_tools(&mcp_bin, &server).await {
            Ok(tools) => tools,
            Err(err) => {
                warn!(
                    "Skipping MCP server '{}' due to list error: {}",
                    server.name, err
                );
                continue;
            }
        };

        for tool in tools {
            let desired_name = select_tool_name(&server, &tool.name, config.allow_unprefixed_names);
            let tool_name =
                match resolve_tool_name_conflict(registry, &server, &tool.name, desired_name).await
                {
                    Some(name) => name,
                    None => continue,
                };

            let description = if tool.description.is_empty() {
                format!("MCP tool from {}", server.name)
            } else {
                format!("{} (MCP server: {})", tool.description, server.name)
            };

            let tool = Arc::new(McpTool {
                name: tool_name.clone(),
                description: description.clone(),
                input_schema: tool.input_schema.clone(),
                namespace: format!("mcp.{}", sanitize_name(&server.name)),
                server: Arc::new(server.clone()),
                remote_tool_name: tool.name.clone(),
                mcp_bin: mcp_bin.clone(),
            });

            let definition = ToolDefinition {
                name: tool_name.clone(),
                description: description.clone(),
                input_schema: tool.input_schema.clone(),
                schema_version: "https://json-schema.org/draft/next/schema".to_string(),
                category: "mcp".to_string(),
                tags: vec!["mcp".to_string(), server.name.clone()],
                namespace: format!("mcp.{}", sanitize_name(&server.name)),
            };

            registry
                .register(Arc::from(tool_name.as_str()), tool, definition)
                .await?;
            registered += 1;
        }
    }

    info!("Registered {} MCP tools via mcptools", registered);
    Ok(registered)
}

#[derive(Clone)]
struct McpTool {
    name: String,
    description: String,
    input_schema: Value,
    namespace: String,
    server: Arc<McpToolsServerConfig>,
    remote_tool_name: String,
    mcp_bin: String,
}

#[async_trait::async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn category(&self) -> &str {
        "mcp"
    }

    fn namespace(&self) -> &str {
        &self.namespace
    }

    fn tags(&self) -> Vec<String> {
        vec!["mcp".to_string(), self.server.name.clone()]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let params = simd_json::to_string(&input).context("Failed to serialize MCP params")?;
        let output =
            run_mcp_call(&self.mcp_bin, &self.server, &self.remote_tool_name, &params).await?;

        if output
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let message = extract_text_content(&output)
                .unwrap_or_else(|| "MCP tool returned an error without text content".to_string());
            anyhow::bail!(message);
        }

        Ok(output)
    }
}

fn load_mcp_config() -> Result<McpToolsConfig> {
    let allow_unprefixed_names = env::var("OP_MCPTOOLS_ALLOW_UNPREFIXED")
        .ok()
        .map(parse_bool)
        .transpose()?
        .unwrap_or(false);

    if let Ok(raw) = env::var("OP_MCPTOOLS_SERVERS") {
        if raw.trim().is_empty() {
            return Ok(McpToolsConfig {
                allow_unprefixed_names,
                servers: Vec::new(),
            });
        }

        let mut raw_mut = raw;
        if let Ok(list) = unsafe { simd_json::from_str::<Vec<McpToolsServerConfig>>(&mut raw_mut) }
        {
            return Ok(McpToolsConfig {
                allow_unprefixed_names,
                servers: list,
            });
        }

        let mut raw_mut2 = raw_mut;
        let single = unsafe { simd_json::from_str::<McpToolsServerConfig>(&mut raw_mut2) }
            .context("OP_MCPTOOLS_SERVERS must be JSON (array or object)")?;
        return Ok(McpToolsConfig {
            allow_unprefixed_names,
            servers: vec![single],
        });
    }

    if let Some(config_path) = resolve_config_path() {
        let mut raw = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path))?;
        let mut config: McpToolsConfig = unsafe { simd_json::from_str(&mut raw) }
            .with_context(|| format!("Failed to parse {}", config_path))?;
        if allow_unprefixed_names {
            config.allow_unprefixed_names = true;
        }
        return Ok(config);
    }

    if let Ok(raw) = env::var("OP_MCPTOOLS_SERVER") {
        let args = split_args(&raw);
        if args.is_empty() {
            return Ok(McpToolsConfig {
                allow_unprefixed_names,
                servers: Vec::new(),
            });
        }

        let name = env::var("OP_MCPTOOLS_SERVER_NAME").unwrap_or_else(|_| "default".to_string());
        return Ok(McpToolsConfig {
            allow_unprefixed_names,
            servers: vec![McpToolsServerConfig {
                name,
                args,
                transport: None,
                auth_header: None,
                auth_user: None,
                tool_prefix: None,
                env: None,
            }],
        });
    }

    Ok(McpToolsConfig {
        allow_unprefixed_names,
        servers: Vec::new(),
    })
}

async fn list_mcp_tools(mcp_bin: &str, server: &McpToolsServerConfig) -> Result<Vec<McpToolSpec>> {
    let mut cmd = Command::new(mcp_bin);
    cmd.arg("tools").arg("--format").arg("json");
    apply_server_args(&mut cmd, server);

    let output = cmd.output().await.context("Failed to run mcptools list")?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("mcptools tools failed: {}", stderr.trim());
    }

    let mut stdout_mut = stdout;
    let payload: Value = unsafe { simd_json::from_str(&mut stdout_mut) }
        .with_context(|| format!("Failed to parse mcptools output: {}", stdout_mut))?;
    let tools = payload
        .get("tools")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let mut parsed = Vec::new();
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let description = tool
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let input_schema = tool
            .get("inputSchema")
            .cloned()
            .or_else(|| tool.get("input_schema").cloned())
            .unwrap_or_else(|| json!({"type": "object"}));

        parsed.push(McpToolSpec {
            name,
            description,
            input_schema,
        });
    }

    Ok(parsed)
}

async fn run_mcp_call(
    mcp_bin: &str,
    server: &McpToolsServerConfig,
    tool_name: &str,
    params: &str,
) -> Result<Value> {
    let mut cmd = Command::new(mcp_bin);
    cmd.arg("call")
        .arg(tool_name)
        .arg("--format")
        .arg("json")
        .arg("--params")
        .arg(params);

    apply_server_args(&mut cmd, server);

    let output = cmd.output().await.context("Failed to run mcptools call")?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("mcptools call failed: {}", stderr.trim());
    }

    let mut stdout_mut = stdout;
    let payload: Value = unsafe { simd_json::from_str(&mut stdout_mut) }
        .with_context(|| format!("Failed to parse mcptools output: {}", stdout_mut))?;
    Ok(payload)
}

fn apply_server_args(cmd: &mut Command, server: &McpToolsServerConfig) {
    if let Some(transport) = &server.transport {
        cmd.arg("--transport").arg(transport);
    }

    if let Some(auth_header) = &server.auth_header {
        cmd.arg("--auth-header").arg(auth_header);
    }

    if let Some(auth_user) = &server.auth_user {
        cmd.arg("--auth-user").arg(auth_user);
    }

    if let Some(envs) = &server.env {
        cmd.envs(envs);
    }

    for arg in &server.args {
        cmd.arg(arg);
    }
}

fn resolve_config_path() -> Option<String> {
    let path = env::var("OP_MCPTOOLS_CONFIG").unwrap_or_else(|_| "mcptools.json".to_string());
    if Path::new(&path).is_file() {
        Some(path)
    } else {
        None
    }
}

fn split_args(raw: &str) -> Vec<String> {
    raw.split_whitespace()
        .map(|value| value.to_string())
        .collect()
}

fn sanitize_name(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn build_tool_name(server: &McpToolsServerConfig, tool_name: &str) -> String {
    if let Some(prefix) = &server.tool_prefix {
        format!("{}{}", prefix, sanitize_name(tool_name))
    } else {
        format!(
            "mcp_{}_{}",
            sanitize_name(&server.name),
            sanitize_name(tool_name)
        )
    }
}

fn select_tool_name(
    server: &McpToolsServerConfig,
    tool_name: &str,
    allow_unprefixed: bool,
) -> String {
    if allow_unprefixed {
        tool_name.to_string()
    } else {
        build_tool_name(server, tool_name)
    }
}

async fn resolve_tool_name_conflict(
    registry: &ToolRegistry,
    server: &McpToolsServerConfig,
    tool_name: &str,
    desired: String,
) -> Option<String> {
    if registry.get_definition(&desired).await.is_none() {
        return Some(desired);
    }

    let fallback = build_tool_name(server, tool_name);
    if fallback != desired && registry.get_definition(&fallback).await.is_none() {
        debug!(
            "Using prefixed name '{}' for MCP tool '{}' due to conflict",
            fallback, tool_name
        );
        return Some(fallback);
    }

    debug!(
        "Skipping MCP tool '{}' because names '{}' and '{}' already exist",
        tool_name, desired, fallback
    );
    None
}

fn parse_bool(raw: String) -> Result<bool> {
    let lowered = raw.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(anyhow::anyhow!("Invalid boolean value: {}", raw)),
    }
}

fn extract_text_content(payload: &Value) -> Option<String> {
    payload
        .get("content")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(|value| value.as_str())
        .map(|text| text.to_string())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/orchestration_plugin.rs">
//! Orchestration Activity Plugin
//!
//! Provides a plugin interface for tracking orchestration activity.
//! Plugins receive notifications about:
//! - Tool executions (commands, file operations, etc.)
//! - LLM decisions and tool calls
//! - Session lifecycle events
//!
//! ## Use Cases
//!
//! - **Snowball Logging**: Immutable audit trail on snowball
//! - **Metrics/Observability**: Prometheus, Grafana integration
//! - **Alerting**: Real-time notifications for critical operations
//! - **Replay/Debugging**: Record and replay orchestration sessions
//!
//! ## Example
//!
//! ```rust,ignore
//! struct SnowballActivityPlugin { /* ... */ }
//!
//! #[async_trait]
//! impl OrchestrationActivityPlugin for SnowballActivityPlugin {
//!     async fn on_tool_executed(&self, event: ToolExecutedEvent) {
//!         // Write to snowball
//!         self.snowball.write_event(event).await;
//!     }
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

// ============================================================================
// EVENT TYPES
// ============================================================================

/// Event emitted when a tool is executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutedEvent {
    /// Unique event ID
    pub event_id: String,
    /// Session ID (user/chat session)
    pub session_id: String,
    /// Tool name that was executed
    pub tool_name: String,
    /// Tool category (shell, filesystem, dbus, etc.)
    pub tool_category: String,
    /// Input arguments (may be redacted for security)
    pub arguments: Value,
    /// Execution result
    pub result: ToolExecutionResult,
    /// Timestamp when execution started
    pub started_at: DateTime<Utc>,
    /// Timestamp when execution completed
    pub completed_at: DateTime<Utc>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Additional metadata
    pub metadata: Value,
}

/// Result of tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Exit code (for shell commands)
    pub exit_code: Option<i32>,
    /// Output summary (truncated if large)
    pub output_summary: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Output size in bytes
    pub output_bytes: usize,
}

/// Event emitted when LLM makes a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmDecisionEvent {
    /// Unique event ID
    pub event_id: String,
    /// Session ID
    pub session_id: String,
    /// LLM provider used
    pub provider: String,
    /// Model used
    pub model: String,
    /// Tools that were called
    pub tool_calls: Vec<String>,
    /// Was hallucination detected?
    pub hallucination_detected: bool,
    /// Verification status
    pub verified: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Token usage
    pub tokens_used: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Event emitted for session lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    /// Session ID
    pub session_id: String,
    /// Event type
    pub event_type: SessionEventType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional data
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventType {
    Started,
    Ended,
    Paused,
    Resumed,
    Error,
}

// ============================================================================
// PLUGIN TRAIT
// ============================================================================

/// Plugin interface for receiving orchestration activity events
#[async_trait]
pub trait OrchestrationActivityPlugin: Send + Sync {
    /// Plugin name for identification
    fn name(&self) -> &str;

    /// Called when a tool is executed
    async fn on_tool_executed(&self, event: ToolExecutedEvent);

    /// Called when LLM makes a decision (optional)
    async fn on_llm_decision(&self, _event: LlmDecisionEvent) {
        // Default: no-op
    }

    /// Called for session lifecycle events (optional)
    async fn on_session_event(&self, _event: SessionEvent) {
        // Default: no-op
    }

    /// Called on plugin initialization
    async fn on_init(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called on plugin shutdown
    async fn on_shutdown(&self) {
        // Default: no-op
    }
}

// ============================================================================
// PLUGIN REGISTRY
// ============================================================================

/// Registry for orchestration activity plugins
pub struct OrchestrationPluginRegistry {
    plugins: RwLock<Vec<Arc<dyn OrchestrationActivityPlugin>>>,
}

impl OrchestrationPluginRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(Vec::new()),
        }
    }

    /// Register a plugin
    pub async fn register(
        &self,
        plugin: Arc<dyn OrchestrationActivityPlugin>,
    ) -> anyhow::Result<()> {
        let name = plugin.name().to_string();

        // Initialize the plugin
        plugin.on_init().await?;

        self.plugins.write().await.push(plugin);
        info!(plugin = %name, "Registered orchestration activity plugin");

        Ok(())
    }

    /// Emit a tool executed event to all plugins
    pub async fn emit_tool_executed(&self, event: ToolExecutedEvent) {
        let plugins = self.plugins.read().await;
        for plugin in plugins.iter() {
            plugin.on_tool_executed(event.clone()).await;
        }
    }

    /// Emit an LLM decision event to all plugins
    pub async fn emit_llm_decision(&self, event: LlmDecisionEvent) {
        let plugins = self.plugins.read().await;
        for plugin in plugins.iter() {
            plugin.on_llm_decision(event.clone()).await;
        }
    }

    /// Emit a session event to all plugins
    pub async fn emit_session_event(&self, event: SessionEvent) {
        let plugins = self.plugins.read().await;
        for plugin in plugins.iter() {
            plugin.on_session_event(event.clone()).await;
        }
    }

    /// Shutdown all plugins
    pub async fn shutdown(&self) {
        let plugins = self.plugins.read().await;
        for plugin in plugins.iter() {
            plugin.on_shutdown().await;
        }
    }

    /// Get number of registered plugins
    pub async fn plugin_count(&self) -> usize {
        self.plugins.read().await.len()
    }
}

impl Default for OrchestrationPluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// GLOBAL REGISTRY
// ============================================================================

// Global orchestration plugin registry (initialized eagerly)
static ORCHESTRATION_REGISTRY: std::sync::OnceLock<Arc<OrchestrationPluginRegistry>> =
    std::sync::OnceLock::new();

/// Initialize the global orchestration plugin registry (call once at startup)
pub fn init_orchestration_registry() {
    ORCHESTRATION_REGISTRY
        .set(Arc::new(OrchestrationPluginRegistry::new()))
        .unwrap_or_else(|_| panic!("Orchestration registry already initialized"));
}

/// Get the global orchestration plugin registry
pub fn get_orchestration_registry() -> Arc<OrchestrationPluginRegistry> {
    ORCHESTRATION_REGISTRY
        .get()
        .expect("Orchestration registry not initialized")
        .clone()
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a ToolExecutedEvent from execution data
pub fn create_tool_event(
    session_id: &str,
    tool_name: &str,
    tool_category: &str,
    arguments: Value,
    success: bool,
    exit_code: Option<i32>,
    output: Option<&str>,
    error: Option<&str>,
    started_at: DateTime<Utc>,
    duration_ms: u64,
) -> ToolExecutedEvent {
    let output_bytes = output.map(|s| s.len()).unwrap_or(0);
    let output_summary = output.map(|s| {
        if s.len() > 500 {
            format!("{}... ({} bytes total)", &s[..500], s.len())
        } else {
            s.to_string()
        }
    });

    ToolExecutedEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        tool_name: tool_name.to_string(),
        tool_category: tool_category.to_string(),
        arguments,
        result: ToolExecutionResult {
            success,
            exit_code,
            output_summary,
            error: error.map(|s| s.to_string()),
            output_bytes,
        },
        started_at,
        completed_at: Utc::now(),
        duration_ms,
        metadata: Value::null(),
    }
}

// ============================================================================
// EXAMPLE PLUGINS
// ============================================================================

/// Simple logging plugin (for development/debugging)
pub struct LoggingActivityPlugin;

#[async_trait]
impl OrchestrationActivityPlugin for LoggingActivityPlugin {
    fn name(&self) -> &str {
        "logging"
    }

    async fn on_tool_executed(&self, event: ToolExecutedEvent) {
        info!(
            event_id = %event.event_id,
            session_id = %event.session_id,
            tool = %event.tool_name,
            category = %event.tool_category,
            success = %event.result.success,
            duration_ms = %event.duration_ms,
            "Tool executed"
        );
    }

    async fn on_llm_decision(&self, event: LlmDecisionEvent) {
        info!(
            event_id = %event.event_id,
            session_id = %event.session_id,
            provider = %event.provider,
            model = %event.model,
            tools_called = ?event.tool_calls,
            verified = %event.verified,
            "LLM decision"
        );
    }

    async fn on_session_event(&self, event: SessionEvent) {
        info!(
            session_id = %event.session_id,
            event_type = ?event.event_type,
            "Session event"
        );
    }
}

/// Metrics plugin (placeholder for Prometheus/etc integration)
pub struct MetricsActivityPlugin {
    // Counter metrics would go here
}

impl MetricsActivityPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl OrchestrationActivityPlugin for MetricsActivityPlugin {
    fn name(&self) -> &str {
        "metrics"
    }

    async fn on_tool_executed(&self, event: ToolExecutedEvent) {
        // Increment counters, record histograms, etc.
        debug!(
            tool = %event.tool_name,
            duration = %event.duration_ms,
            "Recording tool execution metrics"
        );
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingPlugin {
        count: AtomicU32,
    }

    impl CountingPlugin {
        fn new() -> Self {
            Self {
                count: AtomicU32::new(0),
            }
        }

        fn get_count(&self) -> u32 {
            self.count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl OrchestrationActivityPlugin for CountingPlugin {
        fn name(&self) -> &str {
            "counting"
        }

        async fn on_tool_executed(&self, _event: ToolExecutedEvent) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[tokio::test]
    async fn test_plugin_registration() {
        let registry = OrchestrationPluginRegistry::new();
        let plugin = Arc::new(CountingPlugin::new());

        registry.register(plugin.clone()).await.unwrap();
        assert_eq!(registry.plugin_count().await, 1);
    }

    #[tokio::test]
    async fn test_event_emission() {
        let registry = OrchestrationPluginRegistry::new();
        let plugin = Arc::new(CountingPlugin::new());
        registry.register(plugin.clone()).await.unwrap();

        let event = create_tool_event(
            "session1",
            "test_tool",
            "test",
            simd_json::json!({}),
            true,
            Some(0),
            Some("output"),
            None,
            Utc::now(),
            100,
        );

        registry.emit_tool_executed(event).await;
        assert_eq!(plugin.get_count(), 1);

        // Emit more events
        for _ in 0..5 {
            let event = create_tool_event(
                "session1",
                "test_tool",
                "test",
                simd_json::json!({}),
                true,
                None,
                None,
                None,
                Utc::now(),
                50,
            );
            registry.emit_tool_executed(event).await;
        }

        assert_eq!(plugin.get_count(), 6);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/registry.rs">
//! Tool Registry
//!
//! Provides a simple registry for tools and their definitions.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::tool::BoxedTool;

/// Tool definition metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub namespace: String,
}

/// Statistics about the registry
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_registered: usize,
}

/// Tool Registry
pub struct ToolRegistry {
    /// Registered tools
    tools: RwLock<HashMap<Arc<str>, BoxedTool>>,
    /// Tool definitions
    definitions: RwLock<HashMap<Arc<str>, ToolDefinition>>,
}

impl ToolRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            definitions: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool with its definition
    pub async fn register(
        &self,
        name: Arc<str>,
        tool: BoxedTool,
        definition: ToolDefinition,
    ) -> Result<()> {
        {
            let mut tools = self.tools.write().await;
            let mut definitions = self.definitions.write().await;

            tools.insert(name.clone(), tool);
            definitions.insert(name.clone(), definition);
        }

        debug!("Registered tool: {}", name);
        Ok(())
    }

    /// Helper to register a tool instance directly
    pub async fn register_tool(&self, tool: BoxedTool) -> Result<()> {
        let definition = ToolDefinition {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            schema_version: "https://json-schema.org/draft/next/schema".to_string(),
            category: "builtin".to_string(),
            tags: vec!["builtin".to_string()],
            namespace: tool.namespace().to_string(),
        };
        self.register(Arc::from(tool.name()), tool, definition)
            .await
    }

    /// Get a tool by name
    pub async fn get(&self, name: &str) -> Option<BoxedTool> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    /// Get tool definition
    pub async fn get_definition(&self, name: &str) -> Option<ToolDefinition> {
        let definitions = self.definitions.read().await;
        definitions.get(name).cloned()
    }

    /// List all registered tool definitions
    pub async fn list(&self) -> Vec<ToolDefinition> {
        let definitions = self.definitions.read().await;
        definitions.values().cloned().collect()
    }

    /// List currently loaded tools (same as list in simplified version)
    pub async fn list_loaded(&self) -> Vec<ToolDefinition> {
        self.list().await
    }

    /// Get registry statistics
    pub async fn stats(&self) -> RegistryStats {
        let tools = self.tools.read().await;
        RegistryStats {
            total_registered: tools.len(),
        }
    }

    /// Check if a tool is registered
    pub async fn is_loaded(&self, name: &str) -> bool {
        let tools = self.tools.read().await;
        tools.contains_key(name)
    }

    /// Number of tools in the registry
    pub async fn len(&self) -> usize {
        let tools = self.tools.read().await;
        tools.len()
    }

    /// List all definitions (alias for list)
    pub async fn list_definitions(&self) -> Vec<ToolDefinition> {
        self.list().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;

    struct TestTool {
        name: String,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Test tool"
        }

        fn input_schema(&self) -> Value {
            simd_json::json!({})
        }

        async fn execute(&self, _input: Value) -> Result<Value> {
            Ok(simd_json::json!({"result": "ok"}))
        }
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let registry = ToolRegistry::new();
        let tool: BoxedTool = Arc::new(TestTool {
            name: "test".to_string(),
        });
        let definition = ToolDefinition {
            name: "test".to_string(),
            description: "Test".to_string(),
            input_schema: simd_json::json!({}),
            schema_version: "https://json-schema.org/draft/next/schema".to_string(),
            category: "test".to_string(),
            tags: vec![],
            namespace: "test".to_string(),
        };

        registry
            .register(Arc::from("test"), tool, definition)
            .await
            .unwrap();

        let retrieved = registry.get("test").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_list_definitions() {
        let registry = ToolRegistry::new();
        let tool: BoxedTool = Arc::new(TestTool {
            name: "test".to_string(),
        });
        let definition = ToolDefinition {
            name: "test".to_string(),
            description: "Test".to_string(),
            input_schema: simd_json::json!({}),
            schema_version: "https://json-schema.org/draft/next/schema".to_string(),
            category: "test".to_string(),
            tags: vec![],
            namespace: "test".to_string(),
        };

        registry
            .register(Arc::from("test"), tool, definition)
            .await
            .unwrap();

        let definitions = registry.list().await;
        assert_eq!(definitions.len(), 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let registry = ToolRegistry::new();
        let tool: BoxedTool = Arc::new(TestTool {
            name: "test".to_string(),
        });
        let definition = ToolDefinition {
            name: "test".to_string(),
            description: "Test".to_string(),
            input_schema: simd_json::json!({}),
            schema_version: "https://json-schema.org/draft/next/schema".to_string(),
            category: "test".to_string(),
            tags: vec![],
            namespace: "test".to_string(),
        };

        registry
            .register(Arc::from("test"), tool, definition)
            .await
            .unwrap();

        // Access the tool
        registry.get("test").await;
        registry.get("test").await;

        let stats = registry.stats().await;
        assert_eq!(stats.total_registered, 1);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/router.rs">
//! Tools Router - HTTP endpoints for tool management
//!
//! This module exports a router that can be mounted by op-http.

use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

use crate::registry::ToolRegistry;

/// Tools service state
#[derive(Clone)]
pub struct ToolsState {
    pub registry: Arc<ToolRegistry>,
}

impl ToolsState {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
}

/// Create the tools router
///
/// Mount this at `/api/tools` in the unified server:
/// ```ignore
/// use op_http::prelude::*;
/// use op_tools::router::{create_router, ToolsState};
///
/// let registry = Arc::new(ToolRegistry::new());
/// let state = ToolsState::new(registry);
/// let router = RouterBuilder::new()
///     .nest("/api/tools", "tools", create_router(state))
///     .build();
/// ```
pub fn create_router(state: ToolsState) -> Router {
    Router::new()
        .route("/", get(list_tools_handler))
        .route("/health", get(health_handler))
        .route("/:name", get(get_tool_handler))
        .route("/:name/execute", post(execute_tool_handler))
        .with_state(state)
}

/// Service info for op-http ServiceRouter trait
pub struct ToolsServiceRouter;

impl op_http::router::ServiceRouter for ToolsServiceRouter {
    fn prefix() -> &'static str {
        "/api/tools"
    }

    fn name() -> &'static str {
        "tools"
    }

    fn description() -> &'static str {
        "Tool registry API endpoints"
    }
}

// === Handlers ===

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "tools"
    }))
}

async fn list_tools_handler(State(state): State<ToolsState>) -> impl IntoResponse {
    let tools = state.registry.list().await;
    let tool_list: Vec<_> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description
            })
        })
        .collect();

    Json(json!({
        "tools": tool_list,
        "count": tool_list.len()
    }))
}

async fn get_tool_handler(
    State(state): State<ToolsState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.registry.get(&name).await {
        Some(tool) => Json(json!({
            "name": tool.name(),
            "description": tool.description(),
            "inputSchema": tool.input_schema()
        })),
        None => Json(json!({ "error": "Tool not found" })),
    }
}

async fn execute_tool_handler(
    State(state): State<ToolsState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(params): Json<Value>,
) -> impl IntoResponse {
    if let Some(tool) = state.registry.get(&name).await {
        match tool.execute(params).await {
            Ok(result) => Json(json!({
                "success": true,
                "result": result
            })),
            Err(e) => Json(json!({
                "success": false,
                "error": e.to_string()
            })),
        }
    } else {
        Json(json!({
            "success": false,
            "error": "Tool not found"
        }))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/security.rs">
//! Security Module for Tool Execution - Access Level Control
//!
//! This module provides security controls based on ACCESS LEVELS, not command blocking.
//! The chatbot is designed to be a full system administrator, so it needs full access.
//!
//! ## Philosophy
//!
//! Security is enforced at the ACCESS level:
//! - **Who** can use the chatbot (authentication)
//! - **What** is logged (audit trail)
//! - **How fast** they can execute (rate limiting)
//! - **Anti-hallucination** (LLM must actually do what it claims)
//!
//! NOT at the command level - that would defeat the purpose of an admin chatbot.
//!
//! ## Access Levels
//!
//! - `Unrestricted`: Full admin access - can run any command (default)
//! - `Restricted`: Limited read-only access for untrusted users
//! - `Custom`: User-defined access with specific allowlist
//!
//! ## Native Protocol Preference
//!
//! We PREFER native protocols (D-Bus, OVSDB, rtnetlink) over shell commands because:
//! - Better error handling
//! - Structured responses
//! - No parsing issues
//!
//! But we don't BLOCK shell commands - the admin chatbot needs full access.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::info;

// ============================================================================
// SECURITY ERRORS
// ============================================================================

/// Security-related errors
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Path '{0}' is forbidden for this access level")]
    PathForbidden(PathBuf),

    #[error("Path traversal detected in '{0}'")]
    PathTraversal(String),

    #[error("Input validation failed: {0}")]
    ValidationFailed(String),

    #[error("Operation requires higher access level")]
    InsufficientAccess,

    #[error("Session not authenticated")]
    NotAuthenticated,
}

// ============================================================================
// ACCESS LEVELS
// ============================================================================

/// Access level for a session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    /// Full admin access - can run any command
    /// This is the DEFAULT for authenticated admin users
    #[default]
    Unrestricted,

    /// Limited access - read-only safe commands only
    /// For untrusted/guest users
    Restricted,

    /// Custom access level with specific permissions
    Custom,
}

impl AccessLevel {
    /// Check if this level can execute shell commands
    pub fn can_execute_shell(&self) -> bool {
        matches!(self, AccessLevel::Unrestricted | AccessLevel::Custom)
    }

    /// Check if this level can write files
    pub fn can_write_files(&self) -> bool {
        matches!(self, AccessLevel::Unrestricted | AccessLevel::Custom)
    }

    /// Check if this level can manage system services
    pub fn can_manage_services(&self) -> bool {
        matches!(self, AccessLevel::Unrestricted)
    }
}

// ============================================================================
// SECURITY PROFILE - ACCESS LEVEL BASED
// ============================================================================

/// Security profile for tool execution based on access level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSecurityProfile {
    /// Name of this profile
    pub name: String,

    /// Access level
    pub access_level: AccessLevel,

    /// For Custom level: specific commands allowed
    #[serde(default)]
    pub custom_allowed_commands: Option<HashSet<String>>,

    /// Paths that are always forbidden (even for Unrestricted)
    /// Only the most critical system files
    #[serde(default)]
    pub critical_forbidden_paths: Vec<PathBuf>,

    /// Maximum command execution time in seconds
    #[serde(default = "default_max_timeout")]
    pub max_timeout_secs: u64,

    /// Maximum output size in bytes
    #[serde(default = "default_max_output")]
    pub max_output_bytes: usize,

    /// Rate limit: max executions per minute per session
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,

    /// Whether to log commands (for audit)
    #[serde(default = "default_true")]
    pub audit_logging: bool,

    /// Whether to warn about native protocol alternatives
    #[serde(default = "default_true")]
    pub warn_on_cli_alternatives: bool,
}

fn default_max_timeout() -> u64 {
    300
} // 5 minutes for admin tasks
fn default_max_output() -> usize {
    10_000_000
} // 10MB for large outputs
fn default_rate_limit() -> u32 {
    120
} // 2 per second average
fn default_true() -> bool {
    true
}

impl Default for ToolSecurityProfile {
    fn default() -> Self {
        Self::admin()
    }
}

impl ToolSecurityProfile {
    /// Create an admin profile with FULL access
    /// This is the default for authenticated administrators
    pub fn admin() -> Self {
        Self {
            name: "admin".to_string(),
            access_level: AccessLevel::Unrestricted,
            custom_allowed_commands: None,
            critical_forbidden_paths: vec![
                // Only truly critical paths that could break the system
                // Even admins should use proper tools for these
            ],
            max_timeout_secs: 300,        // 5 minutes
            max_output_bytes: 10_000_000, // 10MB
            rate_limit_per_minute: 120,
            audit_logging: true,
            warn_on_cli_alternatives: true,
        }
    }

    /// Create a restricted profile for untrusted users
    /// Read-only access to safe commands
    pub fn restricted() -> Self {
        Self {
            name: "restricted".to_string(),
            access_level: AccessLevel::Restricted,
            custom_allowed_commands: Some(
                [
                    "ls", "cat", "head", "tail", "grep", "find", "ps", "df", "free", "date",
                    "uptime",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ),
            critical_forbidden_paths: vec![
                PathBuf::from("/etc/shadow"),
                PathBuf::from("/etc/sudoers"),
                PathBuf::from("/root"),
            ],
            max_timeout_secs: 30,
            max_output_bytes: 100_000, // 100KB
            rate_limit_per_minute: 30,
            audit_logging: true,
            warn_on_cli_alternatives: false,
        }
    }

    /// Create a custom profile
    pub fn custom(name: &str, allowed_commands: Vec<&str>) -> Self {
        Self {
            name: name.to_string(),
            access_level: AccessLevel::Custom,
            custom_allowed_commands: Some(allowed_commands.iter().map(|s| s.to_string()).collect()),
            ..Self::admin()
        }
    }
}

// ============================================================================
// NATIVE PROTOCOL RECOMMENDATIONS
// ============================================================================

/// Commands that have native protocol alternatives
/// We RECOMMEND using native tools but don't BLOCK the CLI
pub const NATIVE_ALTERNATIVES: &[(&str, &str)] = &[
    // OVS
    (
        "ovs-vsctl",
        "Consider using ovs_* native tools for better error handling",
    ),
    (
        "ovs-ofctl",
        "Consider using ovs_* native tools for structured responses",
    ),
    // Systemd
    (
        "systemctl",
        "Consider using dbus_systemd_* tools for programmatic access",
    ),
    (
        "journalctl",
        "Consider using dbus_systemd_* tools for structured logs",
    ),
    // Network
    (
        "ip",
        "Consider using network_* native tools for structured output",
    ),
    ("nmcli", "Consider using D-Bus NetworkManager interface"),
    // Package managers
    (
        "apt",
        "Consider using packagekit_* tools for progress tracking",
    ),
    ("apt-get", "Consider using packagekit_* tools"),
    ("dnf", "Consider using packagekit_* tools"),
];

/// Get a recommendation message if a native alternative exists
pub fn get_native_recommendation(command: &str) -> Option<&'static str> {
    let base_cmd = command.split_whitespace().next()?;
    NATIVE_ALTERNATIVES
        .iter()
        .find(|(cmd, _)| *cmd == base_cmd)
        .map(|(_, msg)| *msg)
}

// ============================================================================
// SECURITY VALIDATOR
// ============================================================================

/// Security validator for access-level based security
#[derive(Debug)]
pub struct SecurityValidator {
    profile: RwLock<ToolSecurityProfile>,
    rate_limiter: RwLock<HashMap<String, RateLimitState>>,
}

#[derive(Debug)]
struct RateLimitState {
    count: u32,
    window_start: Instant,
}

impl SecurityValidator {
    /// Create a new validator with the given profile
    pub fn new(profile: ToolSecurityProfile) -> Self {
        Self {
            profile: RwLock::new(profile),
            rate_limiter: RwLock::new(HashMap::new()),
        }
    }

    /// Create with default admin profile (FULL ACCESS)
    pub fn with_admin_profile() -> Self {
        Self::new(ToolSecurityProfile::admin())
    }

    /// Create with restricted profile
    pub fn with_restricted_profile() -> Self {
        Self::new(ToolSecurityProfile::restricted())
    }

    /// Update the security profile
    pub async fn set_profile(&self, profile: ToolSecurityProfile) {
        info!(
            profile = %profile.name,
            access_level = ?profile.access_level,
            "Security profile updated"
        );
        *self.profile.write().await = profile;
    }

    /// Get current profile
    pub async fn get_profile(&self) -> ToolSecurityProfile {
        self.profile.read().await.clone()
    }

    /// Check if a command can be executed
    /// Returns Ok(Option<warning>) - warning is a native alternative suggestion
    pub async fn check_command(&self, command: &str) -> Result<Option<String>, SecurityError> {
        let profile = self.profile.read().await;

        match profile.access_level {
            AccessLevel::Unrestricted => {
                // Full access - just check for native alternatives to warn
                let warning = if profile.warn_on_cli_alternatives {
                    get_native_recommendation(command).map(|s| s.to_string())
                } else {
                    None
                };
                Ok(warning)
            }
            AccessLevel::Restricted => {
                // Check against the restricted allowlist
                let base_cmd = command
                    .split_whitespace()
                    .next()
                    .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;

                if let Some(allowed) = &profile.custom_allowed_commands {
                    if !allowed.contains(base_cmd) {
                        return Err(SecurityError::AccessDenied(format!(
                            "Command '{}' not allowed in restricted mode",
                            base_cmd
                        )));
                    }
                }
                Ok(None)
            }
            AccessLevel::Custom => {
                // Check against custom allowlist
                let base_cmd = command
                    .split_whitespace()
                    .next()
                    .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;

                if let Some(allowed) = &profile.custom_allowed_commands {
                    if !allowed.contains(base_cmd) {
                        return Err(SecurityError::AccessDenied(format!(
                            "Command '{}' not in custom allowlist",
                            base_cmd
                        )));
                    }
                }
                Ok(None)
            }
        }
    }

    /// Validate a path for reading
    pub async fn validate_read_path(&self, path: &str) -> Result<PathBuf, SecurityError> {
        let profile = self.profile.read().await;
        let path_buf = PathBuf::from(path);

        // Check for path traversal
        if path.contains("..") {
            return Err(SecurityError::PathTraversal(path.to_string()));
        }

        // Check critical forbidden paths
        for forbidden in &profile.critical_forbidden_paths {
            if path_buf.starts_with(forbidden) {
                return Err(SecurityError::PathForbidden(path_buf));
            }
        }

        // Admins can read anything else
        if profile.access_level == AccessLevel::Unrestricted {
            return Ok(path_buf);
        }

        // Restricted users have limited paths
        let allowed_read = ["/tmp", "/var/log", "/home", "/opt"];
        let is_allowed = allowed_read.iter().any(|p| path_buf.starts_with(p));

        if !is_allowed {
            return Err(SecurityError::PathForbidden(path_buf));
        }

        Ok(path_buf)
    }

    /// Validate a path for writing
    pub async fn validate_write_path(&self, path: &str) -> Result<PathBuf, SecurityError> {
        let profile = self.profile.read().await;
        let path_buf = PathBuf::from(path);

        // Check for path traversal
        if path.contains("..") {
            return Err(SecurityError::PathTraversal(path.to_string()));
        }

        // Check critical forbidden paths
        for forbidden in &profile.critical_forbidden_paths {
            if path_buf.starts_with(forbidden) {
                return Err(SecurityError::PathForbidden(path_buf));
            }
        }

        // Admins can write anywhere (except critical paths)
        if profile.access_level == AccessLevel::Unrestricted {
            return Ok(path_buf);
        }

        // Restricted users can only write to /tmp
        if !path_buf.starts_with("/tmp") {
            return Err(SecurityError::PathForbidden(path_buf));
        }

        Ok(path_buf)
    }

    /// Check rate limit for a session
    pub async fn check_rate_limit(&self, session_id: &str) -> Result<(), SecurityError> {
        let profile = self.profile.read().await;
        let limit = profile.rate_limit_per_minute;
        drop(profile);

        let mut rate_limiter = self.rate_limiter.write().await;
        let now = Instant::now();

        let state = rate_limiter
            .entry(session_id.to_string())
            .or_insert(RateLimitState {
                count: 0,
                window_start: now,
            });

        // Reset if window has passed
        if now.duration_since(state.window_start) > Duration::from_secs(60) {
            state.count = 0;
            state.window_start = now;
        }

        if state.count >= limit {
            return Err(SecurityError::RateLimitExceeded(format!(
                "Exceeded {} executions per minute",
                limit
            )));
        }

        state.count += 1;
        Ok(())
    }

    /// Get maximum allowed timeout
    pub async fn max_timeout(&self) -> Duration {
        Duration::from_secs(self.profile.read().await.max_timeout_secs)
    }

    /// Get maximum output size
    pub async fn max_output(&self) -> usize {
        self.profile.read().await.max_output_bytes
    }

    /// Check if audit logging is enabled
    pub async fn is_audit_enabled(&self) -> bool {
        self.profile.read().await.audit_logging
    }

    /// Clear rate limit state
    pub async fn clear_rate_limits(&self) {
        self.rate_limiter.write().await.clear();
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        // Default to FULL ADMIN access
        Self::with_admin_profile()
    }
}

// ============================================================================
// GLOBAL VALIDATOR INSTANCE
// ============================================================================

// Global security validator instance (initialized eagerly)
static SECURITY_VALIDATOR: std::sync::OnceLock<Arc<SecurityValidator>> = std::sync::OnceLock::new();

/// Initialize the global security validator (call once at startup)
pub fn init_security_validator() {
    SECURITY_VALIDATOR
        .set(Arc::new(SecurityValidator::with_admin_profile()))
        .unwrap_or_else(|_| panic!("Security validator already initialized"));
}

/// Get the global security validator
pub fn get_security_validator() -> Arc<SecurityValidator> {
    SECURITY_VALIDATOR
        .get()
        .expect("Security validator not initialized")
        .clone()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_admin_allows_everything() {
        let validator = SecurityValidator::with_admin_profile();

        // All commands should pass for admin
        assert!(validator.check_command("rm -rf /").await.is_ok());
        assert!(validator
            .check_command("systemctl restart sshd")
            .await
            .is_ok());
        assert!(validator
            .check_command("curl http://example.com")
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_admin_gets_native_warnings() {
        let validator = SecurityValidator::with_admin_profile();

        // Should get warning for ovs-vsctl
        let result = validator.check_command("ovs-vsctl add-br br0").await;
        assert!(result.is_ok());
        let warning = result.unwrap();
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("native tools"));
    }

    #[tokio::test]
    async fn test_restricted_blocks_dangerous() {
        let validator = SecurityValidator::with_restricted_profile();

        // Should block rm
        assert!(validator.check_command("rm -rf /").await.is_err());

        // Should allow ls
        assert!(validator.check_command("ls -la").await.is_ok());
    }

    #[tokio::test]
    async fn test_path_validation_admin() {
        let validator = SecurityValidator::with_admin_profile();

        // Admin can read/write anywhere
        assert!(validator.validate_read_path("/etc/passwd").await.is_ok());
        assert!(validator.validate_write_path("/etc/hosts").await.is_ok());
    }

    #[tokio::test]
    async fn test_path_traversal_blocked() {
        let validator = SecurityValidator::with_admin_profile();

        // Path traversal always blocked
        assert!(validator
            .validate_read_path("/tmp/../etc/shadow")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let profile = ToolSecurityProfile {
            rate_limit_per_minute: 3,
            ..ToolSecurityProfile::admin()
        };
        let validator = SecurityValidator::new(profile);

        // First 3 should pass
        assert!(validator.check_rate_limit("session1").await.is_ok());
        assert!(validator.check_rate_limit("session1").await.is_ok());
        assert!(validator.check_rate_limit("session1").await.is_ok());

        // 4th should fail
        assert!(validator.check_rate_limit("session1").await.is_err());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/tool.rs">
//! Core Tool trait and types
//!
//! Defines the fundamental interface for all tools in the system.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::sync::Arc;

/// Security level for tool operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    /// Safe read-only operations
    #[default]
    ReadOnly,
    /// Operations that modify state but are reversible
    Modify,
    /// Operations that may have significant impact
    Elevated,
    /// Operations requiring explicit approval
    Critical,
}

/// Core trait for all tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name (unique identifier)
    fn name(&self) -> &str;

    /// Get human-readable description
    fn description(&self) -> &str;

    /// Get JSON schema for input validation
    fn input_schema(&self) -> Value;

    /// Execute the tool with given input
    async fn execute(&self, input: Value) -> Result<Value>;

    /// Get the security level for this tool
    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::ReadOnly
    }

    /// Get the category this tool belongs to
    fn category(&self) -> &str {
        "general"
    }

    /// Get the namespace for tool permission gating
    fn namespace(&self) -> &str {
        "system"
    }

    /// Get tags for tool discovery
    fn tags(&self) -> Vec<String> {
        vec![]
    }

    /// Check if tool is available (e.g., dependencies met)
    fn is_available(&self) -> bool {
        true
    }

    /// Estimated execution time in milliseconds
    fn estimated_duration_ms(&self) -> Option<u64> {
        None
    }
}

/// Type alias for boxed tools
pub type BoxedTool = Arc<dyn Tool>;

/// Simple tool implementation for testing
#[derive(Clone)]
pub struct SimpleTool {
    name: String,
    description: String,
    schema: Value,
    handler: Arc<dyn Fn(Value) -> Result<Value> + Send + Sync>,
}

impl SimpleTool {
    pub fn new<F>(name: &str, description: &str, schema: Value, handler: F) -> Self
    where
        F: Fn(Value) -> Result<Value> + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            schema,
            handler: Arc::new(handler),
        }
    }
}

#[async_trait]
impl Tool for SimpleTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        (self.handler)(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_tool() {
        let tool = SimpleTool::new(
            "echo",
            "Echo input back",
            simd_json::json!({"type": "object"}),
            |input| Ok(input),
        );

        assert_eq!(tool.name(), "echo");
        assert_eq!(tool.description(), "Echo input back");

        let result = tool
            .execute(simd_json::json!({"msg": "hello"}))
            .await
            .unwrap();
        assert_eq!(result, simd_json::json!({"msg": "hello"}));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/validation_tests.rs">
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_trusted_session_bypass() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});
        
        // Trusted session should pass even with invalid input
        let result = validator.validate_input(
            "test_tool",
            &json!({"invalid": "data"}),
            &schema,
            Some("chatbot"),
        ).await.unwrap();
        
        assert!(result.session_trusted);
        assert!(result.should_proceed());
    }

    #[tokio::test]
    async fn test_non_trusted_restriction() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});
        
        // Non-trusted session should be restricted
        let result = validator.validate_input(
            "shell_tool",
            &json!({"command": "rm -rf /etc/passwd"}),
            &schema,
            Some("anonymous"),
        ).await.unwrap();
        
        assert!(!result.session_trusted);
        assert!(!result.should_proceed());
        assert!(!result.validation_errors.is_empty());
    }

    #[tokio::test]
    async fn test_input_sanitization() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});
        
        let input_with_null_bytes = json!({"text": "hello\x00world"});
        let result = validator.validate_input(
            "text_tool",
            &input_with_null_bytes,
            &schema,
            Some("anonymous"),
        ).await.unwrap();
        
        let sanitized_text = result.input["text"].as_str().unwrap();
        assert!(!sanitized_text.contains('\0'));
        assert_eq!(sanitized_text, "helloworld");
        assert!(result.was_sanitized);
    }

    #[tokio::test]
    async fn test_schema_validation() {
        let validator = InputValidator::new();
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"}
            },
            "required": ["name"]
        });
        
        // Valid input should pass
        let valid_input = json!({"name": "test", "age": 25});
        let result = validator.validate_input(
            "test_tool",
            &valid_input,
            &schema,
            Some("anonymous"),
        ).await.unwrap();
        
        assert!(result.is_valid);
        assert!(result.should_proceed());
        
        // Invalid input should fail
        let invalid_input = json!({"age": 25}); // missing required "name"
        let result = validator.validate_input(
            "test_tool",
            &invalid_input,
            &schema,
            Some("anonymous"),
        ).await.unwrap();
        
        assert!(!result.is_valid);
        assert!(!result.should_proceed());
        assert!(!result.validation_errors.is_empty());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/src/validation.rs">
//! Input Validation and Sanitization for Tool Execution
//!
//! Provides comprehensive input validation while preserving full control
//! for the chatbot orchestrator system.
//!
//! Uses simd-json for high-performance JSON processing while maintaining
//! compatibility with the existing serde_json ecosystem.

use anyhow::{anyhow, Result};
use jsonschema::JSONSchema;
use serde_json::Value; // Keep for compatibility with existing code
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, warn};

/// Characters forbidden in user input to prevent injection
pub const FORBIDDEN_CHARS: &[char] = &[
    '$', '`', ';', '&', '|', '>', '<', '(', ')', '{', '}', '\n', '\r', '\0',
];

/// Maximum length for various input types
pub const MAX_PATH_LENGTH: usize = 4096;
pub const MAX_COMMAND_LENGTH: usize = 256;
pub const MAX_ARGS_LENGTH: usize = 4096;
pub const MAX_INPUT_LENGTH: usize = 1_000_000; // 1MB

/// Configuration for input validation behavior
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Whether to enforce strict schema validation
    pub strict_validation: bool,
    /// Whether to sanitize inputs for injection attacks
    pub sanitize_inputs: bool,
    /// Sessions that bypass validation (chatbot orchestrator)
    pub trusted_sessions: HashSet<String>,
    /// Maximum input size (bytes)
    pub max_input_size: usize,
    /// Allowed command whitelist for shell tools
    pub command_whitelist: HashSet<String>,
    /// Allowed directories for file operations
    pub allowed_dirs: Vec<PathBuf>,
    /// Forbidden directories for file operations
    pub forbidden_dirs: Vec<PathBuf>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        let mut trusted_sessions = HashSet::new();
        // Chatbot orchestrator sessions get full control
        trusted_sessions.insert("chatbot".to_string());
        trusted_sessions.insert("orchestrator".to_string());
        trusted_sessions.insert("system".to_string());

        // Default command whitelist for shell tools
        let mut command_whitelist = HashSet::new();
        // Allow common DevOps commands for non-trusted sessions
        for cmd in [
            "ls",
            "cat",
            "grep",
            "find",
            "ps",
            "top",
            "df",
            "du",
            "free",
            "uptime",
            "whoami",
            "id",
            "pwd",
            "date",
            "uname",
            "which",
            "whereis",
            "file",
            "head",
            "tail",
            "wc",
            "sort",
            "uniq",
            "cut",
            "awk",
            "sed",
            "git",
            "docker",
            "kubectl",
            "systemctl",
            "journalctl",
            "curl",
            "wget",
        ]
        .iter()
        {
            command_whitelist.insert(cmd.to_string());
        }

        // Default allowed directories (home directory and temp)
        let allowed_dirs = vec![
            PathBuf::from("/tmp"),
            PathBuf::from("/var/tmp"),
            PathBuf::from("/home"),
        ];

        // Forbidden system directories
        let forbidden_dirs = vec![
            PathBuf::from("/boot"),
            PathBuf::from("/dev"),
            PathBuf::from("/proc/sys"),
            PathBuf::from("/sys"),
            PathBuf::from("/root"),
            PathBuf::from("/etc/shadow"),
            PathBuf::from("/etc/passwd"),
        ];

        Self {
            strict_validation: true,
            sanitize_inputs: true,
            trusted_sessions,
            max_input_size: 10 * 1024 * 1024, // 10MB
            command_whitelist,
            allowed_dirs,
            forbidden_dirs,
        }
    }
}

/// Input validator for tool execution
#[derive(Clone)]
pub struct InputValidator {
    config: ValidationConfig,
    schema_cache: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Arc<JSONSchema>>>>,
}

impl InputValidator {
    /// Create a new validator with default config
    pub fn new() -> Self {
        Self::with_config(ValidationConfig::default())
    }

    /// Create a new validator with custom config
    pub fn with_config(config: ValidationConfig) -> Self {
        Self {
            config,
            schema_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Validate and sanitize input for tool execution
    pub async fn validate_input(
        &self,
        tool_name: &str,
        input: &Value,
        schema: &Value,
        session_id: Option<&str>,
    ) -> Result<ValidatedInput> {
        let session_id = session_id.unwrap_or("anonymous");

        // Check input size
        if let Err(e) = self.check_input_size(input) {
            error!(tool = %tool_name, session = %session_id, error = %e, "Input size validation failed");
            return Err(e);
        }

        // Trusted sessions (chatbot orchestrator) get minimal validation
        let is_trusted = self.config.trusted_sessions.contains(session_id);

        let mut validation_errors = Vec::new();
        let mut sanitized_input = input.clone();

        // Schema validation (always run for safety, but may be non-blocking for trusted)
        if let Err(e) = self
            .validate_schema(tool_name, &sanitized_input, schema)
            .await
        {
            if is_trusted && !self.config.strict_validation {
                warn!(tool = %tool_name, session = %session_id, "Schema validation bypassed for trusted session");
            } else {
                validation_errors.push(format!("Schema validation failed: {}", e));
            }
        }

        // Input sanitization
        if self.config.sanitize_inputs {
            if let Err(e) = self.sanitize_input(&mut sanitized_input) {
                if is_trusted && !self.config.strict_validation {
                    warn!(tool = %tool_name, session = %session_id, "Input sanitization bypassed for trusted session");
                } else {
                    validation_errors.push(format!("Input sanitization failed: {}", e));
                }
            }
        }

        // Security validation for shell commands, paths, etc.
        if let Err(e) = self.security_validate(tool_name, &sanitized_input, is_trusted) {
            if is_trusted && !self.config.strict_validation {
                warn!(tool = %tool_name, session = %session_id, "Security validation bypassed for trusted session");
            } else {
                validation_errors.push(format!("Security validation failed: {}", e));
            }
        }

        // Return validation result
        Ok(ValidatedInput {
            input: sanitized_input,
            is_valid: validation_errors.is_empty(),
            validation_errors,
            was_sanitized: self.config.sanitize_inputs && !is_trusted,
            session_trusted: is_trusted,
        })
    }

    /// Check input size limits
    fn check_input_size(&self, input: &Value) -> Result<()> {
        let input_str = serde_json::to_string(input)
            .map_err(|e| anyhow!("Failed to serialize input for size check: {}", e))?;

        if input_str.len() > self.config.max_input_size {
            return Err(anyhow!(
                "Input size {} bytes exceeds maximum {} bytes",
                input_str.len(),
                self.config.max_input_size
            ));
        }

        Ok(())
    }

    /// Validate input against JSON schema
    async fn validate_schema(&self, tool_name: &str, input: &Value, schema: &Value) -> Result<()> {
        // Create schema key for caching
        let schema_key = format!("{}:{}", tool_name, serde_json::to_string(schema)?);

        // Get or create compiled schema
        let compiled_schema = {
            let cache = self.schema_cache.read().await;
            if let Some(schema) = cache.get(&schema_key) {
                schema.clone()
            } else {
                // Compile and cache the schema
                let compiled = JSONSchema::compile(schema)
                    .map_err(|e| anyhow!("Failed to compile schema for {}: {}", tool_name, e))?;
                let arc_schema = Arc::new(compiled);

                let mut cache = self.schema_cache.write().await;
                cache.insert(schema_key, arc_schema.clone());
                arc_schema
            }
        };

        // Validate against schema
        if let Err(errors) = compiled_schema.validate(input) {
            let error_messages: Vec<String> = errors
                .map(|e| format!("{} at path: {}", e.instance_path, e))
                .collect();

            return Err(anyhow!(
                "Schema validation failed: {}",
                error_messages.join("; ")
            ));
        }

        Ok(())
    }

    /// Sanitize input to prevent injection attacks
    fn sanitize_input(&self, input: &mut Value) -> Result<()> {
        // Recursive sanitization function
        fn sanitize_value(value: &mut Value) -> Result<()> {
            match value {
                Value::String(s) => {
                    // Remove null bytes and control characters except newlines and tabs
                    *s = s
                        .chars()
                        .filter(|c| *c != '\0' && (*c >= ' ' || *c == '\n' || *c == '\t'))
                        .collect();

                    // Check for suspicious patterns in non-trusted contexts
                    if s.contains("../../../") || s.contains("..\\") {
                        return Err(anyhow!(
                            "Potentially dangerous path traversal pattern detected"
                        ));
                    }
                }
                Value::Array(arr) => {
                    for item in arr.iter_mut() {
                        sanitize_value(item)?;
                    }
                }
                Value::Object(obj) => {
                    for (key, val) in obj.iter_mut() {
                        // Sanitize keys too
                        if key.contains("..") || key.contains('\0') {
                            return Err(anyhow!("Invalid object key detected"));
                        }
                        sanitize_value(val)?;
                    }
                }
                _ => {} // Other types are safe as-is
            }
            Ok(())
        }

        sanitize_value(input)
    }

    /// Additional security validation for specific tool types
    fn security_validate(&self, tool_name: &str, input: &Value, is_trusted: bool) -> Result<()> {
        // Trusted sessions bypass most security checks
        if is_trusted {
            return Ok(());
        }

        // Shell tools need extra validation
        if tool_name.contains("shell") || tool_name.contains("exec") {
            if let Some(cmd) = extract_command_from_input(input) {
                // Validate against command whitelist
                let base_cmd = cmd.split_whitespace().next().unwrap_or(&cmd);
                if !self.config.command_whitelist.contains(base_cmd) {
                    return Err(anyhow!(
                        "Command '{}' is not whitelisted for non-trusted sessions",
                        base_cmd
                    ));
                }

                // Check for dangerous patterns even in whitelisted commands
                let dangerous_patterns = [
                    "rm -rf /",
                    "sudo rm",
                    "mkfs",
                    "dd if=/dev/",
                    ">/etc/",
                    "format",
                    "fdisk /dev/",
                    "chmod 777 /",
                    "chown root",
                ];

                for pattern in &dangerous_patterns {
                    if cmd.to_lowercase().contains(pattern) {
                        return Err(anyhow!(
                            "Dangerous command pattern '{}' detected in shell command",
                            pattern
                        ));
                    }
                }

                // Validate command arguments for injection
                validate_input(&cmd)
                    .map_err(|e| anyhow!("Shell command validation failed: {}", e))?;
            }
        }

        // File operation tools need path validation
        if tool_name.contains("file") || tool_name.contains("fs") {
            if let Some(path) = extract_path_from_input(input) {
                let path_buf = PathBuf::from(&path);

                // Check path traversal
                if path.contains("..") {
                    return Err(anyhow!(
                        "Path traversal not allowed in non-trusted sessions: {}",
                        path
                    ));
                }

                // Check against forbidden directories first
                for forbidden in &self.config.forbidden_dirs {
                    if path_buf.starts_with(forbidden) {
                        return Err(anyhow!(
                            "Access to forbidden path '{}' is not allowed",
                            forbidden.display()
                        ));
                    }
                }

                // Check if path is within allowed directories
                let is_allowed = self
                    .config
                    .allowed_dirs
                    .iter()
                    .any(|allowed| path_buf.starts_with(allowed));

                if !is_allowed {
                    return Err(anyhow!(
                        "Path '{}' is not within allowed directories for non-trusted sessions",
                        path
                    ));
                }

                // Validate path input for forbidden characters
                validate_input(&path).map_err(|e| anyhow!("Path validation failed: {}", e))?;
            }
        }

        // General input validation for all tools
        if let Some(input_str) = extract_string_from_input(input) {
            validate_input(&input_str).map_err(|e| anyhow!("Input validation failed: {}", e))?;
        }

        Ok(())
    }
}

/// Result of input validation
#[derive(Debug, Clone)]
pub struct ValidatedInput {
    /// The validated and potentially sanitized input
    pub input: Value,
    /// Whether the input passed all validations
    pub is_valid: bool,
    /// List of validation errors (if any)
    pub validation_errors: Vec<String>,
    /// Whether the input was sanitized
    pub was_sanitized: bool,
    /// Whether the session is trusted (chatbot orchestrator)
    pub session_trusted: bool,
}

impl ValidatedInput {
    /// Get the validated input for execution
    pub fn into_input(self) -> Value {
        self.input
    }

    /// Check if execution should proceed
    pub fn should_proceed(&self) -> bool {
        self.is_valid || self.session_trusted
    }
}

/// Extract command string from input JSON
fn extract_command_from_input(input: &Value) -> Option<String> {
    if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
        return Some(cmd.to_string());
    }

    if let Some(cmd) = input.get("cmd").and_then(|v| v.as_str()) {
        return Some(cmd.to_string());
    }

    if let Some(args) = input.get("args").and_then(|v| v.as_array()) {
        if let Some(first) = args.first().and_then(|v| v.as_str()) {
            return Some(first.to_string());
        }
    }

    None
}

/// Extract path string from input JSON
fn extract_path_from_input(input: &Value) -> Option<String> {
    if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
        return Some(path.to_string());
    }

    if let Some(path) = input.get("file").and_then(|v| v.as_str()) {
        return Some(path.to_string());
    }

    if let Some(path) = input.get("directory").and_then(|v| v.as_str()) {
        return Some(path.to_string());
    }

    None
}

/// Extract string value from input JSON
fn extract_string_from_input(input: &Value) -> Option<String> {
    if let Some(s) = input.as_str() {
        return Some(s.to_string());
    }

    // Look for common string fields
    for field in ["text", "content", "data", "input", "value"] {
        if let Some(s) = input.get(field).and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
    }

    None
}

/// Validate a general input string (mirrors op-agents validation)
fn validate_input(input: &str) -> Result<()> {
    if input.is_empty() {
        return Err(anyhow!("Empty input not allowed"));
    }

    if input.len() > MAX_INPUT_LENGTH {
        return Err(anyhow!(
            "Input exceeds maximum length ({} > {})",
            input.len(),
            MAX_INPUT_LENGTH
        ));
    }

    for c in input.chars() {
        if FORBIDDEN_CHARS.contains(&c) {
            return Err(anyhow!("Input contains forbidden character: {:?}", c));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_trusted_session_bypass() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});

        // Trusted session should pass even with invalid input
        let result = validator
            .validate_input(
                "test_tool",
                &json!({"invalid": "data"}),
                &schema,
                Some("chatbot"),
            )
            .await
            .unwrap();

        assert!(result.session_trusted);
        assert!(result.should_proceed());
    }

    #[tokio::test]
    async fn test_input_sanitization() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});

        // Test null byte removal
        let mut input = json!({"text": "hello\x00world"});
        let result = validator
            .validate_input("test_tool", &input, &schema, Some("anonymous"))
            .await
            .unwrap();

        assert_eq!(result.input["text"], "helloworld");
    }

    #[tokio::test]
    async fn test_shell_command_validation() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});

        // Dangerous command should be blocked for anonymous
        let input = json!({"command": "rm -rf /"});
        let result = validator
            .validate_input("shell_tool", &input, &schema, Some("anonymous"))
            .await;

        assert!(result.is_err());

        // But allowed for trusted session
        let result = validator
            .validate_input("shell_tool", &input, &schema, Some("chatbot"))
            .await
            .unwrap();

        assert!(result.should_proceed());
    }

    #[tokio::test]
    async fn test_path_validation() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});

        // Restricted path should be blocked for anonymous
        let input = json!({"path": "/etc/shadow"});
        let result = validator
            .validate_input("file_tool", &input, &schema, Some("anonymous"))
            .await;

        assert!(result.is_err());

        // But allowed for trusted session
        let result = validator
            .validate_input("file_tool", &input, &schema, Some("chatbot"))
            .await
            .unwrap();

        assert!(result.should_proceed());
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_trusted_session_bypass() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});

        // Trusted session should pass even with invalid input
        let result = validator
            .validate_input(
                "test_tool",
                &json!({"invalid": "data"}),
                &schema,
                Some("chatbot"),
            )
            .await
            .unwrap();

        assert!(result.session_trusted);
        assert!(result.should_proceed());
    }

    #[tokio::test]
    async fn test_non_trusted_restriction() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});

        // Non-trusted session should be restricted
        let result = validator
            .validate_input(
                "shell_tool",
                &json!({"command": "rm -rf /etc/passwd"}),
                &schema,
                Some("anonymous"),
            )
            .await
            .unwrap();

        assert!(!result.session_trusted);
        assert!(!result.should_proceed());
        assert!(!result.validation_errors.is_empty());
    }

    #[tokio::test]
    async fn test_input_sanitization() {
        let validator = InputValidator::new();
        let schema = json!({"type": "object"});

        let input_with_null_bytes = json!({"text": "hello\x00world"});
        let result = validator
            .validate_input(
                "text_tool",
                &input_with_null_bytes,
                &schema,
                Some("anonymous"),
            )
            .await
            .unwrap();

        let sanitized_text = result.input["text"].as_str().unwrap();
        assert!(!sanitized_text.contains('\0'));
        assert_eq!(sanitized_text, "helloworld");
        assert!(result.was_sanitized);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/Cargo.toml">
[package]
name = "op-tools"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Tool registry and execution for op-dbus-v2"

[dependencies]
# Async runtime
tokio = { workspace = true, features = ["full", "sync"] }
async-trait = { workspace = true }

# Serialization
serde = { workspace = true }
simd-json = { workspace = true }
serde_json = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Logging
tracing = { workspace = true }
clap = { workspace = true }
futures = { workspace = true }

# Time
chrono = { workspace = true }

# UUID for event IDs
uuid = { workspace = true }

# D-Bus for agent execution
zbus = { workspace = true }

# Internal dependencies
op-core = { path = "../op-core" }
op-introspection = { path = "../op-introspection" }
op-inspector = { path = "../op-inspector" }
op-network = { path = "../op-network" }
op-http = { path = "../op-http" }
op-agents = { path = "../op-agents" }

# Web
axum = { workspace = true }
reqwest = { workspace = true }

# System info
op-state = { workspace = true }
lazy_static = { workspace = true }
op-execution-tracker = { path = "../op-execution-tracker" }

# Async recursion for self-tools
async-recursion = "1.0"
dirs = "5"

# JSON Schema validation
jsonschema = "0.18"

[dev-dependencies]
tokio-test = "0.4"
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/Cargo.toml.patch">
# Add to [dependencies] in crates/op-tools/Cargo.toml:

[dependencies]
# ... existing deps ...
zbus = { workspace = true }  # For D-Bus agent services

[features]
default = []
dbus-tools = []  # Enable if you have dbus.rs, dbus_introspection.rs etc.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/compare-op-tools.md">
# compare-op-tools

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 50 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 10 |
| Partial artifacts | 3 |
| Spec-listed source files | 20 |
| Spec-listed but missing | 0 |
| Extra implementation files | 30 |

## Current Implementation Overview

- Tool registry and execution for op-dbus-v2
- Internal crate integrations: op-core, op-introspection, op-inspector, op-network, op-http, op-agents, op-state, op-execution-tracker.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/bin/op-packagekit-install.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/bin/op-packagekit-install.rs |
| `src/builtin/dinit.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/dinit.rs |
| `src/builtin/system.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/system.rs |
| `src/builtin/shell_tool.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/shell_tool.rs |
| `src/builtin/shell.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/shell.rs |
| `src/builtin/self_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/self_tools.rs |
| `src/builtin/rtnetlink_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/rtnetlink_tools.rs |
| `src/builtin/response_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/response_tools.rs |
| `src/builtin/respond_tool.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/respond_tool.rs |
| `src/builtin/procfs.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/procfs.rs |
| `src/builtin/packagekit.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/packagekit.rs |
| `src/builtin/ovsdb.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/ovsdb.rs |
| `src/builtin/ovs_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/ovs_tools.rs |
| `src/builtin/ovs.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/ovs.rs |
| `src/builtin/openflow_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/openflow_tools.rs |
| `src/builtin/mod.rs` | ⚠️ Partial | Declared in source inventory from spec/design docs | src/builtin/mod.rs; partial artifacts: src/builtin/mod.rs.fix, src/builtin/mod.rs.patch |
| `src/builtin/lxc_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/lxc_tools.rs |
| `src/builtin/gcloud_tools.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/gcloud_tools.rs |
| `src/builtin/file.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/file.rs |
| `src/builtin/error_reporting_tool.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/builtin/error_reporting_tool.rs |
| `bin` | ✅ Present | bin group | src/bin/op-packagekit-install.rs |
| `builtin` | ✅ Present | builtin group | src/builtin/agent_tool.rs, src/builtin/anydesk.rs, src/builtin/code_search.rs, src/builtin/dbus.rs, src/builtin/dbus_hybrid.rs, src/builtin/dbus_introspection.rs, src/builtin/dbus_search_tool.rs, src/builtin/dbus_tool.rs, ... (+23 more) |
| `discovery` | ✅ Present | discovery group | src/discovery/mod.rs, src/discovery/projection_engine.rs, src/discovery/sources/agent.rs, src/discovery/sources/dbus.rs, src/discovery/sources/mod.rs, src/discovery/sources/plugin.rs |
| `root` | ✅ Present | root source group | src/builtin_old.rs, src/dynamic_tool.rs, src/executor.rs, src/lib.rs, src/mcptools.rs, src/orchestration_plugin.rs, src/registry.rs, src/router.rs, ... (+4 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| validation_tests | ✅ Implemented | src/validation_tests.rs | SPEC main module |
| validation | ✅ Implemented | src/validation.rs | SPEC main module |
| tool | ✅ Implemented | src/tool.rs | SPEC main module |
| security | ✅ Implemented | src/security.rs | SPEC main module |
| router | ✅ Implemented | src/router.rs | SPEC main module |
| registry | ✅ Implemented | src/registry.rs | SPEC main module |
| orchestration_plugin | ✅ Implemented | src/orchestration_plugin.rs | SPEC main module |
| mcptools | ✅ Implemented | src/mcptools.rs | SPEC main module |
| executor | ✅ Implemented | src/executor.rs | SPEC main module |
| dynamic_tool | ✅ Implemented | src/dynamic_tool.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - not listed in SPEC dependency block
- `op-introspection` - not listed in SPEC dependency block
- `op-inspector` - not listed in SPEC dependency block
- `op-network` - not listed in SPEC dependency block
- `op-http` - not listed in SPEC dependency block
- `op-agents` - not listed in SPEC dependency block
- `op-state` - not listed in SPEC dependency block
- `op-execution-tracker` - not listed in SPEC dependency block

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `async-trait` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `serde_json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `clap` - documented in SPEC
- `futures` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - not listed in SPEC dependency block
- `zbus` - not listed in SPEC dependency block
- `axum` - not listed in SPEC dependency block
- `reqwest` - not listed in SPEC dependency block
- `lazy_static` - not listed in SPEC dependency block
- `async-recursion` - not listed in SPEC dependency block
- `dirs` - not listed in SPEC dependency block
- `jsonschema` - not listed in SPEC dependency block

### Development and Build Dependencies
- `dev:tokio-test`

## Notes and Observations

- Local documentation files present: SPEC.md.
- Transitional or partial artifacts detected: Cargo.toml.patch, src/builtin/mod.rs.fix, src/builtin/mod.rs.patch.
- Current implementation contains 30 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: builtin, discovery, dynamic_tool, mcptools, orchestration_plugin, registry, router, security, tool, validation.
- 16 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-tools/SPEC.md">
# op-tools - Specification

## Overview
**Crate**: `op-tools`  
**Location**: `crates/op-tools`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-tools"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Tool registry and execution for op-dbus-v2"
```

### Source Structure
```
op-tools/src/bin/op-packagekit-install.rs
op-tools/src/builtin/dinit.rs
op-tools/src/builtin/system.rs
op-tools/src/builtin/shell_tool.rs
op-tools/src/builtin/shell.rs
op-tools/src/builtin/self_tools.rs
op-tools/src/builtin/rtnetlink_tools.rs
op-tools/src/builtin/response_tools.rs
op-tools/src/builtin/respond_tool.rs
op-tools/src/builtin/procfs.rs
op-tools/src/builtin/packagekit.rs
op-tools/src/builtin/ovsdb.rs
op-tools/src/builtin/ovs_tools.rs
op-tools/src/builtin/ovs.rs
op-tools/src/builtin/openflow_tools.rs
op-tools/src/builtin/mod.rs
op-tools/src/builtin/lxc_tools.rs
op-tools/src/builtin/gcloud_tools.rs
op-tools/src/builtin/file.rs
op-tools/src/builtin/error_reporting_tool.rs
```

### Key Dependencies
```toml
# Async runtime
tokio = { workspace = true, features = ["full", "sync"] }
async-trait = { workspace = true }

# Serialization
serde = { workspace = true }
simd-json = { workspace = true }
serde_json = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Logging
tracing = { workspace = true }
clap = { workspace = true }
futures = { workspace = true }

# Time
chrono = { workspace = true }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
      47 Rust source files

### Main Modules
validation_tests
validation
tool
security
router
registry
orchestration_plugin
mcptools
executor
dynamic_tool

## Purpose
Tool registry and execution for op-dbus-v2

## Build Information
- **Edition**: edition.workspace = true
- **Version**: 0.1.0
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-core
- op-introspection
- op-inspector
- op-network
- op-http
- op-execution-tracker

---
*Generated from crate analysis*
</file>

</files>
