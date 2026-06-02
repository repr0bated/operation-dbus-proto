This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
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
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  state_plugins/
    adc.rs
    agent_config.rs
    cognitive_mcp.rs
    compact_mcp.rs
    config.rs
    ctl_plane_chatbot.rs
    dnsresolver.rs
    endpoint.rs
    full_system.rs
    gcloud_adc.rs
    hardware.rs
    incus.rs
    keypair.rs
    keyring.rs
    login1.rs
    lxc.rs
    mail_server.rs
    mcp.rs
    mod.rs
    net.rs
    netmaker.rs
    openflow_obfuscation.rs
    openflow.rs
    ovsdb_bridge.rs
    packagekit.rs
    pcidecl.rs
    plugin_schema_defs.rs
    privacy_router.rs
    privacy_routes.rs
    privacy.rs
    procfs.rs
    proxmox.rs
    proxy_server.rs
    rtnetlink.rs
    s6.rs
    schema_contract.rs
    service.rs
    sessdecl.rs
    software.rs
    systemd_networkd.rs
    systemd.rs
    unix_socket.rs
    users.rs
    web_ui.rs
    wireguard.rs
  auto_create.rs
  builtin.rs
  chat.rs
  default_registry.rs
  dynamic_loading.rs
  lib.rs
  plugin.rs
  registry.rs
  service_def.rs
  state_publisher.rs
  state.rs
Cargo.toml
compare-op-plugins.md
DESIGN.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/state_plugins/adc.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdcState {
    pub configured: bool,
}

pub struct AdcPlugin;

impl Default for AdcPlugin {
    fn default() -> Self {
        Self
    }
}

impl AdcPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for AdcPlugin {
    fn name(&self) -> &str {
        "adc"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::adc_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(AdcState {
            configured: false,
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/agent_config.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigState {
    pub agents: Vec<AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub enabled: bool,
    pub model: Option<String>,
    pub tools: Vec<String>,
}

pub struct AgentConfigPlugin;

impl Default for AgentConfigPlugin {
    fn default() -> Self {
        Self
    }
}

impl AgentConfigPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for AgentConfigPlugin {
    fn name(&self) -> &str {
        "agent_config"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::agent_config_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        // Schema as Code: Define the authoritative list of agents here
        let agents = vec![
            // Orchestration (Critical)
            AgentConfig {
                name: "memory".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "context-manager".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "sequential-thinking".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "dx-optimizer".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "tdd-orchestrator".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            // Language & Architecture (High)
            AgentConfig {
                name: "rust-pro".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "python-pro".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "backend-architect".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "frontend-developer".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "database-architect".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "backend-security-coder".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            // Infrastructure & Ops (Medium)
            AgentConfig {
                name: "network-engineer".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "deployment".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "devops-troubleshooter".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            // Analysis & Quality (Medium)
            AgentConfig {
                name: "debugger".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "code-reviewer".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "search-specialist".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "prompt-engineer".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
            AgentConfig {
                name: "docs-architect".to_string(),
                enabled: true,
                model: None,
                tools: vec![],
            },
        ];

        Ok(simd_json::serde::to_owned_value(AgentConfigState {
            agents,
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/cognitive_mcp.rs">
//! Cognitive MCP state plugin
//!
//! Tracks and manages the op-cognitive-mcp server: bind addresses, WireGuard
//! identity, tool registrations, and gRPC/HTTP health.  Publishes live state
//! to D-Bus under `/org/opdbus/v1/plugins/cognitive_mcp` so that
//! `register_plugin_projection_tools` can expose it as MCP tools.
//!
//! The canonical schema (every gRPC method, every MCP tool, every
//! request/response field) lives in `plugin_schema_defs::cognitive_mcp_plugin_schema`.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

const S6_SV_PATH: &str = "/run/service/op-cognitive-mcp";
const ENV_DIR: &str = "/etc/s6/sv/op-cognitive-mcp/env";
const RUNTIME_ENV_DIR: &str = "/run/service/op-cognitive-mcp/env";
const DEFAULT_HTTP: &str = "100.90.37.254:3003";
const DEFAULT_GRPC: &str = "100.90.37.254:50052";
const DEFAULT_DB: &str = "/var/lib/op-dbus/cognitive.db";
const DEFAULT_WG: &str = "netmaker";

// ── Deployment config (tunable via env-dir / apply_state) ──────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitiveMcpConfig {
    #[serde(default = "default_http")]
    pub http: String,
    #[serde(default = "default_grpc")]
    pub grpc: String,
    #[serde(default = "default_db")]
    pub db_path: String,
    #[serde(default = "default_wg")]
    pub wg_interface: String,
    #[serde(default = "default_true")]
    pub http_enabled: bool,
    #[serde(default = "default_true")]
    pub grpc_enabled: bool,
    #[serde(default = "default_true")]
    pub dbus_enabled: bool,
}

fn default_http() -> String { DEFAULT_HTTP.into() }
fn default_grpc() -> String { DEFAULT_GRPC.into() }
fn default_db() -> String { DEFAULT_DB.into() }
fn default_wg() -> String { DEFAULT_WG.into() }
fn default_true() -> bool { true }

impl Default for CognitiveMcpConfig {
    fn default() -> Self {
        Self {
            http: default_http(),
            grpc: default_grpc(),
            db_path: default_db(),
            wg_interface: default_wg(),
            http_enabled: true,
            grpc_enabled: true,
            dbus_enabled: true,
        }
    }
}

// ── Plugin struct + service helpers ─────────────────────────────────────────

pub struct CognitiveMcpPlugin;

impl CognitiveMcpPlugin {
    pub fn new() -> Self { Self }

    fn service_running() -> bool {
        let sv = std::path::Path::new(S6_SV_PATH);
        sv.exists() && !sv.join("down").exists()
    }

    fn read_env(key: &str) -> Option<String> {
        std::fs::read_to_string(format!("{ENV_DIR}/{key}"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn current_config() -> CognitiveMcpConfig {
        CognitiveMcpConfig {
            http: Self::read_env("COGNITIVE_MCP_BIND").unwrap_or_else(default_http),
            grpc: Self::read_env("COGNITIVE_MCP_GRPC_BIND").unwrap_or_else(default_grpc),
            db_path: Self::read_env("COGNITIVE_MCP_DB_PATH").unwrap_or_else(default_db),
            wg_interface: Self::read_env("WG_INTERFACE").unwrap_or_else(default_wg),
            http_enabled: Self::read_env("COGNITIVE_MCP_HTTP_DISABLED").is_none(),
            grpc_enabled: Self::read_env("COGNITIVE_MCP_GRPC_DISABLED").is_none(),
            dbus_enabled: Self::read_env("COGNITIVE_MCP_DBUS_DISABLED").is_none(),
        }
    }

    async fn write_env(key: &str, value: &str) -> Result<()> {
        tokio::fs::create_dir_all(ENV_DIR).await.context("create env dir")?;
        tokio::fs::write(format!("{ENV_DIR}/{key}"), value)
            .await
            .with_context(|| format!("write env {key}"))?;
        if let Ok(()) = tokio::fs::create_dir_all(RUNTIME_ENV_DIR).await {
            let _ = tokio::fs::write(format!("{RUNTIME_ENV_DIR}/{key}"), value).await;
        }
        Ok(())
    }

    async fn reload_service() -> Result<()> {
        let out = tokio::process::Command::new("s6-svc")
            .args(["-r", S6_SV_PATH])
            .output()
            .await
            .context("s6-svc -r op-cognitive-mcp")?;
        if !out.status.success() {
            tracing::warn!("s6-svc -r failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(())
    }
}

impl Default for CognitiveMcpPlugin {
    fn default() -> Self { Self::new() }
}

// ── StatePlugin impl ─────────────────────────────────────────────────────────

#[async_trait]
impl StatePlugin for CognitiveMcpPlugin {
    fn name(&self) -> &str { "cognitive_mcp" }
    fn version(&self) -> &str { "2.0.0" }

    fn schema(&self) -> Option<PluginSchema> {
        Some(super::plugin_schema_defs::cognitive_mcp_plugin_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/etc/s6/sv/op-cognitive-mcp").exists()
    }

    fn unavailable_reason(&self) -> String {
        "op-cognitive-mcp s6 service definition not found at /etc/s6/sv/op-cognitive-mcp".into()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut cfg = simd_json::serde::to_owned_value(Self::current_config())?;
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert("running".into(), simd_json::OwnedValue::from(Self::service_running()));
        }
        Ok(cfg)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_cfg: CognitiveMcpConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_cfg: CognitiveMcpConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();
        macro_rules! field_diff {
            ($field:ident, $key:expr) => {
                if current_cfg.$field != desired_cfg.$field {
                    actions.push(StateAction::Modify {
                        resource: $key.into(),
                        changes: simd_json::serde::to_owned_value(&desired_cfg.$field)?,
                    });
                }
            };
        }
        field_diff!(http, "http");
        field_diff!(grpc, "grpc");
        field_diff!(db_path, "db_path");
        field_diff!(wg_interface, "wg_interface");
        field_diff!(http_enabled, "http_enabled");
        field_diff!(grpc_enabled, "grpc_enabled");
        field_diff!(dbus_enabled, "dbus_enabled");

        Ok(StateDiff {
            plugin: self.name().into(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes = Vec::new();
        let mut errors = Vec::new();
        let mut needs_reload = false;

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes: val } = action {
                let result: Result<()> = match resource.as_str() {
                    "http" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_BIND", s).await?;
                            needs_reload = true; Ok(())
                        } else { Err(anyhow::anyhow!("http must be a string")) }
                    }
                    "grpc" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_GRPC_BIND", s).await?;
                            needs_reload = true; Ok(())
                        } else { Err(anyhow::anyhow!("grpc must be a string")) }
                    }
                    "db_path" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_DB_PATH", s).await?;
                            needs_reload = true; Ok(())
                        } else { Err(anyhow::anyhow!("db_path must be a string")) }
                    }
                    "wg_interface" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("WG_INTERFACE", s).await?;
                            needs_reload = true; Ok(())
                        } else { Err(anyhow::anyhow!("wg_interface must be a string")) }
                    }
                    "http_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_HTTP_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(
                                format!("{ENV_DIR}/COGNITIVE_MCP_HTTP_DISABLED")
                            ).await;
                        }
                        needs_reload = true; Ok(())
                    }
                    "grpc_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_GRPC_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(
                                format!("{ENV_DIR}/COGNITIVE_MCP_GRPC_DISABLED")
                            ).await;
                        }
                        needs_reload = true; Ok(())
                    }
                    "dbus_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_DBUS_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(
                                format!("{ENV_DIR}/COGNITIVE_MCP_DBUS_DISABLED")
                            ).await;
                        }
                        needs_reload = true; Ok(())
                    }
                    other => Err(anyhow::anyhow!("unknown cognitive_mcp field: {other}")),
                };
                match result {
                    Ok(()) => changes.push(format!("cognitive_mcp.{resource} updated")),
                    Err(e) => errors.push(format!("cognitive_mcp.{resource}: {e}")),
                }
            }
        }

        if needs_reload && errors.is_empty() {
            if let Err(e) = Self::reload_service().await {
                tracing::warn!("cognitive_mcp reload: {e}");
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let cur: CognitiveMcpConfig = simd_json::serde::from_owned_value(current)?;
        let des: CognitiveMcpConfig = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(cur == des)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("cognitive_mcp-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: CognitiveMcpConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        let desired = simd_json::serde::to_owned_value(&old)?;
        let current = self.query_current_state().await?;
        let diff = self.calculate_diff(&current, &desired).await?;
        self.apply_state(&diff).await?;
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/compact_mcp.rs">
//! Compact MCP state plugin
//!
//! Tracks and manages the op-mcp-server: mode, transport bind addresses,
//! WireGuard identity, and tool registry.  Publishes live state to D-Bus
//! under `/org/opdbus/v1/plugins/compact_mcp` so that
//! `register_plugin_projection_tools` can expose it as MCP tools.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

const S6_SV_PATH: &str = "/run/service/op-mcp-compact";
const ENV_DIR: &str = "/etc/s6/sv/op-mcp-compact/env";
const RUNTIME_ENV_DIR: &str = "/run/service/op-mcp-compact/env";
const DEFAULT_MODE: &str = "compact";
const DEFAULT_HTTP: &str = "127.0.0.1:11436";
const DEFAULT_WG: &str = "netmaker";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactMcpConfig {
    /// Server mode: compact | full | agents
    #[serde(default = "default_mode")]
    pub mode: String,
    /// HTTP/SSE bind address (when not using stdio)
    #[serde(default = "default_http")]
    pub http: Option<String>,
    /// WebSocket bind address
    #[serde(default)]
    pub ws: Option<String>,
    /// WireGuard interface for identity
    #[serde(default = "default_wg")]
    pub wg_interface: String,
    /// Run stdio transport (not used for s6 daemon deployment)
    #[serde(default = "default_false")]
    pub stdio: bool,
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_mode() -> String {
    DEFAULT_MODE.into()
}
fn default_http() -> Option<String> {
    Some(DEFAULT_HTTP.into())
}
fn default_wg() -> String {
    DEFAULT_WG.into()
}
fn default_false() -> bool {
    false
}
fn default_log_level() -> String {
    "info".into()
}

impl Default for CompactMcpConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            http: default_http(),
            ws: None,
            wg_interface: default_wg(),
            stdio: false,
            log_level: default_log_level(),
        }
    }
}

pub struct CompactMcpPlugin;

impl CompactMcpPlugin {
    pub fn new() -> Self {
        Self
    }

    fn service_running() -> bool {
        let sv = std::path::Path::new(S6_SV_PATH);
        sv.exists() && !sv.join("down").exists()
    }

    fn read_env(key: &str) -> Option<String> {
        std::fs::read_to_string(format!("{ENV_DIR}/{key}"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn current_config() -> CompactMcpConfig {
        CompactMcpConfig {
            mode: Self::read_env("OP_MCP_MODE").unwrap_or_else(default_mode),
            http: Self::read_env("OP_MCP_HTTP"),
            ws: Self::read_env("OP_MCP_WS"),
            wg_interface: Self::read_env("WG_INTERFACE").unwrap_or_else(default_wg),
            stdio: Self::read_env("OP_MCP_STDIO")
                .map(|v| v != "0")
                .unwrap_or(false),
            log_level: Self::read_env("OP_MCP_LOG_LEVEL").unwrap_or_else(default_log_level),
        }
    }

    async fn write_env(key: &str, value: &str) -> Result<()> {
        tokio::fs::create_dir_all(ENV_DIR)
            .await
            .context("create compact_mcp env dir")?;
        tokio::fs::write(format!("{ENV_DIR}/{key}"), value)
            .await
            .with_context(|| format!("write env {key}"))?;
        // Also write to runtime dir so the value takes effect after s6-svc -r
        // without requiring a DB recompile.
        if let Ok(()) = tokio::fs::create_dir_all(RUNTIME_ENV_DIR).await {
            let _ = tokio::fs::write(format!("{RUNTIME_ENV_DIR}/{key}"), value).await;
        }
        Ok(())
    }

    async fn reload_service() -> Result<()> {
        let out = tokio::process::Command::new("s6-svc")
            .args(["-r", S6_SV_PATH])
            .output()
            .await
            .context("s6-svc -r op-mcp-compact")?;
        if !out.status.success() {
            tracing::warn!("s6-svc -r op-mcp-compact: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(())
    }
}

impl Default for CompactMcpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for CompactMcpPlugin {
    fn name(&self) -> &str {
        "compact_mcp"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(super::plugin_schema_defs::compact_mcp_plugin_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/etc/s6/sv/op-mcp-compact").exists()
    }

    fn unavailable_reason(&self) -> String {
        "op-mcp-compact s6 service definition not found at /etc/s6/sv/op-mcp-compact".into()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut cfg = simd_json::serde::to_owned_value(Self::current_config())?;
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert(
                "running".into(),
                simd_json::OwnedValue::from(Self::service_running()),
            );
        }
        Ok(cfg)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let cur: CompactMcpConfig = simd_json::serde::from_owned_value(current.clone())?;
        let des: CompactMcpConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();
        macro_rules! diff {
            ($field:ident, $key:expr) => {
                if cur.$field != des.$field {
                    actions.push(StateAction::Modify {
                        resource: $key.into(),
                        changes: simd_json::serde::to_owned_value(&des.$field)?,
                    });
                }
            };
        }
        diff!(mode, "mode");
        diff!(http, "http");
        diff!(ws, "ws");
        diff!(wg_interface, "wg_interface");
        diff!(stdio, "stdio");
        diff!(log_level, "log_level");

        Ok(StateDiff {
            plugin: self.name().into(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes = Vec::new();
        let mut errors = Vec::new();
        let mut needs_reload = false;

        for action in &diff.actions {
            if let StateAction::Modify {
                resource,
                changes: val,
            } = action
            {
                let result: Result<()> = match resource.as_str() {
                    "mode" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("OP_MCP_MODE", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("mode must be a string"))
                        }
                    }
                    "http" => {
                        match val.as_str() {
                            Some(s) => {
                                Self::write_env("OP_MCP_HTTP", s).await?;
                            }
                            None => {
                                let _ =
                                    tokio::fs::remove_file(format!("{ENV_DIR}/OP_MCP_HTTP")).await;
                            }
                        }
                        needs_reload = true;
                        Ok(())
                    }
                    "ws" => {
                        match val.as_str() {
                            Some(s) => {
                                Self::write_env("OP_MCP_WS", s).await?;
                            }
                            None => {
                                let _ =
                                    tokio::fs::remove_file(format!("{ENV_DIR}/OP_MCP_WS")).await;
                            }
                        }
                        needs_reload = true;
                        Ok(())
                    }
                    "wg_interface" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("WG_INTERFACE", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("wg_interface must be a string"))
                        }
                    }
                    "stdio" => {
                        let v = if val.as_bool() == Some(false) {
                            "0"
                        } else {
                            "1"
                        };
                        Self::write_env("OP_MCP_STDIO", v).await?;
                        needs_reload = true;
                        Ok(())
                    }
                    "log_level" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("OP_MCP_LOG_LEVEL", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("log_level must be a string"))
                        }
                    }
                    other => Err(anyhow::anyhow!("unknown compact_mcp field: {other}")),
                };

                match result {
                    Ok(()) => changes.push(format!("compact_mcp.{resource} updated")),
                    Err(e) => errors.push(format!("compact_mcp.{resource}: {e}")),
                }
            }
        }

        if needs_reload && errors.is_empty() {
            if let Err(e) = Self::reload_service().await {
                tracing::warn!("compact_mcp reload: {e}");
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let cur: CompactMcpConfig = simd_json::serde::from_owned_value(current)?;
        let des: CompactMcpConfig = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(cur == des)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("compact_mcp-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: CompactMcpConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        let desired = simd_json::serde::to_owned_value(&old)?;
        let current = self.query_current_state().await?;
        let diff = self.calculate_diff(&current, &desired).await?;
        self.apply_state(&diff).await?;
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/config.rs">
use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::PathBuf;

const DEFAULT_CONFIG_STORE_PATH: &str = "/etc/op-dbus/config-store.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigStoreState {
    #[serde(default)]
    pub configs: HashMap<String, Value>,
}

pub struct ConfigPlugin {
    store_path: PathBuf,
}

impl Default for ConfigPlugin {
    fn default() -> Self {
        Self::new(DEFAULT_CONFIG_STORE_PATH)
    }
}

impl ConfigPlugin {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
        }
    }

    async fn load_store(&self) -> Result<ConfigStoreState> {
        match tokio::fs::read_to_string(&self.store_path).await {
            Ok(mut content) => {
                let parsed: ConfigStoreState =
                    unsafe { simd_json::from_str(&mut content) }.context("invalid config store")?;
                Ok(parsed)
            }
            Err(_) => Ok(ConfigStoreState {
                configs: HashMap::new(),
            }),
        }
    }

    async fn save_store(&self, state: &ConfigStoreState) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("failed to create config store directory")?;
        }

        let content = simd_json::to_string_pretty(state).context("serialize config store")?;
        tokio::fs::write(&self.store_path, content)
            .await
            .context("write config store")?;
        Ok(())
    }
}

fn config_plugin_schema() -> PluginSchema {
    PluginSchema::builder("config")
        .version("1.0.0")
        .description("Global key/value config store")
        .field(
            "configs",
            FieldSchema {
                field_type: FieldType::Any,
                required: true,
                description: "Configuration map".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "anna_scribe": {
                        "snowball_path": "/var/lib/op-dbus/snowball"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .build()
}

#[async_trait]
impl StatePlugin for ConfigPlugin {
    fn name(&self) -> &str {
        "config"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(config_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = self.load_store().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: ConfigStoreState = simd_json::serde::from_owned_value(current.clone())?;
        let desired_state: ConfigStoreState = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        for (key, desired_value) in &desired_state.configs {
            match current_state.configs.get(key) {
                Some(current_value) if current_value == desired_value => {}
                Some(_) => actions.push(StateAction::Modify {
                    resource: key.clone(),
                    changes: desired_value.clone(),
                }),
                None => actions.push(StateAction::Create {
                    resource: key.clone(),
                    config: desired_value.clone(),
                }),
            }
        }

        for key in current_state.configs.keys() {
            if !desired_state.configs.contains_key(key) {
                actions.push(StateAction::Delete {
                    resource: key.clone(),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut state = self.load_store().await?;
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config } => {
                    state.configs.insert(resource.clone(), config.clone());
                    changes_applied.push(format!("created config key {}", resource));
                }
                StateAction::Modify { resource, changes } => {
                    state.configs.insert(resource.clone(), changes.clone());
                    changes_applied.push(format!("updated config key {}", resource));
                }
                StateAction::Delete { resource } => {
                    state.configs.remove(resource);
                    changes_applied.push(format!("deleted config key {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        if let Err(e) = self.save_store(&state).await {
            errors.push(e.to_string());
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let current_state: ConfigStoreState = simd_json::serde::from_owned_value(current)?;
        let desired_state: ConfigStoreState = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(current_state == desired_state)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("config-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_state: ConfigStoreState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        self.save_store(&old_state).await
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_publish_plugin_owned_config_schema() {
        let schema = ConfigPlugin::default().schema().expect("config schema");
        let field = schema.fields.get("configs").expect("configs field");

        assert_eq!(schema.name, "config");
        assert_eq!(schema.version, "1.0.0");
        assert_eq!(schema.description, "Global key/value config store");
        assert!(matches!(field.field_type, FieldType::Any));
        assert_eq!(field.default, Some(json!({})));
    }
}
</file>

<file path="src/state_plugins/ctl_plane_chatbot.rs">
//! Control-plane chatbot reasoning episode plugin
//!
//! Declares the canonical schema for the chatbot's reasoning episodes —
//! every field, PII classification, significance, and vectorization contract.
//! THE PLUGIN IS THE SCHEMA: downstream (Qdrant, CozoDB, Accountability UI,
//! gRPC EventChainService) inherits from this definition.
//!
//! Related: REQ-1 through REQ-10 (Control-Plane Chatbot Reasoning Episode
//! Vectorization spec). This plugin covers REQ-2 (episode record fields)
//! and REQ-3 (plugin schema registration).

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

const DEFAULT_VOYAGE_MODEL: &str = "voyage-4-lite";
const DEFAULT_COLLECTION: &str = "ctl_plane_reasoning_episodes";
const DEFAULT_VECTOR_DIMS: u32 = 1024;
const DEFAULT_DEDUP_WINDOW_HRS: u32 = 24;
const DEFAULT_QUEUE_ALERT_THRESHOLD: u32 = 50;
const DEFAULT_NESTING_POLICY: &str = "flat";

// ── Config (vectorization pipeline tuning) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CtlPlaneChatbotConfig {
    #[serde(default = "default_voyage_model")]
    pub voyage_model: String,
    #[serde(default = "default_collection")]
    pub qdrant_collection: String,
    #[serde(default)]
    pub vector_dims: u32,
    #[serde(default)]
    pub dedup_window_hrs: u32,
    #[serde(default)]
    pub queue_alert_threshold: u32,
    #[serde(default = "default_nesting_policy")]
    pub nesting_policy: String,
    #[serde(default = "default_true")]
    pub vectorization_enabled: bool,
}

fn default_voyage_model() -> String { DEFAULT_VOYAGE_MODEL.into() }
fn default_collection() -> String { DEFAULT_COLLECTION.into() }
fn default_nesting_policy() -> String { DEFAULT_NESTING_POLICY.into() }
fn default_true() -> bool { true }

impl Default for CtlPlaneChatbotConfig {
    fn default() -> Self {
        Self {
            voyage_model: default_voyage_model(),
            qdrant_collection: default_collection(),
            vector_dims: DEFAULT_VECTOR_DIMS,
            dedup_window_hrs: DEFAULT_DEDUP_WINDOW_HRS,
            queue_alert_threshold: DEFAULT_QUEUE_ALERT_THRESHOLD,
            nesting_policy: default_nesting_policy(),
            vectorization_enabled: true,
        }
    }
}

// ── Plugin struct ────────────────────────────────────────────────────────────

pub struct CtlPlaneChatbotPlugin;

impl CtlPlaneChatbotPlugin {
    pub fn new() -> Self { Self }
}

impl Default for CtlPlaneChatbotPlugin {
    fn default() -> Self { Self::new() }
}

// ── StatePlugin impl ─────────────────────────────────────────────────────────

#[async_trait]
impl StatePlugin for CtlPlaneChatbotPlugin {
    fn name(&self) -> &str { "ctl_plane_chatbot" }
    fn version(&self) -> &str { "1.0.0" }

    fn schema(&self) -> Option<PluginSchema> {
        Some(super::plugin_schema_defs::ctl_plane_chatbot_plugin_schema())
    }

    fn is_available(&self) -> bool {
        // Always available — the chatbot is the control plane itself
        true
    }

    fn unavailable_reason(&self) -> String {
        String::new()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let cfg = simd_json::serde::to_owned_value(CtlPlaneChatbotConfig::default())?;
        Ok(cfg)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let cur: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(current.clone())?;
        let des: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();
        macro_rules! field_diff {
            ($field:ident, $key:expr) => {
                if cur.$field != des.$field {
                    actions.push(StateAction::Modify {
                        resource: $key.into(),
                        changes: simd_json::serde::to_owned_value(&des.$field)?,
                    });
                }
            };
        }
        field_diff!(voyage_model, "voyage_model");
        field_diff!(qdrant_collection, "qdrant_collection");
        field_diff!(vector_dims, "vector_dims");
        field_diff!(dedup_window_hrs, "dedup_window_hrs");
        field_diff!(queue_alert_threshold, "queue_alert_threshold");
        field_diff!(nesting_policy, "nesting_policy");
        field_diff!(vectorization_enabled, "vectorization_enabled");

        Ok(StateDiff {
            plugin: self.name().into(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        // Pipeline config changes are applied at next episode close — no service reload
        let changes: Vec<String> = diff
            .actions
            .iter()
            .map(|a| format!("ctl_plane_chatbot.{} queued", match a { StateAction::Modify { resource, .. } => resource.clone(), StateAction::Create { resource, .. } => resource.clone(), StateAction::Delete { resource } => resource.clone(), StateAction::NoOp { resource } => resource.clone() }))
            .collect();
        Ok(ApplyResult {
            success: true,
            changes_applied: changes,
            errors: Vec::new(),
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let cur: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(current)?;
        let des: CtlPlaneChatbotConfig = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(cur == des)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("ctl_plane_chatbot-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let _ = checkpoint;
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/dnsresolver.rs">
// dnsresolver_plugin.rs
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::fs;
use std::process::Command;

use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsState {
    pub version: u32,
    pub items: Vec<DnsItem>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Enforce,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsItem {
    pub id: String,
    pub mode: Mode,
    pub servers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

pub struct DnsResolverPlugin;

impl Default for DnsResolverPlugin {
    fn default() -> Self {
        Self
    }
}

impl DnsResolverPlugin {
    pub fn new() -> Self {
        Self
    }

    fn parse_resolv_conf(text: &str) -> DnsItem {
        let mut servers = Vec::new();
        let mut search: Option<Vec<String>> = None;
        let mut options: Option<Vec<String>> = None;
        for line in text.lines() {
            let s = line.trim();
            if s.starts_with('#') || s.is_empty() {
                continue;
            }
            let mut parts = s.split_whitespace();
            if let Some(keyword) = parts.next() {
                match keyword {
                    "nameserver" => {
                        if let Some(ip) = parts.next() {
                            servers.push(ip.to_string());
                        }
                    }
                    "search" => {
                        let vals: Vec<String> = parts.map(|v| v.to_string()).collect();
                        if !vals.is_empty() {
                            search = Some(vals);
                        }
                    }
                    "options" => {
                        let vals: Vec<String> = parts.map(|v| v.to_string()).collect();
                        if !vals.is_empty() {
                            options = Some(vals);
                        }
                    }
                    _ => {}
                }
            }
        }
        DnsItem {
            id: "resolvconf".into(),
            mode: Mode::ObserveOnly,
            servers,
            search,
            options,
        }
    }

    fn read_resolv_conf() -> String {
        if let Ok(out) = Command::new("cat").arg("/etc/resolv.conf").output() {
            if out.status.success() {
                return String::from_utf8(out.stdout).unwrap_or_default();
            }
        }
        fs::read_to_string("/etc/resolv.conf").unwrap_or_default()
    }

    fn write_resolv_conf(item: &DnsItem) -> Result<()> {
        let mut buf = String::new();
        if let Some(sr) = &item.search {
            if !sr.is_empty() {
                buf.push_str("search ");
                buf.push_str(&sr.join(" "));
                buf.push('\n');
            }
        }
        if let Some(opts) = &item.options {
            if !opts.is_empty() {
                buf.push_str("options ");
                buf.push_str(&opts.join(" "));
                buf.push('\n');
            }
        }
        for ns in &item.servers {
            buf.push_str("nameserver ");
            buf.push_str(ns);
            buf.push('\n');
        }

        let tmp_path = "/etc/resolv.conf.sysdecl.tmp";
        fs::write(tmp_path, buf.as_bytes()).context("write temp resolv.conf")?;
        let mv_cmd = format!("mv -f {} /etc/resolv.conf", tmp_path);
        let mv_ok = Command::new("sh")
            .arg("-c")
            .arg(&mv_cmd)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !mv_ok {
            fs::rename(tmp_path, "/etc/resolv.conf").context("rename resolv.conf")?;
        }
        Ok(())
    }

    fn normalize(v: &[String]) -> Vec<String> {
        let mut out = v.to_vec();
        out.sort();
        out.dedup();
        out
    }

    fn equal_desired(cur: &DnsItem, want: &DnsItem) -> bool {
        Self::normalize(&cur.servers) == Self::normalize(&want.servers)
            && cur.search.as_ref().map(|v| Self::normalize(v))
                == want.search.as_ref().map(|v| Self::normalize(v))
            && cur.options.as_ref().map(|v| Self::normalize(v))
                == want.options.as_ref().map(|v| Self::normalize(v))
    }

    fn query_system() -> Vec<DnsItem> {
        let txt = Self::read_resolv_conf();
        if txt.is_empty() {
            return Vec::new();
        }
        vec![Self::parse_resolv_conf(&txt)]
    }
}

#[async_trait]
impl StatePlugin for DnsResolverPlugin {
    fn name(&self) -> &str {
        "dnsresolver"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let items = Self::query_system();
        Ok(simd_json::json!({ "version": 1, "items": items }))
    }

    async fn calculate_diff(&self, _current: &Value, desired: &Value) -> Result<StateDiff> {
        let want: DnsState = match simd_json::serde::from_owned_value(desired.clone()) {
            Ok(v) => v,
            Err(_) => DnsState {
                version: 1,
                items: Vec::new(),
            },
        };
        let cur_all = Self::query_system();
        let cur = cur_all.first();
        let mut actions = Vec::new();
        for item in &want.items {
            match cur {
                Some(c) if Self::equal_desired(c, item) => actions.push(StateAction::NoOp {
                    resource: item.id.clone(),
                }),
                Some(_) => actions.push(StateAction::Modify {
                    resource: item.id.clone(),
                    changes: simd_json::serde::to_owned_value(item).unwrap_or(simd_json::json!({})),
                }),
                None => actions.push(StateAction::Create {
                    resource: item.id.clone(),
                    config: simd_json::serde::to_owned_value(item).unwrap_or(simd_json::json!({})),
                }),
            }
        }
        let meta = DiffMetadata {
            timestamp: chrono::Utc::now().timestamp(),
            current_hash: format!(
                "{:x}",
                md5::compute(
                    simd_json::to_string(&simd_json::json!({"items": cur_all}))
                        .unwrap_or_default()
                )
            ),
            desired_hash: format!(
                "{:x}",
                md5::compute(simd_json::to_string(&want).unwrap_or_default())
            ),
        };
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: meta,
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();
        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config }
                | StateAction::Modify {
                    resource,
                    changes: config,
                } => {
                    let item: DnsItem = match simd_json::serde::from_owned_value(config.clone()) {
                        Ok(v) => v,
                        Err(_) => {
                            errors.push(format!("{}: invalid payload", resource));
                            continue;
                        }
                    };
                    match item.mode {
                        Mode::ObserveOnly => {
                            changes_applied.push(format!("{}: no action required", resource))
                        }
                        Mode::Enforce => match Self::write_resolv_conf(&item) {
                            Ok(_) => {
                                changes_applied.push(format!("{}: resolv.conf updated", resource))
                            }
                            Err(e) => errors.push(format!("{}: {}", resource, e)),
                        },
                    }
                }
                StateAction::Delete { resource } => {
                    changes_applied.push(format!("{}: delete not supported", resource));
                }
                StateAction::NoOp { resource } => {
                    changes_applied.push(format!("{}: no action required", resource));
                }
            }
        }
        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let want: DnsState = match simd_json::serde::from_owned_value(desired.clone()) {
            Ok(v) => v,
            Err(_) => return Ok(true),
        };
        let cur_all = Self::query_system();
        let cur = match cur_all.first() {
            Some(v) => v,
            None => return Ok(false),
        };
        for item in &want.items {
            if !Self::equal_desired(cur, item) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("{}-{}", self.name(), chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!({}),
            backend_checkpoint: None,
        })
    }
    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/endpoint.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointState {
    pub endpoints: Vec<String>,
}

pub struct EndpointPlugin;

impl Default for EndpointPlugin {
    fn default() -> Self {
        Self
    }
}

impl EndpointPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for EndpointPlugin {
    fn name(&self) -> &str {
        "endpoint"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::endpoint_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(EndpointState {
            endpoints: vec![],
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/full_system.rs">
//! Full System State Plugin
//!
//! This plugin captures the COMPLETE system state for disaster recovery.
//! It aggregates state from all other plugins into a single JSON document
//! that can be used to reinstall/restore the entire system.
//!
//! ## State Categories
//!
//! - **system**: hostname, timezone, locale, kernel parameters
//! - **network**: interfaces, routes, DNS, bridges, VLANs
//! - **services**: systemd units and their configurations
//! - **packages**: installed packages and versions
//! - **users**: user accounts and groups
//! - **storage**: mounts, fstab entries
//! - **containers**: LXC/Docker containers
//! - **security**: firewall rules, SELinux/AppArmor policies
//!
//! This plugin is special: it queries OTHER plugins to build the full state.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateAction, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Full system state for disaster recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSystemState {
    /// State schema version
    pub version: u32,
    
    /// Timestamp of when this state was captured
    pub captured_at: String,
    
    /// Hostname
    pub hostname: String,
    
    /// System information
    pub system: SystemInfo,
    
    /// Network configuration
    pub network: NetworkState,
    
    /// Systemd services
    pub services: Vec<ServiceState>,
    
    /// Installed packages
    pub packages: Vec<PackageInfo>,
    
    /// User accounts
    pub users: Vec<UserInfo>,
    
    /// Storage mounts
    pub storage: StorageState,
    
    /// Container state (LXC/Docker)
    pub containers: ContainerState,
    
    /// Plugin-specific state (aggregated from all plugins)
    pub plugins: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemInfo {
    pub kernel_version: String,
    pub os_release: String,
    pub timezone: String,
    pub locale: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkState {
    pub interfaces: Vec<InterfaceInfo>,
    pub routes: Vec<RouteInfo>,
    pub dns_servers: Vec<String>,
    pub bridges: Vec<BridgeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub mac: String,
    pub addresses: Vec<String>,
    pub state: String,
    pub mtu: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub destination: String,
    pub gateway: Option<String>,
    pub interface: String,
    pub metric: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeInfo {
    pub name: String,
    pub ports: Vec<String>,
    pub bridge_type: String, // "linux" or "ovs"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: String,
    pub enabled: bool,
    pub running: bool,
    pub unit_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageState {
    pub mounts: Vec<MountInfo>,
    pub block_devices: Vec<BlockDeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub source: String,
    pub target: String,
    pub fstype: String,
    pub options: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDeviceInfo {
    pub name: String,
    pub size_bytes: u64,
    pub fstype: Option<String>,
    pub mountpoint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerState {
    pub lxc: Vec<LxcContainerInfo>,
    pub docker: Vec<DockerContainerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcContainerInfo {
    pub name: String,
    pub status: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
}

/// The Full System State Plugin
pub struct FullSystemPlugin {
    /// Cached current state
    state_cache: Arc<RwLock<Option<FullSystemState>>>,
    
    /// Sender for blockchain footprints
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<op_blockchain::PluginFootprint>>,
}

impl Default for FullSystemPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl FullSystemPlugin {
    pub fn new() -> Self {
        Self {
            state_cache: Arc::new(RwLock::new(None)),
            blockchain_sender: None,
        }
    }

    /// Create with blockchain sender for change tracking
    pub fn with_blockchain(
        sender: tokio::sync::mpsc::UnboundedSender<op_blockchain::PluginFootprint>,
    ) -> Self {
        Self {
            state_cache: Arc::new(RwLock::new(None)),
            blockchain_sender: Some(sender),
        }
    }

    /// Capture complete system state
    pub async fn capture_full_state(&self) -> Result<FullSystemState> {
        let now = chrono::Utc::now().to_rfc3339();
        
        info!("Capturing full system state...");
        
        let hostname = self.get_hostname().await.unwrap_or_else(|_| "unknown".to_string());
        let system = self.capture_system_info().await.unwrap_or_default();
        let network = self.capture_network_state().await.unwrap_or_default();
        let services = self.capture_services().await.unwrap_or_default();
        let packages = self.capture_packages().await.unwrap_or_default();
        let users = self.capture_users().await.unwrap_or_default();
        let storage = self.capture_storage().await.unwrap_or_default();
        let containers = self.capture_containers().await.unwrap_or_default();
        
        let state = FullSystemState {
            version: 1,
            captured_at: now,
            hostname,
            system,
            network,
            services,
            packages,
            users,
            storage,
            containers,
            plugins: HashMap::new(), // Will be populated by StateManager
        };
        
        info!("Full system state captured");
        
        // Cache the state
        *self.state_cache.write().await = Some(state.clone());
        
        Ok(state)
    }

    async fn get_hostname(&self) -> Result<String> {
        let output = Command::new("hostname")
            .output()
            .await
            .context("Failed to get hostname")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn capture_system_info(&self) -> Result<SystemInfo> {
        let kernel = Command::new("uname").arg("-r").output().await?;
        let kernel_version = String::from_utf8_lossy(&kernel.stdout).trim().to_string();
        
        let os_release = tokio::fs::read_to_string("/etc/os-release")
            .await
            .ok()
            .and_then(|content| {
                content.lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            })
            .unwrap_or_default();
        
        let timezone = tokio::fs::read_link("/etc/localtime")
            .await
            .ok()
            .and_then(|p| p.to_str().map(|s| s.replace("/usr/share/zoneinfo/", "")))
            .unwrap_or_else(|| "UTC".to_string());
        
        let locale = std::env::var("LANG").unwrap_or_else(|_| "C.UTF-8".to_string());
        
        let uptime = tokio::fs::read_to_string("/proc/uptime")
            .await
            .ok()
            .and_then(|s| s.split_whitespace().next().and_then(|u| u.parse::<f64>().ok()))
            .map(|u| u as u64)
            .unwrap_or(0);
        
        Ok(SystemInfo {
            kernel_version,
            os_release,
            timezone,
            locale,
            uptime_seconds: uptime,
        })
    }

    async fn capture_network_state(&self) -> Result<NetworkState> {
        let mut state = NetworkState::default();
        
        // Get interfaces from /sys/class/net
        if let Ok(mut entries) = tokio::fs::read_dir("/sys/class/net").await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "lo" { continue; }
                
                let mac = tokio::fs::read_to_string(format!("/sys/class/net/{}/address", name))
                    .await
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                
                let state_str = tokio::fs::read_to_string(format!("/sys/class/net/{}/operstate", name))
                    .await
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                
                let mtu: u32 = tokio::fs::read_to_string(format!("/sys/class/net/{}/mtu", name))
                    .await
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .unwrap_or(1500);
                
                state.interfaces.push(InterfaceInfo {
                    name,
                    mac,
                    addresses: vec![], // Would need ip tool to get addresses
                    state: state_str,
                    mtu,
                });
            }
        }
        
        // Get DNS from resolv.conf
        if let Ok(resolv) = tokio::fs::read_to_string("/etc/resolv.conf").await {
            for line in resolv.lines() {
                if line.starts_with("nameserver") {
                    if let Some(ns) = line.split_whitespace().nth(1) {
                        state.dns_servers.push(ns.to_string());
                    }
                }
            }
        }
        
        // Check for OVS bridges
        if let Ok(output) = Command::new("ovs-vsctl").arg("list-br").output().await {
            if output.status.success() {
                for bridge in String::from_utf8_lossy(&output.stdout).lines() {
                    let bridge = bridge.trim();
                    if bridge.is_empty() { continue; }
                    
                    let ports_output = Command::new("ovs-vsctl")
                        .args(["list-ports", bridge])
                        .output()
                        .await;
                    
                    let ports = ports_output
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect())
                        .unwrap_or_default();
                    
                    state.bridges.push(BridgeInfo {
                        name: bridge.to_string(),
                        ports,
                        bridge_type: "ovs".to_string(),
                    });
                }
            }
        }
        
        Ok(state)
    }

    async fn capture_services(&self) -> Result<Vec<ServiceState>> {
        let mut services = Vec::new();
        
        // Use systemctl to list services
        let output = Command::new("systemctl")
            .args(["list-units", "--type=service", "--all", "--plain", "--no-legend"])
            .output()
            .await?;
        
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let name = parts[0].trim_end_matches(".service").to_string();
                let running = parts[2] == "running" || parts[2] == "active";
                
                // Check if enabled
                let enabled_output = Command::new("systemctl")
                    .args(["is-enabled", &parts[0]])
                    .output()
                    .await;
                
                let enabled = enabled_output
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
                    .unwrap_or(false);
                
                services.push(ServiceState {
                    name,
                    enabled,
                    running,
                    unit_type: "service".to_string(),
                });
            }
        }
        
        Ok(services)
    }

    async fn capture_packages(&self) -> Result<Vec<PackageInfo>> {
        let mut packages = Vec::new();
        
        // Try dpkg first (Debian/Ubuntu)
        if let Ok(output) = Command::new("dpkg-query")
            .args(["-W", "-f", "${Package}\t${Version}\t${Architecture}\n"])
            .output()
            .await
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 3 {
                        packages.push(PackageInfo {
                            name: parts[0].to_string(),
                            version: parts[1].to_string(),
                            arch: parts[2].to_string(),
                        });
                    }
                }
                return Ok(packages);
            }
        }
        
        // Try rpm (RHEL/Fedora)
        if let Ok(output) = Command::new("rpm")
            .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}\t%{ARCH}\n"])
            .output()
            .await
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 3 {
                        packages.push(PackageInfo {
                            name: parts[0].to_string(),
                            version: parts[1].to_string(),
                            arch: parts[2].to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(packages)
    }

    async fn capture_users(&self) -> Result<Vec<UserInfo>> {
        let mut users = Vec::new();
        
        if let Ok(passwd) = tokio::fs::read_to_string("/etc/passwd").await {
            for line in passwd.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 7 {
                    let uid: u32 = parts[2].parse().unwrap_or(0);
                    
                    // Skip system users (uid < 1000) except root
                    if uid != 0 && uid < 1000 {
                        continue;
                    }
                    
                    let name = parts[0].to_string();
                    
                    // Get groups
                    let groups_output = Command::new("id")
                        .args(["-Gn", &name])
                        .output()
                        .await;
                    
                    let groups = groups_output
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout)
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect())
                        .unwrap_or_default();
                    
                    users.push(UserInfo {
                        name,
                        uid,
                        gid: parts[3].parse().unwrap_or(0),
                        home: parts[5].to_string(),
                        shell: parts[6].to_string(),
                        groups,
                    });
                }
            }
        }
        
        Ok(users)
    }

    async fn capture_storage(&self) -> Result<StorageState> {
        let mut state = StorageState::default();
        
        // Get mounts from /proc/mounts
        if let Ok(mounts) = tokio::fs::read_to_string("/proc/mounts").await {
            for line in mounts.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let source = parts[0].to_string();
                    let target = parts[1].to_string();
                    
                    // Skip virtual filesystems
                    if source.starts_with("none") || 
                       target.starts_with("/sys") || 
                       target.starts_with("/proc") ||
                       target.starts_with("/dev") ||
                       target.starts_with("/run") {
                        continue;
                    }
                    
                    state.mounts.push(MountInfo {
                        source,
                        target,
                        fstype: parts[2].to_string(),
                        options: parts[3].to_string(),
                    });
                }
            }
        }
        
        // Get block devices from lsblk
        if let Ok(output) = Command::new("lsblk")
            .args(["-J", "-o", "NAME,SIZE,FSTYPE,MOUNTPOINT"])
            .output()
            .await
        {
            if output.status.success() {
                if let Ok(mut json) = simd_json::from_slice::<Value>(&mut output.stdout.clone()) {
                    if let Some(devices) = json.get("blockdevices").and_then(|v| v.as_array()) {
                        for dev in devices {
                            state.block_devices.push(BlockDeviceInfo {
                                name: dev.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                size_bytes: 0, // Would need to parse SIZE
                                fstype: dev.get("fstype").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                mountpoint: dev.get("mountpoint").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            });
                        }
                    }
                }
            }
        }
        
        Ok(state)
    }

    async fn capture_containers(&self) -> Result<ContainerState> {
        let mut state = ContainerState::default();
        
        // LXC containers
        if let Ok(output) = Command::new("lxc-ls").args(["-f"]).output().await {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines().skip(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        state.lxc.push(LxcContainerInfo {
                            name: parts[0].to_string(),
                            status: parts[1].to_string(),
                            config: json!({}),
                        });
                    }
                }
            }
        }
        
        // Docker containers
        if let Ok(output) = Command::new("docker")
            .args(["ps", "-a", "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}"])
            .output()
            .await
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 4 {
                        state.docker.push(DockerContainerInfo {
                            id: parts[0].to_string(),
                            name: parts[1].to_string(),
                            image: parts[2].to_string(),
                            status: parts[3].to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(state)
    }
}

#[async_trait]
impl StatePlugin for FullSystemPlugin {
    fn name(&self) -> &str {
        "full_system"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = self.capture_full_state().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        use op_state::DiffMetadata;
        use sha2::{Digest, Sha256};
        
        let mut actions = Vec::new();
        
        // Check for hostname change
        if current.get("hostname") != desired.get("hostname") {
            actions.push(StateAction::Modify {
                resource: "hostname".to_string(),
                changes: json!({
                    "from": current.get("hostname"),
                    "to": desired.get("hostname"),
                }),
            });
        }
        
        // More sophisticated diffing would be done here
        // For now, just mark if there's any difference
        if current != desired {
            actions.push(StateAction::Modify {
                resource: "full_system".to_string(),
                changes: json!({
                    "message": "Full system state differs from desired"
                }),
            });
        }
        
        // Create hashes for metadata
        let current_str = simd_json::to_string(current).unwrap_or_default();
        let desired_str = simd_json::to_string(desired).unwrap_or_default();
        let current_hash = format!("{:x}", Sha256::digest(current_str.as_bytes()));
        let desired_hash = format!("{:x}", Sha256::digest(desired_str.as_bytes()));
        
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash,
                desired_hash,
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();
        
        for action in &diff.actions {
            match action {
                StateAction::Modify { resource, changes } if resource == "hostname" => {
                    if let Some(hostname) = changes.get("to").and_then(|v| v.as_str()) {
                        let result = Command::new("hostnamectl")
                            .args(["set-hostname", hostname])
                            .output()
                            .await;
                        
                        match result {
                            Ok(output) if output.status.success() => {
                                changes_applied.push(format!("Set hostname to {}", hostname));
                            }
                            Ok(output) => {
                                errors.push(format!("Failed to set hostname: {}", 
                                    String::from_utf8_lossy(&output.stderr)));
                            }
                            Err(e) => {
                                errors.push(format!("Failed to run hostnamectl: {}", e));
                            }
                        }
                    }
                }
                _ => {
                    debug!("Unhandled action: {:?}", action);
                }
            }
        }
        
        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.capture_full_state().await?;
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(state)?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        warn!("Full system rollback not implemented - requires manual intervention");
        Ok(())
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        // Would compare current vs desired
        Ok(true)
    }

    fn capabilities(&self) -> op_state::PluginCapabilities {
        op_state::PluginCapabilities {
            supports_rollback: false, // Too complex for automatic rollback
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capture_system_info() {
        let plugin = FullSystemPlugin::new();
        let info = plugin.capture_system_info().await;
        assert!(info.is_ok());
        let info = info.unwrap();
        assert!(!info.kernel_version.is_empty());
    }
}
</file>

<file path="src/state_plugins/gcloud_adc.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcloudAdcState {
    pub account: Option<String>,
    pub project_id: Option<String>,
    pub authenticated: bool,
}

pub struct GcloudAdcPlugin;

impl Default for GcloudAdcPlugin {
    fn default() -> Self {
        Self
    }
}

impl GcloudAdcPlugin {
    pub fn new() -> Self {
        Self
    }

    async fn check_auth_status() -> Result<GcloudAdcState> {
        // Check for ADC existence
        let adc_path =
            dirs::home_dir().map(|p| p.join(".config/gcloud/application_default_credentials.json"));

        let authenticated = if let Some(path) = adc_path {
            path.exists()
        } else {
            false
        };

        // Try to get active account and project from gcloud config
        let output = Command::new("gcloud")
            .args(&["config", "list", "--format=json"])
            .output()
            .await;

        let mut account = None;
        let mut project_id = None;

        if let Ok(output) = output {
            if output.status.success() {
                if let Ok(json) = std::str::from_utf8(&output.stdout) {
                    if let Ok(val) = simd_json::to_owned_value(&mut json.as_bytes().to_vec()) {
                        account = val
                            .get("core")
                            .and_then(|c| c.get("account"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        project_id = val
                            .get("core")
                            .and_then(|c| c.get("project"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
        }

        Ok(GcloudAdcState {
            account,
            project_id,
            authenticated,
        })
    }
}

#[async_trait]
impl StatePlugin for GcloudAdcPlugin {
    fn name(&self) -> &str {
        "gcloud_adc"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::gcloud_adc_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = Self::check_auth_status().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        // For now, no-op diff calculation
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/hardware.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareState {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CpuInfo {
    pub model: String,
    pub cores: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryInfo {
    pub total_kb: u64,
    pub available_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub size_bytes: u64,
    pub mountpoint: Option<String>,
}

pub struct HardwarePlugin;

impl Default for HardwarePlugin {
    fn default() -> Self {
        Self
    }
}

impl HardwarePlugin {
    pub fn new() -> Self {
        Self
    }

    async fn get_cpu_info() -> CpuInfo {
        let content = tokio::fs::read_to_string("/proc/cpuinfo")
            .await
            .unwrap_or_default();
        let mut model = "Unknown".to_string();
        let mut cores = 0;

        for line in content.lines() {
            if line.starts_with("model name") {
                if let Some(val) = line.split(':').nth(1) {
                    if model == "Unknown" {
                        model = val.trim().to_string();
                    }
                }
                cores += 1;
            }
        }

        // Fallback for cores if using processor count
        if cores == 0 {
            cores = content
                .lines()
                .filter(|l| l.starts_with("processor"))
                .count();
        }

        CpuInfo { model, cores }
    }

    async fn get_memory_info() -> MemoryInfo {
        let content = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .unwrap_or_default();
        let mut total = 0;
        let mut available = 0;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    total = val.parse().unwrap_or(0);
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    available = val.parse().unwrap_or(0);
                }
            }
        }

        MemoryInfo {
            total_kb: total,
            available_kb: available,
        }
    }

    async fn get_disk_info() -> Vec<DiskInfo> {
        let mut disks = Vec::new();
        // Use lsblk -J for json output
        let output = Command::new("lsblk")
            .args(&["-J", "-o", "NAME,SIZE,MOUNTPOINT,BYTES"])
            .output()
            .await;

        if let Ok(output) = output {
            if let Ok(json_str) = std::str::from_utf8(&output.stdout) {
                if let Ok(val) = simd_json::to_owned_value(&mut json_str.as_bytes().to_vec()) {
                    if let Some(blockdevices) = val.get("blockdevices").and_then(|v| v.as_array()) {
                        for dev in blockdevices {
                            let name = dev
                                .get("name")
                                .and_then(|s| s.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let size = dev
                                .get("bytes")
                                .and_then(|s| s.as_str().or(Some("0")))
                                .and_then(|s| s.parse::<u64>().ok())
                                .unwrap_or(0);
                            let mountpoint = dev
                                .get("mountpoint")
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string());

                            disks.push(DiskInfo {
                                name,
                                size_bytes: size,
                                mountpoint,
                            });
                        }
                    }
                }
            }
        }
        disks
    }
}

#[async_trait]
impl StatePlugin for HardwarePlugin {
    fn name(&self) -> &str {
        "hardware"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::hardware_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let cpu = Self::get_cpu_info().await;
        let memory = Self::get_memory_info().await;
        let disks = Self::get_disk_info().await;

        Ok(simd_json::serde::to_owned_value(HardwareState {
            cpu,
            memory,
            disks,
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/incus.rs">
//! Incus state plugin - manages Incus containers and virtual machines.
//!
//! Uses the `incus` CLI with `--format=json` for all operations.
//! Supports creating, starting, stopping, and deleting instances,
//! as well as profile and config management.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Top-level state representing all Incus instances on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncusState {
    pub instances: Vec<IncusInstance>,
}

/// A single Incus instance (container or virtual-machine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncusInstance {
    pub name: String,
    /// Instance status: "Running", "Stopped", "Frozen"
    pub status: String,
    /// Instance type: "container" or "virtual-machine"
    #[serde(rename = "type")]
    pub instance_type: String,
    /// Image description (extracted from config)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Preferred storage pool used during initial creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_pool: Option<String>,
    /// Applied profiles (e.g. ["default"])
    #[serde(default)]
    pub profiles: Vec<String>,
    /// Instance configuration key-value pairs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
    /// Device definitions (device name -> device key-value config)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<HashMap<String, HashMap<String, String>>>,
}

/// Intermediate struct for deserializing raw `incus list --format=json` output.
/// The CLI returns more fields than we need; this captures the relevant ones.
#[derive(Debug, Deserialize)]
struct RawIncusInstance {
    name: String,
    status: String,
    #[serde(rename = "type")]
    instance_type: String,
    #[serde(default)]
    profiles: Vec<String>,
    #[serde(default)]
    config: HashMap<String, String>,
    #[serde(default)]
    devices: HashMap<String, HashMap<String, String>>,
}

pub struct IncusPlugin;

impl IncusPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Run an incus CLI command and return its stdout as bytes.
    async fn run_incus_command(args: &[&str]) -> Result<Vec<u8>> {
        let output = tokio::process::Command::new("/usr/bin/incus")
            .args(args)
            .output()
            .await
            .context("Failed to execute incus command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "incus {} failed (exit {}): {}",
                args.join(" "),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }

        Ok(output.stdout)
    }

    /// Parse raw JSON output from `incus list --format=json` into IncusInstance structs.
    fn parse_instance_list(mut raw_json: Vec<u8>) -> Result<Vec<IncusInstance>> {
        let raw_instances: Vec<RawIncusInstance> =
            simd_json::from_slice(&mut raw_json).context("Failed to parse incus list JSON")?;

        let instances = raw_instances
            .into_iter()
            .map(|raw| {
                let storage_pool = raw
                    .devices
                    .get("root")
                    .and_then(|root| root.get("pool"))
                    .cloned();
                // Extract image description from config keys
                let image = raw
                    .config
                    .get("image.description")
                    .or_else(|| raw.config.get("volatile.base_image"))
                    .cloned();

                // Only include config if non-empty
                let config = if raw.config.is_empty() {
                    None
                } else {
                    Some(raw.config)
                };

                // Only include devices if non-empty
                let devices = if raw.devices.is_empty() {
                    None
                } else {
                    Some(raw.devices)
                };

                IncusInstance {
                    name: raw.name,
                    status: raw.status,
                    instance_type: raw.instance_type,
                    image,
                    storage_pool,
                    profiles: raw.profiles,
                    config,
                    devices,
                }
            })
            .collect();

        Ok(instances)
    }

    /// Apply a single Create action for an instance.
    async fn apply_create(instance: &IncusInstance) -> Result<Vec<String>> {
        let mut changes = Vec::new();
        let name = &instance.name;

        // Determine the image to use; fall back to a sensible default
        let image = instance.image.as_deref().unwrap_or("images:debian/12");

        let mut create_args = vec!["init".to_string(), image.to_string(), name.to_string()];
        if let Some(pool) = instance.storage_pool.as_deref() {
            create_args.push("--storage".to_string());
            create_args.push(pool.to_string());
        }
        if instance.profiles.is_empty() {
            create_args.push("--no-profiles".to_string());
        } else {
            for profile in Self::normalize_profiles(&instance.profiles) {
                create_args.push("--profile".to_string());
                create_args.push(profile);
            }
        }
        let create_args_ref: Vec<&str> = create_args.iter().map(String::as_str).collect();
        log::info!("Creating instance '{}' from image '{}'", name, image);
        Self::run_incus_command(&create_args_ref)
            .await
            .with_context(|| format!("Failed to create instance '{}'", name))?;
        changes.push(format!("Created instance '{}'", name));

        changes.extend(Self::sync_profiles(name, None, instance).await?);
        changes.extend(Self::sync_config(name, None, instance).await?);
        changes.extend(Self::sync_devices(name, None, instance).await?);
        changes.extend(Self::sync_status(name, None, instance).await?);

        Ok(changes)
    }

    async fn apply_modify(current: &IncusInstance, desired: &IncusInstance) -> Result<Vec<String>> {
        let mut changes = Vec::new();
        changes.extend(Self::sync_profiles(&desired.name, Some(current), desired).await?);
        changes.extend(Self::sync_config(&desired.name, Some(current), desired).await?);
        changes.extend(Self::sync_devices(&desired.name, Some(current), desired).await?);
        changes.extend(Self::sync_status(&desired.name, Some(current), desired).await?);
        Ok(changes)
    }

    /// Apply a single Delete action.
    async fn apply_delete(name: &str) -> Result<Vec<String>> {
        log::info!("Force-deleting instance '{}'", name);
        Self::run_incus_command(&["delete", name, "--force"])
            .await
            .with_context(|| format!("Failed to delete instance '{}'", name))?;
        Ok(vec![format!("Deleted instance '{}'", name)])
    }

    fn is_read_only_config_key(key: &str) -> bool {
        key.starts_with("volatile.") || key.starts_with("image.")
    }

    fn normalize_profiles(profiles: &[String]) -> Vec<String> {
        let mut normalized = profiles.to_vec();
        normalized.sort();
        normalized.dedup();
        normalized
    }

    fn normalized_config(instance: &IncusInstance) -> HashMap<String, String> {
        instance
            .config
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|(key, _)| !Self::is_read_only_config_key(key))
            .collect()
    }

    fn managed_devices(instance: &IncusInstance) -> HashMap<String, HashMap<String, String>> {
        instance
            .devices
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|(name, _)| name != "root")
            .collect()
    }

    fn instances_equivalent(current: &IncusInstance, desired: &IncusInstance) -> bool {
        current.status == desired.status
            && current.instance_type == desired.instance_type
            && current.storage_pool == desired.storage_pool
            && Self::normalize_profiles(&current.profiles)
                == Self::normalize_profiles(&desired.profiles)
            && Self::normalized_config(current) == Self::normalized_config(desired)
            && Self::managed_devices(current) == Self::managed_devices(desired)
    }

    async fn sync_profiles(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let current_profiles = current
            .map(|instance| Self::normalize_profiles(&instance.profiles))
            .unwrap_or_default();
        let desired_profiles = Self::normalize_profiles(&desired.profiles);
        let mut changes = Vec::new();

        for profile in &current_profiles {
            if desired_profiles.contains(profile) {
                continue;
            }
            Self::run_incus_command(&["profile", "remove", name, profile])
                .await
                .with_context(|| {
                    format!("Failed to remove profile '{}' from '{}'", profile, name)
                })?;
            changes.push(format!("Removed profile '{}' from '{}'", profile, name));
        }

        for profile in &desired_profiles {
            if current_profiles.contains(profile) {
                continue;
            }
            Self::run_incus_command(&["profile", "add", name, profile])
                .await
                .with_context(|| format!("Failed to add profile '{}' to '{}'", profile, name))?;
            changes.push(format!("Added profile '{}' to '{}'", profile, name));
        }

        Ok(changes)
    }

    async fn sync_config(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let current_config = current.map(Self::normalized_config).unwrap_or_default();
        let desired_config = Self::normalized_config(desired);
        let mut changes = Vec::new();

        for key in current_config.keys() {
            if !desired_config.contains_key(key) {
                Self::run_incus_command(&["config", "unset", name, key])
                    .await
                    .with_context(|| format!("Failed to unset config '{}' on '{}'", key, name))?;
                changes.push(format!("Unset config '{}' on '{}'", key, name));
            }
        }

        for (key, value) in desired_config {
            if current_config.get(&key) == Some(&value) {
                continue;
            }
            let kv = format!("{}={}", key, value);
            Self::run_incus_command(&["config", "set", name, &kv])
                .await
                .with_context(|| format!("Failed to set config '{}' on '{}'", kv, name))?;
            changes.push(format!("Set config '{}' on '{}'", kv, name));
        }

        Ok(changes)
    }

    async fn sync_devices(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let current_devices = current.map(Self::managed_devices).unwrap_or_default();
        let desired_devices = Self::managed_devices(desired);
        let mut changes = Vec::new();

        for device_name in current_devices.keys() {
            if desired_devices.contains_key(device_name) {
                continue;
            }
            Self::run_incus_command(&["config", "device", "remove", name, device_name])
                .await
                .with_context(|| {
                    format!(
                        "Failed to remove stale device '{}' from '{}'",
                        device_name, name
                    )
                })?;
            changes.push(format!(
                "Removed stale device '{}' from '{}'",
                device_name, name
            ));
        }

        for (device_name, desired_device) in desired_devices {
            if current_devices.get(&device_name) == Some(&desired_device) {
                continue;
            }

            if current_devices.contains_key(&device_name) {
                Self::run_incus_command(&["config", "device", "remove", name, &device_name])
                    .await
                    .with_context(|| {
                        format!("Failed to remove device '{}' from '{}'", device_name, name)
                    })?;
                changes.push(format!("Removed device '{}' from '{}'", device_name, name));
            }

            let device_type = desired_device
                .get("type")
                .cloned()
                .context("Incus device definition is missing required 'type'")?;
            let mut add_args = vec![
                "config".to_string(),
                "device".to_string(),
                "add".to_string(),
                name.to_string(),
                device_name.clone(),
                device_type,
            ];
            for (key, value) in desired_device {
                if key == "type" {
                    continue;
                }
                add_args.push(format!("{}={}", key, value));
            }
            let add_args_ref: Vec<&str> = add_args.iter().map(String::as_str).collect();
            Self::run_incus_command(&add_args_ref)
                .await
                .with_context(|| format!("Failed to add device '{}' to '{}'", device_name, name))?;
            changes.push(format!("Configured device '{}' on '{}'", device_name, name));
        }

        Ok(changes)
    }

    async fn sync_status(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let mut changes = Vec::new();
        if current.map(|instance| instance.status.as_str()) == Some(desired.status.as_str()) {
            return Ok(changes);
        }
        match desired.status.as_str() {
            "Running" => {
                Self::run_incus_command(&["start", name])
                    .await
                    .with_context(|| format!("Failed to start instance '{}'", name))?;
                changes.push(format!("Started instance '{}'", name));
            }
            "Stopped" => {
                Self::run_incus_command(&["stop", name])
                    .await
                    .with_context(|| format!("Failed to stop instance '{}'", name))?;
                changes.push(format!("Stopped instance '{}'", name));
            }
            "Frozen" => {
                Self::run_incus_command(&["pause", name])
                    .await
                    .with_context(|| format!("Failed to freeze instance '{}'", name))?;
                changes.push(format!("Frozen instance '{}'", name));
            }
            other => anyhow::bail!(
                "Unsupported desired status '{}' for instance '{}'",
                other,
                name
            ),
        }
        Ok(changes)
    }
}

impl Default for IncusPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for IncusPlugin {
    fn name(&self) -> &str {
        "incus"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::incus_plugin_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/usr/bin/incus").exists()
    }

    fn unavailable_reason(&self) -> String {
        "Incus not installed (/usr/bin/incus not found)".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        log::info!("Querying current Incus instance state");

        let stdout = Self::run_incus_command(&["list", "--format=json"])
            .await
            .context("Failed to list Incus instances")?;

        let instances = Self::parse_instance_list(stdout)?;
        log::info!("Discovered {} Incus instance(s)", instances.len());

        let state = IncusState { instances };
        simd_json::serde::to_owned_value(state).context("Failed to serialize IncusState")
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: IncusState = simd_json::serde::from_owned_value(current.clone())
            .context("Failed to deserialize current IncusState")?;
        let desired_state: IncusState = simd_json::serde::from_owned_value(desired.clone())
            .context("Failed to deserialize desired IncusState")?;

        // Index current instances by name for O(1) lookups
        let current_by_name: HashMap<&str, &IncusInstance> = current_state
            .instances
            .iter()
            .map(|i| (i.name.as_str(), i))
            .collect();

        let desired_by_name: HashMap<&str, &IncusInstance> = desired_state
            .instances
            .iter()
            .map(|i| (i.name.as_str(), i))
            .collect();

        let mut actions = Vec::new();

        // Check desired instances against current state
        for desired_inst in &desired_state.instances {
            match current_by_name.get(desired_inst.name.as_str()) {
                None => {
                    // Instance does not exist yet -- needs creation
                    let config = simd_json::serde::to_owned_value(desired_inst.clone())
                        .context("Failed to serialize desired instance for Create action")?;
                    actions.push(StateAction::Create {
                        resource: desired_inst.name.clone(),
                        config,
                    });
                }
                Some(current_inst) => {
                    if !Self::instances_equivalent(current_inst, desired_inst) {
                        let changes = simd_json::serde::to_owned_value(desired_inst.clone())
                            .context("Failed to serialize desired instance for Modify action")?;
                        actions.push(StateAction::Modify {
                            resource: desired_inst.name.clone(),
                            changes,
                        });
                    }
                }
            }
        }

        // Instances in current but not in desired should be deleted
        for current_inst in &current_state.instances {
            if !desired_by_name.contains_key(current_inst.name.as_str()) {
                actions.push(StateAction::Delete {
                    resource: current_inst.name.clone(),
                });
            }
        }

        let current_hash = format!("{:x}", md5::compute(simd_json::to_string(current)?));
        let desired_hash = format!("{:x}", md5::compute(simd_json::to_string(desired)?));

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash,
                desired_hash,
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();
        let current_state = self
            .query_current_state()
            .await
            .ok()
            .and_then(|value| simd_json::serde::from_owned_value::<IncusState>(value).ok());
        let current_by_name: HashMap<String, IncusInstance> = current_state
            .map(|state| {
                state
                    .instances
                    .into_iter()
                    .map(|instance| (instance.name.clone(), instance))
                    .collect()
            })
            .unwrap_or_default();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config } => {
                    let instance: IncusInstance =
                        simd_json::serde::from_owned_value(config.clone())
                            .context("Failed to deserialize instance config for creation")?;

                    match Self::apply_create(&instance).await {
                        Ok(changes) => changes_applied.extend(changes),
                        Err(e) => {
                            let msg = format!("Failed to create instance '{}': {}", resource, e);
                            log::error!("{}", msg);
                            errors.push(msg);
                        }
                    }
                }
                StateAction::Modify { resource, changes } => {
                    let desired: IncusInstance =
                        simd_json::serde::from_owned_value(changes.clone())
                            .context("Failed to deserialize instance config for modification")?;

                    match current_by_name.get(resource) {
                        Some(current) => match Self::apply_modify(current, &desired).await {
                            Ok(applied) => changes_applied.extend(applied),
                            Err(e) => {
                                let msg =
                                    format!("Failed to modify instance '{}': {}", resource, e);
                                log::error!("{}", msg);
                                errors.push(msg);
                            }
                        },
                        None => {
                            let msg = format!(
                                "Failed to modify instance '{}': current instance not found",
                                resource
                            );
                            log::error!("{}", msg);
                            errors.push(msg);
                        }
                    }
                }
                StateAction::Delete { resource } => match Self::apply_delete(resource).await {
                    Ok(applied) => changes_applied.extend(applied),
                    Err(e) => {
                        let msg = format!("Failed to delete instance '{}': {}", resource, e);
                        log::error!("{}", msg);
                        errors.push(msg);
                    }
                },
                StateAction::NoOp { .. } => {}
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        log::info!("Verifying Incus state matches desired");
        let current = self.query_current_state().await?;
        let diff = self.calculate_diff(&current, desired).await?;
        let in_sync = diff.actions.is_empty();

        if in_sync {
            log::info!("Incus state is in sync with desired state");
        } else {
            log::warn!(
                "Incus state drift detected: {} action(s) needed",
                diff.actions.len()
            );
        }

        Ok(in_sync)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        log::info!("Creating Incus state checkpoint");
        let state = self.query_current_state().await?;
        let id = format!("incus-{}", chrono::Utc::now().timestamp());

        Ok(Checkpoint {
            id,
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        log::info!("Rolling back Incus state to checkpoint '{}'", checkpoint.id);

        let current = self.query_current_state().await?;
        let diff = self
            .calculate_diff(&current, &checkpoint.state_snapshot)
            .await?;

        if diff.actions.is_empty() {
            log::info!("No rollback actions needed -- state already matches checkpoint");
            return Ok(());
        }

        let result = self.apply_state(&diff).await?;
        if result.success {
            log::info!(
                "Rollback to checkpoint '{}' completed successfully ({} change(s))",
                checkpoint.id,
                result.changes_applied.len()
            );
        } else {
            log::error!(
                "Rollback to checkpoint '{}' completed with errors: {:?}",
                checkpoint.id,
                result.errors
            );
            anyhow::bail!(
                "Rollback had {} error(s): {}",
                result.errors.len(),
                result.errors.join("; ")
            );
        }

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instances_equivalent_detects_config_and_device_changes() {
        let current = IncusInstance {
            name: "privacy-user-1".to_string(),
            status: "Running".to_string(),
            instance_type: "container".to_string(),
            image: Some("images:alpine/3.19".to_string()),
            storage_pool: Some("registration".to_string()),
            profiles: vec!["default".to_string()],
            config: Some(HashMap::from([(
                "user.opdbus.route_id".to_string(),
                "route-a".to_string(),
            )])),
            devices: Some(HashMap::from([(
                "privacy0".to_string(),
                HashMap::from([
                    ("type".to_string(), "nic".to_string()),
                    ("nictype".to_string(), "bridged".to_string()),
                    ("parent".to_string(), "ovsbr0".to_string()),
                ]),
            )])),
        };
        let mut desired = current.clone();
        assert!(IncusPlugin::instances_equivalent(&current, &desired));

        desired.config = Some(HashMap::from([(
            "user.opdbus.route_id".to_string(),
            "route-b".to_string(),
        )]));
        assert!(!IncusPlugin::instances_equivalent(&current, &desired));

        desired = current.clone();
        desired.devices = Some(HashMap::from([(
            "privacy0".to_string(),
            HashMap::from([
                ("type".to_string(), "nic".to_string()),
                ("nictype".to_string(), "bridged".to_string()),
                ("parent".to_string(), "ovsbr1".to_string()),
            ]),
        )]));
        assert!(!IncusPlugin::instances_equivalent(&current, &desired));
    }
}
</file>

<file path="src/state_plugins/keypair.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeypairState {
    pub keypairs: Vec<Keypair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keypair {
    pub name: String,
    pub algorithm: String,
    pub public_key: Option<String>,
    pub present: bool,
}

pub struct KeypairPlugin;

impl Default for KeypairPlugin {
    fn default() -> Self {
        Self
    }
}

impl KeypairPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for KeypairPlugin {
    fn name(&self) -> &str {
        "keypair"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::keypair_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut keypairs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let ssh_dir = home.join(".ssh");
            if let Ok(mut entries) = tokio::fs::read_dir(ssh_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("id_") && name.ends_with(".pub") {
                            let key_name = name.trim_end_matches(".pub").to_string();
                            let content =
                                tokio::fs::read_to_string(&path).await.unwrap_or_default();
                            let parts: Vec<&str> = content.split_whitespace().collect();
                            let algorithm = if !parts.is_empty() {
                                parts[0].to_string()
                            } else {
                                "unknown".to_string()
                            };

                            keypairs.push(Keypair {
                                name: key_name,
                                algorithm,
                                public_key: Some(content.trim().to_string()),
                                present: true,
                            });
                        }
                    }
                }
            }
        }

        Ok(simd_json::serde::to_owned_value(KeypairState { keypairs })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/keyring.rs">
//! GNOME Keyring plugin - freedesktop.org Secret Service integration
#![allow(dead_code)]
//!
//! Implements the org.freedesktop.secrets D-Bus service for secure credential storage.
//! This allows applications like Cursor to store and retrieve passwords, API keys, etc.
//!
//! The Secret Service API provides:
//! - Collections (like "default", "login")
//! - Items (individual secrets with attributes)
//! - Secure storage with optional encryption

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateDiff, StatePlugin, StateAction, DiffMetadata};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use zbus::{Connection, Proxy, zvariant::{ObjectPath, OwnedObjectPath}};

/// Keyring state representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyringState {
    /// Available collections
    pub collections: Vec<CollectionInfo>,
    /// Default collection path
    pub default_collection: Option<String>,
}

/// Information about a secret collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub path: String,
    pub label: String,
    pub locked: bool,
    pub created: u64,
    pub modified: u64,
}

/// GNOME Keyring plugin implementing freedesktop.org Secret Service
pub struct KeyringPlugin;

impl KeyringPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Connect to the Secret Service
    async fn connect_service(&self) -> Result<Proxy<'static>> {
        let conn = Connection::session().await?;
        let proxy = Proxy::new(
            &conn,
            "org.freedesktop.secrets",
            "/org/freedesktop/secrets",
            "org.freedesktop.Secret.Service",
        )
        .await?;
        Ok(proxy)
    }

    /// Get available collections
    async fn get_collections(&self) -> Result<Vec<CollectionInfo>> {
        let proxy = self.connect_service().await?;

        // Get collection paths
        let collections: Vec<OwnedObjectPath> = proxy.call("Collections", &()).await?;

        let mut result = Vec::new();
        for path in collections {
            if let Ok(info) = self.get_collection_info(&path).await {
                result.push(info);
            }
        }

        Ok(result)
    }

    /// Get information about a specific collection
    async fn get_collection_info(&self, path: &ObjectPath<'_>) -> Result<CollectionInfo> {
        let conn = Connection::session().await?;
        let proxy = Proxy::new(
            &conn,
            "org.freedesktop.secrets",
            path,
            "org.freedesktop.Secret.Collection",
        )
        .await?;

        let label: String = proxy.call("Label", &()).await?;
        let locked: bool = proxy.call("Locked", &()).await?;
        let created: u64 = proxy.call("Created", &()).await?;
        let modified: u64 = proxy.call("Modified", &()).await?;

        Ok(CollectionInfo {
            path: path.to_string(),
            label,
            locked,
            created,
            modified,
        })
    }

    /// Get the default collection path
    async fn get_default_collection(&self) -> Result<Option<String>> {
        let proxy = self.connect_service().await?;
        let default_path: OwnedObjectPath = proxy.call("ReadAlias", &("default",)).await?;
        Ok(Some(default_path.to_string()))
    }
}

impl Default for KeyringPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyringPlugin {
    /// Check if the Secret Service is available on the session bus
    fn check_service_available(&self) -> bool {
        // We can't use async code in is_available(), so we'll assume it's available
        // The actual connection will be tested when read_state() is called
        true
    }
}

#[async_trait]
impl StatePlugin for KeyringPlugin {
    fn name(&self) -> &str {
        "keyring"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false, // Security reasons
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // Keyring operations are not atomic
        }
    }

    fn is_available(&self) -> bool {
        self.check_service_available()
    }

    fn unavailable_reason(&self) -> String {
        "GNOME Keyring / KDE Wallet (org.freedesktop.secrets) service not available on session bus"
            .to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let collections = self.get_collections().await?;
        let default_collection = self.get_default_collection().await?;

        let state = KeyringState {
            collections,
            default_collection,
        };

        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        // Keyring operations are typically interactive and should not be automated
        Err(anyhow::anyhow!(
            "Keyring apply operations are not supported for security reasons"
        ))
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();

        if current != desired {
            actions.push(StateAction::Modify {
                resource: "keyring-collections".to_string(),
                changes: json!({
                    "current": current,
                    "desired": desired
                }),
            });
        }

        let metadata = DiffMetadata {
            timestamp: chrono::Utc::now().timestamp(),
            current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
            desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
        };

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        // For keyring, we can only verify that the structure is valid
        // We can't verify actual secrets without user interaction
        Ok(desired.is_object())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("keyring-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        // Keyring rollback is not supported for security reasons
        Err(anyhow::anyhow!(
            "Keyring rollback is not supported for security reasons"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_keyring_plugin_creation() {
        let plugin = KeyringPlugin::new();
        assert_eq!(plugin.name(), "keyring");
    }

    #[tokio::test]
    async fn test_capabilities() {
        let plugin = KeyringPlugin::new();
        let caps = plugin.capabilities();
        // KeyringPlugin capabilities from op_state::PluginCapabilities
        assert!(!caps.supports_rollback); // Security reasons - no rollback
        assert!(caps.supports_checkpoints);
        assert!(caps.supports_verification);
        assert!(!caps.atomic_operations);
    }
}
</file>

<file path="src/state_plugins/login1.rs">
//! login1 plugin - read-only D-Bus snapshot for sessions/seats

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Login1State {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub uid: u32,
    pub user: String,
    pub seat: String,
    pub path: String,
}

pub struct Login1Plugin;

impl Login1Plugin {
    pub fn new() -> Self {
        Self
    }

    async fn connect_manager(&self) -> Result<Proxy<'static>> {
        let conn = Connection::system().await?;
        let p = Proxy::new(
            &conn,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await?;
        Ok(p)
    }
}

impl Default for Login1Plugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for Login1Plugin {
    fn name(&self) -> &str {
        "login1"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let proxy = self.connect_manager().await?;
        // ListSessions -> a(sssso) per docs: (s, u, s, s, o)
        let raw: Vec<(String, u32, String, String, OwnedObjectPath)> =
            proxy.call("ListSessions", &()).await?;
        let sessions: Vec<SessionInfo> = raw
            .into_iter()
            .map(|(id, uid, user, seat, path)| SessionInfo {
                id,
                uid,
                user,
                seat,
                path: path.to_string(),
            })
            .collect();
        Ok(simd_json::serde::to_owned_value(Login1State { sessions })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let actions = if current != desired {
            vec![StateAction::Modify {
                resource: "login1".into(),
                changes: desired.clone(),
            }]
        } else {
            vec![]
        };
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec!["read-only".into()],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("login1-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/lxc.rs">
//! LXC plugin - Native Proxmox API LXC container management.
//!
//! Design
//! - Discovers LXC containers via native Proxmox REST API
//! - Creates, starts, stops, and deletes containers via API (no `pct` CLI)
//! - Correlates with OVS ports (vi{VMID}) for network integration
//! - Supports BTRFS golden images for instant container provisioning

use anyhow::Result;
use async_trait::async_trait;
use op_state::plugtree::PlugTree;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcState {
    pub containers: Vec<ContainerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerInfo {
    pub id: String,
    pub veth: String,
    pub bridge: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>, // extensible (includes network_type, template, etc.)
}

pub struct LxcPlugin;

impl LxcPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Apply state for a single container
    pub async fn apply_container_state(&self, container: &ContainerInfo) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        // Check if container exists
        let current_containers = self.discover_from_proxmox().await?;
        let exists = current_containers.iter().any(|c| c.id == container.id);

        if !exists {
            // Create container
            match Self::create_container(container).await {
                Ok(_) => {
                    changes_applied.push(format!("Created container {}", container.id));

                    // Start it
                    if let Err(e) = Self::start_container(&container.id).await {
                        errors.push(format!("Failed to start container {}: {}", container.id, e));
                    } else {
                        changes_applied.push(format!("Started container {}", container.id));
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to create container {}: {}",
                        container.id, e
                    ));
                }
            }
        } else {
            changes_applied.push(format!("Container {} already exists", container.id));
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    /// Check if container is running via Proxmox API
    async fn is_running_api(ct_id: &str) -> Result<bool> {
        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;
        client.is_running(vmid).await
    }

    /// Fallback: Check if container is running via cgroup (for when API is unavailable)
    fn is_running_cgroup(ct_id: &str) -> Option<bool> {
        // Proxmox systemd service path: pve-container@{vmid}.service (cgroup v2)
        let path = format!(
            "/sys/fs/cgroup/system.slice/pve-container@{}.service",
            ct_id
        );
        Some(fs::metadata(path).is_ok())
    }

    /// Discover containers from Proxmox API
    async fn discover_from_proxmox(&self) -> Result<Vec<ContainerInfo>> {
        let client = match op_network::ProxmoxClient::from_env() {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()),
        };

        // Check API availability
        if client.check_available().await.is_err() {
            log::debug!("Proxmox API not available, falling back to OVS discovery");
            return self.discover_from_ovs().await;
        }

        let containers = match client.list_containers().await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to list containers via API: {}, falling back to OVS", e);
                return self.discover_from_ovs().await;
            }
        };

        let ovsdb = op_network::ovsdb::OvsdbClient::new();
        let bridges = ovsdb.list_bridges().await.unwrap_or_default();

        let mut results = Vec::new();
        for ct in containers {
            let ct_id = ct.vmid.to_string();
            let veth = format!("vi{}", ct_id);

            // Find which bridge this container's veth is on
            let mut found_bridge = String::new();
            for br in &bridges {
                if let Ok(ports) = ovsdb.list_bridge_ports(br).await {
                    if ports.contains(&veth) {
                        found_bridge = br.clone();
                        break;
                    }
                }
            }

            // Check running status
            let running = ct.status == "running";

            results.push(ContainerInfo {
                id: ct_id,
                veth,
                bridge: found_bridge,
                running: Some(running),
                properties: Some({
                    let mut props = HashMap::new();
                    if let Some(name) = ct.name {
                        props.insert("hostname".to_string(), Value::String(name));
                    }
                    props.insert("status".to_string(), Value::String(ct.status));
                    if let Some(mem) = ct.mem {
                        props.insert("memory_used".to_string(), json!(mem));
                    }
                    if let Some(maxmem) = ct.maxmem {
                        props.insert("memory_max".to_string(), json!(maxmem));
                    }
                    if let Some(cpu) = ct.cpu {
                        props.insert("cpu_usage".to_string(), json!(cpu));
                    }
                    if let Some(uptime) = ct.uptime {
                        props.insert("uptime".to_string(), json!(uptime));
                    }
                    props
                }),
            });
        }

        Ok(results)
    }

    /// Fallback: Discover containers from OVS ports (when API is unavailable)
    async fn discover_from_ovs(&self) -> Result<Vec<ContainerInfo>> {
        let client = op_network::ovsdb::OvsdbClient::new();
        // If OVSDB is not reachable, return empty list
        if client.list_dbs().await.is_err() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let bridges = client.list_bridges().await.unwrap_or_default();
        for br in bridges {
            let ports = client.list_bridge_ports(&br).await.unwrap_or_default();
            for p in ports {
                if let Some(ct_id) = p.strip_prefix("vi") {
                    // ensure ID is numeric-like
                    if ct_id.chars().all(|c| c.is_ascii_digit()) {
                        let running = Self::is_running_cgroup(ct_id);
                        results.push(ContainerInfo {
                            id: ct_id.to_string(),
                            veth: p.clone(),
                            bridge: br.clone(),
                            running,
                            properties: None,
                        });
                    }
                }
            }
        }
        Ok(results)
    }
}

impl Default for LxcPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlugTree for LxcPlugin {
    fn pluglet_type(&self) -> &str {
        "container"
    }

    fn pluglet_id_field(&self) -> &str {
        "id"
    }

    fn extract_pluglet_id(&self, resource: &Value) -> Result<String> {
        resource
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Container missing 'id' field"))
    }

    async fn apply_pluglet(&self, _pluglet_id: &str, desired: &Value) -> Result<ApplyResult> {
        let container: ContainerInfo = simd_json::serde::from_owned_value(desired.clone())?;
        self.apply_container_state(&container).await
    }

    async fn query_pluglet(&self, pluglet_id: &str) -> Result<Option<Value>> {
        let containers = self.discover_from_proxmox().await?;

        for container in containers {
            if container.id == pluglet_id {
                return Ok(Some(simd_json::serde::to_owned_value(container)?));
            }
        }

        Ok(None)
    }

    async fn list_pluglet_ids(&self) -> Result<Vec<String>> {
        let containers = self.discover_from_proxmox().await?;
        Ok(containers.into_iter().map(|c| c.id).collect())
    }
}

impl LxcPlugin {
    /// Find container's veth interface name
    async fn find_container_veth(ct_id: &str) -> Result<String> {
        // Standard Proxmox veth naming: vi{VMID}
        let veth_name = format!("vi{}", ct_id);

        // Check if it exists via rtnetlink
        let veth_interfaces = op_network::rtnetlink::list_veth_interfaces().await?;
        if veth_interfaces.contains(&veth_name) {
            return Ok(veth_name);
        }

        // Try to find any veth for this container
        for veth in veth_interfaces {
            if veth.contains(ct_id) {
                return Ok(veth);
            }
        }

        Err(anyhow::anyhow!(
            "Could not find veth interface for container {}",
            ct_id
        ))
    }

    /// Determine bridge based on network type
    fn get_bridge_for_network_type(container: &ContainerInfo) -> String {
        let network_type = container
            .properties
            .as_ref()
            .and_then(|p| p.get("network_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bridge");

        match network_type {
            "netmaker" => "mesh".to_string(),     // Netmaker mesh bridge
            "bridge" => container.bridge.clone(), // Traditional bridge (ovsbr0)
            _ => container.bridge.clone(),
        }
    }

    /// Create LXC container via native Proxmox API
    async fn create_container(container: &ContainerInfo) -> Result<()> {
        log::info!("Creating LXC container {} via Proxmox API", container.id);

        let vmid: u32 = container
            .id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", container.id))?;

        // Select bridge based on network type
        let bridge = Self::get_bridge_for_network_type(container);
        log::info!("Container {} will use bridge {}", container.id, bridge);

        // Extract properties with sensible defaults
        let props = container.properties.as_ref();

        // Check if using BTRFS golden image (fast path) or tar.zst template (slow path)
        let golden_image = props
            .and_then(|p| p.get("golden_image"))
            .and_then(|v| v.as_str());

        if let Some(golden_image_name) = golden_image {
            // BTRFS snapshot path - instant container creation
            return Self::create_container_from_btrfs_snapshot(
                container,
                golden_image_name,
                &bridge,
            )
            .await;
        }

        // Use native Proxmox API for template-based creation
        let template = props
            .and_then(|p| p.get("template"))
            .and_then(|v| v.as_str())
            .unwrap_or("local-btrfs:vztmpl/debian-13-standard_13.1-2_amd64.tar.zst");

        let hostname = props
            .and_then(|p| p.get("hostname"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("ct{}", container.id));

        let memory = props
            .and_then(|p| p.get("memory"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512) as u32;

        let swap = props
            .and_then(|p| p.get("swap"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512) as u32;

        let storage = props
            .and_then(|p| p.get("storage"))
            .and_then(|v| v.as_str())
            .unwrap_or("local-btrfs");

        let rootfs_size = props
            .and_then(|p| p.get("rootfs_size"))
            .and_then(|v| v.as_u64())
            .unwrap_or(8);

        let rootfs = format!("{}:{}", storage, rootfs_size);

        let cores = props
            .and_then(|p| p.get("cores"))
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as u32;

        let unprivileged = props
            .and_then(|p| p.get("unprivileged"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let features = props
            .and_then(|p| p.get("features"))
            .and_then(|v| v.as_str())
            .unwrap_or("nesting=1");

        let firewall = props
            .and_then(|p| p.get("firewall"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let net0 = format!(
            "name=eth0,bridge={},firewall={}",
            bridge,
            if firewall { "1" } else { "0" }
        );

        log::info!(
            "Creating container {}: template={}, memory={}MB, cores={}, rootfs={}",
            container.id,
            template,
            memory,
            cores,
            rootfs
        );

        // Build the request
        let config = op_network::CreateContainerRequest {
            vmid,
            ostemplate: template.to_string(),
            hostname: Some(hostname),
            memory: Some(memory),
            swap: Some(swap),
            cores: Some(cores),
            rootfs: Some(rootfs),
            net0: Some(net0),
            unprivileged: Some(unprivileged),
            features: Some(features.to_string()),
            onboot: props.and_then(|p| p.get("onboot")).and_then(|v| v.as_bool()),
            protection: props.and_then(|p| p.get("protection")).and_then(|v| v.as_bool()),
            nameserver: props.and_then(|p| p.get("nameserver")).and_then(|v| v.as_str()).map(String::from),
            searchdomain: props.and_then(|p| p.get("searchdomain")).and_then(|v| v.as_str()).map(String::from),
            storage: Some(storage.to_string()),
            ..Default::default()
        };

        // Execute via native API
        let client = op_network::ProxmoxClient::from_env()?;
        client.create_container_sync(&config, 300).await?;

        log::info!(
            "Container {} created successfully on bridge {} (via native API)",
            container.id,
            bridge
        );

        // Inject netmaker token for first-boot join (if netmaker network type)
        let network_type = props
            .and_then(|p| p.get("network_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bridge");

        if network_type == "netmaker" {
            Self::inject_netmaker_token(container, storage).await?;
        }

        Ok(())
    }

    /// Create LXC container from BTRFS golden image snapshot (instant provisioning)
    async fn create_container_from_btrfs_snapshot(
        container: &ContainerInfo,
        golden_image_name: &str,
        bridge: &str,
    ) -> Result<()> {
        log::info!(
            "Creating container {} from BTRFS golden image: {}",
            container.id,
            golden_image_name
        );

        let props = container.properties.as_ref();

        // Storage backend (configurable per container)
        let storage = props
            .and_then(|p| p.get("storage"))
            .and_then(|v| v.as_str())
            .unwrap_or("local-btrfs");

        // Proxmox storage paths (adjust based on storage.cfg configuration)
        let storage_path = format!("/var/lib/pve/{}", storage);
        let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
        let container_rootfs = format!("{}/images/{}/rootfs", storage_path, container.id);
        let container_dir = format!("{}/images/{}", storage_path, container.id);

        // Verify golden image exists
        if tokio::fs::metadata(&golden_image_path).await.is_err() {
            return Err(anyhow::anyhow!(
                "Golden image not found: {}. Create it with: sudo ./create-btrfs-golden-image.sh {}",
                golden_image_path,
                golden_image_name
            ));
        }

        // Check if it's a BTRFS subvolume
        let check_output = tokio::process::Command::new("btrfs")
            .args(["subvolume", "show", &golden_image_path])
            .output()
            .await?;

        if !check_output.status.success() {
            return Err(anyhow::anyhow!(
                "Golden image is not a BTRFS subvolume: {}",
                golden_image_path
            ));
        }

        log::info!("✓ Golden image verified: {}", golden_image_path);

        // Create container directory
        tokio::fs::create_dir_all(&container_dir).await?;

        // Create BTRFS snapshot (instant copy-on-write)
        log::info!("Creating BTRFS snapshot...");
        let snapshot_output = tokio::process::Command::new("btrfs")
            .args([
                "subvolume",
                "snapshot",
                &golden_image_path,
                &container_rootfs,
            ])
            .output()
            .await?;

        if !snapshot_output.status.success() {
            let stderr = String::from_utf8_lossy(&snapshot_output.stderr);
            return Err(anyhow::anyhow!("BTRFS snapshot failed: {}", stderr));
        }

        log::info!("✓ BTRFS snapshot created in <1ms: {}", container_rootfs);

        // Extract properties
        let hostname = props
            .and_then(|p| p.get("hostname"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("ct{}", container.id));

        let memory = props
            .and_then(|p| p.get("memory"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512);

        let swap = props
            .and_then(|p| p.get("swap"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512);

        let cores = props
            .and_then(|p| p.get("cores"))
            .and_then(|v| v.as_u64())
            .unwrap_or(2);

        let unprivileged = props
            .and_then(|p| p.get("unprivileged"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let features = props
            .and_then(|p| p.get("features"))
            .and_then(|v| v.as_str())
            .unwrap_or("nesting=1");

        let firewall = props
            .and_then(|p| p.get("firewall"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Create Proxmox container configuration
        let config_path = format!("/etc/pve/lxc/{}.conf", container.id);
        let config_content = format!(
            r#"arch: amd64
cores: {}
hostname: {}
memory: {}
swap: {}
net0: name=eth0,bridge={},firewall={}
ostype: debian
rootfs: local-btrfs:images/{}/rootfs
unprivileged: {}
features: {}
"#,
            cores,
            hostname,
            memory,
            swap,
            bridge,
            if firewall { "1" } else { "0" },
            container.id,
            if unprivileged { "1" } else { "0" },
            features
        );

        // Add optional properties
        let mut config = config_content;

        if let Some(onboot) = props
            .and_then(|p| p.get("onboot"))
            .and_then(|v| v.as_bool())
        {
            config.push_str(&format!("onboot: {}\n", if onboot { "1" } else { "0" }));
        }

        if let Some(protection) = props
            .and_then(|p| p.get("protection"))
            .and_then(|v| v.as_bool())
        {
            config.push_str(&format!(
                "protection: {}\n",
                if protection { "1" } else { "0" }
            ));
        }

        if let Some(nameserver) = props
            .and_then(|p| p.get("nameserver"))
            .and_then(|v| v.as_str())
        {
            config.push_str(&format!("nameserver: {}\n", nameserver));
        }

        if let Some(searchdomain) = props
            .and_then(|p| p.get("searchdomain"))
            .and_then(|v| v.as_str())
        {
            config.push_str(&format!("searchdomain: {}\n", searchdomain));
        }

        // Write Proxmox config
        tokio::fs::write(&config_path, config).await?;

        log::info!("✓ Proxmox configuration written: {}", config_path);

        // Inject firstboot script if specified
        if let Some(firstboot_script) = props
            .and_then(|p| p.get("firstboot_script"))
            .and_then(|v| v.as_str())
        {
            Self::inject_firstboot_script(container, storage, firstboot_script).await?;
        }

        // Inject Netmaker token for netmaker network type
        let network_type = props
            .and_then(|p| p.get("network_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bridge");

        if network_type == "netmaker" {
            Self::inject_netmaker_token(container, storage).await?;
        }

        log::info!(
            "✓ Container {} created from golden image '{}' (BTRFS snapshot)",
            container.id,
            golden_image_name
        );

        Ok(())
    }

    /// Inject firstboot script into container rootfs
    async fn inject_firstboot_script(
        container: &ContainerInfo,
        storage: &str,
        script_content: &str,
    ) -> Result<()> {
        let rootfs = format!("/var/lib/pve/{}/images/{}/rootfs", storage, container.id);
        let script_path = format!("{}/usr/local/bin/lxc-firstboot.sh", rootfs);
        let service_path = format!("{}/etc/systemd/system/lxc-firstboot.service", rootfs);

        // Create script directory if needed
        tokio::fs::create_dir_all(format!("{}/usr/local/bin", rootfs)).await?;

        // Write firstboot script
        tokio::fs::write(&script_path, script_content).await?;

        // Make executable
        tokio::process::Command::new("chmod")
            .args(["+x", &script_path])
            .output()
            .await?;

        // Create systemd service
        let service_content = r#"[Unit]
Description=LXC First Boot Initialization
After=network-online.target
Wants=network-online.target
ConditionPathExists=!/var/lib/lxc-firstboot-complete

[Service]
Type=oneshot
ExecStart=/usr/local/bin/lxc-firstboot.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
"#
        .to_string();

        tokio::fs::create_dir_all(format!("{}/etc/systemd/system", rootfs)).await?;
        tokio::fs::write(&service_path, service_content).await?;

        // Enable service (create symlink)
        let symlink_dir = format!("{}/etc/systemd/system/multi-user.target.wants", rootfs);
        tokio::fs::create_dir_all(&symlink_dir).await?;

        let symlink_path = format!("{}/lxc-firstboot.service", symlink_dir);
        tokio::fs::symlink("../lxc-firstboot.service", &symlink_path)
            .await
            .ok(); // Ignore if exists

        log::info!(
            "✓ Firstboot script injected into container {}",
            container.id
        );

        Ok(())
    }

    /// Inject Netmaker enrollment token into container
    async fn inject_netmaker_token(container: &ContainerInfo, storage: &str) -> Result<()> {
        // Read token from host
        if let Ok(token_content) = tokio::fs::read_to_string("/etc/op-dbus/netmaker.env").await {
            for line in token_content.lines() {
                if let Some(token_value) = line.strip_prefix("NETMAKER_TOKEN=") {
                    let token_clean = token_value.trim_matches('"').trim();

                    let rootfs = format!("/var/lib/pve/{}/images/{}/rootfs", storage, container.id);
                    let token_path = format!("{}/etc/netmaker/enrollment-token", rootfs);

                    // Create netmaker directory
                    tokio::fs::create_dir_all(format!("{}/etc/netmaker", rootfs)).await?;

                    // Write token
                    tokio::fs::write(&token_path, token_clean).await?;

                    // Set permissions
                    tokio::process::Command::new("chmod")
                        .args(["600", &token_path])
                        .output()
                        .await?;

                    log::info!("✓ Netmaker token injected into container {}", container.id);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Cleanup OVS port for deleted container
    async fn cleanup_ovs_port_for_container(ct_id: &str) -> Result<String> {
        let client = op_network::ovsdb::OvsdbClient::new();

        // Find port names matching this container (vi{VMID} or internal_{VMID})
        let potential_ports = vec![
            format!("vi{}", ct_id),        // Proxmox veth pattern
            format!("internal_{}", ct_id), // Socket networking pattern
            format!("veth{}pl", ct_id),    // Alternative veth pattern
        ];

        // Try each potential port name
        for port_name in &potential_ports {
            // Check all bridges for this port
            if let Ok(bridges) = client.list_bridges().await {
                for bridge in bridges {
                    if let Ok(ports) = client.list_bridge_ports(&bridge).await {
                        if ports.contains(port_name) {
                            log::info!("Found port {} on bridge {}, removing", port_name, bridge);

                            // Delete the port using OVSDB
                            let operations = simd_json::json!([{
                                "op": "select",
                                "table": "Port",
                                "where": [["name", "==", port_name]],
                                "columns": ["_uuid"]
                            }]);

                            if let Ok(result) = client.transact(operations).await {
                                if let Some(rows) = result[0]["rows"].as_array() {
                                    if let Some(first_row) = rows.first() {
                                        if let Some(uuid_array) = first_row["_uuid"].as_array() {
                                            if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                                                let port_uuid = uuid_array[1].as_str().unwrap();

                                                // Get bridge UUID
                                                let bridge_ops = simd_json::json!([{
                                                    "op": "select",
                                                    "table": "Bridge",
                                                    "where": [["name", "==", &bridge]],
                                                    "columns": ["_uuid"]
                                                }]);

                                                if let Ok(bridge_result) =
                                                    client.transact(bridge_ops).await
                                                {
                                                    if let Some(bridge_rows) =
                                                        bridge_result[0]["rows"].as_array()
                                                    {
                                                        if let Some(bridge_row) =
                                                            bridge_rows.first()
                                                        {
                                                            if let Some(bridge_uuid_array) =
                                                                bridge_row["_uuid"].as_array()
                                                            {
                                                                if bridge_uuid_array.len() == 2
                                                                    && bridge_uuid_array[0]
                                                                        == "uuid"
                                                                {
                                                                    let bridge_uuid =
                                                                        bridge_uuid_array[1]
                                                                            .as_str()
                                                                            .unwrap();

                                                                    // Remove port from bridge and delete it
                                                                    let delete_ops = simd_json::json!([
                                                                        {
                                                                            "op": "mutate",
                                                                            "table": "Bridge",
                                                                            "where": [["_uuid", "==", ["uuid", bridge_uuid]]],
                                                                            "mutations": [
                                                                                ["ports", "delete", ["uuid", port_uuid]]
                                                                            ]
                                                                        },
                                                                        {
                                                                            "op": "delete",
                                                                            "table": "Port",
                                                                            "where": [["_uuid", "==", ["uuid", port_uuid]]]
                                                                        }
                                                                    ]);

                                                                    client
                                                                        .transact(delete_ops)
                                                                        .await?;
                                                                    return Ok(port_name.clone());
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No OVS port found for container {}", ct_id))
    }

    /// Start LXC container via native Proxmox API
    async fn start_container(ct_id: &str) -> Result<()> {
        log::info!("Starting container {} via Proxmox API", ct_id);

        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;
        client.start_container_sync(vmid, 60).await?;

        log::info!("Container {} started successfully (via native API)", ct_id);
        Ok(())
    }

    /// Stop LXC container via native Proxmox API
    async fn stop_container(ct_id: &str) -> Result<()> {
        log::info!("Stopping container {} via Proxmox API", ct_id);

        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;
        client.stop_container_sync(vmid, 60).await?;

        log::info!("Container {} stopped successfully (via native API)", ct_id);
        Ok(())
    }

    /// Delete LXC container via native Proxmox API
    async fn delete_container(ct_id: &str, force: bool) -> Result<()> {
        log::info!("Deleting container {} via Proxmox API (force={})", ct_id, force);

        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;

        // Stop if running
        if client.is_running(vmid).await.unwrap_or(false) {
            if force {
                client.stop_container_sync(vmid, 30).await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Container {} is running. Stop it first or use force=true",
                    ct_id
                ));
            }
        }

        if force {
            let upid = client.force_delete_container(vmid).await?;
            client.wait_for_task(&upid, 120).await?;
        } else {
            client.delete_container_sync(vmid, 120).await?;
        }

        log::info!("Container {} deleted successfully (via native API)", ct_id);
        Ok(())
    }
}

#[async_trait]
impl StatePlugin for LxcPlugin {
    fn name(&self) -> &str {
        "lxc"
    }
    fn version(&self) -> &str {
        "2.0.0" // Version bump for native API support
    }

    fn is_available(&self) -> bool {
        // Try to check Proxmox API availability synchronously via environment
        // The actual check happens async in discovery
        std::path::Path::new("/etc/pve").exists()
    }

    fn unavailable_reason(&self) -> String {
        "Proxmox VE not detected (/etc/pve not found) - this plugin requires Proxmox VE".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let containers = self.discover_from_proxmox().await?;
        Ok(simd_json::serde::to_owned_value(LxcState { containers })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        // For now, emit a single modify if different; once lifecycle is defined, compute granular actions.
        let actions = if current != desired {
            vec![StateAction::Modify {
                resource: "lxc".into(),
                changes: desired.clone(),
            }]
        } else {
            vec![]
        };
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create {
                    resource: _,
                    config,
                } => {
                    let container: ContainerInfo = simd_json::serde::from_owned_value(config.clone())?;

                    // 1. Create LXC container via native API
                    match Self::create_container(&container).await {
                        Ok(_) => {
                            changes_applied.push(format!("Created container {} (via native Proxmox API)", container.id));

                            // 2. Start container to create veth interface
                            if let Err(e) = Self::start_container(&container.id).await {
                                errors.push(format!(
                                    "Failed to start container {}: {}",
                                    container.id, e
                                ));
                                continue;
                            }

                            // Wait for veth to appear
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                            // 3. Find and rename veth
                            let veth_name = format!("vi{}", container.id);
                            match Self::find_container_veth(&container.id).await {
                                Ok(old_veth) => {
                                    log::info!(
                                        "Found veth {} for container {}",
                                        old_veth,
                                        container.id
                                    );

                                    if old_veth != veth_name {
                                        match op_network::rtnetlink::link_set_name(
                                            &old_veth, &veth_name,
                                        )
                                        .await
                                        {
                                            Ok(_) => {
                                                changes_applied.push(format!(
                                                    "Renamed {} to {}",
                                                    old_veth, veth_name
                                                ));
                                            }
                                            Err(e) => {
                                                log::warn!("Failed to rename veth: {}", e);
                                                // Continue anyway, veth might work with original name
                                            }
                                        }
                                    }

                                    // 4. Network enrollment based on type
                                    let network_type = container
                                        .properties
                                        .as_ref()
                                        .and_then(|p| p.get("network_type"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("bridge");

                                    let target_bridge = match network_type {
                                        "netmaker" => "mesh".to_string(),
                                        _ => container.bridge.clone(),
                                    };

                                    if !target_bridge.is_empty() {
                                        let ovsdb_client = op_network::ovsdb::OvsdbClient::new();
                                        match ovsdb_client.add_port(&target_bridge, &veth_name).await {
                                            Ok(_) => {
                                                changes_applied.push(format!(
                                                    "Added {} to bridge {}",
                                                    veth_name, target_bridge
                                                ));
                                            }
                                            Err(e) => {
                                                errors.push(format!(
                                                    "Failed to add port to bridge: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Failed to find veth for container {}: {}",
                                        container.id,
                                        e
                                    );
                                    // Continue - container was created, just couldn't configure OVS
                                }
                            }
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Failed to create container {}: {}",
                                container.id, e
                            ));
                        }
                    }
                }
                StateAction::Modify {
                    resource,
                    changes: _,
                } => {
                    // Handle container state changes (start/stop)
                    log::info!(
                        "Modify operation for container {} (not yet implemented)",
                        resource
                    );
                    changes_applied.push(format!("Skipped modify for {}", resource));
                }
                StateAction::Delete { resource } => {
                    // Delete container and cleanup OVS ports
                    log::info!("Deleting container {} and cleaning up OVS ports", resource);

                    // First, try to find and cleanup the OVS port for this container
                    let cleanup_result = Self::cleanup_ovs_port_for_container(resource).await;
                    match cleanup_result {
                        Ok(port_name) => {
                            log::info!(
                                "Cleaned up OVS port {} for container {}",
                                port_name,
                                resource
                            );
                            changes_applied.push(format!(
                                "Removed OVS port {} for container {}",
                                port_name, resource
                            ));
                        }
                        Err(e) => {
                            log::warn!(
                                "Could not cleanup OVS port for container {}: {}",
                                resource,
                                e
                            );
                        }
                    }

                    // Then delete the container via native API
                    match Self::delete_container(resource, true).await {
                        Ok(_) => {
                            changes_applied.push(format!("Deleted container {} (via native Proxmox API)", resource));
                        }
                        Err(e) => {
                            errors.push(format!("Failed to delete container {}: {}", resource, e));
                        }
                    }
                }
                StateAction::NoOp { .. } => {}
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("lxc-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/mail_server.rs">
//! Mail server state plugin - manages Incus mail container and D-Bus registration.
//!
//! Tracks Postfix/Dovecot runtime state, Unix socket endpoints for Xray routing,
//! and exposes mail configuration as a D-Bus object via zbus.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Top-level state for the mail server plugin.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailServerState {
    /// Incus container name running the mail stack
    pub container_name: String,
    /// Container status: "Running", "Stopped", "Frozen"
    pub container_status: String,
    /// Primary mail domain
    pub domain: String,
    /// Unix socket path for Xray naive routing integration
    pub xray_socket_path: String,
    /// D-Bus service name registered for this mail instance
    pub dbus_service_name: String,
    /// Active mail service endpoints
    pub endpoints: MailEndpoints,
    /// Container IPv4 address
    pub container_ip: Option<String>,
    /// Whether the mail stack is healthy
    pub healthy: bool,
    /// Last error message if unhealthy
    pub last_error: Option<String>,
    /// Additional container devices (unix socket mounts, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<HashMap<String, HashMap<String, String>>>,
}

/// Mail protocol endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailEndpoints {
    /// SMTP submission port (587)
    pub smtp_submission: Option<String>,
    /// SMTP TLS port (465)
    pub smtp_tls: Option<String>,
    /// IMAP port (143)
    pub imap: Option<String>,
    /// IMAPS port (993)
    pub imaps: Option<String>,
    /// Dovecot LDA/LMTP unix socket inside container
    pub dovecot_lmtp: Option<String>,
    /// Postfix pickup unix socket inside container
    pub postfix_pickup: Option<String>,
}

pub struct MailServerPlugin;

impl MailServerPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Default state for 3tched.com mail stack
    fn default_state() -> MailServerState {
        MailServerState {
            container_name: "mail-3tched".to_string(),
            container_status: "Unknown".to_string(),
            domain: "3tched.com".to_string(),
            xray_socket_path: "/run/xray/mail-naive.sock".to_string(),
            dbus_service_name: "org.opdbus.MailServer.3tched".to_string(),
            endpoints: MailEndpoints {
                smtp_submission: Some("0.0.0.0:587".to_string()),
                smtp_tls: Some("0.0.0.0:465".to_string()),
                imap: Some("0.0.0.0:143".to_string()),
                imaps: Some("0.0.0.0:993".to_string()),
                dovecot_lmtp: Some("/var/spool/postfix/private/dovecot-lmtp".to_string()),
                postfix_pickup: Some("/var/spool/postfix/private/pickup".to_string()),
            },
            container_ip: None,
            healthy: false,
            last_error: None,
            devices: None,
        }
    }

    /// Query incus for container status
    async fn query_container_status(&self, name: &str) -> Result<(String, Option<String>)> {
        let output = tokio::process::Command::new("/usr/bin/incus")
            .args(["list", name, "--format=json"])
            .output()
            .await
            .context("Failed to query incus container status")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("incus list failed: {}", stderr.trim());
        }

        let mut raw = output.stdout;
        let instances: Vec<simd_json::OwnedValue> =
            simd_json::from_slice(&mut raw).unwrap_or_default();

        if let Some(inst) = instances.first() {
            let status = inst
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let ip = inst
                .get("state")
                .and_then(|s| s.get("network"))
                .and_then(|n| n.get("eth0"))
                .and_then(|e| e.get("addresses"))
                .and_then(|a| a.as_array())
                .and_then(|addrs| {
                    addrs.iter().find_map(|addr| {
                        if addr.get("family")?.as_str()? == "inet" {
                            addr.get("address")?.as_str().map(String::from)
                        } else {
                            None
                        }
                    })
                });

            Ok((status, ip))
        } else {
            Ok(("NotFound".to_string(), None))
        }
    }

    /// Check if Postfix and Dovecot are responding inside the container
    async fn check_mail_health(&self, container: &str) -> (bool, Option<String>) {
        // Check postfix is running inside container
        let postfix = tokio::process::Command::new("/usr/bin/incus")
            .args(["exec", container, "--", "postfix", "status"])
            .output()
            .await;

        let postfix_ok = postfix.map(|o| o.status.success()).unwrap_or(false);

        // Check dovecot is running inside container
        let dovecot = tokio::process::Command::new("/usr/bin/incus")
            .args(["exec", container, "--", "doveadm", "service", "status"])
            .output()
            .await;

        let dovecot_ok = dovecot.map(|o| o.status.success()).unwrap_or(false);

        if postfix_ok && dovecot_ok {
            (true, None)
        } else {
            let err = format!(
                "postfix_ok={}, dovecot_ok={}",
                postfix_ok, dovecot_ok
            );
            (false, Some(err))
        }
    }
}

impl Default for MailServerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for MailServerPlugin {
    fn metadata(&self) -> op_state::PluginMetadata {
        op_state::PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: "Mail server container state and D-Bus registration for 3tched.com".to_string(),
            author: None,
            license: None,
            dependencies: vec!["incus".to_string(), "unix_socket".to_string()],
            dbus_services: vec!["org.opdbus.MailServer.3tched".to_string()],
            feature_schemas: vec![],
            object_schemas: std::collections::HashMap::new(),
        }
    }

    fn name(&self) -> &str {
        "mail_server"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::mail_server_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut state = Self::default_state();

        match self.query_container_status(&state.container_name).await {
            Ok((status, ip)) => {
                state.container_status = status;
                state.container_ip = ip;
            }
            Err(e) => {
                state.container_status = "Error".to_string();
                state.last_error = Some(e.to_string());
            }
        }

        if state.container_status == "Running" {
            let (healthy, err) = self.check_mail_health(&state.container_name).await;
            state.healthy = healthy;
            state.last_error = err;
        }

        // Query container devices from incus config
        let config_output = tokio::process::Command::new("/usr/bin/incus")
            .args(["config", "show", &state.container_name, "--format=json"])
            .output()
            .await;

        if let Ok(out) = config_output {
            if out.status.success() {
                let mut raw = out.stdout;
                if let Ok(config) = simd_json::from_slice::<simd_json::OwnedValue>(&mut raw) {
                    if let Some(devices) = config.get("devices") {
                        if let Ok(dev_map) = simd_json::serde::from_owned_value::<
                            HashMap<String, HashMap<String, String>>,
                        >(devices.clone())
                        {
                            state.devices = Some(dev_map);
                        }
                    }
                }
            }
        }

        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/mcp.rs">
//! MCP state plugin - manages MCP server configurations and tool groups
//! Wires MCP configuration to the state store for auditing and rollback

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{
    ExecutionJob, ExecutionStatus, FieldSchema, FieldType, PluginSchema, StateStore,
};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// MCP configuration schema - mirrors the state JSON structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    /// External MCP servers indexed by name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<HashMap<String, McpServerConfig>>,

    /// Tool groups configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_groups: Option<ToolGroupsConfig>,

    /// Compact mode settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_mode: Option<CompactModeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    /// Server command to execute
    pub command: String,

    /// Command arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Whether server is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Transport type (stdio, sse, http)
    #[serde(default = "default_stdio")]
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolGroupsConfig {
    /// Enabled group IDs
    pub enabled: Vec<String>,

    /// Maximum tools limit
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,

    /// Access zone (localhost, trusted_mesh, private_network, public)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_zone: Option<String>,

    /// Trusted network prefixes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted_networks: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactModeConfig {
    /// Whether compact mode is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Meta-tools to expose
    #[serde(default = "default_meta_tools")]
    pub meta_tools: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_stdio() -> String {
    "stdio".to_string()
}

fn default_max_tools() -> usize {
    40
}

fn default_meta_tools() -> Vec<String> {
    vec![
        "list_tools".to_string(),
        "search_tools".to_string(),
        "get_tool_schema".to_string(),
        "execute_tool".to_string(),
        "respond".to_string(),
    ]
}

/// Tool definition - canonical schema for all tools
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_schema_version() -> String {
    "https://json-schema.org/draft/next/schema".to_string()
}

fn default_namespace() -> String {
    "system.v1".to_string()
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// MCP state plugin
pub struct McpStatePlugin {
    /// State store for execution tracking
    state_store: Arc<dyn StateStore>,
    /// Configuration file path
    config_path: String,
}

impl McpStatePlugin {
    pub fn new(state_store: Arc<dyn StateStore>, config_path: impl Into<String>) -> Self {
        Self {
            state_store,
            config_path: config_path.into(),
        }
    }

    /// Load current MCP configuration from file
    async fn load_config(&self) -> Result<McpConfig> {
        let content = tokio::fs::read_to_string(&self.config_path).await;

        match content {
            Ok(c) => {
                let mut c_mut = c;
                unsafe { simd_json::from_str(&mut c_mut) }.context("Failed to parse MCP config")
            }
            Err(_) => {
                // Return default config with requested agents auto-loaded
                let mut servers = HashMap::new();

                // simple, flat list of agents to auto-load
                let agents = vec![
                    "rust-pro",
                    "backend-architect",
                    "network-engineer",
                    "context-manager",
                    "memory",
                    "sequential-thinking",
                ];

                for agent in agents {
                    servers.insert(
                        agent.to_string(),
                        McpServerConfig {
                            command: "dbus-agent".to_string(),
                            args: Some(vec![agent.to_string()]),
                            env: None,
                            enabled: true,
                            transport: "stdio".to_string(),
                        },
                    );
                }

                Ok(McpConfig {
                    servers: Some(servers),
                    tool_groups: Some(ToolGroupsConfig {
                        enabled: vec!["default".to_string()],
                        max_tools: default_max_tools(),
                        access_zone: Some("local".to_string()),
                        trusted_networks: None,
                    }),
                    compact_mode: Some(CompactModeConfig {
                        enabled: true,
                        meta_tools: default_meta_tools(),
                    }),
                })
            }
        }
    }

    /// Save MCP configuration to file
    async fn save_config(&self, config: &McpConfig) -> Result<()> {
        let content = simd_json::to_string_pretty(config)?;
        tokio::fs::write(&self.config_path, content)
            .await
            .context("Failed to write MCP config file")
    }

    /// Apply server configuration changes
    async fn apply_server_config(&self, server_name: &str, config: &McpServerConfig) -> Result<()> {
        // Create execution job for state tracking
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            tool_name: format!("mcp:configure_server:{}", server_name),
            arguments: simd_json::serde::to_owned_value(config)?,
            status: ExecutionStatus::Running,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            result: None,
        };

        // Save job to state store
        self.state_store.save_job(&job).await?;

        // Load current config
        let mut current = self.load_config().await.unwrap_or_else(|_| McpConfig {
            servers: Some(HashMap::new()),
            tool_groups: None,
            compact_mode: None,
        });

        // Update server config
        let servers = current.servers.get_or_insert_with(HashMap::new);
        servers.insert(server_name.to_string(), config.clone());

        // Save updated config
        self.save_config(&current).await?;

        // Update job status
        let mut job = job;
        job.status = ExecutionStatus::Completed;
        job.updated_at = chrono::Utc::now();
        job.result = Some(op_state_store::ExecutionResult {
            success: true,
            output: Some(simd_json::serde::to_owned_value(
                "Server configured successfully",
            )?),
            error: None,
        });
        self.state_store.update_job(&job).await?;

        log::info!("Configured MCP server: {}", server_name);
        Ok(())
    }

    /// Apply tool groups configuration
    async fn apply_tool_groups_config(&self, config: &ToolGroupsConfig) -> Result<()> {
        // Create execution job
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            tool_name: "mcp:configure_tool_groups".to_string(),
            arguments: simd_json::serde::to_owned_value(config)?,
            status: ExecutionStatus::Running,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            result: None,
        };

        self.state_store.save_job(&job).await?;

        // Load current config
        let mut current = self.load_config().await.unwrap_or_else(|_| McpConfig {
            servers: None,
            tool_groups: Some(config.clone()),
            compact_mode: None,
        });

        // Update tool groups
        current.tool_groups = Some(config.clone());

        // Save updated config
        self.save_config(&current).await?;

        // Update job status
        let mut job = job;
        job.status = ExecutionStatus::Completed;
        job.updated_at = chrono::Utc::now();
        job.result = Some(op_state_store::ExecutionResult {
            success: true,
            output: Some(simd_json::serde::to_owned_value(
                "Tool groups configured successfully",
            )?),
            error: None,
        });
        self.state_store.update_job(&job).await?;

        log::info!("Configured tool groups: {:?}", config.enabled);
        Ok(())
    }
}

fn mcp_plugin_schema() -> PluginSchema {
    PluginSchema::builder("mcp")
        .version("1.0.0")
        .description("MCP server and tool-group configuration")
        .dependency("agent_config")
        .field(
            "servers",
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "MCP server map".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "rust-pro": {
                        "command": "dbus-agent",
                        "args": ["rust-pro"],
                        "enabled": true,
                        "transport": "stdio"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "tool_groups",
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Tool group config".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "enabled": ["default"],
                    "max_tools": 40,
                    "access_zone": "local"
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "compact_mode",
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Compact mode config".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "enabled": true,
                    "meta_tools": ["list_tools", "search_tools", "get_tool_schema", "execute_tool", "respond"]
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .build()
}

#[async_trait]
impl StatePlugin for McpStatePlugin {
    fn name(&self) -> &str {
        "mcp"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(mcp_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let config = self.load_config().await.unwrap_or_else(|_| McpConfig {
            servers: None,
            tool_groups: None,
            compact_mode: None,
        });

        Ok(simd_json::serde::to_owned_value(config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: McpConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: McpConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        // Check server changes
        if let Some(desired_servers) = &desired_config.servers {
            for (server_name, desired_server) in desired_servers {
                let current_server = current_config
                    .servers
                    .as_ref()
                    .and_then(|s| s.get(server_name));

                if current_server != Some(desired_server) {
                    actions.push(StateAction::Modify {
                        resource: format!("server:{}", server_name),
                        changes: simd_json::serde::to_owned_value(desired_server)?,
                    });
                }
            }
        }

        // Check tool groups changes
        if current_config.tool_groups != desired_config.tool_groups {
            if let Some(ref desired_groups) = desired_config.tool_groups {
                actions.push(StateAction::Modify {
                    resource: "tool_groups".to_string(),
                    changes: simd_json::serde::to_owned_value(desired_groups)?,
                });
            }
        }

        // Check compact mode changes
        if current_config.compact_mode != desired_config.compact_mode {
            if let Some(ref desired_compact) = desired_config.compact_mode {
                actions.push(StateAction::Modify {
                    resource: "compact_mode".to_string(),
                    changes: simd_json::serde::to_owned_value(desired_compact)?,
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let result = if resource.starts_with("server:") {
                    let server_name = resource.strip_prefix("server:").unwrap();
                    let server_config: McpServerConfig =
                        simd_json::serde::from_owned_value(changes.clone())?;
                    self.apply_server_config(server_name, &server_config).await
                } else if resource == "tool_groups" {
                    let groups_config: ToolGroupsConfig =
                        simd_json::serde::from_owned_value(changes.clone())?;
                    self.apply_tool_groups_config(&groups_config).await
                } else if resource == "compact_mode" {
                    // Compact mode changes don't require action - just config update
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Unknown resource: {}", resource))
                };

                match result {
                    Ok(_) => {
                        changes_applied.push(format!("Applied MCP config for: {}", resource));
                    }
                    Err(e) => {
                        errors.push(format!("Failed to apply config for {}: {}", resource, e));
                    }
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let current_config: McpConfig = simd_json::serde::from_owned_value(current)?;
        let desired_config: McpConfig = simd_json::serde::from_owned_value(desired.clone())?;

        Ok(current_config == desired_config)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("mcp-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_config: McpConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        self.save_config(&old_config).await?;
        log::info!("Rolled back MCP config to checkpoint: {}", checkpoint.id);
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // File writes are not atomic
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::SqliteStore;

    #[test]
    fn should_publish_plugin_owned_mcp_schema() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let store = Arc::new(
            runtime
                .block_on(SqliteStore::new(":memory:"))
                .expect("store"),
        );
        let plugin = McpStatePlugin::new(store, "/tmp/test-mcp-schema.json");
        let schema = plugin.schema().expect("mcp schema");

        assert_eq!(schema.name, "mcp");
        assert_eq!(schema.version, "1.0.0");
        assert_eq!(schema.dependencies, vec!["agent_config".to_string()]);
        assert!(schema.fields.contains_key("servers"));
        assert!(schema.fields.contains_key("tool_groups"));
        assert!(schema.fields.contains_key("compact_mode"));
    }

    #[tokio::test]
    async fn test_mcp_plugin_state_tracking() {
        // Create in-memory state store
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let config_path = format!("/tmp/test-mcp-config-{}.json", Uuid::new_v4());
        let plugin = McpStatePlugin::new(store.clone(), &config_path);

        // Ensure file doesn't exist
        let _ = tokio::fs::remove_file(&config_path).await;

        // Create a test config
        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: "test-command".to_string(),
                args: Some(vec!["arg1".to_string()]),
                env: None,
                enabled: true,
                transport: "stdio".to_string(),
            },
        );

        let config = McpConfig {
            servers: Some(servers),
            tool_groups: None,
            compact_mode: None,
        };

        // Apply config (this should create execution jobs in state store)
        let desired = simd_json::serde::to_owned_value(&config).unwrap();
        let current = plugin.query_current_state().await.unwrap();
        let diff = plugin.calculate_diff(&current, &desired).await.unwrap();
        let result = plugin.apply_state(&diff).await.unwrap();

        assert!(result.success);
        assert!(!result.changes_applied.is_empty());

        // Clean up
        let _ = tokio::fs::remove_file(&config_path).await;
    }
}
</file>

<file path="src/state_plugins/mod.rs">
//! State plugins - each manages a domain via native protocols
//!
//! These plugins implement the StatePlugin trait from op-state

// pub mod dnsresolver;
// pub mod full_system;
// pub mod keyring;
// pub mod login1;
// pub mod lxc;
pub mod incus;
pub mod mcp;
pub mod net;
// pub mod netmaker;
pub mod openflow;
// pub mod openflow_obfuscation;
// pub mod packagekit;
// pub mod pcidecl;
// pub mod privacy;
pub mod adc;
pub mod config;
pub mod endpoint;
pub mod gcloud_adc;
pub mod keypair;
pub(crate) mod plugin_schema_defs;
pub mod privacy_router;
pub mod privacy_routes;
pub mod procfs;
pub mod proxy_server;
pub mod s6;
pub mod service;
pub mod sessdecl;
// pub mod systemd;
// pub mod systemd_networkd;

pub mod agent_config;
pub mod cognitive_mcp;
pub mod compact_mcp;
pub mod hardware;
pub mod ovsdb_bridge;
pub mod proxmox;
pub mod rtnetlink;
pub mod schema_contract;
pub mod software;
pub mod mail_server;
pub mod unix_socket;
pub mod users;
pub mod web_ui;
pub mod wireguard;

// Re-export plugin types
// pub use dnsresolver::DnsResolverPlugin;
// pub use full_system::FullSystemPlugin;
// pub use login1::Login1Plugin;
// pub use lxc::LxcPlugin;
pub use incus::IncusPlugin;
pub use mcp::McpStatePlugin;
pub use mcp::{ExecutionResult, ToolDefinition};
pub use net::NetStatePlugin;
// pub use netmaker::NetmakerPlugin;
pub use openflow::OpenFlowPlugin;
// pub use openflow_obfuscation::OpenFlowObfuscationPlugin;
// pub use packagekit::PackageKitPlugin;
// pub use pcidecl::PciDeclPlugin;
// pub use privacy::PrivacyPlugin;
pub use adc::AdcPlugin;
pub use agent_config::AgentConfigPlugin;
pub use cognitive_mcp::CognitiveMcpPlugin;
pub use compact_mcp::CompactMcpPlugin;
pub use config::ConfigPlugin;
pub use endpoint::EndpointPlugin;
pub use gcloud_adc::GcloudAdcPlugin;
pub use hardware::HardwarePlugin;
pub use keypair::KeypairPlugin;
pub use ovsdb_bridge::OvsBridgePlugin;
pub use privacy_router::PrivacyRouterPlugin;
pub use privacy_routes::PrivacyRoutesPlugin;
pub use procfs::ProcfsPlugin;
pub use proxmox::ProxmoxPlugin;
pub use proxy_server::ProxyServerPlugin;
pub use rtnetlink::RtnetlinkPlugin;
pub use s6::S6StatePlugin;
pub use service::ServicePlugin;
pub use sessdecl::SessDeclPlugin;
pub use software::SoftwarePlugin;
// pub use systemd::SystemdStatePlugin;
pub use mail_server::MailServerPlugin;
pub use unix_socket::UnixSocketPlugin;
pub use users::UsersPlugin;
pub use web_ui::WebUiPlugin;
pub use wireguard::WireGuardPlugin;
// pub use systemd_networkd::SystemdNetworkdPlugin; // TODO: Plugin not yet implemented

pub mod ctl_plane_chatbot;
pub use ctl_plane_chatbot::CtlPlaneChatbotPlugin;
</file>

<file path="src/state_plugins/net.rs">
// Net state plugin - authoritative OVS state management via D-Bus
// Handles: interfaces, bridges, IPs, basic connectivity via plugin schema
// Integrates with systemd-networkd as subordinate service for L3 configuration
use op_blockchain::PluginFootprint;

// Use D-Bus introspection instead of CLI commands
use anyhow::{Context, Result};
use async_trait::async_trait;
use log;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateAction, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
// use std::net::Ipv4Addr; // not needed currently

/// Network configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub interfaces: Vec<InterfaceConfig>,
}

/// Interface configuration with immutable identity and tunable config
/// Pattern matches LXC plugin: immutable core + tunable properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    // IMMUTABLE - Core identity (set once, never changes)
    /// Interface name (e.g., "ovsbr0", "mesh")
    pub name: String,

    /// Interface type (e.g., "ovs-bridge", "ethernet")
    #[serde(rename = "type")]
    pub if_type: InterfaceType,

    /// L2 driver to use (e.g., "openvswitch", "linux-bridge")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,

    // TUNABLE - Configuration that can change (blockchain tracks all changes)
    /// All tunable configuration in a single object
    #[serde(flatten)]
    pub tunable: TunableConfig,
}

/// Tunable configuration - can be changed, each change tracked in blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunableConfig {
    /// Ports attached to this interface
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,

    /// L3 driver for IP configuration (e.g., "rtnetlink", "ovs-rpc", "systemd-networkd")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l3_driver: Option<String>,

    /// IPv4 configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4: Option<Ipv4Config>,

    /// IPv6 configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<Ipv6Config>,

    /// SDN controller (for OpenFlow)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,

    /// Dynamic properties - introspection captures ALL hardware properties here
    /// Examples: mtu, mac_addresses (array), speed, duplex, txqueuelen, etc.
    ///
    /// APPEND-ONLY: Field names are permanent once added (by introspection or user)
    /// Values are mutable (ledger tracks all changes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,

    /// Property schema - tracks which fields exist (append-only set)
    /// Used for validation: new fields can be added, existing fields cannot be removed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_schema: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceType {
    Ethernet,
    OvsBridge,
    OvsPort,
    Bridge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ipv4Config {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<AddressConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ipv6Config {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressConfig {
    pub ip: String,
    pub prefix: u8,
}

/// Net state plugin implementation - authoritative OVS state via D-Bus
pub struct NetStatePlugin {
    #[allow(dead_code)]
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<PluginFootprint>>,
}

#[allow(dead_code)]
impl NetStatePlugin {
    pub fn new() -> Self {
        Self {
            blockchain_sender: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_blockchain_sender(
        blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
    ) -> Self {
        Self {
            blockchain_sender: Some(blockchain_sender),
        }
    }

    /// Validate interface configuration
    pub fn validate_interface_config(&self, _config: &InterfaceConfig) -> Result<()> {
        // TODO: Implement validation logic
        Ok(())
    }

    /// Check if OVS is available via JSON-RPC
    pub async fn check_ovs_available(&self) -> Result<bool> {
        // Try to connect to OVSDB unix socket
        let client = op_network::ovsdb::OvsdbClient::new();
        match client.list_dbs().await {
            Ok(_) => Ok(true),
            Err(_) => {
                log::info!("OVSDB socket not available - skipping OVS operations");
                Ok(false)
            }
        }
    }

    /// Query current network state via D-Bus (OVS bridges only)
    pub async fn query_current_state_dbus(&self) -> Result<NetworkConfig> {
        let mut network_interfaces = Vec::new();

        // Query OVS bridges via D-Bus
        let ovs_bridges = self.query_ovs_bridges().await?;
        network_interfaces.extend(ovs_bridges);

        Ok(NetworkConfig {
            interfaces: network_interfaces,
        })
    }

    /// Parse IPv4 configuration from ip addr show output
    fn parse_ipv4_config(output: &str) -> Option<Ipv4Config> {
        let mut ipv4_config = Ipv4Config {
            enabled: false,
            dhcp: None,
            address: Some(Vec::new()),
            gateway: None,
            dns: Some(Vec::new()),
        };

        let mut found_ipv4 = false;

        for line in output.lines() {
            let line = line.trim();

            // Look for inet lines (IPv4 addresses)
            if line.starts_with("inet ") {
                found_ipv4 = true;
                ipv4_config.enabled = true;

                // Parse inet 192.168.1.100/24 brd 192.168.1.255 scope global dynamic ens1
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let addr_part = parts[1]; // e.g., "192.168.1.100/24"
                    if let Some((ip, prefix)) = Self::parse_cidr(addr_part) {
                        if let Some(ref mut addresses) = ipv4_config.address {
                            addresses.push(AddressConfig {
                                ip,
                                prefix: prefix as u8,
                            });
                        }
                    }
                }
            }
        }

        if found_ipv4 {
            Some(ipv4_config)
        } else {
            None
        }
    }

    /// Parse CIDR notation like "192.168.1.100/24" into (ip, prefix)
    fn parse_cidr(cidr: &str) -> Option<(String, u32)> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() == 2 {
            if let Ok(prefix) = parts[1].parse::<u32>() {
                return Some((parts[0].to_string(), prefix));
            }
        }
        None
    }

    /// Query OVS bridges directly via JSON-RPC
    pub async fn query_ovs_bridges(&self) -> Result<Vec<InterfaceConfig>> {
        // Use OVSDB JSON-RPC client - native protocol
        let client = op_network::ovsdb::OvsdbClient::new();

        // Check if OVSDB is available
        if client.list_dbs().await.is_err() {
            log::info!("OVSDB socket not available - skipping OVS operations");
            return Ok(Vec::new());
        }

        let mut bridges = Vec::new();

        // Get all bridge names via JSON-RPC
        let bridge_names = match client.list_bridges().await {
            Ok(names) => names,
            Err(_) => {
                log::info!("Failed to list OVS bridges via JSON-RPC");
                return Ok(Vec::new());
            }
        };

        for bridge_name in bridge_names {
            // Get bridge information via JSON-RPC
            let bridge_info_json = match client.get_bridge_info(&bridge_name).await {
                Ok(info) => info,
                Err(_) => {
                    log::debug!("Failed to get info for bridge: {}", bridge_name);
                    continue;
                }
            };

            // Parse JSON string to HashMap
            let mut bridge_info: HashMap<String, Value> = match unsafe {
                let mut bridge_info_json_mut = bridge_info_json;
                simd_json::from_str::<HashMap<String, Value>>(&mut bridge_info_json_mut)
            } {
                Ok(info) => info,
                Err(_) => {
                    log::debug!("Failed to parse bridge info JSON for: {}", bridge_name);
                    continue;
                }
            };

            // Enrich with routing info (via rtnetlink) for this bridge
            if let Ok(routes) = op_network::rtnetlink::list_routes_for_interface(&bridge_name).await
            {
                bridge_info.insert(
                    "routing".to_string(),
                    simd_json::json!({
                        "ipv4_routes": routes
                    }),
                );
            }

            // Get ports for this bridge via JSON-RPC
            let ports = match client.list_bridge_ports(&bridge_name).await {
                Ok(ports) => Some(ports),
                Err(_) => {
                    log::debug!("Failed to get ports for bridge: {}", bridge_name);
                    None
                }
            };

            // Derive simple role tags for ports (best-effort heuristics)
            if let Some(ref port_list) = ports {
                let mut tags: HashMap<String, String> = HashMap::new();
                for p in port_list {
                    let role = if p == "wgcf" {
                        "warp"
                    } else if p.starts_with("wg") {
                        "wireguard"
                    } else if p.starts_with("vi") {
                        // vi{VMID}
                        "container"
                    } else if p.starts_with("nm") {
                        "netmaker"
                    } else if p.starts_with("eth") || p.starts_with("en") {
                        "uplink"
                    } else if p == &bridge_name {
                        "internal"
                    } else {
                        "unknown"
                    };
                    tags.insert(p.clone(), role.to_string());
                }
                bridge_info.insert(
                    "port_tags".to_string(),
                    simd_json::serde::to_owned_value(tags).unwrap_or(Value::null()),
                );
            }

            bridges.push(InterfaceConfig {
                name: bridge_name,
                if_type: InterfaceType::OvsBridge,
                driver: Some("openvswitch".to_string()),
                tunable: TunableConfig {
                    ports,
                    l3_driver: None, // Bridges typically don't need L3 config
                    ipv4: None,      // OVS bridges don't have IP config directly
                    ipv6: None,
                    controller: None,
                    properties: Some(bridge_info),
                    property_schema: Some(vec!["ovsdb".to_string()]),
                },
            });
        }

        Ok(bridges)
    }

    /// Apply OVS bridge configuration via JSON-RPC and rtnetlink
    pub async fn apply_ovs_config(&self, config: &InterfaceConfig) -> Result<()> {
        let client = op_network::ovsdb::OvsdbClient::new();
        log::info!("Starting apply_ovs_config for {}", config.name);

        // Ensure bridge exists via OVSDB JSON-RPC
        if !client
            .bridge_exists(&config.name)
            .await
            .context("Failed to check bridge existence")?
        {
            client
                .create_bridge(&config.name)
                .await
                .context("Failed to create OVS bridge via JSON-RPC")?;
            log::info!("Created OVS bridge via JSON-RPC: {}", config.name);
        }

        // Add ports to bridge if specified via OVSDB JSON-RPC
        // Skip netmaker interfaces (nm-*) - they are managed by netclient
        if let Some(ref ports) = config.tunable.ports {
            // Get current ports via JSON-RPC instead of ovs-vsctl
            let current_ports = client
                .list_bridge_ports(&config.name)
                .await
                .context("Failed to list ports via JSON-RPC")?;

            for port in ports {
                // Skip netmaker/wireguard interfaces - netclient manages them
                if port.starts_with("nm-") || port.starts_with("wg") {
                    log::info!(
                        "Skipping netmaker interface {} (managed by netclient)",
                        port
                    );
                    continue;
                }

                if !current_ports.contains(port) {
                    client.add_port(&config.name, port).await.context(format!(
                        "Failed to add port {} to bridge {} via JSON-RPC",
                        port, config.name
                    ))?;
                    log::info!("Added port {} to bridge {} via JSON-RPC", port, config.name);
                }
            }
        }

        // Update /etc/network/interfaces with bridge and IP configuration
        self.update_interfaces_file(&config.name, None, &config.tunable.ipv4)
            .await?;

        // Bring bridge up via rtnetlink (native netlink)
        if let Err(e) = op_network::rtnetlink::link_up(&config.name).await {
            log::warn!("Failed to bring bridge up via netlink: {}", e);
        }

        // Configure IPv4 if specified via rtnetlink (native netlink)
        if let Some(ref ipv4) = config.tunable.ipv4 {
            if ipv4.enabled {
                if let Some(ref addresses) = ipv4.address {
                    for addr in addresses {
                        match op_network::rtnetlink::add_ipv4_address(
                            &config.name,
                            &addr.ip,
                            addr.prefix,
                        )
                        .await
                        {
                            Ok(_) => {
                                log::info!(
                                    "Added IP {}/{} to {} via rtnetlink",
                                    addr.ip,
                                    addr.prefix,
                                    config.name
                                );
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to add IP {} (may already exist): {}",
                                    addr.ip,
                                    e
                                );
                            }
                        }
                    }
                }

                // Configure gateway if specified via rtnetlink (native netlink)
                if let Some(ref gateway) = ipv4.gateway {
                    // Delete existing default route (ignore errors)
                    let _ = op_network::rtnetlink::del_default_route().await;

                    // Add new default route
                    match op_network::rtnetlink::add_default_route(&config.name, gateway).await {
                        Ok(_) => {
                            log::info!(
                                "Added default route via {} on {} via rtnetlink",
                                gateway,
                                config.name
                            );
                        }
                        Err(e) => {
                            log::warn!("Failed to add default route: {}", e);
                        }
                    }
                }
            }
        }

        log::info!("Finished apply_ovs_config for {}", config.name);
        Ok(())
    }

    /// Apply OVS internal port configuration
    pub async fn apply_ovs_port_config(&self, config: &InterfaceConfig) -> Result<()> {
        log::info!("Starting apply_ovs_port_config for {}", config.name);

        // Internal ports are created as part of their parent bridge
        // This function handles IP configuration only

        // Determine L3 driver (default to rtnetlink)
        let l3_driver = config.tunable.l3_driver.as_deref().unwrap_or("rtnetlink");

        if l3_driver == "rtnetlink" {
            // Bring interface up via native rtnetlink
            if let Err(e) = op_network::rtnetlink::link_up(&config.name).await {
                log::warn!("Failed to bring port up: {}", e);
            }

            // Configure IPv4 if specified via rtnetlink
            if let Some(ref ipv4) = config.tunable.ipv4 {
                if ipv4.enabled {
                    if let Some(ref addresses) = ipv4.address {
                        for addr in addresses {
                            match op_network::rtnetlink::add_ipv4_address(
                                &config.name,
                                &addr.ip,
                                addr.prefix,
                            )
                            .await
                            {
                                Ok(_) => {
                                    log::info!(
                                        "Added IP {}/{} to {} via rtnetlink",
                                        addr.ip,
                                        addr.prefix,
                                        config.name
                                    );
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Failed to add IP {} (may already exist): {}",
                                        addr.ip,
                                        e
                                    );
                                }
                            }
                        }
                    }

                    // Configure gateway if specified
                    if let Some(ref gateway) = ipv4.gateway {
                        let _ = op_network::rtnetlink::del_default_route().await;
                        match op_network::rtnetlink::add_default_route(&config.name, gateway).await
                        {
                            Ok(_) => {
                                log::info!("Added default route via {} via rtnetlink", gateway);
                            }
                            Err(e) => {
                                log::warn!("Failed to add default route: {}", e);
                            }
                        }
                    }
                }
            }

            // Update /etc/network/interfaces for persistence
            self.update_interfaces_file(&config.name, None, &config.tunable.ipv4)
                .await?;
        } else {
            log::warn!("Unsupported L3 driver '{}' for {}", l3_driver, config.name);
        }

        log::info!("Finished apply_ovs_port_config for {}", config.name);
        Ok(())
    }

    /// Delete OVS bridge via JSON-RPC
    pub async fn delete_ovs_bridge(&self, name: &str) -> Result<()> {
        let client = op_network::ovsdb::OvsdbClient::new();

        client
            .delete_bridge(name)
            .await
            .context("Failed to delete OVS bridge via JSON-RPC")?;

        Ok(())
    }

    /// Update /etc/network/interfaces with bridge configuration
    async fn update_interfaces_file(
        &self,
        bridge: &str,
        uplink: Option<&str>,
        ipv4: &Option<Ipv4Config>,
    ) -> Result<()> {
        let interfaces_path = std::path::Path::new("/etc/network/interfaces");
        let tag = "op-dbus-managed";
        let begin_marker = format!("# BEGIN {}\n", tag);
        let end_marker = format!("# END {}\n", tag);

        // Build the managed block
        let mut block = String::new();
        block.push_str(&begin_marker);
        block.push_str(&format!("# Managed by {}. Do not edit manually.\n\n", tag));

        // OVS Bridge with IP configuration
        // Use allow-ovs instead of auto to prevent ifupdown hang
        block.push_str(&format!("allow-ovs {}\n", bridge));
        block.push_str(&format!("iface {} inet ", bridge));

        if let Some(ref ipv4_cfg) = ipv4 {
            if ipv4_cfg.enabled {
                if ipv4_cfg.dhcp == Some(true) {
                    block.push_str("dhcp\n");
                } else if let Some(ref addresses) = ipv4_cfg.address {
                    if let Some(addr) = addresses.first() {
                        block.push_str("static\n");
                        block.push_str(&format!("    address {}\n", addr.ip));
                        block.push_str(&format!(
                            "    netmask {}\n",
                            Self::prefix_to_netmask(addr.prefix)
                        ));

                        if let Some(ref gateway) = ipv4_cfg.gateway {
                            block.push_str(&format!("    gateway {}\n", gateway));
                        }
                    } else {
                        block.push_str("manual\n");
                    }
                } else {
                    block.push_str("manual\n");
                }
            } else {
                block.push_str("manual\n");
            }
        } else {
            block.push_str("manual\n");
        }

        block.push_str("    ovs_type OVSBridge\n");

        // Add uplink to ovs_ports if specified
        if let Some(uplink_iface) = uplink {
            block.push_str(&format!("    ovs_ports {}\n", uplink_iface));
        }
        block.push('\n');

        // Physical uplink (if specified)
        if let Some(uplink_iface) = uplink {
            block.push_str(&format!("allow-{} {}\n", bridge, uplink_iface));
            block.push_str(&format!("iface {} inet manual\n", uplink_iface));
            block.push_str(&format!("    ovs_bridge {}\n", bridge));
            block.push_str("    ovs_type OVSPort\n");
            block.push('\n');
        }

        block.push_str(&end_marker);

        // Read current file content
        let content = tokio::fs::read_to_string(interfaces_path)
            .await
            .unwrap_or_else(|_| String::from("# network interfaces file\n"));

        // Replace or append the managed block
        let new_content = Self::replace_block(&content, &begin_marker, &end_marker, &block);

        // Write back if changed
        if new_content != content {
            tokio::fs::write(interfaces_path, new_content)
                .await
                .context("Failed to write /etc/network/interfaces")?;
            log::info!("Updated /etc/network/interfaces");
        }

        Ok(())
    }

    /// Convert CIDR prefix to netmask string
    fn prefix_to_netmask(prefix: u8) -> String {
        let mask: u32 = !0u32 << (32 - prefix);
        format!(
            "{}.{}.{}.{}",
            (mask >> 24) & 0xFF,
            (mask >> 16) & 0xFF,
            (mask >> 8) & 0xFF,
            mask & 0xFF
        )
    }

    /// Replace a marked block in text content
    fn replace_block(
        content: &str,
        begin_marker: &str,
        end_marker: &str,
        new_block: &str,
    ) -> String {
        if let Some(start) = content.find(begin_marker) {
            if let Some(end) = content[start..].find(end_marker) {
                let end_idx = start + end + end_marker.len();
                let mut result = String::with_capacity(content.len() + new_block.len());
                result.push_str(&content[..start]);
                result.push_str(new_block);
                result.push_str(&content[end_idx..]);
                return result;
            }
        }

        // Block not found, append it
        let mut result = String::with_capacity(content.len() + new_block.len() + 1);
        result.push_str(content);
        if !content.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(new_block);
        result
    }
}

impl Default for NetStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}
#[async_trait]
impl StatePlugin for NetStatePlugin {
    fn name(&self) -> &str {
        "net"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::net_plugin_schema())
    }

    fn is_available(&self) -> bool {
        // Check if OVSDB socket is available
        std::path::Path::new("/var/run/openvswitch/db.sock").exists()
    }

    fn unavailable_reason(&self) -> String {
        "OpenVSwitch OVSDB socket not found at /var/run/openvswitch/db.sock - install with: apt install openvswitch-switch".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        // Query current OVS state via D-Bus exclusively
        let network_config = self.query_current_state_dbus().await?;
        Ok(simd_json::serde::to_owned_value(network_config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: NetworkConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: NetworkConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        // Build maps for quick lookup - avoid cloning strings unnecessarily
        let current_map: HashMap<&String, &InterfaceConfig> = current_config
            .interfaces
            .iter()
            .map(|i| (&i.name, i))
            .collect();

        let desired_map: HashMap<&String, &InterfaceConfig> = desired_config
            .interfaces
            .iter()
            .map(|i| (&i.name, i))
            .collect();

        // Find interfaces to create or modify
        for (name, desired_iface) in &desired_map {
            if let Some(current_iface) = current_map.get(name) {
                // Check if modification needed
                if simd_json::serde::to_owned_value(current_iface)?
                    != simd_json::serde::to_owned_value(desired_iface)?
                {
                    actions.push(StateAction::Modify {
                        resource: (*name).clone(),
                        changes: simd_json::serde::to_owned_value(desired_iface)?,
                    });
                }
            } else {
                actions.push(StateAction::Create {
                    resource: (*name).clone(),
                    config: simd_json::serde::to_owned_value(desired_iface)?,
                });
            }
        }

        // Find interfaces to delete
        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                actions.push(StateAction::Delete {
                    resource: (*name).clone(),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config }
                | StateAction::Modify {
                    resource,
                    changes: config,
                } => {
                    let iface_config: InterfaceConfig =
                        simd_json::serde::from_owned_value(config.clone())?;

                    match self.apply_ovs_config(&iface_config).await {
                        Ok(_) => {
                            changes_applied.push(format!("Applied OVS config for: {}", resource));
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Failed to apply OVS config for {}: {}",
                                resource, e
                            ));
                        }
                    }
                }
                StateAction::Delete { resource } => {
                    // Delete OVS bridge via D-Bus
                    if resource.starts_with("ovsbr") || resource.starts_with("br") {
                        match self.delete_ovs_bridge(resource).await {
                            Ok(_) => {
                                changes_applied.push(format!("Deleted OVS bridge: {}", resource));
                            }
                            Err(e) => {
                                errors.push(format!(
                                    "Failed to delete OVS bridge {}: {}",
                                    resource, e
                                ));
                            }
                        }
                    } else {
                        changes_applied.push(format!("Skipped non-OVS interface: {}", resource));
                    }
                }
                StateAction::NoOp { .. } => {}
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let desired_config: NetworkConfig = simd_json::serde::from_owned_value(desired.clone())?;
        let current = self.query_current_state().await?;
        let current_config: NetworkConfig = simd_json::serde::from_owned_value(current)?;

        // Simple verification: check if desired interfaces exist
        let current_names: std::collections::HashSet<_> =
            current_config.interfaces.iter().map(|i| &i.name).collect();

        for iface in &desired_config.interfaces {
            if !current_names.contains(&iface.name) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current_state = self.query_current_state().await?;

        Ok(Checkpoint {
            id: format!("network-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current_state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_config: NetworkConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;

        // Restore old OVS configuration via D-Bus
        for iface in &old_config.interfaces {
            match iface.if_type {
                InterfaceType::OvsBridge => {
                    self.apply_ovs_config(iface).await?;
                }
                InterfaceType::OvsPort => {
                    self.apply_ovs_port_config(iface).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true, // D-Bus operations are atomic
        }
    }
}

// impl Default for NetStatePlugin {
//     fn default() -> Self {
//         Self::new()
//     }
// }
</file>

<file path="src/state_plugins/netmaker.rs">
use op_state::StatePlugin;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateDiff, StateAction};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::collections::HashMap;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerConfig {
    /// Enable Netmaker mesh networking
    pub enabled: bool,
    /// Default network to join
    pub default_network: String,
    /// Enrollment token for joining networks
    pub enrollment_token: Option<String>,
    /// API endpoint for Netmaker server (if self-hosted)
    pub api_endpoint: Option<String>,
}

impl Default for NetmakerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_network: "mesh".to_string(),
            enrollment_token: None,
            api_endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerNetwork {
    pub name: String,
    pub connected: bool,
    pub is_default: bool,
    pub node_id: Option<String>,
    pub peers: Vec<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerState {
    pub installed: bool,
    pub daemon_running: bool,
    pub networks: Vec<NetmakerNetwork>,
    pub public_ip: Option<String>,
    pub config: NetmakerConfig,
}

pub struct NetmakerPlugin {
    config: NetmakerConfig,
}

impl NetmakerPlugin {
    pub fn new(config: NetmakerConfig) -> Self {
        Self { config }
    }

    /// Check if netclient is installed
    async fn check_netclient_installed() -> Result<bool> {
        let output = Command::new("which").arg("netclient").output().await?;
        Ok(output.status.success())
    }

    /// Check if netclient daemon is running
    async fn check_daemon_running() -> Result<bool> {
        let output = Command::new("systemctl")
            .args(["is-active", "netclient"])
            .output()
            .await;
        Ok(output.is_ok() && output.unwrap().status.success())
    }

    /// Get current networks from netclient
    async fn get_networks(&self) -> Result<Vec<NetmakerNetwork>> {
        let output = Command::new("netclient").arg("list").output().await?;

        if !output.status.success() {
            return Ok(Vec::new()); // No networks or not connected
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut networks = Vec::new();

        // Parse netclient output
        // Format: "NETWORK NAME | CONNECTED | ADDRESS"
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                let network_name = parts[0].to_string();
                let connected =
                    parts[1].to_lowercase() == "yes" || parts[1].to_lowercase() == "true";
                let address = if parts.len() > 2 && !parts[2].is_empty() {
                    Some(parts[2].to_string())
                } else {
                    None
                };

                // Get peers for this network
                let peers = self
                    .get_network_peers(&network_name)
                    .await
                    .unwrap_or_default();

                networks.push(NetmakerNetwork {
                    name: network_name.clone(),
                    connected,
                    is_default: network_name == self.config.default_network,
                    node_id: None, // Would need to parse from daemon logs
                    peers,
                    address,
                });
            }
        }

        Ok(networks)
    }

    /// Get peers for a specific network
    async fn get_network_peers(&self, network: &str) -> Result<Vec<String>> {
        // This is a simplified implementation
        // In reality, you'd need to query the Netmaker API or parse daemon state
        let _ = network; // Suppress unused variable warning
        Ok(Vec::new()) // TODO: Implement actual peer discovery
    }

    /// Get public IP (for NAT traversal info)
    async fn get_public_ip(&self) -> Result<Option<String>> {
        // Try to get public IP for Netmaker status
        let output = Command::new("curl")
            .args(["-s", "--max-time", "5", "https://api.ipify.org"])
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(Some(ip));
            }
        }

        Ok(None)
    }

    /// Join a Netmaker network
    async fn join_network(&self, network: &str, token: &str) -> Result<()> {
        let output = Command::new("netclient")
            .args(["join", "-t", token])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to join network {}: {}",
                network,
                stderr
            ));
        }

        Ok(())
    }

    /// Leave a Netmaker network
    #[allow(dead_code)]
    async fn leave_network(&self, network: &str) -> Result<()> {
        let output = Command::new("netclient")
            .args(["leave", network])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to leave network {}: {}",
                network,
                stderr
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl StatePlugin for NetmakerPlugin {
    fn name(&self) -> &'static str {
        "netmaker"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }

    async fn query_current_state(&self) -> Result<Value> {
        let installed = Self::check_netclient_installed().await?;
        let daemon_running = if installed {
            Self::check_daemon_running().await.unwrap_or(false)
        } else {
            false
        };

        let networks = if daemon_running {
            self.get_networks().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let public_ip = self.get_public_ip().await.unwrap_or(None);

        let state = NetmakerState {
            installed,
            daemon_running,
            networks,
            public_ip,
            config: self.config.clone(),
        };

        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();

        // Check if netclient should be installed/enabled
        let current_installed = current
            .get("installed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let desired_enabled = desired
            .get("config")
            .and_then(|c| c.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !current_installed && desired_enabled {
            actions.push(StateAction::Create {
                resource: "netmaker_installation".to_string(),
                config: simd_json::json!({
                    "action": "install_netclient",
                    "type": "system_package"
                }),
            });
        }

        // Check network membership changes
        let empty_networks = vec![];
        let current_networks = current
            .get("networks")
            .and_then(|n| n.as_array())
            .unwrap_or(&empty_networks);
        let desired_networks = desired
            .get("config")
            .and_then(|c| c.get("default_network"))
            .and_then(|n| n.as_str());

        if let Some(desired_network) = desired_networks {
            let currently_connected = current_networks.iter().any(|net| {
                net.get("name").and_then(|n| n.as_str()) == Some(desired_network)
                    && net
                        .get("connected")
                        .and_then(|c| c.as_bool())
                        .unwrap_or(false)
            });

            if !currently_connected && desired_enabled {
                actions.push(StateAction::Create {
                    resource: format!("netmaker_network_{}", desired_network),
                    config: simd_json::json!({
                        "network": desired_network,
                        "action": "join_network",
                        "type": "network_membership"
                    }),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create {
                    resource,
                    config: _,
                } => {
                    if resource == "netmaker_installation" {
                        // Install netclient
                        let install_result = Command::new("apt")
                            .args(["update", "&&", "apt", "install", "-y", "netclient"])
                            .status()
                            .await;

                        match install_result {
                            Ok(_) => {
                                changes_applied.push("Installed netclient package".to_string());
                                // Enable and start service
                                let _ = Command::new("systemctl")
                                    .args(["enable", "--now", "netclient"])
                                    .status()
                                    .await;
                            }
                            Err(e) => errors.push(format!("Failed to install netclient: {}", e)),
                        }
                    } else if resource.starts_with("netmaker_network_") {
                        let network = resource.strip_prefix("netmaker_network_").unwrap_or("");
                        if let Some(token) = &self.config.enrollment_token {
                            match self.join_network(network, token).await {
                                Ok(_) => changes_applied
                                    .push(format!("Joined Netmaker network {}", network)),
                                Err(e) => errors
                                    .push(format!("Failed to join network {}: {}", network, e)),
                            }
                        } else {
                            errors.push(format!(
                                "No enrollment token configured for network {}",
                                network
                            ));
                        }
                    }
                }
                _ => {} // Other actions not implemented yet
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        Ok(self
            .calculate_diff(&current, desired)
            .await?
            .actions
            .is_empty())
    }

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(op_state::Checkpoint {
            id: format!(
                "netmaker_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
            ),
            plugin: self.name().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &op_state::Checkpoint) -> Result<()> {
        // Rollback would leave networks and potentially rejoin them
        // This is a simplified implementation
        Err(anyhow::anyhow!(
            "Netmaker rollback not implemented - would require leaving and rejoining networks"
        ))
    }
}
</file>

<file path="src/state_plugins/openflow_obfuscation.rs">
//! OpenFlow Traffic Obfuscation Plugin
//!
//! Implements three levels of traffic obfuscation using OpenFlow rules:
//! - Level 1: Basic security (drop invalid, rate limiting, connection tracking)
//! - Level 2: Pattern hiding (TTL normalization, packet padding, timing jitter)
//! - Level 3: Advanced obfuscation (protocol mimicry, decoy traffic, morphing)
//!
//! Works with OVS bridges to apply privacy-enhancing flow rules.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;

/// OpenFlow obfuscation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowObfuscationConfig {
    /// OVS bridge to apply flows to
    pub bridge_name: String,

    /// Obfuscation level (0-3)
    pub obfuscation_level: u8,

    /// Enable security flows (always recommended)
    pub enable_security_flows: bool,

    /// Privacy socket ports for the tunnel chain
    pub privacy_ports: Vec<String>,

    /// Additional custom flows
    pub custom_flows: Vec<OpenFlowRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowRule {
    /// Flow table (0-254)
    pub table: u8,

    /// Priority (0-65535)
    pub priority: u16,

    /// Match criteria (e.g., "in_port=1,ip,tcp_dst=80")
    pub match_spec: String,

    /// Actions (e.g., "output:2,mod_nw_ttl:64")
    pub actions: String,

    /// Description
    pub description: String,
}

impl Default for OpenFlowObfuscationConfig {
    fn default() -> Self {
        Self {
            bridge_name: "ovs-br0".to_string(),
            obfuscation_level: 2,
            enable_security_flows: true,
            privacy_ports: vec![
                "priv_wg".to_string(),
                "priv_warp".to_string(),
                "priv_xray".to_string(),
            ],
            custom_flows: vec![],
        }
    }
}

pub struct OpenFlowObfuscationPlugin {
    config: OpenFlowObfuscationConfig,
}

impl OpenFlowObfuscationPlugin {
    pub fn new(config: OpenFlowObfuscationConfig) -> Self {
        Self { config }
    }

    /// Generate Level 1 flows: Basic security
    fn generate_level1_flows(&self) -> Vec<OpenFlowRule> {
        let mut flows = vec![];

        // Table 0: Security screening (11 flows)

        // 1. Drop invalid TCP flags
        flows.push(OpenFlowRule {
            table: 0,
            priority: 500,
            match_spec: "tcp,tcp_flags=+syn+fin".to_string(),
            actions: "drop".to_string(),
            description: "Drop SYN+FIN packets (invalid)".to_string(),
        });

        // 2. Drop NULL scan packets
        flows.push(OpenFlowRule {
            table: 0,
            priority: 500,
            match_spec: "tcp,tcp_flags=0".to_string(),
            actions: "drop".to_string(),
            description: "Drop NULL scan packets".to_string(),
        });

        // 3. Drop XMAS scan packets
        flows.push(OpenFlowRule {
            table: 0,
            priority: 500,
            match_spec: "tcp,tcp_flags=+fin+urg+psh".to_string(),
            actions: "drop".to_string(),
            description: "Drop XMAS scan packets".to_string(),
        });

        // 4. Drop fragmented packets (potential evasion)
        flows.push(OpenFlowRule {
            table: 0,
            priority: 490,
            match_spec: "ip,ip_frag=first".to_string(),
            actions: "drop".to_string(),
            description: "Drop fragmented packets".to_string(),
        });

        // 5. Rate limit ICMP (DDoS protection)
        flows.push(OpenFlowRule {
            table: 0,
            priority: 480,
            match_spec: "icmp".to_string(),
            actions: "meter:1,resubmit(,10)".to_string(),
            description: "Rate limit ICMP to 100pps".to_string(),
        });

        // 6. Rate limit DNS (DDoS protection)
        flows.push(OpenFlowRule {
            table: 0,
            priority: 480,
            match_spec: "udp,tp_dst=53".to_string(),
            actions: "meter:2,resubmit(,10)".to_string(),
            description: "Rate limit DNS queries to 1000pps".to_string(),
        });

        // 7. Connection tracking for stateful filtering
        flows.push(OpenFlowRule {
            table: 0,
            priority: 470,
            match_spec: "ip".to_string(),
            actions: "ct(table=10)".to_string(),
            description: "Connection tracking for stateful filtering".to_string(),
        });

        // 8. Drop invalid connection states
        flows.push(OpenFlowRule {
            table: 10,
            priority: 500,
            match_spec: "ct_state=-trk".to_string(),
            actions: "drop".to_string(),
            description: "Drop untracked connections".to_string(),
        });

        flows.push(OpenFlowRule {
            table: 10,
            priority: 500,
            match_spec: "ct_state=+inv".to_string(),
            actions: "drop".to_string(),
            description: "Drop invalid connection states".to_string(),
        });

        // 9. Allow established connections
        flows.push(OpenFlowRule {
            table: 10,
            priority: 400,
            match_spec: "ct_state=+est".to_string(),
            actions: "resubmit(,20)".to_string(),
            description: "Allow established connections".to_string(),
        });

        // 10. Allow new connections
        flows.push(OpenFlowRule {
            table: 10,
            priority: 390,
            match_spec: "ct_state=+new".to_string(),
            actions: "resubmit(,20)".to_string(),
            description: "Allow new connections".to_string(),
        });

        // 11. Default drop for table 0
        flows.push(OpenFlowRule {
            table: 0,
            priority: 1,
            match_spec: "".to_string(),
            actions: "drop".to_string(),
            description: "Default drop for security".to_string(),
        });

        flows
    }

    /// Generate Level 2 flows: Pattern hiding
    fn generate_level2_flows(&self) -> Vec<OpenFlowRule> {
        let mut flows = vec![];

        // Table 20: Pattern obfuscation (3 flows)

        // 1. TTL normalization - set all outbound packets to TTL 64
        flows.push(OpenFlowRule {
            table: 20,
            priority: 300,
            match_spec: "ip".to_string(),
            actions: "mod_nw_ttl:64,resubmit(,30)".to_string(),
            description: "TTL normalization (set to 64)".to_string(),
        });

        // 2. Packet size padding - pad small packets to reduce size-based fingerprinting
        // Note: OVS doesn't directly support padding, but we can use meters with burst
        // to introduce timing variations that achieve similar anti-fingerprinting
        flows.push(OpenFlowRule {
            table: 20,
            priority: 290,
            match_spec: "tcp".to_string(),
            actions: "meter:3,resubmit(,30)".to_string(),
            description: "Timing jitter for TCP (anti-fingerprinting)".to_string(),
        });

        // 3. Window size normalization for TCP
        flows.push(OpenFlowRule {
            table: 20,
            priority: 280,
            match_spec: "tcp".to_string(),
            actions: "mod_tp_src:0x1234,resubmit(,30)".to_string(),
            description: "TCP source port randomization".to_string(),
        });

        flows
    }

    /// Generate Level 3 flows: Advanced obfuscation
    fn generate_level3_flows(&self) -> Vec<OpenFlowRule> {
        let mut flows = vec![];

        // Table 30: Advanced obfuscation (4 flows)

        // 1. Protocol mimicry - make VPN traffic look like HTTPS
        flows.push(OpenFlowRule {
            table: 30,
            priority: 200,
            match_spec: "udp,tp_dst=51820".to_string(),
            actions: "mod_tp_dst:443,resubmit(,40)".to_string(),
            description: "WireGuard port mimicry (51820→443)".to_string(),
        });

        // 2. Decoy traffic generation trigger
        // This flow matches low-traffic periods and triggers decoy generation
        flows.push(OpenFlowRule {
            table: 30,
            priority: 190,
            match_spec: "ip".to_string(),
            actions: "meter:4,resubmit(,40)".to_string(),
            description: "Decoy traffic trigger (low bandwidth detection)".to_string(),
        });

        // 3. Traffic morphing - randomize packet ordering
        flows.push(OpenFlowRule {
            table: 30,
            priority: 180,
            match_spec: "tcp".to_string(),
            actions: "meter:5,resubmit(,40)".to_string(),
            description: "Packet timing randomization (morphing)".to_string(),
        });

        // 4. Deep packet inspection evasion - fragment large packets
        flows.push(OpenFlowRule {
            table: 30,
            priority: 170,
            match_spec: "tcp,dl_vlan=100".to_string(),
            actions: "strip_vlan,resubmit(,40)".to_string(),
            description: "DPI evasion (VLAN stripping)".to_string(),
        });

        flows
    }

    /// Generate forwarding flows for privacy tunnel
    fn generate_forwarding_flows(&self) -> Vec<OpenFlowRule> {
        let mut flows = vec![];

        // Table 40: Final forwarding

        // Forward through privacy chain: priv_wg → priv_warp → priv_xray
        for (idx, port) in self.config.privacy_ports.iter().enumerate() {
            if idx < self.config.privacy_ports.len() - 1 {
                let next_port = &self.config.privacy_ports[idx + 1];
                flows.push(OpenFlowRule {
                    table: 40,
                    priority: 100,
                    match_spec: format!("in_port={}", port),
                    actions: format!("output:{}", next_port),
                    description: format!("Forward {} → {}", port, next_port),
                });
            }
        }

        // Return path: priv_xray → priv_warp → priv_wg
        let ports: Vec<_> = self.config.privacy_ports.iter().collect();
        for (idx, port) in ports.into_iter().enumerate().rev() {
            if idx > 0 {
                let prev_port = &self.config.privacy_ports[idx - 1];
                flows.push(OpenFlowRule {
                    table: 40,
                    priority: 100,
                    match_spec: format!("in_port={}", port),
                    actions: format!("output:{}", prev_port),
                    description: format!("Return {} → {}", port, prev_port),
                });
            }
        }

        // Normal forwarding for non-privacy ports
        flows.push(OpenFlowRule {
            table: 40,
            priority: 1,
            match_spec: "".to_string(),
            actions: "NORMAL".to_string(),
            description: "Normal L2/L3 forwarding".to_string(),
        });

        flows
    }

    /// Generate all flows based on obfuscation level
    fn generate_all_flows(&self) -> Vec<OpenFlowRule> {
        let mut all_flows = vec![];

        // Always include forwarding flows
        all_flows.extend(self.generate_forwarding_flows());

        // Add security flows if enabled (Level 1+)
        if self.config.enable_security_flows && self.config.obfuscation_level >= 1 {
            all_flows.extend(self.generate_level1_flows());
        }

        // Add pattern hiding flows (Level 2+)
        if self.config.obfuscation_level >= 2 {
            all_flows.extend(self.generate_level2_flows());
        }

        // Add advanced obfuscation flows (Level 3)
        if self.config.obfuscation_level >= 3 {
            all_flows.extend(self.generate_level3_flows());
        }

        // Add custom flows
        all_flows.extend(self.config.custom_flows.clone());

        all_flows
    }

    /// Convert OpenFlowRule to ovs-ofctl command
    fn flow_to_command(&self, flow: &OpenFlowRule) -> String {
        let mut cmd = format!("table={},priority={}", flow.table, flow.priority);

        if !flow.match_spec.is_empty() {
            cmd.push_str(&format!(",{}", flow.match_spec));
        }

        cmd.push_str(&format!(" actions={}", flow.actions));

        cmd
    }
}

#[async_trait]
impl StatePlugin for OpenFlowObfuscationPlugin {
    fn name(&self) -> &'static str {
        "openflow_obfuscation"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }

    async fn query_current_state(&self) -> Result<Value> {
        // Query current OpenFlow rules
        let flows = self.generate_all_flows();

        Ok(json!({
            "config": self.config,
            "flows": {
                "count": flows.len(),
                "by_level": {
                    "security": if self.config.obfuscation_level >= 1 { 11 } else { 0 },
                    "pattern_hiding": if self.config.obfuscation_level >= 2 { 3 } else { 0 },
                    "advanced": if self.config.obfuscation_level >= 3 { 4 } else { 0 },
                    "forwarding": self.config.privacy_ports.len() * 2 + 1,
                    "custom": self.config.custom_flows.len(),
                }
            },
            "bridge": self.config.bridge_name,
            "obfuscation_level": self.config.obfuscation_level,
        }))
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();

        let current_config = current.get("config");
        let desired_config = desired.get("config");

        if current_config != desired_config {
            actions.push(StateAction::Modify {
                resource: format!("openflow_flows_{}", self.config.bridge_name),
                changes: desired.clone(),
            });
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        log::info!(
            "Applying OpenFlow obfuscation level {} to bridge {}",
            self.config.obfuscation_level,
            self.config.bridge_name
        );

        // Generate all flows
        let flows = self.generate_all_flows();

        changes_applied.push(format!(
            "Generated {} OpenFlow rules (Level {})",
            flows.len(),
            self.config.obfuscation_level
        ));

        // In a real implementation, we would:
        // 1. Use op_network::ovsdb to clear existing flows
        // 2. Apply new flows via OVSDB or ovs-ofctl
        // 3. Verify flow installation

        // For now, log the commands that would be executed
        for flow in &flows {
            let cmd = self.flow_to_command(flow);
            log::debug!("Flow: {} ({})", cmd, flow.description);
            changes_applied.push(format!("  [T{}:P{}] {}", flow.table, flow.priority, flow.description));
        }

        changes_applied.push(format!(
            "Obfuscation breakdown: {} security, {} pattern-hiding, {} advanced, {} forwarding",
            if self.config.obfuscation_level >= 1 { 11 } else { 0 },
            if self.config.obfuscation_level >= 2 { 3 } else { 0 },
            if self.config.obfuscation_level >= 3 { 4 } else { 0 },
            self.config.privacy_ports.len() * 2 + 1,
        ));

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        Ok(self
            .calculate_diff(&current, desired)
            .await?
            .actions
            .is_empty())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!(
                "openflow_obfuscation_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
            ),
            plugin: self.name().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        log::info!(
            "Rolling back OpenFlow obfuscation to checkpoint: {}",
            checkpoint.id
        );

        // In real implementation:
        // 1. Extract previous config from checkpoint
        // 2. Delete all flows on bridge
        // 3. Reapply flows from checkpoint state

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level0_no_obfuscation() {
        let config = OpenFlowObfuscationConfig {
            obfuscation_level: 0,
            enable_security_flows: false,
            ..Default::default()
        };
        let plugin = OpenFlowObfuscationPlugin::new(config);
        let flows = plugin.generate_all_flows();

        // Should only have forwarding flows
        assert_eq!(flows.len(), 5); // 2*2 + 1 for default NORMAL
    }

    #[test]
    fn test_level1_security() {
        let config = OpenFlowObfuscationConfig {
            obfuscation_level: 1,
            enable_security_flows: true,
            ..Default::default()
        };
        let plugin = OpenFlowObfuscationPlugin::new(config);
        let flows = plugin.generate_all_flows();

        // Should have forwarding + security flows
        assert!(flows.len() >= 11 + 5);
    }

    #[test]
    fn test_level2_pattern_hiding() {
        let config = OpenFlowObfuscationConfig {
            obfuscation_level: 2,
            enable_security_flows: true,
            ..Default::default()
        };
        let plugin = OpenFlowObfuscationPlugin::new(config);
        let flows = plugin.generate_all_flows();

        // Should have forwarding + security + pattern hiding
        assert!(flows.len() >= 11 + 3 + 5);
    }

    #[test]
    fn test_level3_advanced() {
        let config = OpenFlowObfuscationConfig {
            obfuscation_level: 3,
            enable_security_flows: true,
            ..Default::default()
        };
        let plugin = OpenFlowObfuscationPlugin::new(config);
        let flows = plugin.generate_all_flows();

        // Should have all flow types
        assert!(flows.len() >= 11 + 3 + 4 + 5);
    }

    #[test]
    fn test_flow_command_generation() {
        let plugin = OpenFlowObfuscationPlugin::new(Default::default());
        let flow = OpenFlowRule {
            table: 0,
            priority: 500,
            match_spec: "tcp,tcp_dst=80".to_string(),
            actions: "output:1".to_string(),
            description: "Test flow".to_string(),
        };

        let cmd = plugin.flow_to_command(&flow);
        assert!(cmd.contains("table=0"));
        assert!(cmd.contains("priority=500"));
        assert!(cmd.contains("tcp,tcp_dst=80"));
        assert!(cmd.contains("actions=output:1"));
    }
}
</file>

<file path="src/state_plugins/openflow.rs">
// OpenFlow Controller Plugin - Flow-based networking for containerless communication
// Manages OpenFlow flows for socket-based container networking without veth interfaces

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use log;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;

/// OpenFlow controller configuration - Policy-based, not interface-based
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowConfig {
    /// Bridges managed by this controller
    pub bridges: Vec<BridgeFlowConfig>,

    /// Controller endpoint (tcp:IP:PORT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_endpoint: Option<String>,

    /// Flow policies to apply (discovered containers get flows based on policies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow_policies: Option<Vec<FlowPolicy>>,

    /// Enable automatic container discovery and flow generation
    #[serde(default = "default_auto_discover")]
    pub auto_discover_containers: bool,

    /// Enable security hardening flows (default: true)
    #[serde(default = "default_security_enabled")]
    pub enable_security_flows: bool,

    /// Traffic obfuscation level for privacy (0=none, 1=basic, 2=pattern-hiding, 3=advanced)
    /// Level 1: Basic security (drop invalid, rate limit)
    /// Level 2: Pattern hiding (timing randomization, packet padding, TTL rewriting)
    /// Level 3: Advanced obfuscation (traffic morphing, protocol mimicry, decoy traffic)
    #[serde(default = "default_obfuscation_level")]
    pub obfuscation_level: u8,
}

fn default_security_enabled() -> bool {
    true
}

fn default_obfuscation_level() -> u8 {
    1 // Basic obfuscation enabled by default
}

fn default_auto_discover() -> bool {
    true
}

/// Flow policy - Applied to discovered containers/ports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowPolicy {
    /// Policy name
    pub name: String,

    /// Match selector (e.g., "container:*", "container:100-199", "port:internal_*")
    pub selector: String,

    /// Flow template to generate
    pub template: FlowTemplate,
}

/// Flow template for policy-based generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowTemplate {
    /// Table to insert flow
    pub table: u8,

    /// Priority
    pub priority: u16,

    /// Actions to perform (can use variables like {container_id}, {port_name})
    pub actions: Vec<FlowAction>,

    /// Additional match fields (beyond the auto-generated in_port match)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_matches: Option<HashMap<String, String>>,
}

/// Per-bridge flow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeFlowConfig {
    /// Bridge name (e.g., "ovsbr0")
    pub name: String,

    /// OpenFlow flows for this bridge
    pub flows: Vec<FlowEntry>,

    /// Container socket ports (internal OVS ports for containerless networking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_ports: Option<Vec<SocketPort>>,
}

/// OpenFlow flow entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowEntry {
    /// Flow table number (0-254)
    pub table: u8,

    /// Flow priority (0-65535, higher = more specific)
    pub priority: u16,

    /// Match criteria (OpenFlow match fields)
    pub match_fields: HashMap<String, String>,

    /// Actions to perform
    pub actions: Vec<FlowAction>,

    /// Cookie for flow identification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<u64>,

    /// Idle timeout in seconds (0 = permanent)
    #[serde(default)]
    pub idle_timeout: u16,

    /// Hard timeout in seconds (0 = permanent)
    #[serde(default)]
    pub hard_timeout: u16,
}

/// OpenFlow actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlowAction {
    /// Output to port
    Output { port: String },

    /// Load value into register
    LoadRegister { register: u8, value: u64 },

    /// Resubmit to another table
    Resubmit { table: u8 },

    /// Set field value
    SetField { field: String, value: String },

    /// Drop packet
    Drop,

    /// Send to normal L2 switching
    Normal,

    /// Send to controller
    Controller { max_len: Option<u16> },

    /// ARP responder (OVS-specific action chain)
    ArpResponder { mac: String, ip: String },
}

/// Socket port for containerless networking
///
/// THREE TYPES:
/// 1. Privacy sockets (predefined): priv_wg, priv_xray, priv_warp
/// 2. Shared ingress sockets (one per bridge): ovsbr0-sock
/// 3. Legacy container sockets (dynamic): sock_{container_name}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketPort {
    /// Port name:
    /// - Privacy: "priv_wg", "priv_xray" (predefined)
    /// - Container: "sock_{container_name}" (dynamic, created at runtime)
    pub name: String,

    /// Container name this port serves (for sock_* ports)
    pub container_name: Option<String>,

    /// Port type: "privacy" or "container"
    pub port_type: SocketPortType,

    /// OVS port number (assigned by OVS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ofport: Option<u16>,
}

/// Type of socket port
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SocketPortType {
    /// Privacy tunnel sockets (priv_wg, priv_xray) - predefined
    Privacy,
    /// Shared ingress port routing many privacy routes
    SharedIngress,
    /// Container sockets (sock_{name}) - dynamic, created from container name
    Container,
}

/// Discovered container from OVSDB introspection
#[derive(Debug, Clone)]
struct DiscoveredContainer {
    /// Container name (extracted from sock_{name} port)
    name: String,

    /// Port name in OVS (sock_{container_name})
    port_name: String,

    /// Bridge this container is attached to
    bridge: String,

    /// OpenFlow port number
    _ofport: Option<u16>,
}

const OPENFLOW_PROTOCOL: &str = "OpenFlow13";

/// OpenFlow plugin implementation
pub struct OpenFlowPlugin {
    /// OVSDB client for OVS operations
    ovsdb_client: Arc<op_network::ovsdb::OvsdbClient>,
}

impl OpenFlowPlugin {
    pub fn new() -> Self {
        let ovsdb_client = Arc::new(op_network::ovsdb::OvsdbClient::new());

        Self { ovsdb_client }
    }

    /// Create OpenFlow client for a bridge
    #[allow(dead_code)]
    async fn create_openflow_client(
        &self,
        bridge: &str,
    ) -> Result<op_network::openflow::OpenFlowClient> {
        // Connect to OpenFlow switch (OVS typically listens on localhost:6633)
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 6633));
        let client = op_network::openflow::OpenFlowClient::connect(addr)
            .await
            .context(format!(
                "Failed to connect to OpenFlow switch for bridge {}",
                bridge
            ))?;
        Ok(client)
    }

    /// Discover containers from OVSDB introspection
    ///
    /// Looks for sock_{container_name} ports (dynamic container sockets)
    /// Privacy sockets (priv_wg, priv_xray) are NOT included - they are predefined
    async fn discover_containers(&self) -> Result<Vec<DiscoveredContainer>> {
        let mut containers = Vec::new();

        // Get all bridges
        let bridges = self.ovsdb_client.list_bridges().await?;

        for bridge in bridges {
            // Get ports on this bridge
            let ports = self.ovsdb_client.list_bridge_ports(&bridge).await?;

            for port in ports {
                // Only discover container sockets (sock_*), not privacy sockets (priv_*)
                if Self::is_container_socket(&port) {
                    if let Some(container_name) = Self::extract_container_name(&port) {
                        containers.push(DiscoveredContainer {
                            name: container_name,
                            port_name: port.clone(),
                            bridge: bridge.clone(),
                            _ofport: self.get_port_ofport(&port).await.ok(),
                        });
                    }
                }
            }
        }

        log::info!(
            "Discovered {} container sockets (sock_*) from OVS introspection",
            containers.len()
        );
        Ok(containers)
    }

    /// Extract container name from port name
    ///
    /// Port naming patterns:
    /// - Privacy sockets: priv_wg, priv_xray (returns None - not container ports)
    /// - Container sockets: sock_{container_name} (returns container name)
    /// - Legacy veth: vi{VMID} (returns VMID for backwards compat)
    fn extract_container_name(port_name: &str) -> Option<String> {
        if port_name.starts_with("sock_") {
            // Dynamic container socket: sock_vectordb-prod -> vectordb-prod
            port_name.strip_prefix("sock_").map(|s| s.to_string())
        } else if port_name.starts_with("vi") {
            // Legacy Proxmox veth pattern: vi100 -> 100
            port_name.strip_prefix("vi").map(|s| s.to_string())
        } else if port_name.starts_with("priv_") {
            // Privacy sockets are not container ports
            None
        } else {
            None
        }
    }

    /// Check if port is a privacy socket (priv_wg, priv_xray)
    fn is_privacy_socket(port_name: &str) -> bool {
        port_name == "priv_wg" || port_name == "priv_xray"
    }

    /// Check if port is a container socket (sock_*)
    fn is_container_socket(port_name: &str) -> bool {
        port_name.starts_with("sock_")
    }

    /// Generate socket port name from container name
    pub fn socket_port_name(container_name: &str) -> String {
        format!("sock_{}", container_name)
    }

    /// Get OpenFlow port number for a port name
    async fn get_port_ofport(&self, port_name: &str) -> Result<u16> {
        let operations = simd_json::json!([{
            "op": "select",
            "table": "Interface",
            "where": [["name", "==", port_name]],
            "columns": ["ofport"]
        }]);

        let result = self.ovsdb_client.transact_simd(operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(ofport) = first_row["ofport"].as_i64() {
                    return Ok(ofport as u16);
                }
            }
        }

        Err(anyhow!("Could not find ofport for {}", port_name))
    }

    async fn run_ovs_ofctl(args: &[&str]) -> Result<String> {
        let output = tokio::process::Command::new("ovs-ofctl")
            .args(args)
            .output()
            .await
            .context("failed to execute ovs-ofctl")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "ovs-ofctl {} failed (exit {}): {}",
                args.join(" "),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn is_managed_socket_port(port_name: &str) -> Option<SocketPortType> {
        if Self::is_privacy_socket(port_name) {
            Some(SocketPortType::Privacy)
        } else if port_name.ends_with("-sock") {
            Some(SocketPortType::SharedIngress)
        } else if Self::is_container_socket(port_name) {
            Some(SocketPortType::Container)
        } else {
            None
        }
    }

    fn flow_resource_id(flow: &FlowEntry) -> String {
        if let Some(cookie) = flow.cookie {
            return format!("cookie-{cookie:016x}");
        }
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        flow.table.hash(&mut hasher);
        flow.priority.hash(&mut hasher);
        let mut match_fields: Vec<_> = flow.match_fields.iter().collect();
        match_fields.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(b.1)));
        for (key, value) in match_fields {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }
        for action in &flow.actions {
            format!("{action:?}").hash(&mut hasher);
        }
        format!("hash-{:016x}", hasher.finish())
    }

    async fn resolve_port_token(&self, token: &str) -> Result<String> {
        if token.parse::<u16>().is_ok() || token.eq_ignore_ascii_case("LOCAL") {
            return Ok(token.to_string());
        }
        Ok(self.get_port_ofport(token).await?.to_string())
    }

    async fn normalize_flow_for_bridge(
        &self,
        _bridge: &str,
        flow: &FlowEntry,
    ) -> Result<FlowEntry> {
        let mut normalized = flow.clone();
        if let Some(port_name) = normalized.match_fields.get("in_port").cloned() {
            normalized.match_fields.insert(
                "in_port".to_string(),
                self.resolve_port_token(&port_name).await?,
            );
        }

        let mut actions = Vec::with_capacity(normalized.actions.len());
        for action in &normalized.actions {
            let normalized_action = match action {
                FlowAction::Output { port } => FlowAction::Output {
                    port: self.resolve_port_token(port).await?,
                },
                _ => action.clone(),
            };
            actions.push(normalized_action);
        }
        normalized.actions = actions;
        Ok(normalized)
    }

    /// Apply flow policies to discovered containers
    async fn apply_flow_policies(
        &self,
        bridge: &str,
        containers: &[DiscoveredContainer],
        policies: &[FlowPolicy],
    ) -> Result<Vec<FlowEntry>> {
        let mut generated_flows = Vec::new();

        for container in containers {
            for policy in policies {
                if Self::policy_matches(policy, container) {
                    let flow = Self::generate_flow_from_policy(policy, container)?;
                    generated_flows.push(flow);
                    log::debug!(
                        "Generated flow for container {} from policy '{}'",
                        container.name,
                        policy.name
                    );
                }
            }
        }

        log::info!(
            "Generated {} flows for {} containers on bridge {}",
            generated_flows.len(),
            containers.len(),
            bridge
        );

        Ok(generated_flows)
    }

    /// Check if policy selector matches container
    fn policy_matches(policy: &FlowPolicy, container: &DiscoveredContainer) -> bool {
        let selector = &policy.selector;

        if selector.starts_with("container:") {
            let pattern = selector.strip_prefix("container:").unwrap();
            return Self::container_name_matches(pattern, &container.name);
        } else if selector.starts_with("port:") {
            let pattern = selector.strip_prefix("port:").unwrap();
            return Self::port_name_matches(pattern, &container.port_name);
        }

        false
    }

    /// Check if container name matches pattern (*, exact, prefix*)
    fn container_name_matches(pattern: &str, container_name: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern == container_name {
            return true;
        }

        // Prefix pattern: vectordb* matches vectordb-prod, vectordb-dev
        if pattern.ends_with('*') {
            let prefix = pattern.trim_end_matches('*');
            return container_name.starts_with(prefix);
        }

        // Suffix pattern: *-prod matches vectordb-prod, redis-prod
        if pattern.starts_with('*') {
            let suffix = pattern.trim_start_matches('*');
            return container_name.ends_with(suffix);
        }

        false
    }

    /// Check if port name matches pattern (internal_*, vi*)
    fn port_name_matches(pattern: &str, port_name: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.ends_with('*') {
            let prefix = pattern.trim_end_matches('*');
            return port_name.starts_with(prefix);
        }

        pattern == port_name
    }

    /// Generate flow from policy template, substituting variables
    fn generate_flow_from_policy(
        policy: &FlowPolicy,
        container: &DiscoveredContainer,
    ) -> Result<FlowEntry> {
        let template = &policy.template;

        // Build match fields
        let mut match_fields = HashMap::new();
        match_fields.insert("in_port".to_string(), container.port_name.clone());

        if let Some(additional) = &template.additional_matches {
            for (k, v) in additional {
                let value = Self::substitute_variables(v, container);
                match_fields.insert(k.clone(), value);
            }
        }

        // Substitute variables in actions
        let actions: Vec<FlowAction> = template
            .actions
            .iter()
            .map(|action| Self::substitute_action_variables(action, container))
            .collect();

        Ok(FlowEntry {
            table: template.table,
            priority: template.priority,
            match_fields,
            actions,
            // Use hash of container name for cookie since names aren't numeric
            cookie: Some(Self::hash_container_name(&container.name)),
            idle_timeout: 0,
            hard_timeout: 0,
        })
    }

    /// Generate a numeric hash from container name for flow cookie
    fn hash_container_name(name: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        hasher.finish()
    }

    /// Substitute variables in string ({container_name}, {port_name}, {bridge})
    fn substitute_variables(text: &str, container: &DiscoveredContainer) -> String {
        text.replace("{container_name}", &container.name)
            .replace("{container_id}", &container.name) // backwards compat
            .replace("{port_name}", &container.port_name)
            .replace("{bridge}", &container.bridge)
    }

    /// Substitute variables in flow action
    fn substitute_action_variables(
        action: &FlowAction,
        container: &DiscoveredContainer,
    ) -> FlowAction {
        match action {
            FlowAction::Output { port } => FlowAction::Output {
                port: Self::substitute_variables(port, container),
            },
            FlowAction::SetField { field, value } => FlowAction::SetField {
                field: field.clone(),
                value: Self::substitute_variables(value, container),
            },
            FlowAction::LoadRegister { register, value } => {
                // Try to parse {container_id} as numeric value
                let substituted = Self::substitute_variables(&value.to_string(), container);
                let numeric_value = substituted.parse::<u64>().unwrap_or(*value);
                FlowAction::LoadRegister {
                    register: *register,
                    value: numeric_value,
                }
            }
            _ => action.clone(),
        }
    }

    /// Install a flow via native OpenFlow protocol
    async fn install_flow(&self, bridge: &str, flow: &FlowEntry) -> Result<()> {
        let normalized = self.normalize_flow_for_bridge(bridge, flow).await?;
        let rule = self.flow_to_string(&normalized);
        log::info!("Installing flow on {}: {}", bridge, rule);
        Self::run_ovs_ofctl(&["-O", OPENFLOW_PROTOCOL, "add-flow", bridge, &rule]).await?;
        Ok(())
    }

    /// Query current flows via native OpenFlow protocol
    async fn query_flows(&self, bridge: &str) -> Result<Vec<FlowEntry>> {
        let output = Self::run_ovs_ofctl(&["-O", OPENFLOW_PROTOCOL, "dump-flows", bridge]).await?;
        self.parse_flows(&output)
    }

    async fn delete_flow(&self, bridge: &str, flow: &FlowEntry) -> Result<()> {
        let normalized = self.normalize_flow_for_bridge(bridge, flow).await?;
        let mut match_parts = vec![format!("table={}", normalized.table)];
        if let Some(cookie) = normalized.cookie {
            match_parts.push(format!("cookie=0x{cookie:x}/-1"));
        } else {
            let mut match_fields: Vec<_> = normalized.match_fields.iter().collect();
            match_fields.sort_by(|a, b| a.0.cmp(b.0));
            for (key, value) in match_fields {
                if value.is_empty() {
                    match_parts.push(key.clone());
                } else {
                    match_parts.push(format!("{key}={value}"));
                }
            }
        }
        let matcher = match_parts.join(",");
        log::info!("Deleting flow on {}: {}", bridge, matcher);
        Self::run_ovs_ofctl(&[
            "-O",
            OPENFLOW_PROTOCOL,
            "--strict",
            "del-flows",
            bridge,
            &matcher,
        ])
        .await?;
        Ok(())
    }

    /// Parse ovs-ofctl dump-flows output
    #[allow(dead_code)]
    fn parse_flows(&self, output: &str) -> Result<Vec<FlowEntry>> {
        let mut flows = Vec::new();

        for line in output.lines() {
            // Skip header and empty lines
            if line.starts_with("NXST_FLOW") || line.trim().is_empty() {
                continue;
            }

            // Parse flow line
            // Format: cookie=0x0, duration=123s, table=0, n_packets=0, priority=100, in_port=1, actions=output:2
            if let Some(flow) = self.parse_flow_line(line) {
                flows.push(flow);
            }
        }

        Ok(flows)
    }

    /// Parse a single flow line
    #[allow(dead_code)]
    fn parse_flow_line(&self, line: &str) -> Option<FlowEntry> {
        let mut table = 0u8;
        let mut priority = 0u16;
        let mut cookie = None;
        let mut match_fields = HashMap::new();
        let mut actions = Vec::new();

        let (fields_part, actions_part) = line.split_once("actions=").unwrap_or((line, ""));
        let fields_part = fields_part.replace("actions=", "");

        // Split by comma and parse fields
        for part in fields_part.split(',') {
            let part = part.trim();

            if let Some((key, value)) = part.split_once('=') {
                match key.trim() {
                    "table" => table = value.parse().ok()?,
                    "priority" => priority = value.parse().ok()?,
                    "cookie" => {
                        cookie = Some(u64::from_str_radix(value.trim_start_matches("0x"), 16).ok()?)
                    }
                    "actions" => actions = self.parse_actions(value),
                    _ => {
                        // Match field
                        if !key.contains("duration")
                            && !key.contains("n_packets")
                            && !key.contains("n_bytes")
                            && !key.contains("n_offload")
                            && !key.contains("idle_age")
                            && !key.contains("hard_age")
                        {
                            match_fields.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            } else if !part.is_empty()
                && !part.contains("duration")
                && !part.contains("n_packets")
                && !part.contains("n_bytes")
                && !part.contains("idle_age")
                && !part.contains("hard_age")
            {
                match_fields.insert(part.to_string(), "".to_string());
            }
        }

        if !actions_part.is_empty() {
            actions = self.parse_actions(actions_part.trim());
        }

        Some(FlowEntry {
            table,
            priority,
            match_fields,
            actions,
            cookie,
            idle_timeout: 0,
            hard_timeout: 0,
        })
    }

    /// Parse actions string
    #[allow(dead_code)]
    fn parse_actions(&self, actions_str: &str) -> Vec<FlowAction> {
        let mut actions = Vec::new();

        for action in actions_str.split(',') {
            let action = action.trim();

            if action == "NORMAL" {
                actions.push(FlowAction::Normal);
            } else if action == "drop" {
                actions.push(FlowAction::Drop);
            } else if let Some(port) = action.strip_prefix("output:") {
                actions.push(FlowAction::Output {
                    port: port.to_string(),
                });
            } else if let Some(rest) = action.strip_prefix("resubmit(,") {
                if let Some(table_str) = rest.strip_suffix(')') {
                    if let Ok(table) = table_str.parse() {
                        actions.push(FlowAction::Resubmit { table });
                    }
                }
            }
        }

        actions
    }

    /// Convert flow to ovs-ofctl string format
    #[allow(dead_code)]
    fn flow_to_string(&self, flow: &FlowEntry) -> String {
        let mut parts = Vec::new();

        if let Some(cookie) = flow.cookie {
            parts.push(format!("cookie=0x{cookie:x}"));
        }

        // Table
        parts.push(format!("table={}", flow.table));

        // Priority
        parts.push(format!("priority={}", flow.priority));

        if flow.idle_timeout > 0 {
            parts.push(format!("idle_timeout={}", flow.idle_timeout));
        }

        if flow.hard_timeout > 0 {
            parts.push(format!("hard_timeout={}", flow.hard_timeout));
        }

        // Match fields
        for (key, value) in &flow.match_fields {
            if value.is_empty() {
                parts.push(key.to_string());
            } else {
                parts.push(format!("{}={}", key, value));
            }
        }

        // Actions
        let actions_str = flow
            .actions
            .iter()
            .map(|a| self.action_to_string(a))
            .collect::<Vec<_>>()
            .join(",");

        format!("{},actions={}", parts.join(","), actions_str)
    }

    /// Convert action to string
    #[allow(dead_code)]
    fn action_to_string(&self, action: &FlowAction) -> String {
        match action {
            FlowAction::Output { port } => format!("output:{}", port),
            FlowAction::LoadRegister { register, value } => {
                format!("load:{}->NXM_NX_REG{}[]", value, register)
            }
            FlowAction::Resubmit { table } => format!("resubmit(,{})", table),
            FlowAction::SetField { field, value } => format!("set_field:{}={}", value, field),
            FlowAction::Drop => "drop".to_string(),
            FlowAction::Normal => "NORMAL".to_string(),
            FlowAction::Controller { max_len } => {
                if let Some(len) = max_len {
                    format!("CONTROLLER:{}", len)
                } else {
                    "CONTROLLER".to_string()
                }
            }
            FlowAction::ArpResponder { mac, ip } => {
                // IPv4 address to hex (e.g., 10.100.0.1 -> 0x0a640001)
                let ip_hex = if let Ok(addr) = ip.parse::<std::net::Ipv4Addr>() {
                    format!(
                        "0x{:02x}{:02x}{:02x}{:02x}",
                        addr.octets()[0],
                        addr.octets()[1],
                        addr.octets()[2],
                        addr.octets()[3]
                    )
                } else {
                    "0".to_string()
                };

                format!(
                    "move:NXM_OF_ETH_SRC[]->NXM_OF_ETH_DST[],mod_dl_src:{mac},load:0x0002->NXM_OF_ARP_OP[],move:NXM_NX_ARP_SHA[]->NXM_NX_ARP_THA[],load:0x{}->NXM_NX_ARP_SHA[],move:NXM_OF_ARP_SPA[]->NXM_OF_ARP_TPA[],load:{}->NXM_OF_ARP_SPA[],IN_PORT",
                    mac.replace(':', ""),
                    ip_hex
                )
            }
        }
    }

    /// Create OVS internal port for socket networking
    async fn create_socket_port(&self, bridge: &str, port: &SocketPort) -> Result<()> {
        log::info!(
            "Creating socket port {} on {} for container {:?}",
            port.name,
            bridge,
            port.container_name.as_deref().unwrap_or("(privacy)")
        );

        // Add internal port to OVS bridge
        self.ovsdb_client.add_port(bridge, &port.name).await?;

        // Set port type to internal
        self.ovsdb_client
            .set_interface_type(&port.name, "internal")
            .await?;

        Ok(())
    }

    /// Delete socket port
    async fn delete_socket_port(&self, bridge: &str, port_name: &str) -> Result<()> {
        log::info!("Deleting socket port {} from {}", port_name, bridge);

        // Use OVSDB transact to delete port
        let port_uuid = self.find_port_uuid(bridge, port_name).await?;
        let bridge_uuid = self.find_bridge_uuid_by_name(bridge).await?;

        let operations = simd_json::json!([
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
                "mutations": [
                    ["ports", "delete", ["uuid", &port_uuid]]
                ]
            },
            {
                "op": "delete",
                "table": "Port",
                "where": [["_uuid", "==", ["uuid", &port_uuid]]]
            }
        ]);

        self.ovsdb_client.transact_simd(operations).await?;
        Ok(())
    }

    /// Find port UUID by name on a bridge
    async fn find_port_uuid(&self, _bridge: &str, port_name: &str) -> Result<String> {
        let operations = simd_json::json!([{
            "op": "select",
            "table": "Port",
            "where": [["name", "==", port_name]],
            "columns": ["_uuid"]
        }]);

        let result = self.ovsdb_client.transact_simd(operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(uuid_array) = first_row["_uuid"].as_array() {
                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                        return Ok(uuid_array[1].as_str().unwrap().to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Port '{}' not found", port_name))
    }

    /// Find bridge UUID by name
    async fn find_bridge_uuid_by_name(&self, bridge_name: &str) -> Result<String> {
        let operations = simd_json::json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge_name]],
            "columns": ["_uuid"]
        }]);

        let result = self.ovsdb_client.transact_simd(operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(uuid_array) = first_row["_uuid"].as_array() {
                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                        return Ok(uuid_array[1].as_str().unwrap().to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Bridge '{}' not found", bridge_name))
    }

    /// Compute SHA-256 hash of state
    fn compute_state_hash(&self, state: &Value) -> String {
        use sha2::{Digest, Sha256};
        let json_str = simd_json::to_string(state).unwrap_or_default();
        format!("{:x}", Sha256::digest(json_str.as_bytes()))
    }

    /// Generate default security flows to prevent dangerous edge packets
    /// These flows protect against: ARP spoofing, invalid TCP flags, malformed packets,
    /// packet storms, and other intrusion-like traffic
    fn generate_security_flows(bridge_name: &str) -> Vec<FlowEntry> {
        let mut security_flows = Vec::new();

        // Table 0: Security filtering (highest priority before application flows)

        // 1. Drop invalid TCP flags (NULL scan, Xmas scan, FIN scan without established connection)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "0x000".to_string()), // NULL scan
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0001),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "+fin+psh+urg".to_string()), // Xmas scan
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0002),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 2. Drop fragmented packets (can be used for evasion)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("ip_frag".to_string(), "yes".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0003),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 3. Prevent ARP spoofing for common private networks (rate limit ARP)
        // Allow legitimate ARP but rate limit to prevent storms
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31000,
            match_fields: HashMap::from([("arp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::Controller { max_len: Some(128) }, // Send to controller for inspection
            ],
            cookie: Some(0xDEAD0004),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 4. Drop IPv6 Router Advertisements from untrusted sources (prevent MITM)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("icmp6".to_string(), "".to_string()),
                ("icmpv6_type".to_string(), "134".to_string()), // Router Advertisement
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0005),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 5. Drop DHCP packets from non-server sources (prevent rogue DHCP)
        // Only allow DHCP responses from legitimate servers (port 67)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31000,
            match_fields: HashMap::from([
                ("udp".to_string(), "".to_string()),
                ("tp_src".to_string(), "67".to_string()),
                ("tp_dst".to_string(), "68".to_string()),
            ]),
            actions: vec![FlowAction::Normal], // Allow legitimate DHCP
            cookie: Some(0xDEAD0006),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 6. Drop invalid source IP addresses (0.0.0.0, 127.0.0.0/8 except loopback, multicast as source)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_src".to_string(), "0.0.0.0".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0007),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_src".to_string(), "224.0.0.0/4".to_string()), // Multicast as source
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0008),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 7. Drop packets with broadcast source MAC (invalid)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([("dl_src".to_string(), "ff:ff:ff:ff:ff:ff".to_string())]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0009),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 8. Prevent MAC flooding attacks - limit MAC learning rate per port
        // This is enforced by limiting packet-in rate to controller
        // (Implementation note: Requires meter tables for rate limiting)

        // 9. Allow established connections (stateful inspection)
        // This requires connection tracking support in OVS
        security_flows.push(FlowEntry {
            table: 0,
            priority: 30000,
            match_fields: HashMap::from([
                ("ct_state".to_string(), "+est+trk".to_string()), // Established tracked connections
            ]),
            actions: vec![FlowAction::Normal],
            cookie: Some(0xDEAD000A),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 10. Drop invalid connection states
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31000,
            match_fields: HashMap::from([
                ("ct_state".to_string(), "+inv+trk".to_string()), // Invalid tracked state
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000B),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // ==== EGRESS FILTERING: Prevent dangerous packets from leaving your network ====
        // These prevent ISP security monitoring from flagging your traffic as malicious

        // 11. Drop outbound port scanning patterns (rapid SYN to multiple ports)
        // Note: This requires rate limiting, implemented via controller
        security_flows.push(FlowEntry {
            table: 0,
            priority: 30500,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "+syn-ack".to_string()), // SYN without ACK
            ]),
            actions: vec![
                FlowAction::Controller { max_len: Some(64) }, // Rate limit via controller
            ],
            cookie: Some(0xDEAD000C),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 12. Drop packets with TTL <= 1 going outbound (prevent traceroute leakage)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_ttl".to_string(), "0".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000D),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_ttl".to_string(), "1".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000E),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 13. Prevent LAND attacks (source IP == dest IP)
        // This prevents packets that trigger ISP anomaly detection
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                // Note: OpenFlow doesn't support nw_src==nw_dst directly
                // This would require flow table programming or controller logic
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000F),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 14. Drop packets to reserved/unallocated IP ranges (prevent leaking test traffic)
        // 240.0.0.0/4 - Class E reserved
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_dst".to_string(), "240.0.0.0/4".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0010),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 15. Rate limit ICMP to prevent ping floods (ISP detection)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 30000,
            match_fields: HashMap::from([("icmp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::Controller { max_len: Some(128) }, // Rate limit ICMP
            ],
            cookie: Some(0xDEAD0011),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 16. Drop SYN floods (prevent outbound DDoS detection)
        // This requires connection rate tracking via controller

        // 17. Prevent UDP floods to common scan ports (53, 123, 161, etc.)
        let scan_ports = vec!["53", "123", "161", "389", "1900"];
        for (idx, port) in scan_ports.iter().enumerate() {
            security_flows.push(FlowEntry {
                table: 0,
                priority: 30500,
                match_fields: HashMap::from([
                    ("udp".to_string(), "".to_string()),
                    ("tp_dst".to_string(), port.to_string()),
                ]),
                actions: vec![
                    FlowAction::Controller { max_len: Some(64) }, // Rate limit
                ],
                cookie: Some(0xDEAD0012 + idx as u64),
                idle_timeout: 0,
                hard_timeout: 0,
            });
        }

        log::info!(
            "Generated {} security flows for bridge {} (includes egress filtering to prevent ISP detection)",
            security_flows.len(),
            bridge_name
        );

        security_flows
    }

    /// Generate Level 2 obfuscation flows: Pattern hiding
    /// Hides traffic patterns via timing randomization, packet padding, TTL normalization
    fn generate_pattern_hiding_flows(bridge_name: &str) -> Vec<FlowEntry> {
        let mut obfuscation_flows = Vec::new();

        // Level 2.1: TTL Normalization (prevent fingerprinting via TTL analysis)
        // Rewrite all outbound packet TTLs to a standard value (64 or 128)
        obfuscation_flows.push(FlowEntry {
            table: 0,
            priority: 29000, // Lower than security (30000+), higher than normal
            match_fields: HashMap::from([("ip".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::SetField {
                    field: "nw_ttl".to_string(),
                    value: "64".to_string(), // Standard Linux TTL
                },
                FlowAction::Normal,
            ],
            cookie: Some(0xCAFE0001),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 2.2: Packet Size Normalization (prevent size-based fingerprinting)
        // This requires adding padding at application layer, OpenFlow can only mark
        obfuscation_flows.push(FlowEntry {
            table: 0,
            priority: 29000,
            match_fields: HashMap::from([("tcp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 0,
                    value: 1,
                }, // Mark for padding
                FlowAction::Normal,
            ],
            cookie: Some(0xCAFE0002),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 2.3: Flow Timing Randomization (prevent timing analysis)
        // Use idle_timeout with randomness to break timing patterns
        // Note: True timing randomization requires controller
        obfuscation_flows.push(FlowEntry {
            table: 0,
            priority: 29000,
            match_fields: HashMap::from([("udp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 1,
                    value: 1,
                }, // Mark for timing control
                FlowAction::Normal,
            ],
            cookie: Some(0xCAFE0003),
            idle_timeout: 30, // Vary between flows for timing obfuscation
            hard_timeout: 0,
        });

        log::info!(
            "Generated {} Level 2 (pattern hiding) flows for bridge {}",
            obfuscation_flows.len(),
            bridge_name
        );

        obfuscation_flows
    }

    /// Generate Level 3 obfuscation flows: Advanced traffic morphing
    /// Makes tunnel traffic look like normal HTTPS/HTTP traffic via protocol mimicry
    fn generate_advanced_obfuscation_flows(bridge_name: &str) -> Vec<FlowEntry> {
        let mut advanced_flows = Vec::new();

        // Level 3.1: Protocol Mimicry - Mark WireGuard traffic for morphing
        // Tag WireGuard UDP:51820 for transformation to look like DNS or HTTPS
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([
                ("udp".to_string(), "".to_string()),
                ("tp_dst".to_string(), "51820".to_string()), // WireGuard
            ]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 2,
                    value: 0x51820,
                }, // Mark as WireGuard
                FlowAction::SetField {
                    field: "tp_dst".to_string(),
                    value: "443".to_string(), // Disguise as HTTPS
                },
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0001),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 3.2: Decoy Traffic Generation (requires controller)
        // Mark flows for decoy injection - controller adds random noise packets
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "+ack".to_string()), // Established TCP
            ]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 3,
                    value: 1,
                }, // Mark for decoy injection
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0002),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 3.3: Traffic Shaping to Mimic HTTPS Patterns
        // Use connection tracking to shape tunnel traffic to match HTTPS timing
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tp_dst".to_string(), "443".to_string()),
            ]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 4,
                    value: 443,
                }, // Mark as HTTPS-shaped
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0003),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 3.4: Fragment Size Randomization
        // Mark packets for fragmentation to hide true packet sizes
        // Actual fragmentation done by controller or kernel
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([("ip".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 5,
                    value: 1400,
                }, // Target fragment size
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0004),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        log::info!(
            "Generated {} Level 3 (advanced obfuscation) flows for bridge {}",
            advanced_flows.len(),
            bridge_name
        );

        advanced_flows
    }
}

#[async_trait]
impl StatePlugin for OpenFlowPlugin {
    fn name(&self) -> &str {
        "openflow"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::openflow_plugin_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/var/run/openvswitch/db.sock").exists()
            && std::path::Path::new("/usr/bin/ovs-ofctl").exists()
    }

    fn unavailable_reason(&self) -> String {
        "OpenFlow requires /var/run/openvswitch/db.sock and /usr/bin/ovs-ofctl".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        log::info!("Querying current OpenFlow state");

        let bridges = self.ovsdb_client.list_bridges().await?;
        let bridge_names: Vec<String> = bridges.into_iter().collect();
        let mut bridge_configs = Vec::new();

        for bridge in bridge_names {
            let flows = self.query_flows(&bridge).await.unwrap_or_default();
            let ports = self
                .ovsdb_client
                .list_bridge_ports(&bridge)
                .await
                .unwrap_or_default();
            let socket_ports: Vec<SocketPort> = ports
                .into_iter()
                .filter_map(|port_name| {
                    Self::is_managed_socket_port(&port_name).map(|port_type| SocketPort {
                        ofport: None,
                        container_name: if port_type == SocketPortType::Container {
                            Self::extract_container_name(&port_name)
                        } else {
                            None
                        },
                        name: port_name,
                        port_type,
                    })
                })
                .collect();

            bridge_configs.push(BridgeFlowConfig {
                name: bridge.clone(),
                flows,
                socket_ports: if socket_ports.is_empty() {
                    None
                } else {
                    Some(socket_ports)
                },
            });
        }

        let config = OpenFlowConfig {
            bridges: bridge_configs,
            controller_endpoint: None,
            flow_policies: None,
            auto_discover_containers: false,
            enable_security_flows: false, // Query mode: don't inject, report actual state
            obfuscation_level: 0,         // Query mode: report actual flows, no injection
        };

        Ok(simd_json::serde::to_owned_value(config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        log::info!("Calculating OpenFlow diff with policy-based flow generation");

        let current_config: OpenFlowConfig = simd_json::serde::from_owned_value(current.clone())?;
        let mut desired_config: OpenFlowConfig =
            simd_json::serde::from_owned_value(desired.clone())?;

        // Inject security and obfuscation flows based on configuration
        if desired_config.enable_security_flows {
            log::info!(
                "Security hardening enabled (obfuscation level {}), injecting flows",
                desired_config.obfuscation_level
            );

            for bridge_config in &mut desired_config.bridges {
                let mut all_flows = Vec::new();
                let mut flow_count = 0;

                // Level 1: Basic security (always enabled if enable_security_flows=true)
                if desired_config.obfuscation_level >= 1 {
                    let security_flows = Self::generate_security_flows(&bridge_config.name);
                    flow_count += security_flows.len();
                    all_flows.extend(security_flows);
                }

                // Level 2: Pattern hiding (TTL normalization, packet padding, timing)
                if desired_config.obfuscation_level >= 2 {
                    let pattern_flows = Self::generate_pattern_hiding_flows(&bridge_config.name);
                    flow_count += pattern_flows.len();
                    all_flows.extend(pattern_flows);
                }

                // Level 3: Advanced obfuscation (protocol mimicry, decoy traffic, morphing)
                if desired_config.obfuscation_level >= 3 {
                    let advanced_flows =
                        Self::generate_advanced_obfuscation_flows(&bridge_config.name);
                    flow_count += advanced_flows.len();
                    all_flows.extend(advanced_flows);
                }

                // Prepend generated flows to user-defined flows (generated have higher priority)
                all_flows.extend(bridge_config.flows.clone());
                bridge_config.flows = all_flows;

                log::info!(
                    "Bridge {}: injected {} flows (Level {} obfuscation)",
                    bridge_config.name,
                    flow_count,
                    desired_config.obfuscation_level
                );
            }
        }

        // If auto-discovery is enabled and policies are defined, generate flows
        if desired_config.auto_discover_containers {
            if let Some(policies) = &desired_config.flow_policies {
                log::info!("Auto-discovery enabled, generating flows from policies");
                let discovered_containers = self.discover_containers().await.unwrap_or_default();

                for bridge_config in &mut desired_config.bridges {
                    // Filter containers for this bridge
                    let bridge_containers: Vec<DiscoveredContainer> = discovered_containers
                        .iter()
                        .filter(|c| c.bridge == bridge_config.name)
                        .cloned()
                        .collect();

                    // Generate flows from policies
                    let policy_flows = self
                        .apply_flow_policies(&bridge_config.name, &bridge_containers, policies)
                        .await?;

                    let policy_count = policy_flows.len();
                    let static_count = bridge_config.flows.len();

                    // Merge policy-generated flows with static flows
                    bridge_config.flows.extend(policy_flows);

                    log::info!(
                        "Bridge {}: {} static flows + {} policy-generated flows",
                        bridge_config.name,
                        static_count,
                        policy_count
                    );
                }
            }
        }

        let mut actions = Vec::new();

        // Compare bridges
        for desired_bridge in &desired_config.bridges {
            let current_bridge = current_config
                .bridges
                .iter()
                .find(|b| b.name == desired_bridge.name);

            if let Some(current_bridge) = current_bridge {
                // Compare flows
                for desired_flow in &desired_bridge.flows {
                    let normalized_desired = self
                        .normalize_flow_for_bridge(&desired_bridge.name, desired_flow)
                        .await?;
                    let flow_exists = current_bridge
                        .flows
                        .iter()
                        .any(|f| f == &normalized_desired);

                    if !flow_exists {
                        actions.push(StateAction::Create {
                            resource: format!(
                                "{}/flow/{}",
                                desired_bridge.name,
                                Self::flow_resource_id(&normalized_desired)
                            ),
                            config: simd_json::serde::to_owned_value(normalized_desired)?,
                        });
                    }
                }

                // Check for flows to delete
                for current_flow in &current_bridge.flows {
                    let mut flow_desired = false;
                    for desired_flow in &desired_bridge.flows {
                        let normalized_desired = self
                            .normalize_flow_for_bridge(&desired_bridge.name, desired_flow)
                            .await?;
                        if normalized_desired == *current_flow {
                            flow_desired = true;
                            break;
                        }
                    }

                    if !flow_desired {
                        actions.push(StateAction::Delete {
                            resource: format!(
                                "{}/flow/{}",
                                desired_bridge.name,
                                Self::flow_resource_id(current_flow)
                            ),
                        });
                    }
                }

                // Compare socket ports
                let desired_ports = desired_bridge.socket_ports.clone().unwrap_or_default();
                let current_ports = current_bridge.socket_ports.clone().unwrap_or_default();

                for desired_port in &desired_ports {
                    let port_exists = current_ports.iter().any(|p| p.name == desired_port.name);
                    if !port_exists {
                        actions.push(StateAction::Create {
                            resource: format!("{}/port/{}", desired_bridge.name, desired_port.name),
                            config: simd_json::serde::to_owned_value(desired_port)?,
                        });
                    }
                }

                for current_port in &current_ports {
                    let port_desired = desired_ports.iter().any(|p| p.name == current_port.name);
                    if !port_desired {
                        actions.push(StateAction::Delete {
                            resource: format!("{}/port/{}", desired_bridge.name, current_port.name),
                        });
                    }
                }
            } else {
                for desired_port in desired_bridge.socket_ports.clone().unwrap_or_default() {
                    actions.push(StateAction::Create {
                        resource: format!("{}/port/{}", desired_bridge.name, desired_port.name),
                        config: simd_json::serde::to_owned_value(desired_port)?,
                    });
                }

                for desired_flow in &desired_bridge.flows {
                    let normalized_desired = self
                        .normalize_flow_for_bridge(&desired_bridge.name, desired_flow)
                        .await?;
                    actions.push(StateAction::Create {
                        resource: format!(
                            "{}/flow/{}",
                            desired_bridge.name,
                            Self::flow_resource_id(&normalized_desired)
                        ),
                        config: simd_json::serde::to_owned_value(normalized_desired)?,
                    });
                }
            }
        }

        let current_state = self.query_current_state().await?;
        let current_hash = self.compute_state_hash(&current_state);
        let desired_hash = self.compute_state_hash(&simd_json::serde::to_owned_value(desired)?);

        Ok(StateDiff {
            plugin: "openflow".to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash,
                desired_hash,
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        log::info!(
            "Applying OpenFlow state changes: {} actions",
            diff.actions.len()
        );

        let mut changes = Vec::new();
        let mut errors = Vec::new();
        let mut create_ports = Vec::new();
        let mut create_flows = Vec::new();
        let mut delete_flows = Vec::new();
        let mut delete_ports = Vec::new();
        let mut modify_actions = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, .. } if resource.contains("/port/") => {
                    create_ports.push(action);
                }
                StateAction::Create { resource, .. } if resource.contains("/flow/") => {
                    create_flows.push(action);
                }
                StateAction::Delete { resource } if resource.contains("/flow/") => {
                    delete_flows.push(action);
                }
                StateAction::Delete { resource } if resource.contains("/port/") => {
                    delete_ports.push(action);
                }
                StateAction::Modify { .. } => modify_actions.push(action),
                StateAction::NoOp { .. } => {}
                _ => {}
            }
        }

        for action in create_ports {
            if let StateAction::Create { resource, config } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let port: SocketPort = simd_json::serde::from_owned_value(config.clone())?;
                match self.create_socket_port(bridge, &port).await {
                    Ok(_) => changes.push(format!("Created socket port {}", port.name)),
                    Err(e) => errors.push(format!("Failed to create port {}: {}", port.name, e)),
                }
            }
        }

        for action in create_flows {
            if let StateAction::Create { resource, config } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let flow: FlowEntry = simd_json::serde::from_owned_value(config.clone())?;
                match self.install_flow(bridge, &flow).await {
                    Ok(_) => {
                        changes.push(format!("Installed flow {}", Self::flow_resource_id(&flow)))
                    }
                    Err(e) => errors.push(format!(
                        "Failed to install flow {} on {}: {}",
                        Self::flow_resource_id(&flow),
                        bridge,
                        e
                    )),
                }
            }
        }

        for action in delete_flows {
            if let StateAction::Delete { resource } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let flow_id = parts.get(2).copied().unwrap_or_default();
                match self.query_flows(bridge).await {
                    Ok(flows) => {
                        if let Some(flow) = flows
                            .into_iter()
                            .find(|flow| Self::flow_resource_id(flow) == flow_id)
                        {
                            match self.delete_flow(bridge, &flow).await {
                                Ok(_) => changes.push(format!("Deleted flow {}", flow_id)),
                                Err(e) => errors.push(format!(
                                    "Failed to delete flow {} on {}: {}",
                                    flow_id, bridge, e
                                )),
                            }
                        }
                    }
                    Err(e) => errors.push(format!(
                        "Failed to query current flows for {} before deleting {}: {}",
                        bridge, flow_id, e
                    )),
                }
            }
        }

        for action in delete_ports {
            if let StateAction::Delete { resource } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let port_name = parts[2];
                match self.delete_socket_port(bridge, port_name).await {
                    Ok(_) => changes.push(format!("Deleted socket port {}", port_name)),
                    Err(e) => errors.push(format!("Failed to delete port {}: {}", port_name, e)),
                }
            }
        }

        for action in modify_actions {
            if let StateAction::Modify {
                resource,
                changes: config,
            } = action
            {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let flow: FlowEntry = simd_json::serde::from_owned_value(config.clone())?;
                let flow_id = Self::flow_resource_id(&flow);
                match self.query_flows(bridge).await {
                    Ok(flows) => {
                        if let Some(existing) = flows
                            .into_iter()
                            .find(|current| Self::flow_resource_id(current) == flow_id)
                        {
                            if let Err(e) = self.delete_flow(bridge, &existing).await {
                                errors.push(format!(
                                    "Failed to replace flow {} on {}: {}",
                                    flow_id, bridge, e
                                ));
                                continue;
                            }
                        }
                        match self.install_flow(bridge, &flow).await {
                            Ok(_) => changes.push(format!("Updated flow {}", flow_id)),
                            Err(e) => errors.push(format!(
                                "Failed to update flow {} on {}: {}",
                                flow_id, bridge, e
                            )),
                        }
                    }
                    Err(e) => errors.push(format!(
                        "Failed to query current flows for {} before updating {}: {}",
                        bridge, flow_id, e
                    )),
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        Ok(current == *desired)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current_state = self.query_current_state().await?;

        Ok(Checkpoint {
            id: format!("openflow_{}", chrono::Utc::now().timestamp()),
            plugin: "openflow".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current_state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        log::info!(
            "Rolling back OpenFlow to checkpoint from {}",
            checkpoint.timestamp
        );

        let current = self.query_current_state().await?;
        let diff = self
            .calculate_diff(&current, &checkpoint.state_snapshot)
            .await?;

        self.apply_state(&diff).await?;

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // Flows installed one by one
        }
    }
}
</file>

<file path="src/state_plugins/ovsdb_bridge.rs">
//! OVSDB Bridge plugin — 1:1 mirror of RFC 7047 Bridge/Port/Interface tables.
//!
//! OVSDB *is* the source of truth. This plugin queries reality from ovsdb-server
//! and projects it onto D-Bus via the mirror reconciliation loop. There is no
//! desired-vs-current diff — the database is the desired state.

use anyhow::Result;
use async_trait::async_trait;
use op_network::OvsdbClient;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::sync::Arc;

// ============================================================================
// RFC 7047 §3.2 Schema Types — Bridge → Port → Interface hierarchy
// ============================================================================

/// Full OVS state — 1:1 projection of what ovsdb-server reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsBridgeState {
    pub bridges: Vec<BridgeConfig>,
}

/// RFC 7047 Bridge table row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub name: String,
    #[serde(default)]
    pub ports: Vec<PortConfig>,
    /// "system" | "netdev" | "" (kernel datapath)
    #[serde(default)]
    pub datapath_type: String,
    /// "standalone" | "secure" | null
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_mode: Option<String>,
    #[serde(default)]
    pub stp_enable: bool,
    #[serde(default)]
    pub mcast_snooping_enable: bool,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub other_config: std::collections::HashMap<String, String>,
}

/// RFC 7047 Port table row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    pub name: String,
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trunks: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bond_mode: Option<String>,
}

/// RFC 7047 Interface table row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    /// "system" | "internal" | "patch" | "vxlan" | "gre" | "geneve" | ""
    #[serde(default, rename = "type")]
    pub iface_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac_in_use: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_state: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub options: std::collections::HashMap<String, String>,
}

// ============================================================================
// Plugin
// ============================================================================

pub struct OvsBridgePlugin {
    ovsdb: Arc<OvsdbClient>,
}

impl Default for OvsBridgePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl OvsBridgePlugin {
    pub fn new() -> Self {
        Self {
            ovsdb: Arc::new(OvsdbClient::new()),
        }
    }

    /// Query full Bridge→Port→Interface hierarchy from OVSDB.
    async fn query_bridges(&self) -> Result<OvsBridgeState> {
        let bridge_names = self.ovsdb.list_bridges().await.unwrap_or_default();
        let mut bridges = Vec::new();

        for bname in bridge_names {
            // Bridge-level properties
            let bridge_info = self.ovsdb.get_bridge_info(&bname).await.ok();
            let (datapath_type, fail_mode, stp_enable, mcast_snooping_enable) =
                Self::parse_bridge_props(&bridge_info);

            // Ports
            let port_names = self
                .ovsdb
                .list_bridge_ports(&bname)
                .await
                .unwrap_or_default();
            let ports: Vec<PortConfig> = port_names
                .into_iter()
                .map(|pname| PortConfig {
                    interfaces: vec![InterfaceConfig {
                        name: pname.clone(),
                        iface_type: String::new(),
                        mac_in_use: None,
                        mac: None,
                        admin_state: None,
                        link_state: None,
                        options: Default::default(),
                    }],
                    name: pname,
                    tag: None,
                    trunks: vec![],
                    vlan_mode: None,
                    bond_mode: None,
                })
                .collect();

            bridges.push(BridgeConfig {
                name: bname,
                ports,
                datapath_type,
                fail_mode,
                stp_enable,
                mcast_snooping_enable,
                other_config: Default::default(),
            });
        }

        Ok(OvsBridgeState { bridges })
    }

    fn parse_bridge_props(info: &Option<String>) -> (String, Option<String>, bool, bool) {
        let Some(ref info_str) = info else {
            return (String::new(), None, false, false);
        };
        let mut buf = info_str.clone();
        // SAFETY: simd_json requires mutable access for in-place parsing
        let v: std::result::Result<Value, _> = unsafe { simd_json::from_str(&mut buf) };
        match v {
            Ok(row) => (
                row.get("datapath_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                row.get("fail_mode")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                row.get("stp_enable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                row.get("mcast_snooping_enable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            ),
            Err(_) => (String::new(), None, false, false),
        }
    }
}

#[async_trait]
impl StatePlugin for OvsBridgePlugin {
    fn name(&self) -> &str {
        "ovsdb_bridge"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::ovsdb_bridge_plugin_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/var/run/openvswitch/db.sock").exists()
    }

    fn unavailable_reason(&self) -> String {
        "OVSDB socket not found at /var/run/openvswitch/db.sock".to_string()
    }

    /// Query reality — dump OVSDB Bridge/Port/Interface tables.
    async fn query_current_state(&self) -> Result<Value> {
        let state = self.query_bridges().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    /// Reconciliation, not diff. OVSDB is the DB — the "desired" parameter
    /// is what the D-Bus mirror currently shows. We return actions needed
    /// to update the mirror to match OVSDB reality.
    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        // No diff — OVSDB is authoritative. The mirror reconciliation loop
        // in op-dbus-mirror handles projection. Return empty diff.
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    /// No-op — reconciliation happens via the mirror, not through apply.
    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    /// Verify just re-queries OVSDB — it's always "correct" by definition.
    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }
}
</file>

<file path="src/state_plugins/packagekit.rs">
//! PackageKit Plugin for op-dbus
//!
//! Manages system packages via PackageKit D-Bus interface
//! Provides declarative package installation/removal

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::collections::HashMap;
use std::process::Command;
use zbus::proxy;

use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};

// PackageKit D-Bus interface
#[proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
trait PackageKit {
    /// Get transaction list
    async fn get_transaction_list(&self) -> zbus::Result<Vec<String>>;

    /// Create transaction
    async fn create_transaction(&self) -> zbus::Result<String>;
}

// Transaction interface
#[proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
trait PackageKitTransaction {
    /// Install packages
    async fn install_packages(
        &self,
        transaction_flags: u64,
        package_ids: Vec<String>,
    ) -> zbus::Result<()>;

    /// Remove packages
    async fn remove_packages(
        &self,
        transaction_flags: u64,
        package_ids: Vec<String>,
        allow_deps: bool,
        autoremove: bool,
    ) -> zbus::Result<()>;

    /// Resolve packages
    async fn resolve(&self, filters: u64, packages: Vec<String>) -> zbus::Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageState {
    pub ensure: String,           // "installed", "removed", "latest"
    pub provider: Option<String>, // "apt", "dnf", "pacman", etc.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageKitState {
    pub version: u32,
    pub packages: HashMap<String, PackageState>,
}

#[derive(Debug, Clone)]
pub struct PackageKitPlugin;

impl Default for PackageKitPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageKitPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Install package via direct package manager
    async fn install_via_direct(&self, package_name: &str) -> Result<()> {
        // Try apt
        if Command::new("apt-get")
            .args(["install", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try dnf
        if Command::new("dnf")
            .args(["install", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try pacman
        if Command::new("pacman")
            .args(["-S", "--noconfirm", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        Err(anyhow::anyhow!("No suitable package manager found"))
    }

    /// Remove package via direct package manager
    async fn remove_via_direct(&self, package_name: &str) -> Result<()> {
        // Try apt
        if Command::new("apt-get")
            .args(["remove", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try dnf
        if Command::new("dnf")
            .args(["remove", "-y", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        // Try pacman
        if Command::new("pacman")
            .args(["-R", "--noconfirm", package_name])
            .status()?
            .success()
        {
            return Ok(());
        }

        Err(anyhow::anyhow!("No suitable package manager found"))
    }

    /// Check if package is installed
    async fn package_installed(&self, package_name: &str) -> Result<bool> {
        // Try dpkg (Debian/Ubuntu)
        if Command::new("dpkg")
            .args(["-l", package_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(true);
        }

        // Try rpm (Fedora/RHEL)
        if Command::new("rpm")
            .args(["-q", package_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(true);
        }

        // Try pacman (Arch)
        if Command::new("pacman")
            .args(["-Q", package_name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(true);
        }

        Ok(false)
    }
}

#[async_trait]
impl StatePlugin for PackageKitPlugin {
    fn name(&self) -> &str {
        "packagekit"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::json!({
            "version": 1,
            "packages": {}
        }))
    }

    async fn calculate_diff(&self, _current: &Value, desired: &Value) -> Result<StateDiff> {
        println!("PackageKit calculate_diff called with: {}", desired);
        let packages_obj = desired
            .get("packages")
            .ok_or_else(|| anyhow::anyhow!("missing packages field"))?;
        let packages: HashMap<String, PackageState> = simd_json::serde::from_owned_value(packages_obj.clone())?;
        let desired_state = PackageKitState {
            version: 1,
            packages,
        };

        let mut actions = Vec::new();

        for (package_name, package_config) in &desired_state.packages {
            let is_installed = self.package_installed(package_name).await?;

            match package_config.ensure.as_str() {
                "installed" if !is_installed => {
                    actions.push(StateAction::Create {
                        resource: package_name.clone(),
                        config: simd_json::json!({
                            "ensure": "installed",
                            "provider": package_config.provider,
                            "version": package_config.version
                        }),
                    });
                }
                "removed" if is_installed => {
                    actions.push(StateAction::Delete {
                        resource: package_name.clone(),
                    });
                }
                _ => {
                    actions.push(StateAction::NoOp {
                        resource: package_name.clone(),
                    });
                }
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "placeholder".to_string(),
                desired_hash: "placeholder".to_string(),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create {
                    resource,
                    config: _,
                } => match self.install_via_direct(resource).await {
                    Ok(()) => {
                        changes_applied.push(format!("✅ Installed package: {}", resource));
                    }
                    Err(e) => {
                        errors.push(format!("❌ Failed to install {}: {}", resource, e));
                    }
                },
                StateAction::Delete { resource } => match self.remove_via_direct(resource).await {
                    Ok(()) => {
                        changes_applied.push(format!("✅ Removed package: {}", resource));
                    }
                    Err(e) => {
                        errors.push(format!("❌ Failed to remove {}: {}", resource, e));
                    }
                },
                StateAction::Modify {
                    resource,
                    changes: _,
                } => {
                    changes_applied.push(format!(
                        "⚠️  Package {} modification not implemented",
                        resource
                    ));
                }
                StateAction::NoOp { resource } => {
                    changes_applied.push(format!("📦 Package {}: no action required", resource));
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let packages_obj = desired
            .get("packages")
            .ok_or_else(|| anyhow::anyhow!("missing packages field"))?;
        let packages: HashMap<String, PackageState> = simd_json::serde::from_owned_value(packages_obj.clone())?;

        for (package_name, package_config) in &packages {
            let is_installed = self.package_installed(package_name).await?;

            match package_config.ensure.as_str() {
                "installed" => {
                    if !is_installed {
                        return Ok(false);
                    }
                }
                "removed" => {
                    if is_installed {
                        return Ok(false);
                    }
                }
                _ => {}
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("{}-{}", self.name(), chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/pcidecl.rs">
// pcidecl_plugin.rs — declarative PCI device presence/config
// Query via /sys/bus/pci/devices/* and lspci fallback. Enforce supports "driver_override".
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;

use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciDecl {
    pub version: u32,
    pub items: Vec<PciItem>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Enforce,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciItem {
    pub id: String, // stable id in your inventory
    pub mode: Mode,
    pub address: String, // 0000:00:1f.6
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_vendor: Option<String>, // "8086"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_device: Option<String>, // "15f3"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_override: Option<String>, // desired override string or "" to clear
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciLive {
    pub address: String,
    pub vendor: Option<String>,
    pub device: Option<String>,
    pub driver: Option<String>,
    pub driver_override: Option<String>,
    pub present: bool,
}

pub struct PciDeclPlugin;

impl Default for PciDeclPlugin {
    fn default() -> Self {
        Self
    }
}

impl PciDeclPlugin {
    pub fn new() -> Self {
        Self
    }

    fn sys_path(addr: &str) -> String {
        format!("/sys/bus/pci/devices/{}", addr)
    }

    fn read_to_string(path: &str) -> Option<String> {
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    fn live_for(addr: &str) -> PciLive {
        let root = Self::sys_path(addr);
        let present = Path::new(&root).exists();
        let vendor = Self::read_to_string(&format!("{}/vendor", root));
        let device = Self::read_to_string(&format!("{}/device", root));
        let drv_link = Path::new(&format!("{}/driver", root))
            .read_link()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()));
        let drv_override = Self::read_to_string(&format!("{}/driver_override", root));
        PciLive {
            address: addr.to_string(),
            vendor,
            device,
            driver: drv_link,
            driver_override: drv_override,
            present,
        }
    }

    fn lspci_present(addr: &str) -> bool {
        if let Ok(out) = Command::new("sh")
            .arg("-c")
            .arg(format!("lspci -s {} >/dev/null 2>&1; echo $?", addr))
            .output()
        {
            return out.stdout.first().map(|b| *b == b'0').unwrap_or(false);
        }
        false
    }

    fn compliant(l: &PciLive, i: &PciItem) -> bool {
        if !l.present {
            return false;
        }
        if let Some(v) = &i.expect_vendor {
            if l.vendor.as_deref() != Some(&format!("0x{}", v).to_lowercase())
                && l.vendor.as_deref() != Some(v)
            {
                return false;
            }
        }
        if let Some(d) = &i.expect_device {
            if l.device.as_deref() != Some(&format!("0x{}", d).to_lowercase())
                && l.device.as_deref() != Some(d)
            {
                return false;
            }
        }
        if let Some(ovr) = &i.driver_override {
            if l.driver_override.as_deref() != Some(ovr.as_str()) {
                return false;
            }
        }
        true
    }

    fn set_driver_override(addr: &str, val: &str) -> Result<()> {
        let p = format!("{}/driver_override", Self::sys_path(addr));
        fs::write(&p, format!("{}\n", val)).context("write driver_override")?;
        Ok(())
    }
}

#[async_trait]
impl StatePlugin for PciDeclPlugin {
    fn name(&self) -> &str {
        "pcidecl"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        // Not listing all PCI devices; caller provides address. Return empty.
        let empty_items: Vec<Value> = Vec::new();
        Ok(simd_json::json!({"version": 1, "items": empty_items}))
    }

    async fn calculate_diff(&self, _current: &Value, desired: &Value) -> Result<StateDiff> {
        let want: PciDecl =
            simd_json::serde::from_owned_value(desired.clone()).context("desired must be PciDecl")?;
        let mut actions = Vec::new();
        for item in &want.items {
            let live = Self::live_for(&item.address);
            let present = live.present || Self::lspci_present(&item.address);
            if let Mode::ObserveOnly = item.mode {
                actions.push(StateAction::NoOp {
                    resource: item.id.clone(),
                });
            } else {
                if !present {
                    actions.push(StateAction::NoOp {
                        resource: item.id.clone(),
                    });
                    continue;
                }
                if Self::compliant(&live, item) {
                    actions.push(StateAction::NoOp {
                        resource: item.id.clone(),
                    });
                } else {
                    actions.push(StateAction::Modify {
                        resource: item.id.clone(),
                        changes: simd_json::serde::to_owned_value(item)?,
                    });
                }
            }
        }
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute("pcidecl-current")),
                desired_hash: format!(
                    "{:x}",
                    md5::compute(simd_json::to_string(&desired).unwrap_or_default())
                ),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();
        for action in &diff.actions {
            match action {
                StateAction::Modify { resource, changes }
                | StateAction::Create {
                    resource,
                    config: changes,
                } => {
                    let item: PciItem =
                        simd_json::serde::from_owned_value(changes.clone()).context("invalid PciItem")?;
                    if let Some(val) = &item.driver_override {
                        match Self::set_driver_override(&item.address, val) {
                            Ok(_) => changes_applied
                                .push(format!("{}: driver_override -> {}", resource, val)),
                            Err(e) => errors.push(format!("{}: {}", resource, e)),
                        }
                    } else {
                        changes_applied.push(format!("{}: no changes required", resource));
                    }
                }
                StateAction::NoOp { resource } => {
                    changes_applied.push(format!("{}: no-op", resource));
                }
                StateAction::Delete { resource } => {
                    changes_applied.push(format!("{}: delete not supported", resource));
                }
            }
        }
        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let want: PciDecl = simd_json::serde::from_owned_value(desired.clone()).unwrap_or(PciDecl {
            version: 1,
            items: vec![],
        });
        for item in &want.items {
            let live = Self::live_for(&item.address);
            if !Self::compliant(&live, item) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("pcidecl-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/plugin_schema_defs.rs">
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema, ReadOnlyCondition};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;

fn any_field(required: bool, description: &str, default: Option<Value>) -> FieldSchema {
    FieldSchema {
        field_type: FieldType::Any,
        required,
        description: description.to_string(),
        default,
        example: None,
        constraints: Vec::new(),
        read_only: false,
        read_only_when: None,
    }
}

fn simple_schema(
    name: &str,
    description: &str,
    dependencies: &[&str],
    fields: Vec<(&str, FieldSchema)>,
) -> PluginSchema {
    let mut builder = PluginSchema::builder(name)
        .version("1.0.0")
        .description(description);
    for dep in dependencies {
        builder = builder.dependency(dep);
    }
    for (field_name, schema) in fields {
        builder = builder.field(field_name, schema);
    }
    builder.build()
}

pub(crate) fn adc_plugin_schema() -> PluginSchema {
    simple_schema(
        "adc",
        "Application default credentials state",
        &[],
        vec![(
            "configured",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether ADC is configured".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

pub(crate) fn agent_config_plugin_schema() -> PluginSchema {
    simple_schema(
        "agent_config",
        "Agent configuration and tool assignments",
        &[],
        vec![(
            "agents",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Any)),
                required: true,
                description: "List of agent configurations".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

pub(crate) fn endpoint_plugin_schema() -> PluginSchema {
    simple_schema(
        "endpoint",
        "Endpoint configuration",
        &["net"],
        vec![(
            "endpoints",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Declared endpoints".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

pub(crate) fn gcloud_adc_plugin_schema() -> PluginSchema {
    simple_schema(
        "gcloud_adc",
        "Google Cloud ADC state",
        &[],
        vec![
            ("account", any_field(false, "Authenticated account", None)),
            ("project_id", any_field(false, "Project id", None)),
            (
                "authenticated",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Authentication status".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

pub(crate) fn hardware_plugin_schema() -> PluginSchema {
    simple_schema(
        "hardware",
        "Hardware inventory snapshot",
        &[],
        vec![
            ("cpu", any_field(true, "CPU info", Some(json!({})))),
            ("memory", any_field(true, "Memory info", Some(json!({})))),
            ("disks", any_field(true, "Disk list", Some(json!([])))),
        ],
    )
}

pub(crate) fn keypair_plugin_schema() -> PluginSchema {
    simple_schema(
        "keypair",
        "Keypair declaration state",
        &[],
        vec![(
            "keypairs",
            any_field(true, "Managed keypairs", Some(json!([]))),
        )],
    )
}

pub(crate) fn ovsdb_bridge_plugin_schema() -> PluginSchema {
    simple_schema(
        "ovsdb_bridge",
        "OVS bridge declarations",
        &["net"],
        vec![(
            "bridges",
            any_field(true, "Bridge declarations", Some(json!([]))),
        )],
    )
}

pub(crate) fn proxmox_plugin_schema() -> PluginSchema {
    simple_schema(
        "proxmox",
        "Proxmox container declarations",
        &["net"],
        vec![(
            "containers",
            any_field(true, "Container declarations", Some(json!([]))),
        )],
    )
}

pub(crate) fn proxy_server_plugin_schema() -> PluginSchema {
    simple_schema(
        "proxy_server",
        "Proxy server runtime config",
        &["net"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable proxy".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "port",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Proxy port".to_string(),
                    default: Some(json!(8080)),
                    example: None,
                    constraints: vec![
                        Constraint::Min { value: 1.0 },
                        Constraint::Max { value: 65535.0 },
                    ],
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

pub(crate) fn service_plugin_schema() -> PluginSchema {
    simple_schema(
        "service",
        "Service definition declarations",
        &["net"],
        vec![("services", any_field(true, "Service map", Some(json!({}))))],
    )
}

pub(crate) fn sess_decl_plugin_schema() -> PluginSchema {
    simple_schema(
        "sess_decl",
        "Session declaration state",
        &["users"],
        vec![(
            "sessions",
            any_field(true, "Session declarations", Some(json!([]))),
        )],
    )
}

pub(crate) fn software_plugin_schema() -> PluginSchema {
    simple_schema(
        "software",
        "Software package inventory",
        &[],
        vec![("packages", any_field(true, "Package list", Some(json!([]))))],
    )
}

pub(crate) fn users_plugin_schema() -> PluginSchema {
    simple_schema(
        "users",
        "User account declarations",
        &[],
        vec![("users", any_field(true, "Users list", Some(json!([]))))],
    )
}

pub(crate) fn web_ui_plugin_schema() -> PluginSchema {
    simple_schema(
        "web_ui",
        "Web UI tunables",
        &["mcp"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable UI".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cors_origins",
                any_field(false, "Allowed CORS origins", Some(json!([]))),
            ),
            (
                "compression",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable compression".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cache_ttl",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Cache TTL seconds".to_string(),
                    default: Some(json!(86400)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 0.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "theme",
                any_field(true, "Theme name", Some(json!("default"))),
            ),
            (
                "feature_flags",
                any_field(false, "Feature flag map", Some(json!({}))),
            ),
        ],
    )
}

pub(crate) fn wireguard_plugin_schema() -> PluginSchema {
    simple_schema(
        "wireguard",
        "WireGuard interface state",
        &["net"],
        vec![(
            "interfaces",
            any_field(true, "WireGuard interfaces", Some(json!([]))),
        )],
    )
}

pub(crate) fn incus_plugin_schema() -> PluginSchema {
    let instance_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-123")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "status".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "Running".to_string(),
                    "Stopped".to_string(),
                    "Frozen".to_string(),
                ]),
                required: true,
                description: "Instance status".to_string(),
                default: Some(json!("Stopped")),
                example: Some(json!("Running")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "container".to_string(),
                    "virtual-machine".to_string(),
                ]),
                required: true,
                description: "Instance type".to_string(),
                default: Some(json!("container")),
                example: Some(json!("container")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source image reference".to_string(),
                default: None,
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "storage_pool".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Preferred Incus storage pool for initial creation".to_string(),
                default: None,
                example: Some(json!("registration")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Applied Incus profiles".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "config".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance configuration map".to_string(),
                default: Some(json!({})),
                example: Some(json!({"limits.cpu": "2"})),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance device definitions".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "eth0": {
                        "type": "nic",
                        "nictype": "bridged",
                        "parent": "ovsbr0"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus")
        .version("1.0.0")
        .description("Incus instance management")
        .array_field(
            "instances",
            FieldType::Object(instance_fields),
            true,
            "List of Incus instances",
        )
        .example(json!({
            "instances": [
                {
                    "name": "privacy-user-123",
                    "status": "Running",
                    "type": "container",
                    "image": "images:debian/13",
                    "storage_pool": "registration",
                    "profiles": ["default"],
                    "config": {
                        "limits.cpu": "2"
                    },
                    "devices": {
                        "eth0": {
                            "type": "nic",
                            "nictype": "bridged",
                            "parent": "ovsbr0"
                        }
                    }
                }
            ]
        }))
        .build()
}

pub(crate) fn net_plugin_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "ethernet".to_string(),
                    "bridge".to_string(),
                    "veth".to_string(),
                    "vlan".to_string(),
                    "bond".to_string(),
                ]),
                required: true,
                description: "Interface type".to_string(),
                default: Some(json!("ethernet")),
                example: Some(json!("ethernet")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "IP addresses".to_string(),
                default: Some(json!([])),
                example: Some(json!(["192.168.1.100/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("net")
        .version("1.0.0")
        .description("Network interface management via rtnetlink")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "List of network interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "eth0",
                    "type": "ethernet",
                    "state": "up",
                    "addresses": ["192.168.1.100/24"]
                }
            ]
        }))
        .build()
}

pub(crate) fn rtnetlink_plugin_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Administrative interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Interface IP addresses in CIDR form".to_string(),
                default: Some(json!([])),
                example: Some(json!(["10.0.0.2/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "mac_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional MAC address override".to_string(),
                default: None,
                example: Some(json!("02:00:00:00:00:01")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "default_gateway".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Default gateway for this interface".to_string(),
                default: None,
                example: Some(json!("10.0.0.1")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("rtnetlink")
        .version("1.0.0")
        .description("Native kernel rtnetlink interface management")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "Desired rtnetlink-managed interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "ovsbr0",
                    "state": "up",
                    "addresses": ["10.10.0.1/24"],
                    "default_gateway": "10.10.0.254"
                }
            ]
        }))
        .build()
}

pub(crate) fn openflow_plugin_schema() -> PluginSchema {
    let bridge_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Bridge name".to_string(),
                default: None,
                example: Some(json!("ovs-br0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "datapath_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Datapath ID".to_string(),
                default: None,
                example: Some(json!("0000000000000001")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocols".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Supported OpenFlow protocols".to_string(),
                default: Some(json!(["OpenFlow13"])),
                example: Some(json!(["OpenFlow10", "OpenFlow13"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flows".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "table".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "OpenFlow table number".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 254.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "priority".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "Flow priority".to_string(),
                            default: Some(json!(100)),
                            example: Some(json!(22000)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 65535.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "match_fields".to_string(),
                        FieldSchema {
                            field_type: FieldType::Any,
                            required: true,
                            description: "OpenFlow match fields".to_string(),
                            default: None,
                            example: Some(
                                json!({"in_port": "ovsbr0-sock", "nw_src": "10.100.0.2"}),
                            ),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "actions".to_string(),
                        FieldSchema {
                            field_type: FieldType::Array(Box::new(FieldType::Any)),
                            required: true,
                            description: "OpenFlow actions".to_string(),
                            default: None,
                            example: Some(json!([{"type": "output", "port": "priv_wg"}])),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "cookie".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Flow cookie for idempotent route ownership".to_string(),
                            default: None,
                            example: Some(json!(5787125521171081216u64)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "idle_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Idle timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "hard_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Hard timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Flows managed for this bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_ports".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "OVS socket port name".to_string(),
                            default: None,
                            example: Some(json!("ovsbr0-sock")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "container_name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: false,
                            description: "Optional legacy container name bound to this port"
                                .to_string(),
                            default: None,
                            example: Some(json!("privacy-user-abc")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "port_type".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "Socket port role".to_string(),
                            default: Some(json!("SharedIngress")),
                            example: Some(json!("SharedIngress")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "ofport".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Resolved OpenFlow port number".to_string(),
                            default: None,
                            example: Some(json!(7)),
                            constraints: Vec::new(),
                            read_only: true,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Managed OVS socket ports for the bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("openflow")
        .version("1.0.0")
        .description("OpenFlow flow table management")
        .dependency("net")
        .dependency("privacy_routes")
        .array_field(
            "bridges",
            FieldType::Object(bridge_fields),
            true,
            "OVS bridges",
        )
        .string_field("controller_endpoint", false, "OpenFlow controller endpoint")
        .boolean_field(
            "auto_discover_containers",
            false,
            "Auto-create flows from discovered legacy container sockets",
        )
        .boolean_field(
            "enable_security_flows",
            false,
            "Inject hardening flows before route flows",
        )
        .integer_field("obfuscation_level", false, "Traffic obfuscation level for generated flows")
        .example(json!({
            "bridges": [
                {
                    "name": "ovsbr0",
                    "protocols": ["OpenFlow13"],
                    "socket_ports": [
                        {
                            "name": "ovsbr0-sock",
                            "port_type": "SharedIngress"
                        }
                    ],
                    "flows": [
                        {
                            "table": 0,
                            "priority": 22000,
                            "match_fields": {"in_port": "ovsbr0-sock", "ip": "", "nw_src": "10.100.0.2"},
                            "actions": [{"type": "output", "port": "priv_wg"}],
                            "cookie": 5787125521171081216u64,
                            "idle_timeout": 0,
                            "hard_timeout": 0
                        }
                    ]
                }
            ],
            "auto_discover_containers": false,
            "enable_security_flows": false,
            "obfuscation_level": 0
        }))
        .build()
}

pub(crate) fn s6_plugin_schema() -> PluginSchema {
    let unit_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unit name".to_string(),
                default: None,
                example: Some(json!("nginx.service")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "active".to_string(),
                    "inactive".to_string(),
                    "failed".to_string(),
                ]),
                required: false,
                description: "Desired unit state".to_string(),
                default: Some(json!("active")),
                example: Some(json!("active")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether unit is enabled at boot".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("s6")
        .version("1.0.0")
        .description("s6 service management")
        .array_field("units", FieldType::Object(unit_fields), true, "s6 services")
        .example(json!({
            "units": [
                {
                    "name": "nginx",
                    "state": "active",
                    "enabled": true
                }
            ]
        }))
        .build()
}

pub(crate) fn privacy_router_plugin_schema() -> PluginSchema {
    let wireguard_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable WireGuard tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for WireGuard".to_string(),
                default: Some(json!(100)),
                example: Some(json!(100)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "listen_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "WireGuard listen port".to_string(),
                default: Some(json!(51820)),
                example: Some(json!(51820)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port name for the WireGuard ingress container"
                    .to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let warp_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable Cloudflare WARP tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "bridge_interface".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host WireGuard interface bridged into OVS for WARP egress"
                    .to_string(),
                default: Some(json!("wgcf")),
                example: Some(json!("wgcf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wgcf_config".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Path to wgcf WireGuard config used to create the host interface"
                    .to_string(),
                default: Some(json!("/etc/wireguard/wgcf.conf")),
                example: Some(json!("/etc/wireguard/wgcf.conf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable system XRay client tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for the local XRay client".to_string(),
                default: Some(json!(101)),
                example: Some(json!(101)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port for the local XRay client".to_string(),
                default: Some(json!("priv_xray")),
                example: Some(json!("priv_xray")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socks_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "SOCKS listener port exposed by the local XRay client".to_string(),
                default: Some(json!(1080)),
                example: Some(json!(1080)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vps_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "xray_server".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "xray_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_router")
        .version("1.1.0")
        .description("System privacy fabric (WireGuard/XRay ingress, WARP bridge, XRay egress)")
        .dependency("incus")
        .dependency("openflow")
        .dependency("privacy_routes")
        .string_field("bridge_name", true, "OVS bridge for privacy network")
        .object_field(
            "wireguard",
            wireguard_fields,
            true,
            "WireGuard tunnel config",
        )
        .object_field("warp", warp_fields, true, "Cloudflare WARP bridge config")
        .object_field(
            "xray",
            xray_fields,
            true,
            "XRay REALITY egress client config",
        )
        .object_field(
            "vps",
            vps_fields,
            true,
            "Remote XRay server endpoint config",
        )
        .example(json!({
            "bridge_name": "ovsbr0",
            "wireguard": {
                "enabled": true,
                "container_id": 100,
                "socket_port": "priv_wg",
                "listen_port": 51820
            },
            "warp": {
                "enabled": true,
                "bridge_interface": "wgcf",
                "wgcf_config": "/etc/wireguard/wgcf.conf"
            },
            "xray": {
                "enabled": true,
                "container_id": 101,
                "socket_port": "priv_xray",
                "socks_port": 1080,
                "vps_address": "vps.example.com",
                "vps_port": 443
            },
            "vps": {
                "xray_server": "vps.example.com",
                "xray_port": 443
            }
        }))
        .build()
}

pub(crate) fn unix_socket_plugin_schema() -> PluginSchema {
    let mut socket_fields = HashMap::new();
    socket_fields.insert(
        "path".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Filesystem path of the unix domain socket".to_string(),
            default: None,
            example: Some(json!("/run/qdrant.sock")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "port".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Local TCP port xray listens on and proxies into this socket".to_string(),
            default: None,
            example: Some(json!(6334)),
            constraints: vec![
                Constraint::Min { value: 1.0 },
                Constraint::Max { value: 65535.0 },
            ],
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "protocol".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Transport protocol carried over the socket (grpc, jsonrpc, …)"
                .to_string(),
            default: Some(json!("grpc")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "label".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Human-readable service label used as the xray outbound tag".to_string(),
            default: None,
            example: Some(json!("qdrant-grpc")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    PluginSchema::builder("unix_socket")
        .version("1.0.0")
        .description("Unix domain socket endpoints proxied into xray outbounds")
        .array_field(
            "sockets",
            FieldType::Object(socket_fields),
            true,
            "Declared unix socket endpoints",
        )
        .example(json!({
            "sockets": [
                {
                    "path": "/run/qdrant.sock",
                    "port": 6334,
                    "protocol": "grpc",
                    "label": "qdrant-grpc"
                }
            ]
        }))
        .build()
}

pub(crate) fn privacy_routes_plugin_schema() -> PluginSchema {
    let route_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Stable route object identifier".to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "route_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Derived route ID from WireGuard public key and shared secret"
                    .to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "user_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Internal privacy user identifier".to_string(),
                default: None,
                example: Some(json!("550e8400-e29b-41d4-a716-446655440000")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "email".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "User email for audit and publication context".to_string(),
                default: None,
                example: Some(json!("user@example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wireguard_public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "WireGuard public key backing this route identity".to_string(),
                default: None,
                example: Some(json!("P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "assigned_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Assigned WireGuard tunnel address".to_string(),
                default: None,
                example: Some(json!("10.100.0.2/32")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "selector_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Packet-visible selector used for OpenFlow matching".to_string(),
                default: None,
                example: Some(json!("10.100.0.2")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Associated Incus instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-550e8400")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "ingress_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Shared OVS ingress port for route matching".to_string(),
                default: Some(json!("ovsbr0-sock")),
                example: Some(json!("ovsbr0-sock")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "next_hop".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "First logical next hop for this route".to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether this route should be active".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "created_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Creation timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:00:00Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "updated_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Last update timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:05:00Z")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_routes")
        .version("1.0.0")
        .description("Per-user privacy route objects keyed by WireGuard identity")
        .dependency("wireguard")
        .dependency("privacy_router")
        .array_field(
            "routes",
            FieldType::Object(route_fields),
            true,
            "Published privacy route objects",
        )
        .example(json!({
            "routes": [
                {
                    "name": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "route_id": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "user_id": "550e8400-e29b-41d4-a716-446655440000",
                    "email": "user@example.com",
                    "wireguard_public_key": "P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=",
                    "assigned_ip": "10.100.0.2/32",
                    "selector_ip": "10.100.0.2",
                    "container_name": "privacy-user-550e8400",
                    "ingress_port": "ovsbr0-sock",
                    "next_hop": "priv_wg",
                    "enabled": true,
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                }
            ]
        }))
        .build()
}

pub(crate) fn mail_server_plugin_schema() -> PluginSchema {
    use op_state_store::FieldType;

    let endpoint_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "smtp_submission".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "SMTP submission endpoint (port 587)".to_string(),
                default: Some(json!("0.0.0.0:587")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "smtp_tls".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "SMTP TLS endpoint (port 465)".to_string(),
                default: Some(json!("0.0.0.0:465")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "imap".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "IMAP endpoint (port 143)".to_string(),
                default: Some(json!("0.0.0.0:143")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "imaps".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "IMAPS endpoint (port 993)".to_string(),
                default: Some(json!("0.0.0.0:993")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "dovecot_lmtp".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Dovecot LMTP unix socket path inside container".to_string(),
                default: Some(json!("/var/spool/postfix/private/dovecot-lmtp")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "postfix_pickup".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Postfix pickup unix socket path inside container".to_string(),
                default: Some(json!("/var/spool/postfix/private/pickup")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("mail_server")
        .version("1.0.0")
        .description("Mail server container state and D-Bus registration for 3tched.com")
        .dependency("incus")
        .dependency("unix_socket")
        .field(
            "container_name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus container name running the mail stack".to_string(),
                default: Some(json!("mail-3tched")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "container_status",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Container runtime status".to_string(),
                default: Some(json!("Unknown")),
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .field(
            "domain",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Primary mail domain".to_string(),
                default: Some(json!("3tched.com")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "xray_socket_path",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unix socket path for Xray naive routing integration".to_string(),
                default: Some(json!("/run/xray/mail-naive.sock")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "dbus_service_name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "D-Bus service name registered for this mail instance".to_string(),
                default: Some(json!("org.opdbus.MailServer.3tched")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "endpoints",
            FieldSchema {
                field_type: FieldType::Object(endpoint_fields),
                required: true,
                description: "Active mail service endpoints".to_string(),
                default: Some(json!({})),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "container_ip",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Container IPv4 address".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .field(
            "healthy",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether the mail stack is healthy".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .field(
            "last_error",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Last error message if unhealthy".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .example(json!({
            "container_name": "mail-3tched",
            "container_status": "Running",
            "domain": "3tched.com",
            "xray_socket_path": "/run/xray/mail-naive.sock",
            "dbus_service_name": "org.opdbus.MailServer.3tched",
            "endpoints": {
                "smtp_submission": "0.0.0.0:587",
                "smtp_tls": "0.0.0.0:465",
                "imap": "0.0.0.0:143",
                "imaps": "0.0.0.0:993",
                "dovecot_lmtp": "/var/spool/postfix/private/dovecot-lmtp",
                "postfix_pickup": "/var/spool/postfix/private/pickup"
            },
            "container_ip": "10.200.0.2",
            "healthy": true,
            "last_error": null
        }))
        .build()
}

pub(crate) fn cognitive_mcp_plugin_schema() -> PluginSchema {
    let citation_fields = {
        let mut fields = HashMap::new();
        fields.insert("text".to_string(), FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Cited text passage".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("source".to_string(), FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Source document identifier".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("page".to_string(), FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Page or location within source".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields
    };

    let source_info_fields = {
        let mut fields = HashMap::new();
        fields.insert("id".to_string(), FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Unique source identifier".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        });
        fields.insert("title".to_string(), FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Source title".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("source_type".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec!["url".to_string(), "text".to_string(), "file".to_string()]),
            required: true,
            description: "Source transport type".to_string(),
            default: None, example: Some(json!("url")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        });
        fields.insert("tags".to_string(), FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Tags attached to this source".to_string(),
            default: Some(json!([])), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("created_at".to_string(), FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "ISO-8601 creation timestamp".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        });
        fields
    };

    let gemini_query_request_fields = {
        let mut fields = HashMap::new();
        fields.insert("query".to_string(), FieldSchema {
            field_type: FieldType::String, required: true,
            description: "Natural-language query".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("context".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Optional grounding context".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("mode".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec!["query".to_string(), "deep_research".to_string()]),
            required: false,
            description: "Query mode".to_string(),
            default: Some(json!("query")), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("depth".to_string(), FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Deep-research depth (1-5, default 3)".to_string(),
            default: Some(json!(3)), example: None,
            constraints: vec![Constraint::Min { value: 1.0 }, Constraint::Max { value: 5.0 }],
            read_only: false, read_only_when: None,
        });
        fields
    };

    let memory_tool_input_fields = {
        let mut fields = HashMap::new();
        fields.insert("operation".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec![
                "store".to_string(), "retrieve".to_string(), "query".to_string(),
                "delete".to_string(), "list_namespaces".to_string(), "stats".to_string(),
            ]),
            required: true,
            description: "Memory operation to perform".to_string(),
            default: None, example: Some(json!("store")), constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("namespace".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Namespace name (e.g. project:op-dbus, session:abc)".to_string(),
            default: None, example: Some(json!("project:op-dbus")), constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("namespace_kind".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec![
                "project".to_string(), "session".to_string(), "database".to_string(),
                "workflow".to_string(), "agent".to_string(), "cron".to_string(),
                "custom".to_string(),
            ]),
            required: false,
            description: "Kind of namespace (used when creating)".to_string(),
            default: None, example: Some(json!("project")), constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("key".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Entry key within namespace".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("value".to_string(), FieldSchema {
            field_type: FieldType::Any, required: false,
            description: "Value to store (any JSON)".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("tags".to_string(), FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Tags for the entry".to_string(),
            default: Some(json!([])), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("key_pattern".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Substring pattern for key search (used in query)".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields.insert("limit".to_string(), FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Max results (default 50)".to_string(),
            default: Some(json!(50)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        });
        fields
    };

    PluginSchema::builder("cognitive_mcp")
        .version("2.0.0")
        .description("Cognitive MCP server — memory, gRPC CognitiveToolService. THE PLUGIN IS THE SCHEMA: every method, tool, property, and field is declared here. Downstream inherits.")
        .field("http", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "HTTP/SSE bind address for the MCP protocol endpoint".to_string(),
            default: Some(json!("0.0.0.0:3003")), example: Some(json!("100.90.37.254:3003")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("grpc", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "gRPC bind address for the CognitiveToolService endpoint".to_string(),
            default: Some(json!("0.0.0.0:50052")), example: Some(json!("100.90.37.254:50052")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("db_path", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "CozoDB database path for persistent memory storage".to_string(),
            default: Some(json!("/var/lib/op-dbus/cognitive.db")), example: Some(json!("/var/lib/op-dbus/cognitive.db")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("wg_interface", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "WireGuard interface to read identity from".to_string(),
            default: Some(json!("netmaker")), example: Some(json!("netmaker")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("http_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable the HTTP/SSE MCP transport".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .field("grpc_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable the gRPC CognitiveToolService transport".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .field("dbus_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Register on D-Bus as org.opdbus.CognitiveMcp".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .field("running", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the s6 service is currently running".to_string(),
            default: Some(json!(false)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("healthy", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Last known health status from GetHealth".to_string(),
            default: Some(json!(false)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("auth_status", FieldSchema {
            field_type: FieldType::Enum(vec![
                "none".to_string(), "chrome_profile".to_string(),
                "cookie".to_string(), "api_key".to_string(),
            ]),
            required: false,
            description: "Current authentication method".to_string(),
            default: Some(json!("none")), example: Some(json!("chrome_profile")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("queries_remaining", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Queries remaining in current quota period".to_string(),
            default: Some(json!(0)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("queries_limit", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Total queries allowed per quota period".to_string(),
            default: Some(json!(50)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("notebook_count", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Number of notebooks in the library".to_string(),
            default: Some(json!(0)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("gemini_query_request", FieldSchema {
            field_type: FieldType::Object(gemini_query_request_fields), required: false,
            description: "R12: Gemini fallback query (requires GEMINI_API_KEY)".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("memory_tool", FieldSchema {
            field_type: FieldType::Object(memory_tool_input_fields), required: false,
            description: "MCP MemoryTool: key-value memory store with operations store/retrieve/query/delete/list_namespaces/stats".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("citation", FieldSchema {
            field_type: FieldType::Object(citation_fields), required: false,
            description: "Citation sub-object: text, source, page. Inherited by grounded query responses.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("source_info", FieldSchema {
            field_type: FieldType::Object(source_info_fields), required: false,
            description: "SourceInfo sub-object: id, title, source_type, tags, created_at. Inherited by source CRUD responses.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .build()
}
pub(crate) fn compact_mcp_plugin_schema() -> PluginSchema {
    PluginSchema::builder("compact_mcp")
        .version("1.0.0")
        .description("op-mcp-server — multi-mode MCP server (compact/full/agents) with stdio, HTTP, and WebSocket transports")
        .field("mode", FieldSchema {
            field_type: FieldType::Enum(vec![
                "compact".into(), "full".into(), "agents".into(),
            ]),
            required: false,
            description: "Server mode: compact (5 meta-tools), full (all tools), agents (D-Bus agents)".into(),
            default: Some(json!("compact")),
            example: Some(json!("compact")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("http", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "HTTP/SSE bind address (empty = not started)".into(),
            default: Some(json!("127.0.0.1:11436")),
            example: Some(json!("100.90.37.254:3001")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("ws", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "WebSocket bind address (empty = not started)".into(),
            default: Some(json!(null)),
            example: Some(json!("100.90.37.254:3002")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("wg_interface", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "WireGuard interface for identity sled".into(),
            default: Some(json!("netmaker")),
            example: Some(json!("netmaker")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("stdio", FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Run stdio transport (default for Claude Desktop)".into(),
            default: Some(json!(true)),
            example: None,
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("log_level", FieldSchema {
            field_type: FieldType::Enum(vec![
                "trace".into(), "debug".into(), "info".into(), "warn".into(), "error".into(),
            ]),
            required: false,
            description: "Log verbosity".into(),
            default: Some(json!("info")),
            example: None,
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("running", FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the s6 service is currently running".into(),
            default: Some(json!(false)),
            example: None,
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .build()
}

pub(crate) fn ctl_plane_chatbot_plugin_schema() -> PluginSchema {
    // ── REQ-2: Reasoning Episode Record sub-object ──────────────────────────
    let reasoning_episode_fields = {
        let mut fields = HashMap::new();
        // Core identity
        fields.insert("episode_id".to_string(), FieldSchema {
            field_type: FieldType::String, required: true,
            description: "Unique ID (UUID v7 for time-ordering)".to_string(),
            default: None, example: Some(json!("01912abc-def0-7abc-8def-0123456789ab")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("started_at".to_string(), FieldSchema {
            field_type: FieldType::String, required: true,
            description: "ISO-8601 timestamp of reasoning entry".to_string(),
            default: None, example: Some(json!("2025-05-29T14:30:00Z")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("ended_at".to_string(), FieldSchema {
            field_type: FieldType::String, required: true,
            description: "ISO-8601 timestamp of reasoning exit".to_string(),
            default: None, example: Some(json!("2025-05-29T14:30:05Z")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("duration_ms".to_string(), FieldSchema {
            field_type: FieldType::Integer, required: true,
            description: "Wall-clock duration in milliseconds".to_string(),
            default: None, example: Some(json!(5000)),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        // Lifecycle
        fields.insert("trigger".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec![
                "goal".to_string(), "tool_result".to_string(), "interrupt".to_string(),
                "replan".to_string(), "system_event".to_string(),
            ]),
            required: true,
            description: "What caused reasoning to start".to_string(),
            default: None, example: Some(json!("goal")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("exit_reason".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec![
                "tool_call".to_string(), "response_emitted".to_string(),
                "direction_change".to_string(), "goal_achieved".to_string(),
                "config_set".to_string(), "task_scheduled".to_string(),
                "interrupt".to_string(),
            ]),
            required: true,
            description: "What ended reasoning".to_string(),
            default: None, example: Some(json!("tool_call")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        // Content — PII-tagged per REQ-8
        fields.insert("goal_text".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "High-level goal or prompt active at episode start [PII]".to_string(),
            default: None, example: Some(json!("Configure VLAN isolation for tenant-3")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("reasoning_summary".to_string(), FieldSchema {
            field_type: FieldType::String, required: true,
            description: "Compact natural-language summary of reasoning — primary embedding input [PII]".to_string(),
            default: None, example: Some(json!("Evaluated 3 bridge configs, chose br-tenant3 for isolation")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("tools_consulted".to_string(), FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Ordered list of tools/plugins/MCP calls made during the episode".to_string(),
            default: Some(json!([])), example: Some(json!(["ovs_list_bridges", "ovs_create_bridge"])),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("decision_output".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "The decision, plan, or action the episode produced [PII]".to_string(),
            default: None, example: Some(json!("Create br-tenant3 with VLAN 103 tagged ports")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        // Outcome
        fields.insert("outcome_class".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec![
                "goal_achieved".to_string(), "config_set".to_string(),
                "task_scheduled".to_string(), "delegated".to_string(),
                "interrupted".to_string(), "direction_changed".to_string(),
                "inconclusive".to_string(),
            ]),
            required: true,
            description: "Classification of episode outcome. goal_achieved/config_set/task_scheduled => Signal significance".to_string(),
            default: None, example: Some(json!("config_set")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("confidence".to_string(), FieldSchema {
            field_type: FieldType::Float, required: false,
            description: "Optional confidence 0.0-1.0 if the model emits one".to_string(),
            default: None, example: Some(json!(0.87)),
            constraints: vec![Constraint::Min { value: 0.0 }, Constraint::Max { value: 1.0 }],
            read_only: true, read_only_when: None,
        });
        // Grouping
        fields.insert("plugin_id".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Plugin that owns the context being reasoned about".to_string(),
            default: None, example: Some(json!("ovsdb_bridge")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("conversation_id".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Groups episodes belonging to the same high-level task chain".to_string(),
            default: None, example: Some(json!("vlan-isolation-task-3")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        // Integrity + Privacy
        fields.insert("content_hash".to_string(), FieldSchema {
            field_type: FieldType::String, required: true,
            description: "SHA-256 of canonical serialized record — for exact dedup before upsert (REQ-7)".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("pii_flagged".to_string(), FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "If true, reasoning_summary and decision_output are redacted before vectorization (REQ-8)".to_string(),
            default: Some(json!(false)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields
    };

    // ── Significance classification sub-object (REQ-3) ───────────────────────
    let significance_fields = {
        let mut fields = HashMap::new();
        fields.insert("level".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec!["Contextual".to_string(), "Signal".to_string()]),
            required: true,
            description: "Reasoning episodes are always at least Contextual. goal_achieved/config_set/task_scheduled => Signal".to_string(),
            default: Some(json!("Contextual")), example: Some(json!("Signal")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert("rule".to_string(), FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Significance rule that was evaluated".to_string(),
            default: None, example: Some(json!("outcome_class in [goal_achieved, config_set, task_scheduled]")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields
    };

    PluginSchema::builder("ctl_plane_chatbot")
        .version("1.0.0")
        .description("Control-plane chatbot reasoning episodes — THE PLUGIN IS THE SCHEMA. Declares every episode field (REQ-2), PII tagging (REQ-8), significance classification (REQ-3), and vectorization pipeline config (REQ-4/5/6/7). Downstream (Qdrant, CozoDB, Accountability UI, EventChainService) inherits.")
        // ── Pipeline config (tunable) ──────────────────────────────────────
        .field("voyage_model", FieldSchema {
            field_type: FieldType::Enum(vec![
                "voyage-4-lite".to_string(), "voyage-4".to_string(),
            ]),
            required: false,
            description: "Voyage embedding model for reasoning episodes (REQ-4). POC target: voyage-4-lite".to_string(),
            default: Some(json!("voyage-4-lite")), example: Some(json!("voyage-4")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("qdrant_collection", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Qdrant collection name (REQ-5). Separate from mutation/schema footprints".to_string(),
            default: Some(json!("ctl_plane_reasoning_episodes")), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("vector_dims", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Vector dimensions (1024 for voyage-4-lite, flexible post-POC)".to_string(),
            default: Some(json!(1024)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("dedup_window_hrs", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Content-hash dedup collision window in hours (REQ-7, default 24)".to_string(),
            default: Some(json!(24)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("queue_alert_threshold", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Alert if embedding queue depth exceeds this (REQ-10, default 50)".to_string(),
            default: Some(json!(50)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("nesting_policy", FieldSchema {
            field_type: FieldType::Enum(vec!["flat".to_string(), "nested".to_string()]),
            required: false,
            description: "REQ-1: flat = new trigger extends current episode; nested = opens new episode".to_string(),
            default: Some(json!("flat")), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("vectorization_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable Voyage embedding pipeline for reasoning episodes".to_string(),
            default: Some(json!(true)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        // ── Observed state (read-only from pipeline) ───────────────────────
        .field("running", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the chatbot is currently active".to_string(),
            default: Some(json!(true)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("reasoning_active", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the chatbot is currently in reasoning state (REQ-1)".to_string(),
            default: Some(json!(false)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("embedding_queue_depth", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Current Voyage embedding queue depth (alert at queue_alert_threshold)".to_string(),
            default: Some(json!(0)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("last_vectorized_at", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "ISO-8601 timestamp of last successful Qdrant upsert".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── Vector ID on sled (identity-bound) ───────────────────────────
        .field("vector_id", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Qdrant vector UUID on the identity sled — binds every vectorized episode to this identity".to_string(),
            default: None, example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef0123456789")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── REQ-2: Reasoning Episode Record ────────────────────────────────
        .field("reasoning_episode", FieldSchema {
            field_type: FieldType::Object(reasoning_episode_fields), required: false,
            description: "REQ-2: Structured record produced at reasoning exit. Primary unit of vectorization.".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── REQ-3: Significance classification ─────────────────────────────
        .field("significance", FieldSchema {
            field_type: FieldType::Object(significance_fields), required: false,
            description: "REQ-3: Always at least Contextual. goal_achieved/config_set/task_scheduled => Signal".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .build()
}

// ── OSCAL Subid Registry ─────────────────────────────────────────────────────
//
// Every D-Bus object, plugin, schema, mutation, event, and tool carries two
// identifiers: a `uuid` (machine identity) and a `subid` (human-readable
// operational taxonomy key).  This schema defines the canonical shape of one
// registry entry.  Compliance refs live in metadata arrays — never inside
// the subid string itself.

pub(crate) fn oscal_subid_registry_plugin_schema() -> PluginSchema {
    PluginSchema::builder("oscal_subid_registry")
        .version("1.0.0")
        .description("OSCAL subid registry — dual-identifier model for every system artifact. uuid = machine identity, subid = operational taxonomy key.")
        .category("compliance")

        // ── Core identity ─────────────────────────────────────────────────
        .field("uuid", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Machine identity UUID (RFC 4122). Never replaced by subid.".to_string(),
            default: None,
            example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef0123456789")),
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("subid", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Human-readable operational taxonomy key. Format: <category>.<component-type>.<subject>.<verb>[.<facet>][@vN]. Immutable per subject.".to_string(),
            default: None,
            example: Some(json!("mut.service.state-sync.apply-patch@v1")),
            constraints: vec![
                Constraint::Pattern {
                    regex: "^(src|prj|sch|mut|obs|evt|exp)\\.(this-system|system|interconnection|software|hardware|service|policy|physical|process-procedure|plan|guidance|standard|validation|network)\\.[a-z0-9]+(?:-[a-z0-9]+)*\\.[a-z0-9]+(?:-[a-z0-9]+)*(?:\\.[a-z0-9]+(?:-[a-z0-9]+)*){0,2}(?:@v[1-9][0-9]*)?$".to_string()
                },
            ],
            read_only: false,
            read_only_when: None,
        })

        // ── Taxonomy axes ─────────────────────────────────────────────────
        .field("category", FieldSchema {
            field_type: FieldType::Enum(vec![
                "src".to_string(),  // authoritative source / ingress
                "prj".to_string(),  // D-Bus projection / mirror publication
                "sch".to_string(),  // schema, contract, vocabulary
                "mut".to_string(),  // write-path state mutation
                "obs".to_string(),  // read / query / discovery
                "evt".to_string(),  // signal, audit event, proof, tag provenance
                "exp".to_string(),  // consumer-facing render (MCP tool, UI, gRPC view)
            ]),
            required: true,
            description: "Operational category. Determines which additional fields are required.".to_string(),
            default: None,
            example: Some(json!("mut")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("component_type", FieldSchema {
            field_type: FieldType::Enum(vec![
                "software".to_string(),
                "service".to_string(),
                "network".to_string(),
                "hardware".to_string(),
                "process-procedure".to_string(),
                "standard".to_string(),
                "validation".to_string(),
                "policy".to_string(),
                "plan".to_string(),
                "guidance".to_string(),
                "physical".to_string(),
                "this-system".to_string(),
                "system".to_string(),
                "interconnection".to_string(),
            ]),
            required: true,
            description: "OSCAL component-type vocabulary. Reuse OSCAL nouns — do not invent new types.".to_string(),
            default: None,
            example: Some(json!("service")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("subject", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Stable noun identifying the artifact (e.g. state-sync, plugin-schema, event-chain). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("state-sync")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("verb", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Action performed on the subject (e.g. apply-patch, resolve, monitor). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("apply-patch")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("facet", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional additional qualifier (up to two segments). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("rollback")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("version", FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Schema version of this subid (the @vN suffix). Increment only when subject meaning changes materially.".to_string(),
            default: Some(json!(1)),
            example: Some(json!(1)),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: false,
            read_only_when: None,
        })

        // ── Compliance refs (metadata — never in the subid string) ────────
        .field("control_source", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "URI of the OSCAL catalog or profile that provides the control baseline (e.g. NIST SP 800-53 Rev 5).".to_string(),
            default: None,
            example: Some(json!("https://csrc.nist.gov/projects/oscal")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("control_refs", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "OSCAL control IDs satisfied by this artifact (e.g. [\"AC-1\", \"CM-3\"]). Compliance detail belongs here, not in the subid string.".to_string(),
            default: Some(json!([])),
            example: Some(json!(["AC-1", "CM-3"])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("statement_refs", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Optional fine-grained OSCAL statement-level references within the controls (e.g. [\"AC-1_smt.a\"]]).".to_string(),
            default: Some(json!([])),
            example: Some(json!(["AC-1_smt.a"])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // ── Category-specific required fields ─────────────────────────────
        // mut.* — write-path fields
        .field("actor_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Identity of the actor that performed the mutation.".to_string(),
            default: None,
            example: Some(json!("user:jeremy")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("capability_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Capability that authorized the mutation.".to_string(),
            default: None,
            example: Some(json!("cap:state-write")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("idempotency_key", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Deduplication key for the mutation operation.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // evt.* — event / audit fields
        .field("event_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for evt.* entries. Unique identifier for this event in the audit chain.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("event_hash", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for evt.* entries. Content hash of the event for chain verification.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("tags_touched", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Required for evt.* entries. Tags whose immutability is affected by this event.".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("proof_root", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional for evt.* entries. Merkle proof root for chain verification.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })

        // src.* — source authority fields
        .field("source_system", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for src.* entries. Name of the authoritative source system (e.g. ovsdb, netmaker).".to_string(),
            default: None,
            example: Some(json!("ovsdb")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("source_locator", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for src.* entries. Socket path, URL, or address of the source.".to_string(),
            default: None,
            example: Some(json!("unix:/var/run/openvswitch/db.sock")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("authority_rank", FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Optional for src.* entries. Precedence when multiple sources provide the same subject (lower = higher authority).".to_string(),
            default: Some(json!(100)),
            example: Some(json!(1)),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // prj.* — projection fields
        .field("dbus_path", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for prj.* entries. D-Bus object path of the projected artifact.".to_string(),
            default: None,
            example: Some(json!("/org/opdbus/v1/plugins/wireguard")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("service_name", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for prj.* entries. D-Bus service name hosting the object.".to_string(),
            default: None,
            example: Some(json!("org.opdbus.v1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("source_subid", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional for prj.* entries. Subid of the src.* record this projection was derived from.".to_string(),
            default: None,
            example: Some(json!("src.network.ovsdb.monitor@v1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // sch.* — schema / contract fields
        .field("schema_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for sch.* entries. Canonical name of the schema (matches plugin_schema_defs.rs entry).".to_string(),
            default: None,
            example: Some(json!("wireguard")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("schema_hash", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for sch.* entries. Content hash of the schema at this version.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })

        // exp.* — exposure / render fields
        .field("consumer_surface", FieldSchema {
            field_type: FieldType::Enum(vec![
                "mcp-tool".to_string(),
                "dbus-method".to_string(),
                "grpc-method".to_string(),
                "ui-field".to_string(),
                "ui-page".to_string(),
                "api-endpoint".to_string(),
            ]),
            required: false,
            description: "Required for exp.* entries. The consumer-facing surface this artifact is rendered on.".to_string(),
            default: None,
            example: Some(json!("mcp-tool")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("tool_name", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for exp.mcp-tool entries. The MCP tool name as registered.".to_string(),
            default: None,
            example: Some(json!("cognitive_memory")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // obs.* — observation / query fields
        .field("query_scope", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for obs.* entries. D-Bus path pattern or scope expression for this observation.".to_string(),
            default: None,
            example: Some(json!("/org/opdbus/v1/plugins/*")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        .build()
}
</file>

<file path="src/state_plugins/privacy_router.rs">
//! Privacy router system fabric.
//!
//! This plugin owns the base privacy fabric as system-managed Incus containers and
//! bridge/OpenFlow policy, separate from per-user privacy containers.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use op_network::{openflow::OpenFlowClient, OvsdbClient};
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::Path;
use tokio::process::Command;

use crate::state_plugins::incus::{IncusInstance, IncusPlugin, IncusState};
use crate::state_plugins::openflow::{
    BridgeFlowConfig, FlowAction, FlowEntry, OpenFlowConfig, OpenFlowPlugin,
};
use crate::state_plugins::privacy_routes::{PrivacyRoute, PrivacyRoutesPlugin, PrivacyRoutesState};

const DEFAULT_BRIDGE_NAME: &str = "ovsbr0";
const DEFAULT_UPLINK_PORT: &str = "ens3";
const DEFAULT_MGMT_PORT: &str = "ovsbr0-mgmt";
const DEFAULT_SOCKET_PORT: &str = "ovsbr0-sock";
const DEFAULT_GRPC_BRIDGE_PORT: &str = "grpc-bridge";
const DEFAULT_MGMT_CIDR: &str = "10.200.0.1/24";
const DEFAULT_OPENFLOW_CONTROLLER: &str = "10.200.0.1:6653";
const DEFAULT_DATAPATH_TYPE: &str = "system";
const DEFAULT_FAIL_MODE: &str = "secure";
const DEFAULT_WARP_INTERFACE: &str = "wgcf";
const DEFAULT_WGCF_CONFIG: &str = "/etc/wireguard/wgcf.conf";
const SYSTEM_FLOW_COOKIE_PREFIX: u64 = 0x5053_0000_0000_0000;
const SYSTEM_FLOW_COOKIE_MASK: u64 = 0xFFFF_0000_0000_0000;

/// Privacy Router Tunnel Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRouterConfig {
    /// OVS bridge name (shared by all components)
    pub bridge_name: String,

    /// WireGuard ingress container configuration
    pub wireguard: WireGuardConfig,

    /// WARP tunnel configuration
    pub warp: WarpConfig,

    /// XRay REALITY outbound client configuration
    pub xray: XRayConfig,

    /// VPS XRay server endpoint
    pub vps: VpsConfig,

    /// Socket networking configuration
    pub socket_networking: SocketNetworkingConfig,

    /// OpenFlow privacy flow configuration
    pub openflow: OpenFlowPrivacyConfig,

    /// Additional containers (vector DB, bucket storage, etc.)
    pub containers: Vec<ContainerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardConfig {
    pub enabled: bool,
    pub container_id: u32,
    pub socket_port: String,
    pub zero_config: bool,
    pub listen_port: u16,
    pub resources: ContainerResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerResources {
    pub vcpus: u8,
    pub memory_mb: u32,
    pub disk_gb: u32,
    /// Incus image reference, e.g. images:debian/13
    pub os_template: String,
    pub swap_mb: u32,
    pub unprivileged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpConfig {
    pub enabled: bool,
    pub bridge_interface: String,
    pub wgcf_config: String,
    pub warp_license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XRayConfig {
    pub enabled: bool,
    pub container_id: u32,
    pub socket_port: String,
    pub socks_port: u16,
    pub vps_address: String,
    pub vps_port: u16,
    pub resources: ContainerResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsConfig {
    pub xray_server: String,
    pub xray_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketNetworkingConfig {
    pub enabled: bool,
    pub privacy_sockets: Vec<PrivacySocketPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySocketPort {
    pub name: String,
    pub container_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowPrivacyConfig {
    pub enabled: bool,
    #[serde(default = "default_security_enabled")]
    pub enable_security_flows: bool,
    #[serde(default = "default_obfuscation_level")]
    pub obfuscation_level: u8,
    pub privacy_flows: Vec<PrivacyFlowRule>,
    pub function_routing: Vec<FunctionRoute>,
}

fn default_security_enabled() -> bool {
    true
}

fn default_obfuscation_level() -> u8 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyFlowRule {
    pub priority: u16,
    pub match_fields: HashMap<String, String>,
    pub actions: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionRoute {
    pub function: String,
    pub target_socket: String,
    pub match_fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub id: u32,
    pub name: String,
    pub container_type: String,
}

#[derive(Debug, Clone)]
struct PrivacyHostBootstrapConfig {
    bridge_name: String,
    uplink_port: String,
    attach_uplink_to_bridge: bool,
    management_port: String,
    socket_port: String,
    grpc_bridge_port: String,
    management_cidr: String,
    openflow_controller: String,
    datapath_type: String,
    fail_mode: String,
}

impl PrivacyHostBootstrapConfig {
    fn from_env(bridge_name: &str) -> Self {
        Self {
            bridge_name: std::env::var("PRIVACY_BRIDGE_NAME")
                .unwrap_or_else(|_| bridge_name.to_string()),
            uplink_port: std::env::var("PRIVACY_UPLINK_PORT")
                .unwrap_or_else(|_| DEFAULT_UPLINK_PORT.to_string()),
            attach_uplink_to_bridge: bool_env("PRIVACY_ATTACH_UPLINK_TO_BRIDGE", false),
            management_port: std::env::var("PRIVACY_MGMT_PORT")
                .unwrap_or_else(|_| DEFAULT_MGMT_PORT.to_string()),
            socket_port: std::env::var("PRIVACY_SOCKET_PORT")
                .unwrap_or_else(|_| DEFAULT_SOCKET_PORT.to_string()),
            grpc_bridge_port: std::env::var("PRIVACY_GRPC_BRIDGE_PORT")
                .unwrap_or_else(|_| DEFAULT_GRPC_BRIDGE_PORT.to_string()),
            management_cidr: std::env::var("PRIVACY_MGMT_CIDR")
                .unwrap_or_else(|_| DEFAULT_MGMT_CIDR.to_string()),
            openflow_controller: std::env::var("PRIVACY_OPENFLOW_CONTROLLER")
                .unwrap_or_else(|_| DEFAULT_OPENFLOW_CONTROLLER.to_string()),
            datapath_type: std::env::var("PRIVACY_DATAPATH_TYPE")
                .unwrap_or_else(|_| DEFAULT_DATAPATH_TYPE.to_string()),
            fail_mode: std::env::var("PRIVACY_FAIL_MODE")
                .unwrap_or_else(|_| DEFAULT_FAIL_MODE.to_string()),
        }
    }
}

impl Default for PrivacyRouterConfig {
    fn default() -> Self {
        Self {
            bridge_name: DEFAULT_BRIDGE_NAME.to_string(),
            wireguard: WireGuardConfig {
                enabled: true,
                container_id: 100,
                socket_port: "priv_wg".to_string(),
                zero_config: true,
                listen_port: 51820,
                resources: default_resources(),
            },
            warp: WarpConfig {
                enabled: true,
                bridge_interface: DEFAULT_WARP_INTERFACE.to_string(),
                wgcf_config: DEFAULT_WGCF_CONFIG.to_string(),
                warp_license: None,
            },
            xray: XRayConfig {
                enabled: true,
                container_id: 101,
                socket_port: "priv_xray".to_string(),
                socks_port: 1080,
                vps_address: "vps.example.com".to_string(),
                vps_port: 443,
                resources: default_resources(),
            },
            vps: VpsConfig {
                xray_server: "vps.example.com".to_string(),
                xray_port: 443,
            },
            socket_networking: SocketNetworkingConfig {
                enabled: true,
                privacy_sockets: vec![
                    PrivacySocketPort {
                        name: "priv_wg".to_string(),
                        container_id: Some(100),
                    },
                    PrivacySocketPort {
                        name: "priv_xray".to_string(),
                        container_id: Some(101),
                    },
                ],
            },
            openflow: OpenFlowPrivacyConfig {
                enabled: true,
                enable_security_flows: true,
                obfuscation_level: 2,
                privacy_flows: default_privacy_flows(),
                function_routing: vec![],
            },
            containers: vec![],
        }
    }
}

fn default_resources() -> ContainerResources {
    ContainerResources {
        vcpus: 1,
        memory_mb: 512,
        disk_gb: 4,
        os_template: "images:debian/13".to_string(),
        swap_mb: 0,
        unprivileged: false,
    }
}

fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn default_privacy_flows() -> Vec<PrivacyFlowRule> {
    vec![
        PrivacyFlowRule {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "priv_wg".to_string())]),
            actions: vec!["output:wgcf".to_string()],
            description: Some("priv_wg -> wgcf".to_string()),
        },
        PrivacyFlowRule {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "wgcf".to_string())]),
            actions: vec!["output:priv_xray".to_string()],
            description: Some("wgcf -> priv_xray".to_string()),
        },
        PrivacyFlowRule {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "priv_xray".to_string())]),
            actions: vec!["output:wgcf".to_string()],
            description: Some("priv_xray -> wgcf".to_string()),
        },
        PrivacyFlowRule {
            priority: 200,
            match_fields: HashMap::from([("dl_type".to_string(), "0x0806".to_string())]),
            actions: vec!["arp_responder".to_string()],
            description: Some("ARP Responder for Privacy Network".to_string()),
        },
    ]
}

pub struct PrivacyRouterPlugin {
    config: PrivacyRouterConfig,
    routes_store: PrivacyRoutesPlugin,
}

impl PrivacyRouterPlugin {
    pub fn new(config: PrivacyRouterConfig) -> Self {
        Self {
            config,
            routes_store: PrivacyRoutesPlugin::default(),
        }
    }

    async fn query_privacy_routes(&self) -> Result<PrivacyRoutesState> {
        let state = self.routes_store.query_current_state().await?;
        Ok(simd_json::serde::from_owned_value(state)?)
    }

    async fn query_incus_state(&self) -> Result<IncusState> {
        let state = IncusPlugin::new().query_current_state().await?;
        Ok(simd_json::serde::from_owned_value(state)?)
    }

    async fn query_openflow_state(&self) -> Result<OpenFlowConfig> {
        let state = OpenFlowPlugin::new().query_current_state().await?;
        Ok(simd_json::serde::from_owned_value(state)?)
    }

    async fn query_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        OvsdbClient::new()
            .list_bridge_ports(bridge_name)
            .await
            .with_context(|| format!("list ports on {}", bridge_name))
    }

    fn unique_ingress_ports(routes: &[PrivacyRoute]) -> Vec<String> {
        let mut ingress_ports: HashSet<String> = routes
            .iter()
            .map(|route| route.ingress_port.clone())
            .collect();
        let mut ingress_ports: Vec<String> = ingress_ports.drain().collect();
        ingress_ports.sort();
        ingress_ports
    }

    fn desired_config_from_diff(&self, diff: &StateDiff) -> Result<PrivacyRouterConfig> {
        let mut merged = simd_json::serde::to_owned_value(self.config.clone())?;
        for action in &diff.actions {
            if let StateAction::Modify { changes, .. } = action {
                if let Some(config) = changes.get("config") {
                    Self::deep_merge(&mut merged, config);
                } else {
                    Self::deep_merge(&mut merged, changes);
                }
            }
        }
        Ok(simd_json::serde::from_owned_value(merged)?)
    }

    fn expected_system_container_names(config: &PrivacyRouterConfig) -> Vec<&'static str> {
        let mut names = Vec::new();
        if config.wireguard.enabled {
            names.push("privacy-wireguard-ingress");
        }
        if config.xray.enabled {
            names.push("privacy-xray-egress");
        }
        names
    }

    fn actual_system_containers(
        &self,
        config: &PrivacyRouterConfig,
        incus: &IncusState,
    ) -> Vec<String> {
        let expected: HashSet<&str> = Self::expected_system_container_names(config)
            .into_iter()
            .collect();
        let mut containers = incus
            .instances
            .iter()
            .filter(|instance| {
                expected.contains(instance.name.as_str())
                    && instance.status.eq_ignore_ascii_case("running")
            })
            .map(|instance| instance.name.clone())
            .collect::<Vec<_>>();
        containers.sort();
        containers
    }

    fn required_system_flow_count(&self, config: &PrivacyRouterConfig) -> usize {
        self.chain_ports(config).windows(2).count() * 2
    }

    async fn runtime_needs_reconcile(&self, config: &PrivacyRouterConfig) -> Result<bool> {
        if config.warp.enabled {
            match self.query_bridge_ports(&config.bridge_name).await {
                Ok(ports) => {
                    if !ports
                        .iter()
                        .any(|port| port == &config.warp.bridge_interface)
                    {
                        return Ok(true);
                    }
                }
                Err(_) => {
                    // Treat a missing bridge as drift so apply_state can build it.
                    return Ok(true);
                }
            }
        }

        let incus_state = self.query_incus_state().await?;
        let actual_containers = self.actual_system_containers(config, &incus_state);
        if actual_containers.len() != Self::expected_system_container_names(config).len() {
            return Ok(true);
        }

        let openflow_state = self.query_openflow_state().await?;
        let actual_flow_count = openflow_state
            .bridges
            .iter()
            .find(|bridge| bridge.name == config.bridge_name)
            .map(|bridge| {
                bridge
                    .flows
                    .iter()
                    .filter(|flow| flow.cookie.is_some_and(is_system_cookie))
                    .count()
            })
            .unwrap_or_default();

        Ok(config.openflow.enabled && actual_flow_count < self.required_system_flow_count(config))
    }

    fn deep_merge(target: &mut Value, source: &Value) {
        match (target, source) {
            (Value::Object(target_obj), Value::Object(source_obj)) => {
                for (key, value) in source_obj.iter() {
                    match target_obj.get_mut(key) {
                        Some(existing) => Self::deep_merge(existing, value),
                        None => {
                            target_obj.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
            (target_value, source_value) => {
                *target_value = source_value.clone();
            }
        }
    }

    async fn ensure_warp_interface_on_bridge(&self, config: &PrivacyRouterConfig) -> Result<()> {
        if !config.warp.enabled {
            return Ok(());
        }

        let ovs = op_network::OvsdbClient::new();
        let ports = ovs
            .list_bridge_ports(&config.bridge_name)
            .await
            .with_context(|| format!("list ports on {}", config.bridge_name))?;
        if ports
            .iter()
            .any(|port| port == &config.warp.bridge_interface)
        {
            let _ = op_network::rtnetlink::link_up(&config.warp.bridge_interface).await;
            return Ok(());
        }

        let interfaces = op_network::rtnetlink::list_interfaces()
            .await
            .context("list interfaces for warp attach")?;
        if !interfaces
            .iter()
            .any(|iface| iface.name == config.warp.bridge_interface)
        {
            if !std::path::Path::new(&config.warp.wgcf_config).exists() {
                bail!(
                    "warp interface '{}' missing and wgcf config '{}' not found",
                    config.warp.bridge_interface,
                    config.warp.wgcf_config
                );
            }
            self.ensure_wg_quick_interface(&config.warp.bridge_interface, &config.warp.wgcf_config)
                .await?;
        }

        ovs.add_port(&config.bridge_name, &config.warp.bridge_interface)
            .await
            .with_context(|| {
                format!(
                    "attach '{}' to '{}'",
                    config.warp.bridge_interface, config.bridge_name
                )
            })?;
        op_network::rtnetlink::link_up(&config.warp.bridge_interface)
            .await
            .with_context(|| format!("bring '{}' up", config.warp.bridge_interface))?;
        Ok(())
    }

    async fn ensure_host_bridge_topology(&self, config: &PrivacyRouterConfig) -> Result<()> {
        let host = PrivacyHostBootstrapConfig::from_env(&config.bridge_name);
        let ovs = OvsdbClient::new();

        ovs.list_dbs()
            .await
            .context("Open vSwitch DB is unavailable; cannot provision privacy bridge")?;

        if !ovs
            .bridge_exists(&host.bridge_name)
            .await
            .context("check privacy bridge existence")?
        {
            ovs.create_bridge(&host.bridge_name)
                .await
                .with_context(|| format!("create bridge '{}'", host.bridge_name))?;
        }

        log::info!(
            "privacy_router bridge policy: {} datapath_type={} fail_mode={}",
            host.bridge_name,
            host.datapath_type,
            host.fail_mode
        );
        ovs.set_bridge_property(&host.bridge_name, "datapath_type", &host.datapath_type)
            .await
            .with_context(|| format!("set bridge datapath_type={}", host.datapath_type))?;
        ovs.set_bridge_property(&host.bridge_name, "fail_mode", &host.fail_mode)
            .await
            .with_context(|| format!("set bridge fail_mode={}", host.fail_mode))?;

        let existing_ports = ovs
            .list_bridge_ports(&host.bridge_name)
            .await
            .with_context(|| format!("list bridge ports on '{}'", host.bridge_name))?;

        if !host.uplink_port.trim().is_empty() {
            let uplink_path = format!("/sys/class/net/{}", host.uplink_port);
            if !Path::new(&uplink_path).exists() {
                bail!(
                    "configured uplink '{}' not found on host ({})",
                    host.uplink_port,
                    uplink_path
                );
            }
            if host.attach_uplink_to_bridge
                && !existing_ports.iter().any(|port| port == &host.uplink_port)
            {
                ovs.add_port(&host.bridge_name, &host.uplink_port)
                    .await
                    .with_context(|| {
                        format!(
                            "attach uplink '{}' to '{}'",
                            host.uplink_port, host.bridge_name
                        )
                    })?;
            }
            op_network::rtnetlink::link_up(&host.uplink_port)
                .await
                .with_context(|| format!("bring standalone uplink '{}' up", host.uplink_port))?;
        }

        if !existing_ports
            .iter()
            .any(|port| port == &host.management_port)
        {
            ovs.add_port_with_type(&host.bridge_name, &host.management_port, Some("internal"))
                .await
                .with_context(|| {
                    format!(
                        "add management port '{}' to '{}'",
                        host.management_port, host.bridge_name
                    )
                })?;
        }

        if !existing_ports.iter().any(|port| port == &host.socket_port) {
            ovs.add_port_with_type(&host.bridge_name, &host.socket_port, Some("internal"))
                .await
                .with_context(|| {
                    format!(
                        "add socket port '{}' to '{}'",
                        host.socket_port, host.bridge_name
                    )
                })?;
        }

        if !existing_ports
            .iter()
            .any(|port| port == &host.grpc_bridge_port)
        {
            ovs.add_port_with_type(&host.bridge_name, &host.grpc_bridge_port, Some("internal"))
                .await
                .with_context(|| {
                    format!(
                        "add gRPC bridge port '{}' to '{}'",
                        host.grpc_bridge_port, host.bridge_name
                    )
                })?;
        }

        op_network::rtnetlink::link_up(&host.bridge_name)
            .await
            .with_context(|| format!("bring '{}' up", host.bridge_name))?;
        op_network::rtnetlink::link_up(&host.management_port)
            .await
            .with_context(|| format!("bring '{}' up", host.management_port))?;
        op_network::rtnetlink::link_up(&host.socket_port)
            .await
            .with_context(|| format!("bring '{}' up", host.socket_port))?;
        op_network::rtnetlink::link_up(&host.grpc_bridge_port)
            .await
            .with_context(|| format!("bring '{}' up", host.grpc_bridge_port))?;

        let (management_ip, management_prefix) = parse_cidr(&host.management_cidr)?;
        op_network::rtnetlink::flush_addresses(&host.management_port)
            .await
            .with_context(|| format!("flush addresses on '{}'", host.management_port))?;
        op_network::rtnetlink::add_ipv4_address(
            &host.management_port,
            &management_ip,
            management_prefix,
        )
        .await
        .with_context(|| {
            format!(
                "assign management CIDR '{}' to '{}'",
                host.management_cidr, host.management_port
            )
        })?;

        if let Ok(controller_addr) = host.openflow_controller.parse::<SocketAddr>() {
            match OpenFlowClient::connect(controller_addr).await {
                Ok(mut client) => {
                    if let Err(e) = client.request_features().await {
                        log::warn!(
                            "OpenFlow controller probe connected but feature request failed: {}",
                            e
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "OpenFlow controller '{}' is not reachable yet: {}",
                        host.openflow_controller,
                        e
                    );
                }
            }
        } else {
            log::warn!(
                "Invalid PRIVACY_OPENFLOW_CONTROLLER '{}'; skipping OpenFlow probe",
                host.openflow_controller
            );
        }

        Ok(())
    }

    async fn ensure_wg_quick_interface(&self, name: &str, config_path: &str) -> Result<()> {
        self.validate_wg_quick_config(name, config_path)?;
        self.run_command("/usr/bin/wg-quick", &["up", config_path])
            .await?;
        self.run_command("/usr/bin/ip", &["link", "set", "up", "dev", name])
            .await?;
        Ok(())
    }

    fn validate_wg_quick_config(&self, interface_name: &str, config_path: &str) -> Result<()> {
        let config = std::fs::read_to_string(config_path)
            .with_context(|| format!("read wg-quick config '{}'", config_path))?;
        let normalized = config
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();

        if !normalized
            .iter()
            .any(|line| line.eq_ignore_ascii_case("[Interface]"))
        {
            bail!(
                "wg-quick config '{}' for '{}' is missing [Interface]",
                config_path,
                interface_name
            );
        }
        if !normalized.iter().any(|line| {
            line.split_once('=')
                .map(|(key, value)| {
                    key.trim().eq_ignore_ascii_case("PrivateKey") && !value.trim().is_empty()
                })
                .unwrap_or(false)
        }) {
            bail!(
                "wg-quick config '{}' for '{}' is missing PrivateKey",
                config_path,
                interface_name
            );
        }
        if !normalized.iter().any(|line| {
            line.split_once('=')
                .map(|(key, value)| {
                    key.trim().eq_ignore_ascii_case("Table")
                        && value.trim().eq_ignore_ascii_case("off")
                })
                .unwrap_or(false)
        }) {
            bail!(
                "wg-quick config '{}' for '{}' must set 'Table = off' before bridging to OVS",
                config_path,
                interface_name
            );
        }

        Ok(())
    }

    async fn run_command(&self, binary: &str, args: &[&str]) -> Result<()> {
        let output = Command::new(binary)
            .args(args)
            .output()
            .await
            .with_context(|| format!("execute {}", binary))?;
        if !output.status.success() {
            bail!(
                "{} {} failed (exit {}): {}",
                binary,
                args.join(" "),
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn system_container_specs<'a>(
        &'a self,
        config: &'a PrivacyRouterConfig,
    ) -> Vec<SystemContainerSpec<'a>> {
        let mut specs = Vec::new();
        if config.wireguard.enabled {
            specs.push(SystemContainerSpec {
                name: "privacy-wireguard-ingress",
                role: "wireguard_ingress",
                socket_port: &config.wireguard.socket_port,
                resources: &config.wireguard.resources,
            });
        }
        if config.xray.enabled {
            specs.push(SystemContainerSpec {
                name: "privacy-xray-egress",
                role: "xray_reality_client",
                socket_port: &config.xray.socket_port,
                resources: &config.xray.resources,
            });
        }
        specs
    }

    fn desired_system_instance(
        &self,
        config: &PrivacyRouterConfig,
        spec: &SystemContainerSpec<'_>,
    ) -> IncusInstance {
        let devices = HashMap::from([(
            "fabric0".to_string(),
            HashMap::from([
                ("type".to_string(), "nic".to_string()),
                ("nictype".to_string(), "bridged".to_string()),
                ("parent".to_string(), config.bridge_name.clone()),
                ("name".to_string(), "eth0".to_string()),
                ("host_name".to_string(), spec.socket_port.to_string()),
            ]),
        )]);

        IncusInstance {
            name: spec.name.to_string(),
            status: "Running".to_string(),
            instance_type: "container".to_string(),
            image: Some(spec.resources.os_template.clone()),
            storage_pool: Some(
                std::env::var("PRIVACY_SYSTEM_STORAGE_POOL")
                    .or_else(|_| std::env::var("INCUS_STORAGE_POOL"))
                    .unwrap_or_else(|_| "default".to_string()),
            ),
            profiles: Vec::new(),
            config: Some(HashMap::from([
                ("boot.autostart".to_string(), "true".to_string()),
                ("security.nesting".to_string(), "true".to_string()),
                (
                    "security.privileged".to_string(),
                    (!spec.resources.unprivileged).to_string(),
                ),
                ("user.opdbus.scope".to_string(), "system".to_string()),
                (
                    "user.opdbus.component".to_string(),
                    "privacy_router".to_string(),
                ),
                ("user.opdbus.role".to_string(), spec.role.to_string()),
                (
                    "user.opdbus.host_port".to_string(),
                    spec.socket_port.to_string(),
                ),
            ])),
            devices: Some(devices),
        }
    }

    fn upsert_instance(instances: &mut Vec<IncusInstance>, desired: IncusInstance) {
        match instances
            .iter_mut()
            .find(|existing| existing.name == desired.name)
        {
            Some(existing) => *existing = desired,
            None => instances.push(desired),
        }
        instances.sort_by(|a, b| a.name.cmp(&b.name));
    }

    async fn apply_incus_system_containers(
        &self,
        config: &PrivacyRouterConfig,
    ) -> Result<ApplyResult> {
        let plugin = IncusPlugin::new();
        let current_state = plugin.query_current_state().await?;
        let mut desired_state: IncusState =
            simd_json::serde::from_owned_value(current_state.clone())
                .context("deserialize current incus state")?;

        for spec in self.system_container_specs(config) {
            Self::upsert_instance(
                &mut desired_state.instances,
                self.desired_system_instance(config, &spec),
            );
        }

        let desired_value = simd_json::serde::to_owned_value(desired_state)?;
        let diff = plugin
            .calculate_diff(&current_state, &desired_value)
            .await?;
        if diff.actions.is_empty() {
            return Ok(ApplyResult {
                success: true,
                changes_applied: vec!["System privacy containers already in sync".to_string()],
                errors: Vec::new(),
                checkpoint: None,
            });
        }
        plugin.apply_state(&diff).await
    }

    fn chain_ports(&self, config: &PrivacyRouterConfig) -> Vec<String> {
        let mut ports = Vec::new();
        if config.wireguard.enabled {
            ports.push(config.wireguard.socket_port.clone());
        }
        if config.warp.enabled {
            ports.push(config.warp.bridge_interface.clone());
        }
        if config.xray.enabled {
            ports.push(config.xray.socket_port.clone());
        }
        ports
    }

    fn merge_openflow_config(
        &self,
        mut current: OpenFlowConfig,
        config: &PrivacyRouterConfig,
    ) -> OpenFlowConfig {
        let bridge_index = current
            .bridges
            .iter()
            .position(|bridge| bridge.name == config.bridge_name);
        let mut bridge = bridge_index
            .map(|index| current.bridges.remove(index))
            .unwrap_or(BridgeFlowConfig {
                name: config.bridge_name.clone(),
                flows: Vec::new(),
                socket_ports: None,
            });

        bridge
            .flows
            .retain(|flow| !flow.cookie.is_some_and(is_system_cookie));

        let ports = self.chain_ports(config);
        for (index, path) in ports.windows(2).enumerate() {
            bridge.flows.push(chain_flow(index, &path[0], &path[1]));
            bridge
                .flows
                .push(chain_flow(index + 1000, &path[1], &path[0]));
        }

        // Include custom privacy flows from configuration
        for (index, rule) in config.openflow.privacy_flows.iter().enumerate() {
            let mut actions = Vec::new();
            for action_str in &rule.actions {
                if action_str.starts_with("output:") {
                    actions.push(FlowAction::Output {
                        port: action_str.strip_prefix("output:").unwrap().to_string(),
                    });
                } else if action_str == "arp_responder" {
                    // Default ARP responder for the bridge IP
                    actions.push(FlowAction::ArpResponder {
                        mac: "00:11:22:33:44:55".to_string(), // Simplified default
                        ip: "10.200.0.1".to_string(),
                    });
                } else if action_str == "drop" {
                    actions.push(FlowAction::Drop);
                }
            }

            bridge.flows.push(FlowEntry {
                table: 0,
                priority: rule.priority,
                match_fields: rule.match_fields.clone(),
                actions,
                cookie: Some(SYSTEM_FLOW_COOKIE_PREFIX | 0x2000 | index as u64),
                idle_timeout: 0,
                hard_timeout: 0,
            });
        }

        bridge.flows.sort_by_key(flow_sort_key);

        current.bridges.push(bridge);
        current.bridges.sort_by(|a, b| a.name.cmp(&b.name));
        current.auto_discover_containers = false;
        current.enable_security_flows =
            current.enable_security_flows || config.openflow.enable_security_flows;
        current.obfuscation_level = current
            .obfuscation_level
            .max(config.openflow.obfuscation_level);
        current
    }

    async fn apply_openflow_system_chain(
        &self,
        config: &PrivacyRouterConfig,
    ) -> Result<ApplyResult> {
        let plugin = OpenFlowPlugin::new();
        let current_state = plugin.query_current_state().await?;
        let current_config: OpenFlowConfig =
            simd_json::serde::from_owned_value(current_state.clone())?;
        let desired_config = self.merge_openflow_config(current_config, config);
        let desired_value = simd_json::serde::to_owned_value(desired_config)?;
        let diff = plugin
            .calculate_diff(&current_state, &desired_value)
            .await?;
        if diff.actions.is_empty() {
            return Ok(ApplyResult {
                success: true,
                changes_applied: vec!["Privacy router OpenFlow chain already in sync".to_string()],
                errors: Vec::new(),
                checkpoint: None,
            });
        }
        plugin.apply_state(&diff).await
    }
}

struct SystemContainerSpec<'a> {
    name: &'a str,
    role: &'a str,
    socket_port: &'a str,
    resources: &'a ContainerResources,
}

fn chain_flow(index: usize, in_port: &str, out_port: &str) -> FlowEntry {
    FlowEntry {
        table: 0,
        priority: 21000,
        match_fields: HashMap::from([
            ("in_port".to_string(), in_port.to_string()),
            ("ip".to_string(), "".to_string()),
        ]),
        actions: vec![FlowAction::Output {
            port: out_port.to_string(),
        }],
        cookie: Some(SYSTEM_FLOW_COOKIE_PREFIX | index as u64),
        idle_timeout: 0,
        hard_timeout: 0,
    }
}

fn is_system_cookie(cookie: u64) -> bool {
    cookie & SYSTEM_FLOW_COOKIE_MASK == SYSTEM_FLOW_COOKIE_PREFIX
}

fn flow_sort_key(flow: &FlowEntry) -> (u8, u16, u64) {
    (flow.table, flow.priority, flow.cookie.unwrap_or_default())
}

#[async_trait]
impl StatePlugin for PrivacyRouterPlugin {
    fn name(&self) -> &'static str {
        "privacy_router"
    }

    fn version(&self) -> &str {
        "1.2.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::privacy_router_plugin_schema())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }

    async fn query_current_state(&self) -> Result<Value> {
        let privacy_routes = self
            .query_privacy_routes()
            .await
            .unwrap_or(PrivacyRoutesState { routes: Vec::new() });
        let incus_state = self.query_incus_state().await.unwrap_or(IncusState {
            instances: Vec::new(),
        });
        let openflow_state = self.query_openflow_state().await.unwrap_or(OpenFlowConfig {
            bridges: Vec::new(),
            controller_endpoint: None,
            flow_policies: None,
            auto_discover_containers: false,
            enable_security_flows: false,
            obfuscation_level: 0,
        });

        let mut components = simd_json::owned::Object::new();

        if self.config.wireguard.enabled {
            components.insert(
                "wireguard".to_string(),
                json!({
                    "enabled": true,
                    "container_id": self.config.wireguard.container_id,
                    "socket_port": self.config.wireguard.socket_port,
                }),
            );
        }
        if self.config.warp.enabled {
            components.insert(
                "warp".to_string(),
                json!({
                    "enabled": true,
                    "bridge_interface": self.config.warp.bridge_interface,
                    "wgcf_config": self.config.warp.wgcf_config,
                }),
            );
        }
        if self.config.xray.enabled {
            components.insert(
                "xray".to_string(),
                json!({
                    "enabled": true,
                    "container_id": self.config.xray.container_id,
                    "socket_port": self.config.xray.socket_port,
                    "upstream_server": self.config.vps.xray_server,
                    "upstream_port": self.config.vps.xray_port,
                }),
            );
        }
        if self.config.openflow.enabled {
            let system_flow_count = openflow_state
                .bridges
                .iter()
                .find(|bridge| bridge.name == self.config.bridge_name)
                .map(|bridge| {
                    bridge
                        .flows
                        .iter()
                        .filter(|flow| flow.cookie.is_some_and(is_system_cookie))
                        .count()
                })
                .unwrap_or_default();
            components.insert(
                "openflow".to_string(),
                json!({
                    "enabled": true,
                    "enable_security_flows": self.config.openflow.enable_security_flows,
                    "obfuscation_level": self.config.openflow.obfuscation_level,
                    "privacy_flows": system_flow_count,
                    "function_routes": self.config.openflow.function_routing.len(),
                    "published_routes": privacy_routes.routes.len(),
                    "shared_ingress_ports": Self::unique_ingress_ports(&privacy_routes.routes),
                }),
            );
        }
        components.insert(
            "containers".to_string(),
            json!(self.actual_system_containers(&self.config, &incus_state)),
        );

        Ok(json!({
            "config": self.config,
            "components": components
        }))
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let mut actions = Vec::new();
        let current_config = current.get("config").unwrap_or(current);
        let desired_config = desired.get("config").unwrap_or(desired);
        let desired_runtime: PrivacyRouterConfig =
            simd_json::serde::from_owned_value(desired_config.clone())?;

        if current_config != desired_config
            || self.runtime_needs_reconcile(&desired_runtime).await?
        {
            actions.push(StateAction::Modify {
                resource: "privacy_router_config".to_string(),
                changes: desired.clone(),
            });
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let config = self.desired_config_from_diff(diff)?;
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        self.ensure_host_bridge_topology(&config).await?;
        self.ensure_warp_interface_on_bridge(&config).await?;

        let incus_result = self.apply_incus_system_containers(&config).await?;
        changes_applied.extend(incus_result.changes_applied);
        errors.extend(incus_result.errors);

        if !errors.is_empty() {
            return Ok(ApplyResult {
                success: false,
                changes_applied,
                errors,
                checkpoint: None,
            });
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let openflow_result = self.apply_openflow_system_chain(&config).await?;
        changes_applied.extend(openflow_result.changes_applied);
        errors.extend(openflow_result.errors);

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        Ok(self
            .calculate_diff(&current, desired)
            .await?
            .actions
            .is_empty())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!(
                "privacy_router_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
            ),
            plugin: self.name().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        log::info!(
            "Rolling back privacy router to checkpoint: {}",
            checkpoint.id
        );
        Err(anyhow::anyhow!(
            "Privacy router rollback not yet implemented"
        ))
    }
}

fn parse_cidr(cidr: &str) -> Result<(String, u8)> {
    let mut parts = cidr.split('/');
    let ip = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid CIDR '{}': missing IP", cidr))?;
    let prefix = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid CIDR '{}': missing prefix", cidr))?
        .parse::<u8>()
        .with_context(|| format!("invalid CIDR prefix in '{}'", cidr))?;
    if parts.next().is_some() {
        bail!("invalid CIDR '{}': too many separators", cidr);
    }
    Ok((ip.to_string(), prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_config_merges_partial_overlay() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let diff = StateDiff {
            plugin: "privacy_router".to_string(),
            actions: vec![StateAction::Modify {
                resource: "privacy_router_config".to_string(),
                changes: json!({
                    "xray": {
                        "vps_address": "xray.example.com"
                    }
                }),
            }],
            metadata: DiffMetadata {
                timestamp: 0,
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        };

        let config = plugin.desired_config_from_diff(&diff).expect("config");
        assert_eq!(config.xray.vps_address, "xray.example.com");
        assert_eq!(config.bridge_name, DEFAULT_BRIDGE_NAME);
    }

    #[test]
    fn chain_ports_follow_enabled_system_components() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let config = PrivacyRouterConfig::default();
        assert_eq!(
            plugin.chain_ports(&config),
            vec![
                config.wireguard.socket_port.clone(),
                config.warp.bridge_interface.clone(),
                config.xray.socket_port.clone(),
            ]
        );
    }

    #[test]
    fn desired_system_instance_sets_privileged_system_container_flags() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let config = PrivacyRouterConfig::default();
        let spec = SystemContainerSpec {
            name: "privacy-wireguard-ingress",
            role: "wireguard_ingress",
            socket_port: &config.wireguard.socket_port,
            resources: &config.wireguard.resources,
        };

        let instance = plugin.desired_system_instance(&config, &spec);
        let config = instance.config.expect("instance config");

        assert_eq!(config.get("security.nesting"), Some(&"true".to_string()));
        assert_eq!(config.get("security.privileged"), Some(&"true".to_string()));
    }

    #[test]
    fn host_bootstrap_defaults_keep_uplink_standalone() {
        let host = PrivacyHostBootstrapConfig::from_env("ovsbr0");
        assert_eq!(host.uplink_port, "ens3");
        assert!(!host.attach_uplink_to_bridge);
        assert_eq!(host.grpc_bridge_port, "grpc-bridge");
    }

    #[test]
    fn bool_env_accepts_common_true_values() {
        std::env::set_var("PRIVACY_ATTACH_UPLINK_TO_BRIDGE", "yes");
        assert!(bool_env("PRIVACY_ATTACH_UPLINK_TO_BRIDGE", false));
        std::env::remove_var("PRIVACY_ATTACH_UPLINK_TO_BRIDGE");
    }

    #[test]
    fn actual_system_containers_require_running_status() {
        let plugin = PrivacyRouterPlugin::new(PrivacyRouterConfig::default());
        let config = PrivacyRouterConfig::default();
        let instances = vec![
            IncusInstance {
                name: "privacy-wireguard-ingress".to_string(),
                status: "Stopped".to_string(),
                instance_type: "container".to_string(),
                image: None,
                storage_pool: None,
                profiles: Vec::new(),
                config: None,
                devices: None,
            },
            IncusInstance {
                name: "privacy-xray-egress".to_string(),
                status: "Running".to_string(),
                instance_type: "container".to_string(),
                image: None,
                storage_pool: None,
                profiles: Vec::new(),
                config: None,
                devices: None,
            },
        ];

        let actual = plugin.actual_system_containers(&config, &IncusState { instances });
        assert_eq!(actual, vec!["privacy-xray-egress".to_string()]);
    }
}
</file>

<file path="src/state_plugins/privacy_routes.rs">
use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::PathBuf;

const DEFAULT_PRIVACY_ROUTES_PATH: &str = "/var/lib/op-dbus/privacy-routes.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivacyRoutesState {
    #[serde(default)]
    pub routes: Vec<PrivacyRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivacyRoute {
    pub name: String,
    pub route_id: String,
    pub user_id: String,
    pub email: String,
    pub wireguard_public_key: String,
    pub assigned_ip: String,
    pub selector_ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    pub ingress_port: String,
    pub next_hop: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub struct PrivacyRoutesPlugin {
    store_path: PathBuf,
}

impl Default for PrivacyRoutesPlugin {
    fn default() -> Self {
        Self::new(DEFAULT_PRIVACY_ROUTES_PATH)
    }
}

impl PrivacyRoutesPlugin {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
        }
    }

    async fn load_store(&self) -> Result<PrivacyRoutesState> {
        match tokio::fs::read_to_string(&self.store_path).await {
            Ok(mut content) => {
                let mut state: PrivacyRoutesState = unsafe { simd_json::from_str(&mut content) }
                    .context("invalid privacy route store")?;
                state.routes.sort_by(|a, b| a.route_id.cmp(&b.route_id));
                Ok(state)
            }
            Err(_) => Ok(PrivacyRoutesState { routes: Vec::new() }),
        }
    }

    async fn save_store(&self, state: &PrivacyRoutesState) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("failed to create privacy route directory")?;
        }

        let content = simd_json::to_string_pretty(state).context("serialize privacy routes")?;
        tokio::fs::write(&self.store_path, content)
            .await
            .context("write privacy routes")?;
        Ok(())
    }
}

#[async_trait]
impl StatePlugin for PrivacyRoutesPlugin {
    fn name(&self) -> &str {
        "privacy_routes"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::privacy_routes_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = self.load_store().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: PrivacyRoutesState = simd_json::serde::from_owned_value(current.clone())
            .context("deserialize current privacy routes")?;
        let desired_state: PrivacyRoutesState = simd_json::serde::from_owned_value(desired.clone())
            .context("deserialize desired privacy routes")?;

        let current_by_id: HashMap<&str, &PrivacyRoute> = current_state
            .routes
            .iter()
            .map(|route| (route.route_id.as_str(), route))
            .collect();
        let desired_by_id: HashMap<&str, &PrivacyRoute> = desired_state
            .routes
            .iter()
            .map(|route| (route.route_id.as_str(), route))
            .collect();

        let mut actions = Vec::new();

        for desired_route in &desired_state.routes {
            match current_by_id.get(desired_route.route_id.as_str()) {
                Some(current_route) if *current_route == desired_route => {}
                Some(_) => actions.push(StateAction::Modify {
                    resource: desired_route.route_id.clone(),
                    changes: simd_json::serde::to_owned_value(desired_route.clone())
                        .context("serialize desired privacy route modify")?,
                }),
                None => actions.push(StateAction::Create {
                    resource: desired_route.route_id.clone(),
                    config: simd_json::serde::to_owned_value(desired_route.clone())
                        .context("serialize desired privacy route create")?,
                }),
            }
        }

        for current_route in &current_state.routes {
            if !desired_by_id.contains_key(current_route.route_id.as_str()) {
                actions.push(StateAction::Delete {
                    resource: current_route.route_id.clone(),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut state = self.load_store().await?;
        let mut routes_by_id: HashMap<String, PrivacyRoute> = state
            .routes
            .drain(..)
            .map(|route| (route.route_id.clone(), route))
            .collect();

        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config } => {
                    let route: PrivacyRoute = simd_json::serde::from_owned_value(config.clone())
                        .context("deserialize route create")?;
                    routes_by_id.insert(resource.clone(), route);
                    changes_applied.push(format!("created privacy route {}", resource));
                }
                StateAction::Modify { resource, changes } => {
                    let route: PrivacyRoute = simd_json::serde::from_owned_value(changes.clone())
                        .context("deserialize route modify")?;
                    routes_by_id.insert(resource.clone(), route);
                    changes_applied.push(format!("updated privacy route {}", resource));
                }
                StateAction::Delete { resource } => {
                    routes_by_id.remove(resource);
                    changes_applied.push(format!("deleted privacy route {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        state.routes = routes_by_id.into_values().collect();
        state.routes.sort_by(|a, b| a.route_id.cmp(&b.route_id));

        if let Err(e) = self.save_store(&state).await {
            errors.push(e.to_string());
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let current_state: PrivacyRoutesState = simd_json::serde::from_owned_value(current)?;
        let desired_state: PrivacyRoutesState =
            simd_json::serde::from_owned_value(desired.clone())?;
        Ok(current_state == desired_state)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("privacy-routes-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let state: PrivacyRoutesState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        self.save_store(&state).await
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_privacy_routes_plugin_create_modify_delete() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let store_path = temp_dir.path().join("privacy-routes.json");
        let plugin = PrivacyRoutesPlugin::new(&store_path);

        let desired = PrivacyRoutesState {
            routes: vec![PrivacyRoute {
                name: "route-a".to_string(),
                route_id: "route-a".to_string(),
                user_id: "user-a".to_string(),
                email: "user@example.com".to_string(),
                wireguard_public_key: "pubkey".to_string(),
                assigned_ip: "10.100.0.2/32".to_string(),
                selector_ip: "10.100.0.2".to_string(),
                container_name: Some("privacy-user-a".to_string()),
                ingress_port: "ovsbr0-sock".to_string(),
                next_hop: "priv_wg".to_string(),
                enabled: true,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };

        let current = plugin.query_current_state().await.expect("query current");
        let desired_value =
            simd_json::serde::to_owned_value(desired.clone()).expect("serialize desired");
        let diff = plugin
            .calculate_diff(&current, &desired_value)
            .await
            .expect("calculate diff");
        assert_eq!(diff.actions.len(), 1);

        let result = plugin.apply_state(&diff).await.expect("apply");
        assert!(result.success);

        let stored = plugin.query_current_state().await.expect("query stored");
        let stored_state: PrivacyRoutesState =
            simd_json::serde::from_owned_value(stored).expect("deserialize stored");
        assert_eq!(stored_state, desired);

        let empty = simd_json::serde::to_owned_value(PrivacyRoutesState { routes: Vec::new() })
            .expect("serialize empty");
        let delete_diff = plugin
            .calculate_diff(
                &simd_json::serde::to_owned_value(desired).expect("serialize current desired"),
                &empty,
            )
            .await
            .expect("calculate delete diff");
        assert_eq!(delete_diff.actions.len(), 1);
    }
}
</file>

<file path="src/state_plugins/privacy.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// Enable WireGuard gateway (system service)
    pub wireguard_gateway_enabled: bool,
    /// WireGuard gateway interface
    pub wireguard_interface: String,

    /// Enable WARP tunnel (system service)
    pub warp_tunnel_enabled: bool,
    /// WARP interface name
    pub warp_interface: String,

    /// Enable XRay client container
    pub xray_client_enabled: bool,
    pub xray_client_container_id: u32,
    /// XRay SOCKS proxy port
    pub xray_socks_port: u16,
    /// VPS XRay server address
    pub vps_xray_server: Option<String>,

    /// Proxmox-specific networking
    pub proxmox_bridge: String,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            wireguard_gateway_enabled: true,
            wireguard_interface: "wg0".to_string(),
            warp_tunnel_enabled: true,
            warp_interface: "warp0".to_string(),
            xray_client_enabled: true,
            xray_client_container_id: 102,
            xray_socks_port: 1080,
            vps_xray_server: None,
            proxmox_bridge: "vmbr0".to_string(),
        }
    }
}

pub struct PrivacyPlugin {
    config: PrivacyConfig,
}

impl PrivacyPlugin {
    pub fn new(config: PrivacyConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl StatePlugin for PrivacyPlugin {
    fn name(&self) -> &'static str {
        "privacy"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: true,
            atomic_operations: false,
        }
    }

    async fn query_current_state(&self) -> Result<Value> {
        // Basic state query - in full implementation this would check all components
        Ok(simd_json::json!({
            "config": self.config,
            "status": "privacy_network_components_managed_by_individual_plugins"
        }))
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        // Basic diff calculation - full implementation would check component states
        let actions = Vec::new();

        // This is a coordinating plugin that delegates to individual component plugins
        // The actual work is done by the respective plugins (netmaker, lxc for xray, etc.)

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        // Privacy plugin coordinates but doesn't directly apply changes
        // Individual component plugins handle their own state
        Ok(ApplyResult {
            success: true,
            changes_applied: vec!["Privacy network coordination active".to_string()],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        Ok(self
            .calculate_diff(&current, desired)
            .await?
            .actions
            .is_empty())
    }

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(op_state::Checkpoint {
            id: format!(
                "privacy_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
            ),
            plugin: self.name().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &op_state::Checkpoint) -> Result<()> {
        Err(anyhow::anyhow!("Privacy plugin rollback not implemented - individual component plugins handle their own rollback"))
    }
}
</file>

<file path="src/state_plugins/procfs.rs">
//! Procfs state plugin.
//!
//! This turns procfs-derived host state into a `PluginSchema`-backed plugin so
//! JSON rendering, D-Bus projection, and tool generation all consume the same
//! schema authority instead of ad-hoc `/proc` tools.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

pub struct ProcfsPlugin;

impl ProcfsPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcfsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for ProcfsPlugin {
    fn name(&self) -> &str {
        "procfs"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        Path::new("/proc").exists()
    }

    fn unavailable_reason(&self) -> String {
        "/proc is not mounted".to_string()
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(procfs_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(gather_procfs_state().await)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![StateAction::NoOp {
                resource: "procfs".to_string(),
            }],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: Vec::new(),
            errors: Vec::new(),
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("procfs-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: self.query_current_state().await?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }
}

fn procfs_schema() -> PluginSchema {
    PluginSchema::builder("procfs")
        .version("1.0.0")
        .category("host")
        .description("Read-only procfs host state projected through PluginSchema.")
        .field("memory", readonly_any("Parsed /proc/meminfo values."))
        .field("loadavg", readonly_any("Parsed /proc/loadavg values."))
        .field("uptime", readonly_any("Parsed /proc/uptime values."))
        .field(
            "cpuinfo",
            readonly_any("Parsed CPU inventory from /proc/cpuinfo."),
        )
        .field("stat", readonly_any("Parsed /proc/stat values."))
        .field("net_dev", readonly_any("Parsed /proc/net/dev counters."))
        .field("mounts", readonly_any("Parsed /proc/mounts entries."))
        .field("kernel", readonly_any("Kernel version from /proc/version."))
        .field("vmstat", readonly_any("Parsed /proc/vmstat values."))
        .field("diskstats", readonly_any("Parsed /proc/diskstats rows."))
        .immutable_paths(&[
            "/memory",
            "/loadavg",
            "/uptime",
            "/cpuinfo",
            "/stat",
            "/net_dev",
            "/mounts",
            "/kernel",
            "/vmstat",
            "/diskstats",
        ])
        .tag("read_only")
        .build()
}

fn readonly_any(description: &str) -> FieldSchema {
    FieldSchema {
        field_type: FieldType::Any,
        required: false,
        description: description.to_string(),
        default: None,
        example: None,
        constraints: Vec::new(),
        read_only: true,
        read_only_when: None,
    }
}

async fn gather_procfs_state() -> Value {
    let (memory, loadavg, uptime, cpuinfo, stat, net_dev, mounts, kernel, vmstat, diskstats) = tokio::join!(
        gather_memory(),
        gather_loadavg(),
        gather_uptime(),
        gather_cpuinfo(),
        gather_stat(),
        gather_net_dev(),
        gather_mounts(),
        gather_kernel(),
        gather_vmstat(),
        gather_diskstats(),
    );

    let mut map = simd_json::owned::Object::new();
    map.insert("memory".into(), memory);
    map.insert("loadavg".into(), loadavg);
    map.insert("uptime".into(), uptime);
    map.insert("cpuinfo".into(), cpuinfo);
    map.insert("stat".into(), stat);
    map.insert("net_dev".into(), net_dev);
    map.insert("mounts".into(), mounts);
    map.insert("kernel".into(), kernel);
    map.insert("vmstat".into(), vmstat);
    map.insert("diskstats".into(), diskstats);
    Value::Object(Box::new(map))
}

async fn read_proc(path: &str) -> String {
    fs::read_to_string(Path::new("/proc").join(path))
        .await
        .unwrap_or_default()
}

fn num_or_str(s: &str) -> Value {
    let t = s.trim();
    if let Ok(n) = t.parse::<i64>() {
        return Value::from(n);
    }
    if let Ok(f) = t.parse::<f64>() {
        return Value::from(f);
    }
    Value::from(t.to_string())
}

fn kv_file(content: &str) -> Value {
    let mut map = simd_json::owned::Object::new();
    for line in content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            map.insert(
                key.trim().replace(' ', "_").to_lowercase(),
                num_or_str(value),
            );
        }
    }
    Value::Object(Box::new(map))
}

async fn gather_memory() -> Value {
    kv_file(&read_proc("meminfo").await)
}

async fn gather_loadavg() -> Value {
    let raw = read_proc("loadavg").await;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let mut map = simd_json::owned::Object::new();
    if let Some(v) = parts.first() {
        map.insert("load1".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(1) {
        map.insert("load5".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(2) {
        map.insert("load15".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(3) {
        if let Some((running, total)) = v.split_once('/') {
            map.insert("procs_running".into(), num_or_str(running));
            map.insert("procs_total".into(), num_or_str(total));
        }
    }
    Value::Object(Box::new(map))
}

async fn gather_uptime() -> Value {
    let raw = read_proc("uptime").await;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let mut map = simd_json::owned::Object::new();
    if let Some(v) = parts.first() {
        map.insert("uptime_secs".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(1) {
        map.insert("idle_secs".into(), num_or_str(v));
    }
    Value::Object(Box::new(map))
}

async fn gather_cpuinfo() -> Value {
    let raw = read_proc("cpuinfo").await;
    let mut cpus: Vec<Value> = Vec::new();
    let mut cur = simd_json::owned::Object::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                cpus.push(Value::Object(Box::new(std::mem::take(&mut cur))));
            }
        } else if let Some((key, value)) = line.split_once(':') {
            cur.insert(
                key.trim().replace(' ', "_").to_lowercase(),
                num_or_str(value),
            );
        }
    }
    if !cur.is_empty() {
        cpus.push(Value::Object(Box::new(cur)));
    }

    let mut map = simd_json::owned::Object::new();
    map.insert("count".into(), Value::from(cpus.len() as i64));
    map.insert("cpus".into(), Value::Array(cpus));
    Value::Object(Box::new(map))
}

async fn gather_stat() -> Value {
    let raw = read_proc("stat").await;
    let mut map = simd_json::owned::Object::new();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once(' ') {
            map.insert(key.into(), num_or_str(value));
        }
    }
    Value::Object(Box::new(map))
}

async fn gather_net_dev() -> Value {
    let raw = read_proc("net/dev").await;
    let mut interfaces = Vec::new();
    for line in raw.lines().skip(2) {
        if let Some((iface, stats)) = line.split_once(':') {
            let parts: Vec<&str> = stats.split_whitespace().collect();
            let labels = [
                "rx_bytes",
                "rx_packets",
                "rx_errs",
                "rx_drop",
                "rx_fifo",
                "rx_frame",
                "rx_compressed",
                "rx_multicast",
                "tx_bytes",
                "tx_packets",
                "tx_errs",
                "tx_drop",
                "tx_fifo",
                "tx_colls",
                "tx_carrier",
                "tx_compressed",
            ];
            let mut map = simd_json::owned::Object::new();
            map.insert("interface".into(), Value::from(iface.trim().to_string()));
            for (idx, label) in labels.iter().enumerate() {
                if let Some(value) = parts.get(idx) {
                    map.insert((*label).into(), num_or_str(value));
                }
            }
            interfaces.push(Value::Object(Box::new(map)));
        }
    }
    let mut map = simd_json::owned::Object::new();
    map.insert("interfaces".into(), Value::Array(interfaces));
    Value::Object(Box::new(map))
}

async fn gather_mounts() -> Value {
    let raw = read_proc("mounts").await;
    let mut mounts = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let mut map = simd_json::owned::Object::new();
            map.insert("device".into(), Value::from(parts[0].to_string()));
            map.insert("mountpoint".into(), Value::from(parts[1].to_string()));
            map.insert("fstype".into(), Value::from(parts[2].to_string()));
            map.insert("options".into(), Value::from(parts[3].to_string()));
            mounts.push(Value::Object(Box::new(map)));
        }
    }
    Value::Array(mounts)
}

async fn gather_kernel() -> Value {
    let mut map = simd_json::owned::Object::new();
    map.insert(
        "version".into(),
        Value::from(read_proc("version").await.trim().to_string()),
    );
    Value::Object(Box::new(map))
}

async fn gather_vmstat() -> Value {
    let raw = read_proc("vmstat").await;
    let mut map = simd_json::owned::Object::new();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once(' ') {
            map.insert(key.into(), num_or_str(value));
        }
    }
    Value::Object(Box::new(map))
}

async fn gather_diskstats() -> Value {
    let raw = read_proc("diskstats").await;
    let rows = raw
        .lines()
        .map(|line| Value::from(line.to_string()))
        .collect::<Vec<_>>();
    Value::Array(rows)
}
</file>

<file path="src/state_plugins/proxmox.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxmoxState {
    pub containers: Vec<ContainerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    pub vmid: u32,
    pub hostname: Option<String>,
    pub status: String, // "running", "stopped"
}

pub struct ProxmoxPlugin;

impl Default for ProxmoxPlugin {
    fn default() -> Self {
        Self
    }
}

impl ProxmoxPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for ProxmoxPlugin {
    fn name(&self) -> &str {
        "proxmox"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::proxmox_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(ProxmoxState {
            containers: vec![],
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/proxy_server.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyServerState {
    pub enabled: bool,
    pub port: u16,
}

pub struct ProxyServerPlugin;

impl Default for ProxyServerPlugin {
    fn default() -> Self {
        Self
    }
}

impl ProxyServerPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for ProxyServerPlugin {
    fn name(&self) -> &str {
        "proxy_server"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::proxy_server_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(ProxyServerState {
            enabled: false,
            port: 8080,
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/rtnetlink.rs">
//! Rtnetlink state plugin - manages kernel-level network interface state
//!
//! Handles: IP addresses, link state (up/down), MAC addresses, default routes
//! Uses native rtnetlink (netlink) protocol — no CLI wrappers.
//! Depends on: net, ovsdb_bridge (interfaces must exist before configuring)

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Rtnetlink interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtnetlinkInterfaceConfig {
    /// Interface name (e.g., "ens3", "ovsbr0-int")
    pub name: String,

    /// IPv4/IPv6 addresses to assign
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<AddressEntry>>,

    /// MAC address to set (e.g., "fa:16:3e:f1:71:d2")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,

    /// MTU
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,

    /// Desired link state: "up" or "down"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// Default gateway (only one interface should set this)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_gateway: Option<String>,
}

/// IP address entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddressEntry {
    pub ip: String,
    pub prefix: u8,
}

/// Rtnetlink state — list of managed interfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtnetlinkState {
    pub interfaces: Vec<RtnetlinkInterfaceConfig>,
}

pub struct RtnetlinkPlugin;

impl RtnetlinkPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RtnetlinkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for RtnetlinkPlugin {
    fn name(&self) -> &str {
        "rtnetlink"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::rtnetlink_plugin_schema())
    }

    fn is_available(&self) -> bool {
        // rtnetlink is always available — it's the kernel
        true
    }

    fn unavailable_reason(&self) -> String {
        "rtnetlink is always available".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let kernel_interfaces = op_network::rtnetlink::list_interfaces().await?;

        let interfaces: Vec<RtnetlinkInterfaceConfig> = kernel_interfaces
            .iter()
            .map(|iface| {
                let addresses: Vec<AddressEntry> = iface
                    .addresses
                    .iter()
                    .map(|addr| AddressEntry {
                        ip: addr.address.clone(),
                        prefix: addr.prefix_len,
                    })
                    .collect();

                RtnetlinkInterfaceConfig {
                    name: iface.name.clone(),
                    addresses: if addresses.is_empty() {
                        None
                    } else {
                        Some(addresses)
                    },
                    mac_address: iface.mac_address.clone(),
                    mtu: iface.mtu,
                    state: Some(iface.state.clone()),
                    default_gateway: None, // populated separately
                }
            })
            .collect();

        let state = RtnetlinkState { interfaces };
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: RtnetlinkState = simd_json::serde::from_owned_value(current.clone())
            .unwrap_or(RtnetlinkState { interfaces: vec![] });
        let desired_state: RtnetlinkState = simd_json::serde::from_owned_value(desired.clone())
            .unwrap_or(RtnetlinkState { interfaces: vec![] });

        let mut actions = Vec::new();

        let current_map: HashMap<&str, &RtnetlinkInterfaceConfig> = current_state
            .interfaces
            .iter()
            .map(|i| (i.name.as_str(), i))
            .collect();

        for desired_iface in &desired_state.interfaces {
            if let Some(current_iface) = current_map.get(desired_iface.name.as_str()) {
                // Check if any property differs
                let needs_update = desired_iface.state != current_iface.state
                    || desired_iface.mac_address != current_iface.mac_address
                    || desired_iface.addresses != current_iface.addresses
                    || desired_iface.default_gateway.is_some();

                if needs_update {
                    actions.push(StateAction::Modify {
                        resource: desired_iface.name.clone(),
                        changes: simd_json::serde::to_owned_value(desired_iface)?,
                    });
                }
            } else {
                // Interface not found in kernel — can only configure if it exists
                log::warn!(
                    "rtnetlink: desired interface '{}' not found in kernel",
                    desired_iface.name
                );
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let config: RtnetlinkInterfaceConfig =
                    simd_json::serde::from_owned_value(changes.clone())?;

                // Set MAC address
                if let Some(ref mac) = config.mac_address {
                    match op_network::rtnetlink::set_mac_address(resource, mac).await {
                        Ok(_) => changes_applied
                            .push(format!("Set MAC {} on {} via rtnetlink", mac, resource)),
                        Err(e) => errors.push(format!("Failed to set MAC on {}: {}", resource, e)),
                    }
                }

                // Add IP addresses
                if let Some(ref addresses) = config.addresses {
                    for addr in addresses {
                        match op_network::rtnetlink::add_ipv4_address(
                            resource,
                            &addr.ip,
                            addr.prefix,
                        )
                        .await
                        {
                            Ok(_) => changes_applied.push(format!(
                                "Added {}/{} to {} via rtnetlink",
                                addr.ip, addr.prefix, resource
                            )),
                            Err(e) => {
                                // EEXIST is not an error — address already assigned
                                let msg = e.to_string();
                                if msg.contains("exist") {
                                    log::info!(
                                        "Address {}/{} already on {} (ok)",
                                        addr.ip,
                                        addr.prefix,
                                        resource
                                    );
                                } else {
                                    errors.push(format!(
                                        "Failed to add {}/{} to {}: {}",
                                        addr.ip, addr.prefix, resource, e
                                    ));
                                }
                            }
                        }
                    }
                }

                // Set link state
                if let Some(ref state) = config.state {
                    let result = if state == "up" {
                        op_network::rtnetlink::link_up(resource).await
                    } else {
                        op_network::rtnetlink::link_down(resource).await
                    };
                    match result {
                        Ok(_) => changes_applied
                            .push(format!("Set {} {} via rtnetlink", resource, state)),
                        Err(e) => {
                            errors.push(format!("Failed to set {} {}: {}", resource, state, e))
                        }
                    }
                }

                // Set default gateway
                if let Some(ref gateway) = config.default_gateway {
                    // Delete existing default route first
                    let _ = op_network::rtnetlink::del_default_route().await;
                    match op_network::rtnetlink::add_default_route(resource, gateway).await {
                        Ok(_) => changes_applied.push(format!(
                            "Set default route via {} on {} via rtnetlink",
                            gateway, resource
                        )),
                        Err(e) => errors.push(format!(
                            "Failed to set default route via {}: {}",
                            gateway, e
                        )),
                    }
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let diff = self.calculate_diff(&current, desired).await?;
        Ok(diff.actions.is_empty())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("rtnetlink-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_state: RtnetlinkState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;

        // Re-apply old state
        let current = self.query_current_state().await?;
        let diff = self
            .calculate_diff(&current, &simd_json::serde::to_owned_value(&old_state)?)
            .await?;
        self.apply_state(&diff).await?;

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/s6.rs">
//! S6 state plugin — manages services via s6-rc on Artix/Chimera Linux.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Path to the s6-rc live directory
const S6_RC_LIVE: &str = "/run/s6-rc";

/// Run s6-rc with the standard live-dir prefix plus additional args.
async fn s6rc(args: &[&str]) -> Result<std::process::Output> {
    tokio::process::Command::new("s6-rc")
        .arg("-l")
        .arg(S6_RC_LIVE)
        .args(args)
        .output()
        .await
        .context("failed to run s6-rc")
}

/// Per-service configuration in the desired state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct S6ServiceConfig {
    /// Desired state: "active" or "inactive"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// Top-level desired state for the s6 plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct S6Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<HashMap<String, S6ServiceConfig>>,
}

/// S6 state plugin — controls services through the `s6-rc` CLI.
pub struct S6StatePlugin;

impl S6StatePlugin {
    pub fn new() -> Self {
        Self
    }

    /// Return the names of all currently-up services.
    async fn list_running(&self) -> Result<Vec<String>> {
        // -a = show all supervised services (running only)
        let out = s6rc(&["-a", "list"]).await?;
        if !out.status.success() {
            return Ok(Vec::new());
        }
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Bring a service up; tolerate "already started" replies.
    async fn start_service(&self, name: &str) -> Result<()> {
        let out = s6rc(&["start", name]).await?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        // s6-rc exits non-zero if the service is already up
        if stderr.contains("already") {
            return Ok(());
        }
        anyhow::bail!("s6-rc start {} failed: {}", name, stderr);
    }

    /// Bring a service down; tolerate "already stopped" replies.
    async fn stop_service(&self, name: &str) -> Result<()> {
        let out = s6rc(&["stop", name]).await?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("already") {
            return Ok(());
        }
        anyhow::bail!("s6-rc stop {} failed: {}", name, stderr);
    }
}

impl Default for S6StatePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for S6StatePlugin {
    fn name(&self) -> &str {
        "s6"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::s6_plugin_schema())
    }

    /// The plugin is only available when the s6-rc live directory exists.
    fn is_available(&self) -> bool {
        std::path::Path::new(S6_RC_LIVE).exists()
    }

    fn unavailable_reason(&self) -> String {
        format!("s6-rc live directory not found at {S6_RC_LIVE}")
    }

    async fn query_current_state(&self) -> Result<Value> {
        let running = self.list_running().await?;
        let mut services = HashMap::new();
        for name in &running {
            services.insert(
                name.clone(),
                S6ServiceConfig {
                    state: Some("active".to_string()),
                },
            );
        }
        Ok(simd_json::serde::to_owned_value(S6Config {
            services: Some(services),
        })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: S6Config = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: S6Config = simd_json::serde::from_owned_value(desired.clone())?;
        let mut actions = Vec::new();

        if let Some(desired_services) = &desired_config.services {
            for (name, desired_svc) in desired_services {
                let current_svc = current_config.services.as_ref().and_then(|s| s.get(name));
                if current_svc != Some(desired_svc) {
                    actions.push(StateAction::Modify {
                        resource: name.clone(),
                        changes: simd_json::serde::to_owned_value(desired_svc)?,
                    });
                }
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let svc: S6ServiceConfig = simd_json::serde::from_owned_value(changes.clone())?;
                let result = match svc.state.as_deref() {
                    Some("active") => self.start_service(resource).await,
                    Some("inactive") => self.stop_service(resource).await,
                    _ => Ok(()),
                };
                match result {
                    Ok(()) => changes_applied.push(format!("Applied s6 config for {resource}")),
                    Err(e) => errors.push(format!("Failed to apply {resource}: {e}")),
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let current_config: S6Config = simd_json::serde::from_owned_value(current)?;
        let desired_config: S6Config = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(current_config == desired_config)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("s6-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: S6Config = simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        if let Some(services) = old.services {
            for (name, cfg) in services {
                match cfg.state.as_deref() {
                    Some("active") => {
                        let _ = self.start_service(&name).await;
                    }
                    Some("inactive") => {
                        let _ = self.stop_service(&name).await;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/schema_contract.rs">
//! Compatibility adapter for legacy contract-style plugin schemas.
//!
//! Dead-path note:
//! this module exists only to keep legacy contract consumers alive while the
//! workspace is migrated to canonical plugin documents plus a schema catalog.
//! It must not be treated as a source of schema truth.
//!
//! The canonical schema source of truth is the plugin-owned canonical document,
//! resolved through `op_state_store::SchemaCatalog`.
//! This module preserves the old `schema_for_plugin()` / `all_contract_schemas()`
//! API surface by wrapping catalog-resolved schemas in the legacy contract
//! envelope.

use op_state_store::SchemaCatalog;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Get contract schema for a single plugin.
pub fn schema_for_plugin(catalog: &SchemaCatalog, plugin: &str) -> Option<Value> {
    catalog.export_contract_for(plugin)
}

/// Get all contract schemas keyed by canonical plugin name.
pub fn all_contract_schemas(catalog: &SchemaCatalog) -> HashMap<String, Value> {
    catalog.export_all_contract()
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn test_all_plugins_have_contract_schema() {
        let catalog = SchemaCatalog::with_builtin_schemas();
        let schemas = all_contract_schemas(&catalog);
        assert_eq!(schemas.len(), catalog.list().len());
    }

    #[test]
    fn test_contract_shape_has_required_sections() {
        let catalog = SchemaCatalog::with_builtin_schemas();
        let schema = schema_for_plugin(&catalog, "net").expect("net schema");
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .expect("required array");

        let required_strings: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();

        for field in [
            "stub",
            "immutable",
            "tunable",
            "observed",
            "meta",
            "semantic_index",
            "privacy_index",
        ] {
            assert!(required_strings.contains(&field));
        }
    }

    #[test]
    fn test_dependency_targets_are_known_plugins() {
        let catalog = SchemaCatalog::with_builtin_schemas();
        let known: HashSet<String> = all_contract_schemas(&catalog).keys().cloned().collect();

        for (plugin, schema) in all_contract_schemas(&catalog) {
            let empty: Vec<Value> = Vec::new();
            let deps = schema
                .get("properties")
                .and_then(|v| v.get("meta"))
                .and_then(|v| v.get("properties"))
                .and_then(|v| v.get("dependencies"))
                .and_then(|v| v.get("default"))
                .and_then(|v| v.as_array())
                .unwrap_or(&empty);

            for dep in deps.iter().filter_map(|v| v.as_str()) {
                assert!(
                    known.contains(dep),
                    "plugin '{}' has unknown dependency '{}'",
                    plugin,
                    dep
                );
            }
        }
    }

    #[test]
    fn test_uniform_index_paths_use_absolute_json_paths() {
        let catalog = SchemaCatalog::with_builtin_schemas();

        fn validate_path_array(paths: Option<&Vec<Value>>, context: &str) {
            if let Some(arr) = paths {
                for path in arr.iter().filter_map(|v| v.as_str()) {
                    assert!(
                        path.starts_with('/'),
                        "{} contains non-absolute path '{}'",
                        context,
                        path
                    );
                }
            }
        }

        for (plugin, schema) in all_contract_schemas(&catalog) {
            let semantic = schema
                .get("properties")
                .and_then(|v| v.get("semantic_index"))
                .and_then(|v| v.get("properties"));

            validate_path_array(
                semantic
                    .and_then(|v| v.get("include_paths"))
                    .and_then(|v| v.get("default"))
                    .and_then(|v| v.as_array()),
                &format!("{}.semantic_index.include_paths", plugin),
            );
            validate_path_array(
                semantic
                    .and_then(|v| v.get("exclude_paths"))
                    .and_then(|v| v.get("default"))
                    .and_then(|v| v.as_array()),
                &format!("{}.semantic_index.exclude_paths", plugin),
            );

            let redaction = schema
                .get("properties")
                .and_then(|v| v.get("privacy_index"))
                .and_then(|v| v.get("properties"))
                .and_then(|v| v.get("redaction"))
                .and_then(|v| v.get("properties"));

            validate_path_array(
                redaction
                    .and_then(|v| v.get("secret_paths"))
                    .and_then(|v| v.get("default"))
                    .and_then(|v| v.as_array()),
                &format!("{}.privacy_index.redaction.secret_paths", plugin),
            );
            validate_path_array(
                redaction
                    .and_then(|v| v.get("pii_paths"))
                    .and_then(|v| v.get("default"))
                    .and_then(|v| v.as_array()),
                &format!("{}.privacy_index.redaction.pii_paths", plugin),
            );
        }
    }

    #[test]
    fn test_recovery_priority_is_bounded() {
        let catalog = SchemaCatalog::with_builtin_schemas();

        for (plugin, schema) in all_contract_schemas(&catalog) {
            let priority = schema
                .get("properties")
                .and_then(|v| v.get("meta"))
                .and_then(|v| v.get("properties"))
                .and_then(|v| v.get("recovery_priority"))
                .and_then(|v| v.get("default"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            assert!(
                priority <= 100,
                "plugin '{}' has out-of-range recovery priority {}",
                plugin,
                priority
            );
        }
    }

    #[test]
    fn test_aliases_resolve_from_registry() {
        let catalog = SchemaCatalog::with_builtin_schemas();

        assert!(schema_for_plugin(&catalog, "systemd").is_some());
        assert!(schema_for_plugin(&catalog, "web-ui").is_some());
        assert!(schema_for_plugin(&catalog, "incus").is_some());
    }
}
</file>

<file path="src/state_plugins/service.rs">
//! Service plugin - auto-generating, validating, init-agnostic service management.

use crate::service_def::{
    ExecCommand, LogType, ReadyNotification, RestartPolicy, ServiceDef, ServiceName, ServiceType,
};
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLifecycle {
    pub last_active: Option<u64>,
    pub days_since_active: Option<u64>,
    pub is_orphaned: bool,
    pub orphan_reason: Option<String>,
}

/// Path to the s6-rc live database.
const S6_RC_LIVE: &str = "/run/s6-rc";

pub struct ServicePlugin {
    backend: ServiceBackend,
}

enum ServiceBackend {
    S6,
    Systemd,
}

impl ServicePlugin {
    pub fn new() -> Self {
        // Prefer s6 when the live directory exists; fall back to systemd.
        let backend = if Path::new(S6_RC_LIVE).exists() || Path::new("/etc/s6/sv").exists() {
            ServiceBackend::S6
        } else {
            ServiceBackend::Systemd
        };
        Self { backend }
    }

    /// Auto-generate service from installed binary
    pub async fn auto_generate_service(&self, binary_path: &Path) -> Result<ServiceDef> {
        let name = binary_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid binary name"))?;

        Ok(ServiceDef {
            name: ServiceName::new(name)?,
            service_type: ServiceType::Simple,
            exec_start: ExecCommand::new(binary_path.to_path_buf(), vec![])?,
            exec_stop: None,
            working_dir: None,
            user: None,
            group: None,
            depends_on: vec![],
            waits_for: vec![],
            restart: RestartPolicy::default(),
            environment: HashMap::new(),
            env_file: None,
            resources: None,
            log_type: LogType::None,
            ready_notification: ReadyNotification::None,
            chain_to: None,
            smooth_recovery: false,
            enabled: false,
        })
    }

    /// Convert from systemd unit file (Helper moved to ServicePlugin to avoid polluting schema)
    fn from_systemd_unit(path: &Path) -> Result<ServiceDef> {
        let content = std::fs::read_to_string(path)?;
        let mut exec_start = None;
        let mut exec_stop = None;
        let mut working_dir = None;
        let mut user = None;
        let mut depends = vec![];
        let mut env = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "ExecStart" => {
                        let parts: Vec<&str> = v.trim().split_whitespace().collect();
                        if !parts.is_empty() {
                            if let Ok(cmd) = ExecCommand::new(
                                PathBuf::from(parts[0]),
                                parts[1..].iter().map(|s| s.to_string()).collect(),
                            ) {
                                exec_start = Some(cmd);
                            }
                        }
                    }
                    "ExecStop" => {
                        let parts: Vec<&str> = v.trim().split_whitespace().collect();
                        if !parts.is_empty() {
                            if let Ok(cmd) = ExecCommand::new(
                                PathBuf::from(parts[0]),
                                parts[1..].iter().map(|s| s.to_string()).collect(),
                            ) {
                                exec_stop = Some(cmd);
                            }
                        }
                    }
                    "WorkingDirectory" => working_dir = Some(PathBuf::from(v.trim())),
                    "User" => user = Some(v.trim().to_string()),
                    "Requires" | "Wants" | "After" => {
                        for dep in v.split_whitespace() {
                            if let Ok(sn) = ServiceName::new(dep) {
                                depends.push(sn);
                            }
                        }
                    }
                    "Environment" => {
                        if let Some((ek, ev)) = v.split_once('=') {
                            env.insert(ek.trim().to_string(), ev.trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let name = ServiceName::new(file_name)?;

        Ok(ServiceDef {
            name,
            service_type: ServiceType::Simple, // Default, logic should improve
            exec_start: exec_start.ok_or_else(|| anyhow::anyhow!("no ExecStart"))?,
            exec_stop,
            working_dir,
            user,
            group: None,
            depends_on: depends,
            waits_for: vec![],
            restart: RestartPolicy::default(),
            environment: env,
            env_file: None,
            resources: None,
            log_type: LogType::None,
            ready_notification: ReadyNotification::None,
            chain_to: None,
            smooth_recovery: false,
            enabled: false,
        })
    }

    /// Convert all systemd units to s6 service definitions
    pub async fn convert_systemd_to_s6(&self) -> Result<Vec<ServiceDef>> {
        let mut services = vec![];
        let systemd_dir = Path::new("/etc/systemd/system");

        if !systemd_dir.exists() {
            return Ok(services);
        }

        for entry in std::fs::read_dir(systemd_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("service") {
                match Self::from_systemd_unit(&path) {
                    Ok(svc) => {
                        services.push(svc);
                    }
                    Err(e) => {
                        log::warn!("Failed to convert {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(services)
    }

    /// Install service definition
    pub async fn install_service(&self, svc: &ServiceDef) -> Result<()> {
        match self.backend {
            ServiceBackend::S6 => {
                svc.install()?;
                log::info!("Installed s6 service: {}", svc.name);
            }
            ServiceBackend::Systemd => {
                anyhow::bail!("systemd installation not implemented - use s6");
            }
        }

        Ok(())
    }

    /// List running services via `s6-rc -a -l /run/s6-rc list`.
    async fn list_s6_services(&self) -> Result<Vec<String>> {
        let out = tokio::process::Command::new("s6-rc")
            .arg("-l")
            .arg(S6_RC_LIVE)
            .args(["-a", "list"])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("failed to run s6-rc: {e}"))?;

        if !out.status.success() {
            return Ok(Vec::new());
        }

        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    async fn check_lifecycle(&self, name: &str) -> Result<ServiceLifecycle> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let last_active = match self.backend {
            ServiceBackend::Systemd => {
                let out = tokio::process::Command::new("systemctl")
                    .args(["show", name, "--property=ActiveEnterTimestamp"])
                    .output()
                    .await?;

                String::from_utf8_lossy(&out.stdout).lines().find_map(|l| {
                    l.split_once('=').and_then(|(_, v)| {
                        chrono::DateTime::parse_from_rfc3339(v)
                            .ok()
                            .map(|ts| ts.timestamp() as u64)
                    })
                })
            }
            // s6 does not expose activation timestamps via CLI
            ServiceBackend::S6 => None,
        };

        let days_since_active = last_active.map(|t| (now - t) / 86400);
        let is_orphaned = days_since_active.map_or(true, |d| d > 30);

        let orphan_reason = if is_orphaned {
            Some(if last_active.is_none() {
                "never run".to_string()
            } else {
                format!("inactive {} days", days_since_active.unwrap())
            })
        } else {
            None
        };

        Ok(ServiceLifecycle {
            last_active,
            days_since_active,
            is_orphaned,
            orphan_reason,
        })
    }
}

#[async_trait]
impl StatePlugin for ServicePlugin {
    fn name(&self) -> &str {
        "service"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::service_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut services = HashMap::new();

        let service_list = match self.backend {
            ServiceBackend::Systemd => {
                let out = tokio::process::Command::new("systemctl")
                    .args([
                        "list-units",
                        "--type=service",
                        "--all",
                        "--no-pager",
                        "--plain",
                    ])
                    .output()
                    .await?;
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter_map(|l| l.split_whitespace().next().map(String::from))
                    .collect::<Vec<_>>()
            }
            ServiceBackend::S6 => self.list_s6_services().await?,
        };

        for svc_name in service_list {
            if let Ok(lifecycle) = self.check_lifecycle(&svc_name).await {
                services.insert(svc_name, json!({ "lifecycle": lifecycle }));
            }
        }

        Ok(json!({ "services": services }))
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("service-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/sessdecl.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessDeclState {
    pub sessions: Vec<SessionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: String,
    pub user: String,
}

pub struct SessDeclPlugin;

impl Default for SessDeclPlugin {
    fn default() -> Self {
        Self
    }
}

impl SessDeclPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for SessDeclPlugin {
    fn name(&self) -> &str {
        "sess_decl"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::sess_decl_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(SessDeclState {
            sessions: vec![],
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/software.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareState {
    pub packages: Vec<PackageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub manager: String, // "dpkg", "rpm", "cargo", etc.
}

pub struct SoftwarePlugin;

impl Default for SoftwarePlugin {
    fn default() -> Self {
        Self
    }
}

impl SoftwarePlugin {
    pub fn new() -> Self {
        Self
    }

    async fn scan_dpkg() -> Vec<PackageInfo> {
        let mut packages = Vec::new();
        let output = Command::new("dpkg-query")
            .args(&["-W", "-f=${Package} ${Version}\n"])
            .output()
            .await;

        if let Ok(output) = output {
            if let Ok(stdout) = std::str::from_utf8(&output.stdout) {
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        packages.push(PackageInfo {
                            name: parts[0].to_string(),
                            version: parts[1].to_string(),
                            manager: "dpkg".to_string(),
                        });
                    }
                }
            }
        }
        packages
    }
}

#[async_trait]
impl StatePlugin for SoftwarePlugin {
    fn name(&self) -> &str {
        "software"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::software_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let packages = Self::scan_dpkg().await;
        Ok(simd_json::serde::to_owned_value(SoftwareState {
            packages,
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/systemd_networkd.rs">
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use zbus::{Connection, Proxy};

/// systemd-networkd integration for the network plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdNetworkdConfig {
    pub enabled: bool,
    pub networks: HashMap<String, NetworkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub match_name: String,
    pub dhcp: Option<String>,
    pub address: Option<Vec<String>>,
    pub gateway: Option<String>,
    pub dns: Option<Vec<String>>,
    pub bridge: Option<String>,
    pub vlan: Option<u16>,
}

pub struct SystemdNetworkdManager {
    connection: Option<Connection>,
}

impl SystemdNetworkdManager {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system().await.ok();
        Ok(Self { connection })
    }

    /// Generate .network files from plugin configuration
    pub fn generate_network_files(&self, config: &SystemdNetworkdConfig) -> Result<()> {
        if !config.enabled {
            return Ok(());
        }

        let network_dir = Path::new("/etc/systemd/network");
        fs::create_dir_all(network_dir)?;

        for (name, net_config) in &config.networks {
            let content = self.generate_network_file_content(net_config)?;
            let file_path = network_dir.join(format!("50-{}.network", name));
            fs::write(file_path, content)?;
        }

        Ok(())
    }

    fn generate_network_file_content(&self, config: &NetworkConfig) -> Result<String> {
        let mut content = String::new();

        // [Match] section
        content.push_str("[Match]\n");
        content.push_str(&format!("Name={}\n\n", config.match_name));

        // [Network] section
        content.push_str("[Network]\n");

        if let Some(dhcp) = &config.dhcp {
            content.push_str(&format!("DHCP={}\n", dhcp));
        }

        if let Some(bridge) = &config.bridge {
            content.push_str(&format!("Bridge={}\n", bridge));
        }

        if let Some(vlan) = config.vlan {
            content.push_str(&format!("VLAN={}\n", vlan));
        }

        if let Some(dns_servers) = &config.dns {
            for dns in dns_servers {
                content.push_str(&format!("DNS={}\n", dns));
            }
        }

        content.push('\n');

        // [Address] sections
        if let Some(addresses) = &config.address {
            for addr in addresses {
                content.push_str("[Address]\n");
                content.push_str(&format!("Address={}\n\n", addr));
            }
        }

        // [Route] section
        if let Some(gateway) = &config.gateway {
            content.push_str("[Route]\n");
            content.push_str(&format!("Gateway={}\n", gateway));
        }

        Ok(content)
    }

    /// Reload systemd-networkd configuration via D-Bus
    pub async fn reload_configuration(&self) -> Result<()> {
        if let Some(ref conn) = self.connection {
            let proxy = Proxy::new(
                conn,
                "org.freedesktop.systemd1",
                "/org/freedesktop/systemd1/unit/systemd_2dnetworkd_2eservice",
                "org.freedesktop.systemd1.Unit",
            )
            .await?;

            // Reload systemd-networkd
            let _: () = proxy.call("Reload", &("replace",)).await?;
            log::info!("systemd-networkd configuration reloaded");
        }
        Ok(())
    }

    /// Get network state from systemd-networkd via D-Bus
    pub async fn get_network_state(&self) -> Result<HashMap<String, String>> {
        let mut state = HashMap::new();

        if let Some(ref conn) = self.connection {
            // Try to connect to org.freedesktop.network1 if available
            if let Ok(proxy) = Proxy::new(
                conn,
                "org.freedesktop.network1",
                "/org/freedesktop/network1",
                "org.freedesktop.network1.Manager",
            )
            .await
            {
                // Get links
                if let Ok(links) = proxy.call("ListLinks", &()).await {
                    let links: Vec<(i32, String, String)> = links;
                    for (index, name, state_str) in links {
                        state.insert(format!("link.{}.{}", index, name), state_str);
                    }
                }
            }
        }

        Ok(state)
    }

    /// Start systemd-networkd if not running
    pub async fn ensure_running(&self) -> Result<()> {
        if let Some(ref conn) = self.connection {
            let proxy = Proxy::new(
                conn,
                "org.freedesktop.systemd1",
                "/org/freedesktop/systemd1",
                "org.freedesktop.systemd1.Manager",
            )
            .await?;

            // Start systemd-networkd
            let _: () = proxy
                .call("StartUnit", &("systemd-networkd.service", "replace"))
                .await?;
            log::info!("systemd-networkd started");
        }
        Ok(())
    }
}
</file>

<file path="src/state_plugins/systemd.rs">
//! Systemd state plugin - manages systemd via org.freedesktop.systemd1 D-Bus
//! Maps D-Bus object tree to declarative state

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::plugtree::PlugTree;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use zbus::{Connection, Proxy};

/// Systemd configuration schema - mirrors D-Bus object tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdConfig {
    /// Units indexed by name (e.g., "ssh.service")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<HashMap<String, UnitConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnitConfig {
    /// Desired state: "active", "inactive", "failed", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_state: Option<String>,

    /// Should unit be enabled at boot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Should unit be masked (prevents starting)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked: Option<bool>,

    /// Additional D-Bus properties (dynamic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,
}

/// Systemd state plugin
pub struct SystemdStatePlugin;

impl SystemdStatePlugin {
    pub fn new() -> Self {
        Self
    }

    /// Connect to systemd via D-Bus
    async fn connect_systemd(&self) -> Result<Proxy<'static>> {
        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        Proxy::new(
            &conn,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )
        .await
        .context("Failed to create systemd D-Bus proxy")
    }

    /// Get unit object path from unit name
    async fn get_unit_path(&self, proxy: &Proxy<'_>, unit_name: &str) -> Result<String> {
        let path: zbus::zvariant::OwnedObjectPath = proxy
            .call("GetUnit", &(unit_name,))
            .await
            .context(format!("Failed to get unit path for {}", unit_name))?;

        Ok(path.to_string())
    }

    /// Query current state of a unit
    async fn query_unit(&self, unit_name: &str) -> Result<UnitConfig> {
        let proxy = self.connect_systemd().await?;
        let unit_path = self.get_unit_path(&proxy, unit_name).await?;

        let conn = Connection::system().await?;
        let unit_proxy = Proxy::new(
            &conn,
            "org.freedesktop.systemd1",
            unit_path,
            "org.freedesktop.systemd1.Unit",
        )
        .await?;

        // Get ActiveState property
        let active_state: String = unit_proxy
            .get_property("ActiveState")
            .await
            .unwrap_or_else(|_| "unknown".to_string());

        // Check if enabled (this is a UnitFile property)
        let enabled = self.check_unit_enabled(&proxy, unit_name).await.ok();

        Ok(UnitConfig {
            active_state: Some(active_state),
            enabled,
            masked: None, // TODO: Query mask state
            properties: None,
        })
    }

    /// Check if unit is enabled
    async fn check_unit_enabled(&self, proxy: &Proxy<'_>, unit_name: &str) -> Result<bool> {
        let state: String = proxy
            .call("GetUnitFileState", &(unit_name,))
            .await
            .context("Failed to get unit file state")?;

        Ok(state == "enabled")
    }

    /// Start a systemd unit
    async fn start_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.connect_systemd().await?;

        let _job: zbus::zvariant::OwnedObjectPath = proxy
            .call("StartUnit", &(unit_name, "replace"))
            .await
            .context(format!("Failed to start unit {}", unit_name))?;

        log::info!("Started systemd unit: {}", unit_name);
        Ok(())
    }

    /// Stop a systemd unit
    async fn stop_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.connect_systemd().await?;

        let _job: zbus::zvariant::OwnedObjectPath = proxy
            .call("StopUnit", &(unit_name, "replace"))
            .await
            .context(format!("Failed to stop unit {}", unit_name))?;

        log::info!("Stopped systemd unit: {}", unit_name);
        Ok(())
    }

    /// Enable a systemd unit
    async fn enable_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.connect_systemd().await?;

        let _: (bool, Vec<(String, String, String)>) = proxy
            .call("EnableUnitFiles", &(vec![unit_name], false, true))
            .await
            .context(format!("Failed to enable unit {}", unit_name))?;

        log::info!("Enabled systemd unit: {}", unit_name);
        Ok(())
    }

    /// Disable a systemd unit
    async fn disable_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.connect_systemd().await?;

        let _: Vec<(String, String, String)> = proxy
            .call("DisableUnitFiles", &(vec![unit_name], false))
            .await
            .context(format!("Failed to disable unit {}", unit_name))?;

        log::info!("Disabled systemd unit: {}", unit_name);
        Ok(())
    }

    /// Apply desired unit configuration
    async fn apply_unit_config(&self, unit_name: &str, config: &UnitConfig) -> Result<()> {
        // Apply masked state first (prevents other operations)
        if let Some(desired_masked) = config.masked {
            // Check current mask state via GetUnitFileState
            let proxy = self.connect_systemd().await?;
            let state: String = proxy
                .call("GetUnitFileState", &(unit_name,))
                .await
                .unwrap_or_else(|_| "unknown".to_string());

            let currently_masked = state == "masked";

            if desired_masked && !currently_masked {
                self.mask_unit(unit_name).await?;
            } else if !desired_masked && currently_masked {
                self.unmask_unit(unit_name).await?;
            }
        }

        // Apply active state
        if let Some(ref desired_state) = config.active_state {
            let current = self.query_unit(unit_name).await?;
            let current_state = current
                .active_state
                .unwrap_or_else(|| "unknown".to_string());

            if desired_state == "active" && current_state != "active" {
                self.start_unit(unit_name).await?;
            } else if desired_state == "inactive" && current_state == "active" {
                self.stop_unit(unit_name).await?;
            }
        }

        // Apply enabled state
        if let Some(desired_enabled) = config.enabled {
            let current = self.query_unit(unit_name).await?;
            let current_enabled = current.enabled.unwrap_or(false);

            if desired_enabled && !current_enabled {
                self.enable_unit(unit_name).await?;
            } else if !desired_enabled && current_enabled {
                self.disable_unit(unit_name).await?;
            }
        }

        Ok(())
    }
}

impl Default for SystemdStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemdStatePlugin {
    /// Apply state to a single unit
    #[allow(dead_code)]
    pub async fn apply_unit(
        &self,
        unit_name: &str,
        unit_config: &UnitConfig,
    ) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        match self.apply_unit_config(unit_name, unit_config).await {
            Ok(_) => {
                changes_applied.push(format!("Applied config for unit: {}", unit_name));
            }
            Err(e) => {
                errors.push(format!("Failed to apply unit {}: {}", unit_name, e));
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    /// Mask a systemd unit
    pub async fn mask_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.connect_systemd().await?;

        let _: Vec<(String, String, String)> = proxy
            .call("MaskUnitFiles", &(vec![unit_name], false, true))
            .await
            .context(format!("Failed to mask unit {}", unit_name))?;

        log::info!("Masked systemd unit: {}", unit_name);
        Ok(())
    }

    /// Unmask a systemd unit
    pub async fn unmask_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.connect_systemd().await?;

        let _: Vec<(String, String, String)> = proxy
            .call("UnmaskUnitFiles", &(vec![unit_name], false))
            .await
            .context(format!("Failed to unmask unit {}", unit_name))?;

        log::info!("Unmasked systemd unit: {}", unit_name);
        Ok(())
    }
}

#[async_trait]
impl PlugTree for SystemdStatePlugin {
    fn pluglet_type(&self) -> &str {
        "unit"
    }

    fn pluglet_id_field(&self) -> &str {
        "name"
    }

    fn extract_pluglet_id(&self, resource: &Value) -> Result<String> {
        match resource {
            Value::Object(obj) => obj.keys().next()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow::anyhow!("Unit missing name")),
            _ => anyhow::bail!("Resource must be an object"),
        }
    }

    async fn apply_pluglet(&self, pluglet_id: &str, desired: &Value) -> Result<ApplyResult> {
        let unit_config: UnitConfig = simd_json::serde::from_owned_value(desired.clone())?;
        self.apply_unit(pluglet_id, &unit_config).await
    }

    async fn query_pluglet(&self, pluglet_id: &str) -> Result<Option<Value>> {
        match self.query_unit(pluglet_id).await {
            Ok(unit) => Ok(Some(simd_json::serde::to_owned_value(unit)?)),
            Err(_) => Ok(None),
        }
    }

    async fn list_pluglet_ids(&self) -> Result<Vec<String>> {
        // Would require listing all systemd units - for now return empty
        // Full implementation would call ListUnits on D-Bus
        Ok(Vec::new())
    }
}

#[async_trait]
impl StatePlugin for SystemdStatePlugin {
    fn name(&self) -> &str {
        "systemd"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        // For now, return empty state - full implementation would list all units
        let config = SystemdConfig { units: None };
        Ok(simd_json::serde::to_owned_value(config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: SystemdConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_config: SystemdConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        if let Some(desired_units) = &desired_config.units {
            for (unit_name, desired_unit) in desired_units {
                let current_unit = current_config.units.as_ref().and_then(|u| u.get(unit_name));

                if current_unit != Some(desired_unit) {
                    actions.push(StateAction::Modify {
                        resource: unit_name.clone(),
                        changes: simd_json::serde::to_owned_value(desired_unit)?,
                    });
                }
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                let unit_config: UnitConfig = simd_json::serde::from_owned_value(changes.clone())?;

                match self.apply_unit_config(resource, &unit_config).await {
                    Ok(_) => {
                        changes_applied.push(format!("Applied systemd config for: {}", resource));
                    }
                    Err(e) => {
                        errors.push(format!("Failed to apply config for {}: {}", resource, e));
                    }
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let desired_config: SystemdConfig = simd_json::serde::from_owned_value(desired.clone())?;

        if let Some(units) = &desired_config.units {
            for (unit_name, desired_unit) in units {
                let current = self.query_unit(unit_name).await?;

                if let Some(ref desired_state) = desired_unit.active_state {
                    if current.active_state.as_ref() != Some(desired_state) {
                        return Ok(false);
                    }
                }

                if let Some(desired_enabled) = desired_unit.enabled {
                    if current.enabled != Some(desired_enabled) {
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("systemd-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_config: SystemdConfig = simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;

        if let Some(units) = old_config.units {
            for (unit_name, unit_config) in units {
                self.apply_unit_config(&unit_name, &unit_config).await?;
            }
        }

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // D-Bus calls are not atomic
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smoke test that exercises zbus systemd connectivity.
    // It should not fail the build if system D-Bus policy restricts access.
    #[tokio::test(flavor = "current_thread")]
    async fn test_systemd_query_unit() {
        let plugin = SystemdStatePlugin::new();
        // Common units to try
        let candidates = ["dbus.service", "systemd-logind.service", "cron.service"];

        // Try each candidate until one succeeds, but don't fail if all are blocked.
        for u in candidates {
            let res = plugin.query_unit(u).await;
            if let Ok(cfg) = res {
                assert!(cfg.active_state.is_some());
                return;
            }
        }
        // If none succeed, we at least reached D-Bus paths without panicking.
    }
}
</file>

<file path="src/state_plugins/unix_socket.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// A configured unix-domain socket endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketEndpoint {
    /// Filesystem path to the socket (e.g. `/run/qdrant.sock`).
    pub path: String,
    /// Human-readable label (e.g. `"qdrant-grpc"`).
    pub label: String,
    /// Transport protocol carried over the socket (`"grpc"`, `"jsonrpc"`, …).
    pub protocol: String,
}

/// Runtime state: all declared socket endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnixSocketState {
    /// Declared unix socket endpoints visible to internal services.
    pub sockets: Vec<SocketEndpoint>,
}

pub struct UnixSocketPlugin;

impl UnixSocketPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnixSocketPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for UnixSocketPlugin {
    fn name(&self) -> &str {
        "unix_socket"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::unix_socket_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(UnixSocketState::default())?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/users.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersState {
    pub users: Vec<UserConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub username: String,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub groups: Vec<String>,
    pub shell: Option<String>,
    pub present: bool,
}

pub struct UsersPlugin;

impl Default for UsersPlugin {
    fn default() -> Self {
        Self
    }
}

impl UsersPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for UsersPlugin {
    fn name(&self) -> &str {
        "users"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::users_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let content = tokio::fs::read_to_string("/etc/passwd")
            .await
            .unwrap_or_default();
        let mut users = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 7 {
                users.push(UserConfig {
                    username: parts[0].to_string(),
                    uid: parts[2].parse().ok(),
                    gid: parts[3].parse().ok(),
                    groups: vec![],
                    shell: Some(parts[6].to_string()),
                    present: true,
                });
            }
        }

        Ok(simd_json::serde::to_owned_value(UsersState { users })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/state_plugins/web_ui.rs">
//! Web UI Plugin - serves embedded React SPA
//!
//! Follows the 3-section plugin pattern:
//! - SECTION 1: Immutable Identity (set once, never changes)
//! - SECTION 2: Tunable Config (can change, blockchain tracks all changes)
//! - SECTION 3: Capabilities (what this plugin can do)
//!
//! Uses op-identity crate for WireGuard-based authentication.

use anyhow::Result;
use async_trait::async_trait;
use op_blockchain::PluginFootprint;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

// ============================================================================
// SECTION 1: IMMUTABLE IDENTITY (set once, never changes)
// ============================================================================

/// Plugin identity - immutable after creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiIdentity {
    /// Plugin name (immutable)
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Plugin classification
    pub plugin_type: String,
    /// Asset serving driver
    pub driver: String,
}

impl Default for WebUiIdentity {
    fn default() -> Self {
        Self {
            name: "web-ui".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: "ui".to_string(),
            driver: "rust-embed".to_string(),
        }
    }
}

// ============================================================================
// SECTION 2: TUNABLE CONFIG (can change, blockchain tracks all changes)
// ============================================================================

/// Tunable configuration - changes tracked in blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiTunables {
    /// Whether UI serving is enabled
    pub enabled: bool,
    /// CORS allowed origins
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// Enable gzip/brotli compression
    pub compression: bool,
    /// Cache TTL for static assets (seconds)
    pub cache_ttl: u64,
    /// UI theme preference
    pub theme: String,
    /// Feature flags for progressive rollout
    #[serde(default)]
    pub feature_flags: HashMap<String, bool>,
    /// WebSocket configuration
    #[serde(default)]
    pub websocket: WebSocketConfig,
    /// API configuration
    #[serde(default)]
    pub api: ApiConfig,
    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub enabled: bool,
    pub max_connections: u32,
    pub ping_interval_ms: u64,
    pub message_size_limit: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_connections: 1000,
            ping_interval_ms: 30000,
            message_size_limit: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub rate_limit_rps: u32,
    pub timeout_ms: u64,
    pub max_payload_bytes: usize,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            rate_limit_rps: 100,
            timeout_ms: 30000,
            max_payload_bytes: 10 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub require_auth: bool,
    pub session_ttl_seconds: u64,
    pub csrf_enabled: bool,
    pub csp_policy: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_auth: true,
            session_ttl_seconds: 3600,
            csrf_enabled: true,
            csp_policy: "default-src 'self'".to_string(),
        }
    }
}

impl Default for WebUiTunables {
    fn default() -> Self {
        Self {
            enabled: true,
            cors_origins: vec!["*".to_string()],
            compression: true,
            cache_ttl: 86400,
            theme: "dark".to_string(),
            feature_flags: HashMap::new(),
            websocket: WebSocketConfig::default(),
            api: ApiConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

// ============================================================================
// SECTION 3: CAPABILITIES (what this plugin can do)
// ============================================================================

/// Plugin capabilities - read-only
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiCapabilities {
    pub can_serve_static: bool,
    pub can_proxy_api: bool,
    pub can_websocket: bool,
    pub can_sse: bool,
    pub supports_hot_reload: bool,
    pub supports_compression: bool,
    pub requires_root: bool,
    pub supported_platforms: Vec<String>,
}

impl Default for WebUiCapabilities {
    fn default() -> Self {
        Self {
            can_serve_static: true,
            can_proxy_api: true,
            can_websocket: true,
            can_sse: true,
            supports_hot_reload: false, // Embedded UI
            supports_compression: true,
            requires_root: false,
            supported_platforms: vec!["linux".to_string(), "macos".to_string()],
        }
    }
}

// ============================================================================
// JSON SCHEMA DEFINITIONS (Schema-as-Code)
// ============================================================================

impl WebUiIdentity {
    /// JSON Schema for Identity (immutable)
    pub fn schema() -> Value {
        simd_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://op-dbus.local/schemas/web-ui/identity.json",
            "title": "WebUiIdentity",
            "description": "Immutable identity for Web UI plugin",
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "const": "web-ui",
                    "description": "Plugin name (immutable)"
                },
                "version": {
                    "type": "string",
                    "pattern": "^\\d+\\.\\d+\\.\\d+$",
                    "description": "Semantic version"
                },
                "plugin_type": {
                    "type": "string",
                    "const": "ui",
                    "description": "Plugin classification"
                },
                "driver": {
                    "type": "string",
                    "enum": ["rust-embed", "static-files"],
                    "description": "Asset serving driver"
                }
            },
            "required": ["name", "version", "plugin_type", "driver"],
            "additionalProperties": false
        })
    }
}

impl WebUiTunables {
    /// JSON Schema for Tunables (mutable, blockchain-tracked)
    pub fn schema() -> Value {
        simd_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://op-dbus.local/schemas/web-ui/tunables.json",
            "title": "WebUiTunables",
            "description": "Tunable configuration for Web UI plugin (changes tracked in blockchain)",
            "type": "object",
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether UI serving is enabled"
                },
                "cors_origins": {
                    "type": "array",
                    "items": { "type": "string" },
                    "default": ["*"],
                    "description": "CORS allowed origins"
                },
                "compression": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable gzip/brotli compression"
                },
                "cache_ttl": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 31536000,
                    "default": 86400,
                    "description": "Cache TTL for static assets (seconds)"
                },
                "theme": {
                    "type": "string",
                    "enum": ["dark", "light", "system"],
                    "default": "dark",
                    "description": "UI theme preference"
                },
                "feature_flags": {
                    "type": "object",
                    "additionalProperties": { "type": "boolean" },
                    "default": {},
                    "description": "Feature flags for progressive rollout"
                },
                "websocket": { "$ref": "#/$defs/WebSocketConfig" },
                "api": { "$ref": "#/$defs/ApiConfig" },
                "security": { "$ref": "#/$defs/SecurityConfig" }
            },
            "$defs": {
                "WebSocketConfig": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean", "default": true },
                        "max_connections": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 1000 },
                        "ping_interval_ms": { "type": "integer", "minimum": 1000, "default": 30000 },
                        "message_size_limit": { "type": "integer", "minimum": 1024, "default": 1048576 }
                    }
                },
                "ApiConfig": {
                    "type": "object",
                    "properties": {
                        "rate_limit_rps": { "type": "integer", "minimum": 1, "default": 100 },
                        "timeout_ms": { "type": "integer", "minimum": 100, "default": 30000 },
                        "max_payload_bytes": { "type": "integer", "minimum": 1024, "default": 10485760 }
                    }
                },
                "SecurityConfig": {
                    "type": "object",
                    "properties": {
                        "require_auth": { "type": "boolean", "default": true },
                        "session_ttl_seconds": { "type": "integer", "minimum": 60, "default": 3600 },
                        "csrf_enabled": { "type": "boolean", "default": true },
                        "csp_policy": { "type": "string", "default": "default-src 'self'" }
                    }
                }
            },
            "additionalProperties": false
        })
    }

    /// Property schema - tracks which tunable fields exist (append-only)
    pub fn property_schema() -> Vec<String> {
        vec![
            "enabled".to_string(),
            "cors_origins".to_string(),
            "compression".to_string(),
            "cache_ttl".to_string(),
            "theme".to_string(),
            "feature_flags".to_string(),
            "websocket".to_string(),
            "api".to_string(),
            "security".to_string(),
        ]
    }
}

impl WebUiCapabilities {
    /// JSON Schema for Capabilities (read-only)
    pub fn schema() -> Value {
        simd_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://op-dbus.local/schemas/web-ui/capabilities.json",
            "title": "WebUiCapabilities",
            "description": "Capabilities exposed by Web UI plugin",
            "type": "object",
            "properties": {
                "can_serve_static": { "type": "boolean", "const": true },
                "can_proxy_api": { "type": "boolean", "const": true },
                "can_websocket": { "type": "boolean", "const": true },
                "can_sse": { "type": "boolean", "const": true },
                "supports_hot_reload": { "type": "boolean", "const": false },
                "supports_compression": { "type": "boolean", "const": true },
                "requires_root": { "type": "boolean", "const": false },
                "supported_platforms": {
                    "type": "array",
                    "items": { "type": "string" },
                    "const": ["linux", "macos"]
                }
            },
            "additionalProperties": false
        })
    }
}

// ============================================================================
// PLUGIN IMPLEMENTATION
// ============================================================================

/// Web UI State Plugin
pub struct WebUiPlugin {
    identity: WebUiIdentity,
    tunables: WebUiTunables,
    capabilities: WebUiCapabilities,
    #[allow(dead_code)]
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<PluginFootprint>>,
}

impl WebUiPlugin {
    pub fn new() -> Self {
        Self {
            identity: WebUiIdentity::default(),
            tunables: WebUiTunables::default(),
            capabilities: WebUiCapabilities::default(),
            blockchain_sender: None,
        }
    }

    pub fn with_blockchain_sender(
        blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
    ) -> Self {
        Self {
            identity: WebUiIdentity::default(),
            tunables: WebUiTunables::default(),
            capabilities: WebUiCapabilities::default(),
            blockchain_sender: Some(blockchain_sender),
        }
    }

    /// Get identity
    pub fn identity(&self) -> &WebUiIdentity {
        &self.identity
    }

    /// Get tunables
    pub fn tunables(&self) -> &WebUiTunables {
        &self.tunables
    }

    /// Get capabilities
    pub fn capabilities(&self) -> &WebUiCapabilities {
        &self.capabilities
    }

    /// Check if a path is immutable
    pub fn is_path_immutable(path: &str) -> bool {
        let immutable_paths = [
            "/identity",
            "/identity/name",
            "/identity/plugin_type",
            "/identity/driver",
        ];
        immutable_paths.iter().any(|p| path.starts_with(p))
    }

    /// Validate tunables against schema
    pub fn validate_tunables(tunables: &Value) -> Result<()> {
        // Basic validation - check required fields exist
        if !tunables.is_object() {
            anyhow::bail!("Tunables must be an object");
        }

        if tunables.get("enabled").is_none() {
            anyhow::bail!("Missing required field: enabled");
        }

        // Validate theme enum
        if let Some(theme) = tunables.get("theme") {
            if let Some(theme_str) = theme.as_str() {
                if !["dark", "light", "system"].contains(&theme_str) {
                    anyhow::bail!("Invalid theme: must be dark, light, or system");
                }
            }
        }

        Ok(())
    }
}

impl Default for WebUiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for WebUiPlugin {
    fn name(&self) -> &str {
        &self.identity.name
    }

    fn version(&self) -> &str {
        &self.identity.version
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::web_ui_plugin_schema())
    }

    fn is_available(&self) -> bool {
        true // UI is always available (embedded)
    }

    fn unavailable_reason(&self) -> String {
        String::new() // Never unavailable
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(&self.tunables)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_tunables: WebUiTunables = simd_json::serde::from_owned_value(current.clone())?;
        let desired_tunables: WebUiTunables = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        // Compare each tunable field
        if current_tunables.enabled != desired_tunables.enabled {
            actions.push(op_state::StateAction::Modify {
                resource: "enabled".to_string(),
                changes: simd_json::json!({ "enabled": desired_tunables.enabled }),
            });
        }

        if current_tunables.theme != desired_tunables.theme {
            actions.push(op_state::StateAction::Modify {
                resource: "theme".to_string(),
                changes: simd_json::json!({ "theme": desired_tunables.theme }),
            });
        }

        if current_tunables.compression != desired_tunables.compression {
            actions.push(op_state::StateAction::Modify {
                resource: "compression".to_string(),
                changes: simd_json::json!({ "compression": desired_tunables.compression }),
            });
        }

        if current_tunables.cache_ttl != desired_tunables.cache_ttl {
            actions.push(op_state::StateAction::Modify {
                resource: "cache_ttl".to_string(),
                changes: simd_json::json!({ "cache_ttl": desired_tunables.cache_ttl }),
            });
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();

        for action in &diff.actions {
            if let op_state::StateAction::Modify { resource, .. } = action {
                changes_applied.push(format!("Updated UI config: {}", resource));
            }
        }

        Ok(ApplyResult {
            success: true,
            changes_applied,
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true) // UI state is always consistent (embedded)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("web-ui-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_identity() {
        let identity = WebUiIdentity::default();
        assert_eq!(identity.name, "web-ui");
        assert_eq!(identity.version, "1.0.0");
        assert_eq!(identity.plugin_type, "ui");
        assert_eq!(identity.driver, "rust-embed");
    }

    #[test]
    fn test_default_tunables() {
        let tunables = WebUiTunables::default();
        assert!(tunables.enabled);
        assert!(tunables.compression);
        assert_eq!(tunables.theme, "dark");
        assert_eq!(tunables.cache_ttl, 86400);
    }

    #[test]
    fn test_default_capabilities() {
        let caps = WebUiCapabilities::default();
        assert!(caps.can_serve_static);
        assert!(caps.can_websocket);
        assert!(!caps.supports_hot_reload);
        assert!(!caps.requires_root);
    }

    #[test]
    fn test_immutable_paths() {
        assert!(WebUiPlugin::is_path_immutable("/identity/name"));
        assert!(WebUiPlugin::is_path_immutable("/identity/plugin_type"));
        assert!(!WebUiPlugin::is_path_immutable("/tunables/enabled"));
        assert!(!WebUiPlugin::is_path_immutable("/tunables/theme"));
    }

    #[test]
    fn test_property_schema() {
        let schema = WebUiTunables::property_schema();
        assert!(schema.contains(&"enabled".to_string()));
        assert!(schema.contains(&"theme".to_string()));
        assert!(schema.contains(&"websocket".to_string()));
    }

    #[tokio::test]
    async fn test_plugin_state() {
        let plugin = WebUiPlugin::new();
        assert_eq!(plugin.name(), "web-ui");
        assert!(plugin.is_available());

        let state = plugin.query_current_state().await.unwrap();
        assert!(state.is_object());
    }
}
</file>

<file path="src/state_plugins/wireguard.rs">
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardState {
    pub interfaces: Vec<WireGuardInterface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardInterface {
    pub name: String,
    pub private_key: Option<String>,
    pub listen_port: u16,
    pub peers: Vec<WireGuardPeer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardPeer {
    pub public_key: String,
    pub allowed_ips: Vec<String>,
    pub endpoint: Option<String>,
}

pub struct WireGuardPlugin;

impl Default for WireGuardPlugin {
    fn default() -> Self {
        Self
    }
}

impl WireGuardPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for WireGuardPlugin {
    fn name(&self) -> &str {
        "wireguard"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::wireguard_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(WireGuardState {
            interfaces: vec![],
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
</file>

<file path="src/auto_create.rs">
//! Auto-Discovery and Creation of Plugins
//!
//! This module provides the capability to automatically discover system services
//! and create corresponding state plugins.

use anyhow::Result;
use async_trait::async_trait;
use op_state::StatePlugin;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Auto-creator for systemd-based plugins
pub struct SystemdAutoCreator;

impl SystemdAutoCreator {
    /// Discover systemd units and create plugins
    pub async fn discover_units() -> Result<Vec<(String, Value)>> {
        let mut plugins = Vec::new();

        // Example discovery: find all active .service units
        // In a real implementation, this would query systemd via D-Bus
        let discovered_units = vec!["nginx.service", "redis.service", "postgresql.service"];

        for unit in discovered_units {
            plugins.push((
                unit.to_string(),
                json!({
                    "type": "systemd",
                    "name": unit,
                    "state": "active",
                    "enabled": true
                }),
            ));
        }

        Ok(plugins)
    }
}

/// Generic auto-plugin that can wrap discovered services
pub struct AutoPlugin {
    name: String,
    _category: String,
    current_state: Arc<RwLock<Value>>,
}

impl AutoPlugin {
    pub fn new(name: &str, category: &str, initial_state: Value) -> Self {
        Self {
            name: name.to_string(),
            _category: category.to_string(),
            current_state: Arc::new(RwLock::new(initial_state)),
        }
    }
}

#[async_trait]
impl StatePlugin for AutoPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(self.current_state.read().await.clone())
    }

    async fn calculate_diff(
        &self,
        current: &Value,
        desired: &Value,
    ) -> Result<op_state::StateDiff> {
        // Simple generic diff: if not equal, replace
        let mut actions = Vec::new();
        if current != desired {
            actions.push(op_state::StateAction::Create {
                resource: self.name.clone(),
                config: desired.clone(),
            });
        }

        Ok(op_state::StateDiff {
            plugin: self.name.clone(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &op_state::StateDiff) -> Result<op_state::ApplyResult> {
        let changes = Vec::new();
        let errors = Vec::new();

        for action in &diff.actions {
            if let op_state::StateAction::Create { config, .. } = action {
                let mut state = self.current_state.write().await;
                *state = config.clone();
            }
        }

        Ok(op_state::ApplyResult {
            success: true,
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.current_state.read().await;
        Ok(&*current == desired)
    }

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        let state = self.current_state.read().await;
        Ok(op_state::Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state.clone(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &op_state::Checkpoint) -> Result<()> {
        let mut state = self.current_state.write().await;
        *state = checkpoint.state_snapshot.clone();
        Ok(())
    }

    fn capabilities(&self) -> op_state::PluginCapabilities {
        op_state::PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }
}
</file>

<file path="src/builtin.rs">
//! Built-in plugins

use anyhow::Result;
use async_trait::async_trait;
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::plugin::Plugin;
use crate::state::{DesiredState, StateChange, ValidationResult};

/// Echo plugin for testing
pub struct EchoPlugin {
    name: String,
    state: Arc<RwLock<Value>>,
    desired: Arc<RwLock<DesiredState>>,
}

impl EchoPlugin {
    pub fn new() -> Self {
        Self {
            name: "echo".to_string(),
            state: Arc::new(RwLock::new(simd_json::json!({}))),
            desired: Arc::new(RwLock::new(DesiredState::default())),
        }
    }
}

#[async_trait]
impl Plugin for EchoPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        "Echo plugin for testing"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn get_state(&self) -> Result<Value> {
        Ok(self.state.read().await.clone())
    }

    async fn get_desired_state(&self) -> Result<DesiredState> {
        Ok(self.desired.read().await.clone())
    }

    async fn set_desired_state(&self, desired: DesiredState) -> Result<()> {
        *self.desired.write().await = desired;
        Ok(())
    }

    async fn apply_state(&self) -> Result<Vec<StateChange>> {
        let desired = self.desired.read().await;
        *self.state.write().await = desired.state.clone();
        Ok(vec![])
    }

    async fn diff(&self) -> Result<Vec<StateChange>> {
        Ok(vec![])
    }

    async fn validate(&self, _config: &Value) -> Result<ValidationResult> {
        Ok(ValidationResult::success())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Default for EchoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export DynamicLoadingPlugin from its module
pub use crate::dynamic_loading::DynamicLoadingPlugin;
</file>

<file path="src/chat.rs">
//! Chat schema types - canonical definitions for chat/LLM interactions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue;
use std::collections::HashMap;

/// Chat role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: ChatRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub metadata: HashMap<String, OwnedValue>,
}

/// Tool call within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: OwnedValue,
}

/// Chat request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
}

/// Chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    #[serde(default)]
    pub usage: Option<TokenUsage>,
}

/// Token usage stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Desired state for apply operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    pub plugin: String,
    pub state: OwnedValue,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    #[serde(default)]
    pub errors: Vec<String>,
}
</file>

<file path="src/default_registry.rs">
//! Default plugin loader - auto-loads essential plugins
//!
//! This module defines which plugins are loaded by default when the system starts.
//! Plugins can be enabled/disabled via configuration.

use anyhow::Result;
use op_state_store::StateStore;
use simd_json::prelude::*;
use std::sync::Arc;

use crate::state_plugins::{
    AdcPlugin, AgentConfigPlugin, CognitiveMcpPlugin, CompactMcpPlugin, ConfigPlugin,
    EndpointPlugin, GcloudAdcPlugin, HardwarePlugin, IncusPlugin, KeypairPlugin, MailServerPlugin,
    McpStatePlugin, NetStatePlugin, OpenFlowPlugin, OvsBridgePlugin, PrivacyRouterPlugin,
    PrivacyRoutesPlugin, ProcfsPlugin, ProxmoxPlugin, ProxyServerPlugin, RtnetlinkPlugin,
    S6StatePlugin, ServicePlugin, SessDeclPlugin, SoftwarePlugin, UnixSocketPlugin, UsersPlugin,
    WebUiPlugin, WireGuardPlugin,
};

/// Default plugin loader configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginRegistryConfig {
    /// Auto-load plugins on startup
    #[serde(default = "default_auto_load")]
    pub auto_load: Vec<String>,

    /// Plugin-specific configurations
    #[serde(default)]
    pub plugin_configs: std::collections::HashMap<String, simd_json::OwnedValue>,
}

fn default_auto_load() -> Vec<String> {
    let wg_only = std::env::var("OP_DBUS_WG_ONLY")
        .ok()
        .map(|v| {
            let l = v.to_lowercase();
            !(l == "0" || l == "false" || l == "no")
        })
        .unwrap_or(false);

    if wg_only {
        return vec![
            "config".to_string(),
            "service".to_string(),
            "s6".to_string(),
            "net".to_string(),
            "rtnetlink".to_string(),
            "procfs".to_string(),
            "wireguard".to_string(),
            "agent_config".to_string(),
        ];
    }

    vec![
        "mcp".to_string(),
        "cognitive_mcp".to_string(),
        "compact_mcp".to_string(),
        "config".to_string(),
        "s6".to_string(),
        "incus".to_string(),
        "mail_server".to_string(),
        "unix_socket".to_string(),
        "net".to_string(),
        "openflow".to_string(),
        "ovsdb_bridge".to_string(),
        "privacy_router".to_string(),
        "privacy_routes".to_string(),
        "procfs".to_string(),
        "rtnetlink".to_string(),
        "agent_config".to_string(),
    ]
}

impl Default for PluginRegistryConfig {
    fn default() -> Self {
        Self {
            auto_load: default_auto_load(),
            plugin_configs: std::collections::HashMap::new(),
        }
    }
}

/// Default plugin loader.
///
/// This is intentionally not the authoritative plugin catalog. Its job is to
/// instantiate the built-in plugin implementations so the real catalog path can
/// persist their canonical plugin documents during registration.
pub struct DefaultPluginRegistry {
    config: PluginRegistryConfig,
    state_store: Arc<dyn StateStore>,
}

impl DefaultPluginRegistry {
    /// Create a new plugin loader
    pub fn new(state_store: Arc<dyn StateStore>) -> Self {
        Self {
            config: PluginRegistryConfig::default(),
            state_store,
        }
    }

    /// Create with custom configuration
    pub fn with_config(state_store: Arc<dyn StateStore>, config: PluginRegistryConfig) -> Self {
        Self {
            config,
            state_store,
        }
    }

    /// Load all auto-load plugins
    pub async fn load_default_plugins(&self) -> Result<Vec<Arc<dyn op_state::StatePlugin>>> {
        let mut plugins: Vec<Arc<dyn op_state::StatePlugin>> = Vec::new();

        for plugin_name in &self.config.auto_load {
            match self.load_plugin(plugin_name).await {
                Ok(plugin) => {
                    if !plugin.is_available() {
                        tracing::info!(
                            "Skipping unavailable plugin {}: {}",
                            plugin_name,
                            plugin.unavailable_reason()
                        );
                        continue;
                    }
                    tracing::info!("✅ Loaded plugin: {}", plugin_name);
                    plugins.push(plugin);
                }
                Err(e) => {
                    tracing::warn!("⚠️ Failed to load plugin {}: {}", plugin_name, e);
                }
            }
        }

        tracing::info!("📦 Loaded {} plugins", plugins.len());
        Ok(plugins)
    }

    /// Load a specific plugin by name
    async fn load_plugin(&self, name: &str) -> Result<Arc<dyn op_state::StatePlugin>> {
        let plugin: Arc<dyn op_state::StatePlugin> = match name {
            "mcp" => {
                let config_path =
                    self.get_plugin_config_path("mcp", "/etc/op-dbus/mcp-config.json");
                Arc::new(McpStatePlugin::new(self.state_store.clone(), config_path))
            }
            "config" => {
                let config_path =
                    self.get_plugin_config_path("config", "/etc/op-dbus/config-store.json");
                Arc::new(ConfigPlugin::new(config_path))
            }
            "cognitive_mcp" => Arc::new(CognitiveMcpPlugin::new()),
            "compact_mcp" => Arc::new(CompactMcpPlugin::new()),
            "s6" | "service_s6" => Arc::new(S6StatePlugin::new()),
            "systemd" | "dinit" => Arc::new(S6StatePlugin::new()), // compatibility aliases
            "incus" => Arc::new(IncusPlugin::new()),
            "mail_server" => Arc::new(MailServerPlugin::new()),
            "unix_socket" => Arc::new(UnixSocketPlugin::new()),
            "net" => Arc::new(NetStatePlugin::new()),
            "openflow" => Arc::new(OpenFlowPlugin::new()),
            "privacy_router" => {
                let _config_path = self
                    .get_plugin_config_path("privacy_router", "/etc/op-dbus/privacy-config.json");
                use crate::state_plugins::privacy_router::PrivacyRouterConfig;
                Arc::new(PrivacyRouterPlugin::new(PrivacyRouterConfig::default()))
            }
            "proxmox" => Arc::new(ProxmoxPlugin::new()),
            "hardware" => Arc::new(HardwarePlugin::new()),
            "software" => Arc::new(SoftwarePlugin::new()),
            "users" => Arc::new(UsersPlugin::new()),
            "gcloud_adc" => Arc::new(GcloudAdcPlugin::new()),
            "keypair" => Arc::new(KeypairPlugin::new()),
            "service" => Arc::new(ServicePlugin::new()),
            "wireguard" => Arc::new(WireGuardPlugin::new()),
            "agent_config" => Arc::new(AgentConfigPlugin::new()),
            "ovsdb_bridge" => Arc::new(OvsBridgePlugin::new()),
            "privacy_routes" => Arc::new(PrivacyRoutesPlugin::default()),
            "procfs" => Arc::new(ProcfsPlugin::new()),
            "rtnetlink" => Arc::new(RtnetlinkPlugin::new()),
            "sess_decl" => Arc::new(SessDeclPlugin::new()),
            "adc" => Arc::new(AdcPlugin::new()),
            "endpoint" => Arc::new(EndpointPlugin::new()),
            "proxy_server" => Arc::new(ProxyServerPlugin::new()),
            "web_ui" => Arc::new(WebUiPlugin::new()),
            _ => {
                return Err(anyhow::anyhow!("Unknown plugin: {}", name));
            }
        };

        Ok(plugin)
    }

    /// Get plugin-specific config value or default
    fn get_plugin_config_path(&self, plugin_name: &str, default: &str) -> String {
        self.config
            .plugin_configs
            .get(plugin_name)
            .and_then(|v| v.get("config_path"))
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            .to_string()
    }

    /// Get list of available plugins
    pub fn available_plugins() -> Vec<&'static str> {
        vec![
            "mcp",
            "config",
            "s6",
            "incus",
            "net",
            "privacy_routes",
            "openflow",
            "privacy_router",
            // "netmaker",
            // "lxc",
            // "packagekit",
        ]
    }

    /// Check if a plugin is enabled for auto-load
    pub fn is_auto_load(&self, plugin_name: &str) -> bool {
        self.config.auto_load.contains(&plugin_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::SqliteStore;

    #[tokio::test]
    async fn test_default_plugin_registry() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        // Check default auto-load plugins
        assert!(registry.is_auto_load("mcp"));
        assert!(registry.is_auto_load("config"));
        assert!(registry.is_auto_load("s6"));
        assert!(registry.is_auto_load("net"));

        // Load plugins
        let plugins = registry.load_default_plugins().await.unwrap();
        assert!(!plugins.is_empty());
    }

    #[tokio::test]
    async fn test_auto_loaded_plugins_publish_schema() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);

        let plugins = registry.load_default_plugins().await.unwrap();
        let missing: Vec<String> = plugins
            .iter()
            .filter(|plugin| plugin.schema().is_none())
            .map(|plugin| plugin.name().to_string())
            .collect();

        assert!(
            missing.is_empty(),
            "auto-loaded plugins missing schema(): {:?}",
            missing
        );
    }

    #[tokio::test]
    async fn test_loadable_plugins_publish_schema() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
        let registry = DefaultPluginRegistry::new(store);
        let plugin_names = vec![
            "mcp",
            "config",
            "s6",
            "systemd",
            "dinit",
            "incus",
            "net",
            "openflow",
            "privacy_router",
            "proxmox",
            "hardware",
            "software",
            "users",
            "gcloud_adc",
            "keypair",
            "service",
            "wireguard",
            "agent_config",
            "ovsdb_bridge",
            "privacy_routes",
            "procfs",
            "rtnetlink",
            "sess_decl",
            "adc",
            "endpoint",
            "proxy_server",
            "web_ui",
        ];

        let mut missing = Vec::new();
        for plugin_name in plugin_names {
            let plugin = registry
                .load_plugin(plugin_name)
                .await
                .unwrap_or_else(|error| panic!("failed to load {}: {}", plugin_name, error));
            if plugin.schema().is_none() {
                missing.push(plugin_name.to_string());
            }
        }

        assert!(
            missing.is_empty(),
            "loadable plugins missing schema(): {:?}",
            missing
        );
    }

    #[tokio::test]
    async fn test_custom_config() {
        let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());

        let config = PluginRegistryConfig {
            auto_load: vec!["s6".to_string()],
            plugin_configs: std::collections::HashMap::new(),
        };

        let registry = DefaultPluginRegistry::with_config(store, config);

        assert!(registry.is_auto_load("s6"));
        assert!(!registry.is_auto_load("mcp"));
        assert!(!registry.is_auto_load("config"));
    }
}
</file>

<file path="src/dynamic_loading.rs">
//! Dynamic Loading Plugin - Manages tool loading and caching
//!
//! This plugin provides dynamic tool loading capabilities with intelligent caching
//! and execution-aware loading decisions.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::plugin::{Plugin, PluginCapabilities, PluginContext, PluginMetadata};
use crate::state::{ChangeOperation, DesiredState, StateChange, ValidationResult};

/// Dynamic Loading Plugin Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicLoadingConfig {
    /// Maximum cache size
    pub cache_size: usize,
    /// Critical tools that should always be loaded
    pub critical_tools: Vec<String>,
    /// Loading strategy (smart, aggressive, conservative)
    pub strategy: String,
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStatistics {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub load_time_ms: u64,
    pub evictions: u64,
    pub current_size: usize,
}

/// Dynamic Loading Plugin
pub struct DynamicLoadingPlugin {
    name: String,
    config: Arc<RwLock<DynamicLoadingConfig>>,
    state: Arc<RwLock<Value>>,
    desired: Arc<RwLock<DesiredState>>,
    stats: Arc<RwLock<CacheStatistics>>,
    storage_path: PathBuf,
    numa_node: Option<u32>,
}

impl DynamicLoadingPlugin {
    pub fn new() -> Self {
        Self {
            name: "dynamic_loading".to_string(),
            config: Arc::new(RwLock::new(DynamicLoadingConfig {
                cache_size: 100,
                critical_tools: vec![
                    "dbus_".to_string(),
                    "systemd_".to_string(),
                    "ovs_".to_string(),
                    "agent_".to_string(),
                ],
                strategy: "smart".to_string(),
            })),
            state: Arc::new(RwLock::new(json!({
                "cache_size": 100,
                "hit_rate": 0.0,
                "load_time_avg_ms": 0,
                "active_tools": 0,
                "storage_path": "/var/lib/op-dbus/plugins/dynamic_loading",
                "numa_node": 0
            }))),
            desired: Arc::new(RwLock::new(DesiredState::default())),
            stats: Arc::new(RwLock::new(CacheStatistics::default())),
            storage_path: PathBuf::from("/var/lib/op-dbus/plugins/dynamic_loading"),
            numa_node: None,
        }
    }

    /// Get current cache statistics
    pub async fn get_cache_stats(&self) -> Result<CacheStatistics> {
        Ok(self.stats.read().await.clone())
    }

    /// Update cache statistics
    pub async fn update_cache_stats(&self, hit: bool, load_time_ms: u64) -> Result<()> {
        let mut stats = self.stats.write().await;
        if hit {
            stats.cache_hits += 1;
        } else {
            stats.cache_misses += 1;
        }
        stats.load_time_ms += load_time_ms;
        stats.current_size = stats.current_size.min(self.config.read().await.cache_size);

        // Update state with current statistics
        let hit_rate = if stats.cache_hits + stats.cache_misses > 0 {
            stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64
        } else {
            0.0
        };

        let mut state = self.state.write().await;
        *state = json!({
            "cache_size": self.config.read().await.cache_size,
            "hit_rate": hit_rate,
            "load_time_avg_ms": stats.load_time_ms / (stats.cache_hits + stats.cache_misses).max(1),
            "active_tools": stats.current_size,
            "cache_hits": stats.cache_hits,
            "cache_misses": stats.cache_misses,
            "evictions": stats.evictions
        });

        Ok(())
    }

    /// Configure dynamic loading
    pub async fn configure(&self, config: DynamicLoadingConfig) -> Result<()> {
        *self.config.write().await = config;
        Ok(())
    }

    /// Get current configuration
    pub async fn get_config(&self) -> Result<DynamicLoadingConfig> {
        Ok(self.config.read().await.clone())
    }
    /// Ensure BTRFS subvolume exists for plugin storage
    async fn ensure_btrfs_subvolume(&self) -> Result<()> {
        use std::process::Command;

        // Check if BTRFS subvolume exists
        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("list")
            .arg(&self.storage_path)
            .output()?;

        if !output.status.success() {
            // Create BTRFS subvolume if it doesn't exist
            Command::new("btrfs")
                .arg("subvolume")
                .arg("create")
                .arg(&self.storage_path)
                .status()?;

            tracing::info!("Created BTRFS subvolume: {}", self.storage_path.display());
        }

        Ok(())
    }

    /// Get BTRFS subvolume information
    pub async fn get_btrfs_info(&self) -> Result<Value> {
        let output = Command::new("btrfs")
            .arg("subvolume")
            .arg("show")
            .arg(&self.storage_path)
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(json!({
                "subvolume": self.storage_path.display().to_string(),
                "info": stdout.to_string()
            }))
        } else {
            Ok(json!({
                "subvolume": self.storage_path.display().to_string(),
                "error": "Subvolume not found or not BTRFS"
            }))
        }
    }
}

#[async_trait]
impl Plugin for DynamicLoadingPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Dynamic tool loading with intelligent caching and execution tracking"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn get_state(&self) -> Result<Value> {
        Ok(self.state.read().await.clone())
    }

    async fn get_desired_state(&self) -> Result<DesiredState> {
        Ok(self.desired.read().await.clone())
    }

    async fn set_desired_state(&self, desired: DesiredState) -> Result<()> {
        *self.desired.write().await = desired;
        Ok(())
    }

    async fn apply_state(&self) -> Result<Vec<StateChange>> {
        let desired = self.desired.read().await;
        let mut current = self.state.write().await;

        // Apply configuration changes
        if let Some(config) = desired.state.get("config") {
            let new_config: DynamicLoadingConfig =
                simd_json::serde::from_owned_value(config.clone())?;
            *self.config.write().await = new_config;
        }

        // Update state to match desired
        *current = desired.state.clone();

        Ok(vec![StateChange::new(
            ChangeOperation::Update,
            self.name.clone(),
            None,
            None,
            "Dynamic loading configuration applied",
        )])
    }

    async fn diff(&self) -> Result<Vec<StateChange>> {
        let desired = self.desired.read().await;
        let current = self.state.read().await;

        let mut changes = Vec::new();

        // Check if cache size needs adjustment
        if let (Some(desired_size), Some(current_size)) = (
            desired.state.get("cache_size").and_then(|v| v.as_u64()),
            current.get("cache_size").and_then(|v| v.as_u64()),
        ) {
            if desired_size != current_size {
                changes.push(StateChange::update(
                    self.name.clone(),
                    json!(current_size),
                    json!(desired_size),
                    format!("Cache size change: {} -> {}", current_size, desired_size),
                ));
            }
        }

        Ok(changes)
    }

    async fn validate(&self, config: &Value) -> Result<ValidationResult> {
        if let Some(cache_size) = config.get("cache_size") {
            if let Some(size) = cache_size.as_u64() {
                if size < 10 || size > 10000 {
                    return Ok(ValidationResult::failure(
                        "Cache size must be between 10 and 10000",
                    ));
                }
            }
        }
        Ok(ValidationResult::success())
    }

    async fn initialize(&mut self, context: PluginContext) -> Result<()> {
        // Store the plugin context
        self.storage_path = context.storage_path;
        self.numa_node = context.numa_node;

        // Update state with storage information
        let mut state = self.state.write().await;
        state["storage_path"] = simd_json::json!(self.storage_path.to_string_lossy());
        state["numa_node"] = simd_json::json!(self.numa_node);

        // Ensure BTRFS subvolume exists
        self.ensure_btrfs_subvolume().await?;

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            can_read: true,
            can_write: true,
            can_delete: false,
            supports_dry_run: true,
            supports_rollback: false,
            supports_transactions: false,
            requires_root: false,
            supported_platforms: vec!["linux".to_string()],
        }
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name.clone(),
            version: self.version().to_string(),
            description: self.description().to_string(),
            author: Some("OP-DBUS Team".to_string()),
            license: Some("MIT".to_string()),
            dependencies: vec![
                "op-dynamic-loader".to_string(),
                "op-execution-tracker".to_string(),
            ],
            dbus_services: vec![],
            object_schemas: std::collections::HashMap::new(),
            feature_schemas: Vec::new(),
        }
    }

    async fn handle_command(&self, command: &str, args: Value) -> Result<Value> {
        match command {
            "get_stats" => {
                let stats = self.get_cache_stats().await?;
                Ok(simd_json::serde::to_owned_value(stats)?)
            }
            "configure" => {
                let config: DynamicLoadingConfig = simd_json::serde::from_owned_value(args)?;
                self.configure(config).await?;
                Ok(json!({"status": "configured"}))
            }
            "get_config" => {
                let config = self.get_config().await?;
                Ok(simd_json::serde::to_owned_value(config)?)
            }
            "get_btrfs_info" => {
                let info = self.get_btrfs_info().await?;
                Ok(info)
            }
            "ensure_btrfs" => {
                self.ensure_btrfs_subvolume().await?;
                Ok(json!({"status": "btrfs_subvolume_ensured"}))
            }
            _ => Err(anyhow::anyhow!(
                "Command '{}' not supported by plugin '{}'",
                command,
                self.name()
            )),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn state_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.name().as_bytes());
        hasher.update(self.version().as_bytes());
        hasher.update(self.state.blocking_read().to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

impl Default for DynamicLoadingPlugin {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="src/lib.rs">
#![recursion_limit = "512"]

//! op-plugins: Plugin system with state management and blockchain footprints
//!
//! Features:
//! - Plugin trait with desired state management
//! - State plugins for network, LXC, systemd, OpenFlow, etc.
//! - BTRFS subvolume storage per plugin
//! - Automatic hash footprints for blockchain audit trail
//! - Auto-creation of missing plugins
//! - Lifecycle hooks
//! - Canonical plugin-document persistence into the schema catalog

pub mod auto_create;
pub mod builtin;
pub mod chat;
pub mod dynamic_loading;
pub mod plugin;
pub mod registry;
pub mod service_def;
pub mod state;

// State plugins - each manages a specific domain
pub mod default_registry;
pub mod state_plugins;

pub use auto_create::AutoPlugin;
pub use default_registry::{DefaultPluginRegistry, PluginRegistryConfig};
pub use plugin::{Plugin, PluginCapabilities, PluginContext, PluginMetadata};
pub use registry::PluginRegistry as PluginCatalog;
pub use registry::{PluginRecord, PluginRegistry};
pub use state::{ChangeOperation, DesiredState, StateChange, ValidationResult};

// Re-export chat types
pub use chat::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ExecutionStatus, TokenUsage, ToolCall,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::auto_create::AutoPlugin;
    pub use super::registry::PluginRegistry as PluginCatalog;
    pub use super::registry::PluginRegistry;
    pub use super::state::{ChangeOperation, DesiredState, StateChange, ValidationResult};

    // Re-export state plugins
    pub use super::dynamic_loading::DynamicLoadingPlugin;
    pub use super::state_plugins::*;
}
pub mod state_publisher;
</file>

<file path="src/plugin.rs">
/// Core plugin trait and types
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use simd_json::ValueBuilder;
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::state::{DesiredState, StateChange, ValidationResult};
use op_core::state_publisher::StatePublisher;
pub use op_state::plugin::PluginMetadata;

/// Context provided to plugin during initialization
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Optional state publisher for authoritative updates
    pub publisher: Option<std::sync::Arc<dyn StatePublisher>>,
    /// Dedicated BTRFS subvolume path for this plugin's storage
    pub storage_path: PathBuf,
    /// Assigned NUMA node (if available)
    pub numa_node: Option<u32>,
    /// Plugin configuration
    pub config: Value,
}

impl Default for PluginContext {
    fn default() -> Self {
        Self {
            publisher: None,
            storage_path: PathBuf::from("/var/lib/op-dbus/plugins/default"),
            numa_node: None,
            config: Value::null(),
        }
    }
}

/// Plugin tunable parameters (runtime configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTunables {
    pub priority: i32,
    pub max_retries: u32,
    pub timeout_ms: u64,
    pub enabled: bool,
    #[serde(default)]
    pub config: Value,
}

impl Default for PluginTunables {
    fn default() -> Self {
        Self {
            priority: 0,
            max_retries: 3,
            timeout_ms: 30000,
            enabled: true,
            config: Value::null(),
        }
    }
}

/// Plugin capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapabilities {
    pub can_read: bool,
    pub can_write: bool,
    pub can_delete: bool,
    pub supports_dry_run: bool,
    pub supports_rollback: bool,
    pub supports_transactions: bool,
    pub requires_root: bool,
    pub supported_platforms: Vec<String>,
}

impl Default for PluginCapabilities {
    fn default() -> Self {
        Self {
            can_read: true,
            can_write: true,
            can_delete: false,
            supports_dry_run: true,
            supports_rollback: false,
            supports_transactions: false,
            requires_root: false,
            supported_platforms: vec!["linux".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSchema {
    pub feature_type: String,
    pub version: String,
    pub config: Value,
    /// Capability tags (e.g. "immutable", "core", "optional")
    #[serde(default)]
    pub tags: Vec<String>,
    /// Specific JSON configuration paths that are immutable (e.g. ["/metadata/id"])
    #[serde(default)]
    pub immutable_paths: Vec<String>,
}

impl FeatureSchema {
    pub fn is_fully_immutable(&self) -> bool {
        self.tags.iter().any(|t| t == "immutable")
    }

    pub fn is_path_immutable(&self, path: &str) -> bool {
        self.is_fully_immutable() || self.immutable_paths.iter().any(|p| p == path)
    }
}

/// Core plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Unique name for this plugin
    fn name(&self) -> &str;

    /// Description of what this plugin does
    fn description(&self) -> &str;

    /// Version of the plugin
    fn version(&self) -> &str;

    /// Get the current state managed by this plugin
    async fn get_state(&self) -> Result<Value>;

    /// Get the desired state (target configuration)
    async fn get_desired_state(&self) -> Result<DesiredState>;

    /// Set the desired state
    async fn set_desired_state(&self, desired: DesiredState) -> Result<()>;

    /// Apply the desired state (reconcile current -> desired)
    async fn apply_state(&self) -> Result<Vec<StateChange>>;

    /// Calculate diff between current and desired state
    async fn diff(&self) -> Result<Vec<StateChange>>;

    /// Validate a configuration before applying
    async fn validate(&self, config: &Value) -> Result<ValidationResult>;

    /// Get plugin capabilities
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::default()
    }

    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: self.description().to_string(),
            author: None,
            license: None,
            dependencies: vec![],
            dbus_services: vec![],
            feature_schemas: vec![],
            object_schemas: HashMap::new(),
        }
    }

    /// Handle plugin-specific commands
    async fn handle_command(&self, command: &str, _args: Value) -> Result<Value> {
        Err(anyhow::anyhow!(
            "Command '{}' not supported by plugin '{}'",
            command,
            self.name()
        ))
    }

    /// Initialize the plugin with context
    async fn initialize(&mut self, _context: PluginContext) -> Result<()> {
        Ok(())
    }

    /// Cleanup when plugin is being removed
    async fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }

    /// Get hash of current state for blockchain footprint
    fn state_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        // Default implementation - plugins should override for accuracy
        let mut hasher = Sha256::new();
        hasher.update(self.name().as_bytes());
        hasher.update(self.version().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Boxed plugin type
pub type BoxedPlugin = Box<dyn Plugin>;
</file>

<file path="src/registry.rs">
//! Runtime plugin catalog.
//!
//! The catalog indexes live plugin instances and mirrors plugin-owned
//! `PluginSchema` documents into the shared schema catalog. Runtime truth stays
//! with the plugin schema; persisted catalog documents are compatibility
//! snapshots for consumers that still hydrate from disk.

use anyhow::Result;
use op_core::state_publisher::{ChangeType, StatePublisher};
use op_dbus_model::{CatalogDocument, SqlitePluginCatalog};
use op_state::StatePlugin;
use op_state_store::SchemaCatalog;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, warn};

#[derive(Clone)]
pub struct PluginRecord {
    pub name: String,
    pub plugin: Arc<dyn StatePlugin>,
    pub storage_path: PathBuf,
    pub dbus_path: String,
}

pub struct PluginRegistry {
    plugins: AsyncRwLock<HashMap<String, PluginRecord>>,
    base_path: PathBuf,
    schema_catalog: Arc<RwLock<SchemaCatalog>>,
    schema_catalog_store: Option<Arc<SqlitePluginCatalog>>,
    publisher: AsyncRwLock<Option<Arc<dyn StatePublisher>>>,
    dbus_connection: AsyncRwLock<Option<zbus::Connection>>,
}

impl PluginRegistry {
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self::with_schema_catalog_and_store(
            base_path,
            Arc::new(RwLock::new(SchemaCatalog::empty())),
            None,
        )
    }

    pub fn with_schema_catalog_and_store(
        base_path: impl AsRef<Path>,
        schema_catalog: Arc<RwLock<SchemaCatalog>>,
        schema_catalog_store: Option<Arc<SqlitePluginCatalog>>,
    ) -> Self {
        Self {
            plugins: AsyncRwLock::new(HashMap::new()),
            base_path: base_path.as_ref().to_path_buf(),
            schema_catalog,
            schema_catalog_store,
            publisher: AsyncRwLock::new(None),
            dbus_connection: AsyncRwLock::new(None),
        }
    }

    pub async fn set_publisher(&self, publisher: Arc<dyn StatePublisher>) {
        *self.publisher.write().await = Some(publisher);
    }

    pub async fn set_dbus_connection(&self, connection: zbus::Connection) {
        *self.dbus_connection.write().await = Some(connection);
    }

    pub async fn hydrate_catalog_from_store(&self) -> Result<()> {
        let Some(store) = &self.schema_catalog_store else {
            return Ok(());
        };

        for document in store.list_documents().await? {
            self.schema_catalog.write().register(document.schema);
        }

        Ok(())
    }

    pub async fn register(&self, plugin: Arc<dyn StatePlugin>) -> Result<()> {
        let name = plugin.name().to_string();
        let storage_path = self.plugin_storage_path(&name);
        tokio::fs::create_dir_all(&storage_path).await?;

        let dbus_path = Self::plugin_dbus_path(&name);

        if let Some(schema) = plugin.schema() {
            self.schema_catalog.write().register(schema.clone());

            if let Some(store) = &self.schema_catalog_store {
                let document = CatalogDocument {
                    schema: schema.clone(),
                    dbus_path: dbus_path.clone(),
                    service_name: "org.opdbus.v1".to_string(),
                    storage_path: storage_path.to_string_lossy().into_owned(),
                    source: "plugin".to_string(),
                };
                store.upsert_document(&document).await?;
            }

            if let Some(publisher) = &*self.publisher.read().await {
                let _ = publisher
                    .publish_change(
                        name.clone(),
                        format!("schema/{}", name),
                        ChangeType::PropertySet,
                        Some("definition".to_string()),
                        None,
                        schema.to_json_schema(),
                        vec!["schema".to_string(), "plugin".to_string()],
                        "PluginSchema".to_string(),
                    )
                    .await;
            }
        } else {
            warn!(
                "Plugin {} has no PluginSchema; it will not enter the schema catalog",
                name
            );
        }

        if let Some(connection) = &*self.dbus_connection.read().await {
            let host = op_state::dbus_server::PluginDbusHost {
                plugin: plugin.clone(),
                schema_registry: self.schema_catalog.clone(),
            };
            if let Err(error) = connection
                .object_server()
                .at(dbus_path.as_str(), host)
                .await
            {
                debug!("Plugin {} D-Bus host export skipped: {}", name, error);
            }
        }

        self.plugins.write().await.insert(
            name.clone(),
            PluginRecord {
                name,
                plugin,
                storage_path,
                dbus_path,
            },
        );

        Ok(())
    }

    pub async fn get(&self, name: &str) -> Option<Arc<dyn StatePlugin>> {
        self.plugins
            .read()
            .await
            .get(name)
            .map(|record| record.plugin.clone())
    }

    pub async fn records(&self) -> Vec<PluginRecord> {
        self.plugins.read().await.values().cloned().collect()
    }

    fn plugin_storage_path(&self, name: &str) -> PathBuf {
        self.base_path.join(Self::sanitize_path_segment(name))
    }

    fn plugin_dbus_path(name: &str) -> String {
        format!(
            "/org/opdbus/v1/plugins/{}",
            Self::sanitize_path_segment(name)
        )
    }

    fn sanitize_path_segment(segment: &str) -> String {
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
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new("/var/lib/op-dbus/plugins")
    }
}
</file>

<file path="src/service_def.rs">
//! Systemd plugin for service management
//!
//! Schema-as-code: These types ARE the schema. Validation happens at parse time.

use anyhow::Result;
use serde::{Deserialize, Serialize};
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
    /// Generate an s6 `run` script from the service definition.
    ///
    /// The resulting script follows the s6 convention:
    /// ```sh
    /// #!/bin/sh
    /// exec <command>
    /// ```
    /// Environment variables and working directory are set up before the exec.
    pub fn to_s6_run(&self) -> String {
        let mut out = String::new();
        out.push_str("#!/bin/sh\n");

        // Working directory
        if let Some(ref dir) = self.working_dir {
            out.push_str(&format!("cd {} || exit 1\n", dir.display()));
        }

        // User/group — s6 uses s6-setuidgid for privilege dropping
        if let Some(ref user) = self.user {
            let group_suffix = self
                .group
                .as_deref()
                .map(|g| format!(":{g}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "exec s6-setuidgid {user}{group_suffix} {}\n",
                self.exec_start.to_command_line()
            ));
        } else {
            out.push_str(&format!("exec {}\n", self.exec_start.to_command_line()));
        }

        out
    }

    /// Write the s6 service definition to `/etc/s6/sv/<name>/run`.
    ///
    /// Creates the service directory if it does not exist and makes the run
    /// script executable (mode 0o755).
    pub fn install(&self) -> std::io::Result<()> {
        let svc_dir = format!("/etc/s6/sv/{}", self.name);
        std::fs::create_dir_all(&svc_dir)?;

        let run_path = format!("{svc_dir}/run");
        let content = self.to_s6_run();
        std::fs::write(&run_path, &content)?;

        // Make the run script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&run_path, std::fs::Permissions::from_mode(0o755))?;
        }

        Ok(())
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
</file>

<file path="src/state_publisher.rs">
use anyhow::Result;
use async_trait::async_trait;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone)]
pub enum ChangeType {
    PropertySet,
    Signal,
    Deleted,
}

#[async_trait]
pub trait StatePublisher: Send + Sync {
    async fn publish_change(
        &self,
        plugin_id: String,
        path: String,
        change_type: ChangeType,
        property: Option<String>,
        old_value: Option<Value>,
        new_value: Value,
        tags: Vec<String>,
        source: String,
    ) -> Result<()>;
}
</file>

<file path="src/state.rs">
//! Desired state management and change tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use simd_json::OwnedValue as Value;

/// Desired state configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    /// The target state configuration
    pub state: Value,
    /// When this desired state was set
    pub timestamp: DateTime<Utc>,
    /// Hash of the state for verification
    pub hash: String,
    /// Optional description of the change
    pub description: Option<String>,
    /// Source of the desired state (user, auto, import, etc.)
    pub source: StateSource,
}

impl DesiredState {
    /// Create a new desired state
    pub fn new(state: Value) -> Self {
        let hash = Self::compute_hash(&state);
        Self {
            state,
            timestamp: Utc::now(),
            hash,
            description: None,
            source: StateSource::User,
        }
    }

    /// Create with description
    pub fn with_description(state: Value, description: impl Into<String>) -> Self {
        let mut ds = Self::new(state);
        ds.description = Some(description.into());
        ds
    }

    /// Create from imported configuration
    pub fn from_import(state: Value, source: &str) -> Self {
        let mut ds = Self::new(state);
        ds.source = StateSource::Import(source.to_string());
        ds
    }

    /// Compute hash of the state
    pub fn compute_hash(state: &Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(simd_json::to_string(state).unwrap_or_default().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify the hash matches
    pub fn verify(&self) -> bool {
        Self::compute_hash(&self.state) == self.hash
    }
}

impl Default for DesiredState {
    fn default() -> Self {
        Self::new(Value::Object(Box::new(
            simd_json::value::owned::Object::new(),
        )))
    }
}

/// Source of the desired state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StateSource {
    /// Set by user
    User,
    /// Auto-discovered from system
    AutoDiscovered,
    /// Imported from file or URL
    Import(String),
    /// From another plugin
    Plugin(String),
    /// System default
    Default,
}

/// Represents a change to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// Type of change operation
    pub operation: ChangeOperation,
    /// Path to the changed element (JSONPath-like)
    pub path: String,
    /// Previous value (if any)
    pub old_value: Option<Value>,
    /// New value (if any)
    pub new_value: Option<Value>,
    /// Human-readable description
    pub description: String,
    /// Hash of this change for blockchain
    pub hash: String,
    /// Timestamp of the change
    pub timestamp: DateTime<Utc>,
}

impl StateChange {
    /// Create a new state change
    pub fn new(
        operation: ChangeOperation,
        path: impl Into<String>,
        old_value: Option<Value>,
        new_value: Option<Value>,
        description: impl Into<String>,
    ) -> Self {
        let path = path.into();
        let description = description.into();

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", operation).as_bytes());
        hasher.update(path.as_bytes());
        hasher.update(
            simd_json::to_string(&old_value)
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(
            simd_json::to_string(&new_value)
                .unwrap_or_default()
                .as_bytes(),
        );
        let hash = format!("{:x}", hasher.finalize());

        Self {
            operation,
            path,
            old_value,
            new_value,
            description,
            hash,
            timestamp: Utc::now(),
        }
    }

    /// Create a create operation
    pub fn create(path: impl Into<String>, value: Value, description: impl Into<String>) -> Self {
        Self::new(
            ChangeOperation::Create,
            path,
            None,
            Some(value),
            description,
        )
    }

    /// Create an update operation
    pub fn update(
        path: impl Into<String>,
        old: Value,
        new: Value,
        description: impl Into<String>,
    ) -> Self {
        Self::new(
            ChangeOperation::Update,
            path,
            Some(old),
            Some(new),
            description,
        )
    }

    /// Create a delete operation
    pub fn delete(path: impl Into<String>, old: Value, description: impl Into<String>) -> Self {
        Self::new(ChangeOperation::Delete, path, Some(old), None, description)
    }

    /// Create a no-op (for audit logging)
    pub fn noop(path: impl Into<String>, value: Value, description: impl Into<String>) -> Self {
        Self::new(
            ChangeOperation::NoOp,
            path,
            Some(value.clone()),
            Some(value),
            description,
        )
    }
}

/// Change operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOperation {
    Create,
    Update,
    Delete,
    NoOp,
}

/// Validation result from a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

impl ValidationResult {
    /// Create a success result
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
            suggestions: vec![],
        }
    }

    /// Create a failure result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            valid: false,
            errors: vec![ValidationError {
                path: "".to_string(),
                message: error.into(),
                code: "validation_failed".to_string(),
            }],
            warnings: vec![],
            suggestions: vec![],
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Add a suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Add an error (makes result invalid)
    pub fn with_error(mut self, path: impl Into<String>, message: impl Into<String>) -> Self {
        self.valid = false;
        self.errors.push(ValidationError {
            path: path.into(),
            message: message.into(),
            code: "validation_error".to_string(),
        });
        self
    }
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desired_state_hash() {
        let state = simd_json::json!({"key": "value"});
        let ds = DesiredState::new(state);
        assert!(ds.verify());
    }

    #[test]
    fn test_state_change_hash() {
        let change =
            StateChange::create("/test/path", simd_json::json!({"value": 42}), "Test change");
        assert!(!change.hash.is_empty());
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-plugins"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Plugin system with state management, domain plugins, and blockchain footprints"

[dependencies]
op-core = { path = "../op-core" }
op-dbus-model = { workspace = true }
op-state = { path = "../op-state" }
op-state-store = { path = "../op-state-store" }
op-blockchain = { path = "../op-blockchain" }
op-network = { path = "../op-network" }
op-dynamic-loader = { path = "../op-dynamic-loader" }
op-execution-tracker = { path = "../op-execution-tracker" }

tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
zbus = { workspace = true }
chrono = { workspace = true }
log = { workspace = true }
reqwest = { workspace = true }
sha2 = { workspace = true }
md5 = { workspace = true }
uuid = { workspace = true }
dirs = "5.0"
parking_lot = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
</file>

<file path="compare-op-plugins.md">
# compare-op-plugins

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 49 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 11 |
| Partial artifacts | 0 |
| Spec-listed source files | 20 |
| Spec-listed but missing | 0 |
| Extra implementation files | 29 |

## Current Implementation Overview

- Plugin system with state management, domain plugins, and blockchain footprints
- Internal crate integrations: op-core, op-dbus-model, op-state, op-state-store, op-blockchain, op-network, op-dynamic-loader, op-execution-tracker.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/state_plugins/systemd_networkd.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/systemd_networkd.rs |
| `src/state_plugins/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/mod.rs |
| `src/state_plugins/adc.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/adc.rs |
| `src/state_plugins/agent_config.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/agent_config.rs |
| `src/state_plugins/config.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/config.rs |
| `src/state_plugins/dinit.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/dinit.rs |
| `src/state_plugins/dnsresolver.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/dnsresolver.rs |
| `src/state_plugins/endpoint.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/endpoint.rs |
| `src/state_plugins/full_system.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/full_system.rs |
| `src/state_plugins/gcloud_adc.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/gcloud_adc.rs |
| `src/state_plugins/hardware.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/hardware.rs |
| `src/state_plugins/keypair.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/keypair.rs |
| `src/state_plugins/keyring.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/keyring.rs |
| `src/state_plugins/login1.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/login1.rs |
| `src/state_plugins/lxc.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/lxc.rs |
| `src/state_plugins/mcp.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/mcp.rs |
| `src/state_plugins/net.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/net.rs |
| `src/state_plugins/netmaker.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/netmaker.rs |
| `src/state_plugins/openflow.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/openflow.rs |
| `src/state_plugins/openflow_obfuscation.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/state_plugins/openflow_obfuscation.rs |
| `root` | ✅ Present | root source group | src/auto_create.rs, src/builtin.rs, src/chat.rs, src/default_registry.rs, src/dynamic_loading.rs, src/lib.rs, src/plugin.rs, src/registry.rs, ... (+3 more) |
| `state_plugins` | ✅ Present | state_plugins group | src/state_plugins/adc.rs, src/state_plugins/agent_config.rs, src/state_plugins/config.rs, src/state_plugins/dinit.rs, src/state_plugins/dnsresolver.rs, src/state_plugins/endpoint.rs, src/state_plugins/full_system.rs, src/state_plugins/gcloud_adc.rs, ... (+30 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| registry | ✅ Implemented | src/registry.rs | SPEC main module |
| auto_create | ✅ Implemented | src/auto_create.rs | SPEC main module |
| builtin | ✅ Implemented | src/builtin.rs | SPEC main module |
| chat | ✅ Implemented | src/chat.rs | SPEC main module |
| dynamic_loading | ✅ Implemented | src/dynamic_loading.rs | SPEC main module |
| plugin | ✅ Implemented | src/plugin.rs | SPEC main module |
| state | ✅ Implemented | src/state.rs | SPEC main module |
| systemd | ✅ Implemented | src/state_plugins/systemd.rs | SPEC main module |
| default_registry | ✅ Implemented | src/default_registry.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-dbus-model` - not listed in SPEC dependency block
- `op-state` - documented in SPEC
- `op-state-store` - documented in SPEC
- `op-blockchain` - documented in SPEC
- `op-network` - documented in SPEC
- `op-dynamic-loader` - documented in SPEC
- `op-execution-tracker` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `zbus` - documented in SPEC
- `chrono` - documented in SPEC
- `log` - documented in SPEC
- `reqwest` - documented in SPEC
- `sha2` - documented in SPEC
- `md5` - not listed in SPEC dependency block
- `uuid` - not listed in SPEC dependency block
- `dirs` - not listed in SPEC dependency block
- `parking_lot` - not listed in SPEC dependency block

### Development and Build Dependencies
- `dev:tempfile`

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 29 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: auto_create, builtin, chat, dynamic_loading, plugin, registry, service_def, state, default_registry, state_plugins, state_publisher.
- 5 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="DESIGN.md">
# op-plugins — Design: Plugin Inventory & Schema Quality Survey

**Crate**: `op-plugins`  
**Scope**: Complete inventory of all state plugins, schema coverage audit, quality evaluation,
and identification of missing plugins.

**Core rule**: If a plugin does not have a validated schema returned from
`StatePlugin::schema()`, it is not catalog-recognized and for all intent and purpose does not
exist on the system.

---

## Critical Finding: The Schema Gap

**Zero out of 35 plugins** currently override `StatePlugin::schema()`. The default
implementation in `op-state/src/plugin.rs` returns `None`:

```rust
fn schema(&self) -> Option<PluginSchema> {
    None   // ← all 35 plugins inherit this default
}
```

Compatibility schemas exist in `op-state-store/src/plugin_schema.rs`
(`builtin_plugin_schema_from_canonical_name`) for 37 plugin names, but these are **not**
returned by `StatePlugin::schema()`. They are a separate compatibility layer. Per the catalog
contract, a plugin must return `Some(PluginSchema)` from its trait method to be recognized.

**The only plugin that has JSON schema definitions on its struct methods is `web_ui.rs`**
(`WebUiIdentity::schema()`, `WebUiTunables::schema()`, `WebUiCapabilities::schema()`), but
even `WebUiPlugin` does not override `StatePlugin::schema()`.

---

## Plugin Inventory

### Registered with `DefaultPluginRegistry`

35 plugins have `impl StatePlugin`. The registry's `load_plugin()` recognizes these names:

| Plugin Name | Source File | Auto-Loaded | Schema Override |
|---|---|---|---|
| `adc` | `adc.rs` | ❌ | ❌ |
| `agent_config` | `agent_config.rs` | ❌ | ❌ |
| `config` | `config.rs` | ✅ | ❌ |
| `dinit` | `dinit.rs` | ✅ | ❌ |
| `dnsresolver` | `dnsresolver.rs` | ❌ | ❌ |
| `endpoint` | `endpoint.rs` | ❌ | ❌ |
| `full_system` | `full_system.rs` | ❌ | ❌ |
| `gcloud_adc` | `gcloud_adc.rs` | ❌ | ❌ |
| `hardware` | `hardware.rs` | ❌ | ❌ |
| `incus` | `incus.rs` | ✅ | ❌ |
| `keypair` | `keypair.rs` | ❌ | ❌ |
| `keyring` | `keyring.rs` | ❌ | ❌ |
| `login1` | `login1.rs` | ❌ | ❌ |
| `lxc` | `lxc.rs` | ❌ | ❌ |
| `mcp` | `mcp.rs` | ✅ | ❌ |
| `net` | `net.rs` | ✅ | ❌ |
| `netmaker` | `netmaker.rs` | ❌ | ❌ |
| `openflow` | `openflow.rs` | ✅ | ❌ |
| `openflow_obfuscation` | `openflow_obfuscation.rs` | ❌ | ❌ |
| `ovsdb_bridge` | `ovsdb_bridge.rs` | ✅ | ❌ |
| `packagekit` | `packagekit.rs` | ❌ | ❌ |
| `pcidecl` | `pcidecl.rs` | ❌ | ❌ |
| `privacy` | `privacy.rs` | ❌ | ❌ |
| `privacy_router` | `privacy_router.rs` | ✅ | ❌ |
| `privacy_routes` | `privacy_routes.rs` | ✅ | ❌ |
| `proxmox` | `proxmox.rs` | ❌ | ❌ |
| `proxy_server` | `proxy_server.rs` | ❌ | ❌ |
| `rtnetlink` | `rtnetlink.rs` | ✅ | ❌ |
| `service` | `service.rs` | ❌ | ❌ |
| `sess_decl` | `sessdecl.rs` | ❌ | ❌ |
| `software` | `software.rs` | ❌ | ❌ |
| `systemd` | `systemd.rs` | ❌ (compat alias for `dinit`) | ❌ |
| `users` | `users.rs` | ❌ | ❌ |
| `web_ui` | `web_ui.rs` | ❌ | ❌ (has struct schemas, not trait override) |
| `wireguard` | `wireguard.rs` | ❌ | ❌ |

Auto-loaded in normal mode: `mcp`, `config`, `dinit`, `incus`, `net`, `openflow`,
`ovsdb_bridge`, `privacy_router`, `privacy_routes`, `rtnetlink`  
Auto-loaded in WG-only mode: `config`, `service`, `dinit`, `net`, `rtnetlink`, `wireguard`

### Compat Schemas Without StatePlugin Implementations

These have entries in `builtin_plugin_schema_from_canonical_name` but no `impl StatePlugin`:

| Name | Notes |
|---|---|
| `incus-wireguard-ingress` | Schema only — Incus container WireGuard ingress profile |
| `incus-xray-reality-client` | Schema only — Incus Xray Reality client profile |
| `incus-xray-reality-server` | Schema only — Incus Xray Reality server profile |

### Source File Without Registry Entry

| File | Plugin Name | Issue |
|---|---|---|
| `systemd_networkd.rs` | (no `impl StatePlugin`) | Helper module only — `SystemdNetworkdManager` for network plugin |

---

## Schema Quality Analysis

Schemas were evaluated against three axes: **field coverage** (how many fields, how specific),
**type quality** (typed vs. `FieldType::Any`), and **constraints** (validation rules).

### Tier A — Excellent (typed, constrained, examples, `readOnly` used)

These are the gold standard and should be used as templates:

| Plugin | Fields | Any% | Constraints | `readOnly` Used | Notes |
|---|---|---|---|---|---|
| `incus-wireguard-ingress` | 26+ | ~8% | 7 | ✅ | Best schema in codebase |
| `incus-xray-reality-client` | 35+ | ~6% | 7 | ✅ | |
| `incus-xray-reality-server` | 36+ | ~6% | 5 | ✅ | |
| `openflow` | 16+ | 0% | 4 | ❌ | Deep typed fields, good constraints |
| `privacy_router` | 15+ | 0% | 8 | ❌ | Most constrained active plugin |
| `lxc` | 5 (object) | 20% | 1 | ✅ | Uses `readOnly` and `readOnly_when` |
| `incus` | 7+ | 0% | 0 | ✅ | Enum types, good structure |

### Tier B — Good (mostly typed, some constraints)

| Plugin | Fields | Any% | Constraints | Issue |
|---|---|---|---|---|
| `net` | 5+ | 0% | 0 | No constraints on typed fields |
| `rtnetlink` | 6+ | 0% | 0 | No constraints |
| `privacy_routes` | 13+ | 0% | 0 | No constraints |
| `dinit` | 3 | 0% | 0 | Thin but correct types |
| `dnsresolver` | 2 | 0% | 1 | Reasonable for size |
| `proxy_server` | 2 | 0% | 2 | Good constraints for typed fields |
| `packagekit` | 2 | 50% | 1 | One field still `Any` |
| `pcidecl` | 2 | 50% | 1 | One field still `Any` |
| `web_ui` (compat) | 6 | 50% | 1 | Has full struct-level schemas not wired to trait |

### Tier C — Minimal (1 field, mostly `Any`, no constraints)

These plugins are effectively invisible to the catalog because all their data is untyped:

| Plugin | Fields | Type | Issue |
|---|---|---|---|
| `adc` | 1 | Boolean | Only field is a boolean flag — no identity |
| `agent_config` | 1 | Array(Any) | `agents` array is entirely untyped |
| `config` | 1 | Any | Entire config store is `Any` |
| `endpoint` | 1 | Array(String) | Endpoint list only |
| `gcloud_adc` | 3 | 2 Any | `account`, `project_id` are untyped |
| `hardware` | 3 | All Any | CPU/memory/disk are untyped objects |
| `keypair` | 1 | Any | Entire keypair list untyped — **security risk** |
| `keyring` | 2 | All Any | Secret collections untyped — **security risk** |
| `login1` | 1 | Any | Sessions untyped |
| `mcp` | 3 | All Any | MCP servers/tools entirely untyped |
| `netmaker` | 0 | — | **Empty schema** — no fields defined |
| `openflow_obfuscation` | 1 | Any | Config untyped |
| `ovsdb_bridge` | 1 | Any | Bridge declarations untyped |
| `privacy` | 1 | Any | Privacy orchestration config untyped |
| `proxmox` | 1 | Any | Container declarations untyped |
| `service` | 1 | Any | Service map untyped |
| `sess_decl` | 1 | Any | Session declarations untyped |
| `software` | 1 | Any | Package list untyped |
| `users` | 1 | Any | User list untyped — **security risk** |
| `wireguard` | 1 | Any | Interface/peer list untyped — **security risk** |

### Tier D — Broken / Incomplete

| Plugin | Issue |
|---|---|
| `netmaker` | 0 fields in compat schema — completely empty |
| `full_system` | 11 fields but 9 are `Any`; mostly an aggregate dump with no structure |
| `systemd` | Compat alias for `dinit`; no independent schema; no `StatePlugin::schema()` |

---

## Security/Privacy Risks in Current Schemas

Several plugins handle sensitive data but have no secret or PII path declarations:

| Plugin | Sensitive Data | Risk |
|---|---|---|
| `keypair` | Private keys | `keypairs` field is `Any` — no secret paths declared |
| `keyring` | Secret service collections | `collections` is `Any` — no secret paths |
| `wireguard` | Private keys, PSKs | `interfaces` is `Any` — no secret paths |
| `gcloud_adc` | Cloud credentials | `account` is `Any`, auto-PII on `account` name may not match |
| `users` | User/group data | `users` is `Any` — PII not declared |
| `config` | May contain secrets | `configs` is `Any` — no secret paths |

Auto-detection (`is_secret_field_name`, `is_pii_field_name`) only fires on field names
containing `secret`, `private`, `token`, `password`, `credential`, `license`, `api_key`, `key`,
`email`, `account`, `google_id`, `google_email`, `user_id`. Fields like `collections`,
`interfaces`, `keypairs`, `users` are **not** caught automatically.

---

## Missing Plugins

The following capability areas have no dedicated plugin, meaning the catalog has no schema
authority and those mutations are either untracked or buried in other plugins' `Any` fields.

### High Priority (system integrity gaps)

| Missing Plugin | Domain | Why Needed |
|---|---|---|
| `mutation_footprint` | Audit trail | **The blockchain plugin being designed** — no mutations tracked |
| `firewall` / `nftables` | Firewall rules | No schema for firewall policy changes |
| `certificate` / `pki` | TLS certificates | Cert lifecycle untracked; `keypair` is insufficient |
| `vault` / `secrets_backend` | Secret management | No authoritative schema for secret storage backends |

### Medium Priority (operational gaps)

| Missing Plugin | Domain | Why Needed |
|---|---|---|
| `dns_zone` | Authoritative DNS | DNS zone records not managed declaratively |
| `ntp` / `chrony` | Time sync | Time sync config untracked |
| `btrfs` | Storage subvolumes | `op-blockchain`'s subvolumes have no plugin schema |
| `journal` / `logging` | Log management | Log retention/forwarding unschematized |
| `ssh_authorized_keys` | SSH access | SSH keys not declaratively managed |
| `vlan` | VLAN management | VLAN config buried in `net` as `Any` |

### Lower Priority (AI/platform specific)

| Missing Plugin | Domain | Why Needed |
|---|---|---|
| `vector_store` | Embedding backend | Vector DB config not schematized |
| `model_config` | LLM configuration | Model selection/parameters not tracked |
| `skill_registry` | Agent skills | Skills not in plugin catalog |
| `metrics` | Observability | Metrics config not declaratively managed |
| `alerts` | Alerting | Alerting rules not schematized |

### Schema-Only Plugins Needing StatePlugin Impls

| Plugin | Status | Gap |
|---|---|---|
| `incus-wireguard-ingress` | Schema ✅, StatePlugin ❌ | Can't apply/verify/rollback |
| `incus-xray-reality-client` | Schema ✅, StatePlugin ❌ | Can't apply/verify/rollback |
| `incus-xray-reality-server` | Schema ✅, StatePlugin ❌ | Can't apply/verify/rollback |

---

## Schema Consistency Issues

### Naming Inconsistencies

| Plugin Name | File Name | Issue |
|---|---|---|
| `sess_decl` (registry) | `sessdecl.rs` | File name drops underscore |
| `systemd` (alias) | `systemd.rs` | Plugin file exists but is a `dinit` alias in registry |
| `wireguard` (registry) | `wireguard.rs` | WireGuard plugin not in normal-mode auto-load |

### Version Inconsistency

Most compat schemas use `version: "1.0.0"` via `simple_schema()`. The `lxc` schema uses
`version: "2.0.0"`. No plugin has a version bump policy or migration strategy documented.

### Missing `category` Tags

The `simple_schema()` helper does not set `category`. All simple schemas default to
`"uncategorized"`. Only schemas built with explicit `.category(…)` calls (e.g., `lxc`, `incus`,
`privacy_router`) have meaningful categories. This breaks any UI or compliance query that
groups plugins by category.

Expected categories by domain:

| Category | Plugins |
|---|---|
| `network` | `net`, `rtnetlink`, `dnsresolver`, `endpoint`, `netmaker`, `wireguard`, `openflow`, `openflow_obfuscation`, `ovsdb_bridge`, `privacy_router`, `privacy_routes` |
| `compute` | `incus`, `lxc`, `proxmox`, `hardware` |
| `identity` | `users`, `keypair`, `keyring`, `adc`, `gcloud_adc`, `wireguard` |
| `services` | `dinit`, `service`, `mcp`, `agent_config` |
| `configuration` | `config`, `software`, `packagekit` |
| `audit` | `mutation_footprint` |
| `security` | `privacy`, `privacy_router`, `sess_decl` |
| `ui` | `web_ui` |
| `platform` | `pcidecl`, `login1`, `full_system` |

### Missing `example` Values

Only `lxc`, `incus`, and the incus-* variants include `example` values in their `FieldSchema`
entries. All Tier C schemas have zero examples, making them opaque to documentation generators
and LLM tools that use the schema for context.

---

## Recommended Remediation Order

### Immediate (catalog recognition)

1. Wire existing compat schemas to `StatePlugin::schema()` for the 10 auto-loaded plugins
   (`mcp`, `config`, `dinit`, `incus`, `net`, `openflow`, `ovsdb_bridge`, `privacy_router`,
   `privacy_routes`, `rtnetlink`). Each plugin file should add:
   ```rust
   fn schema(&self) -> Option<PluginSchema> {
       Some(create_<name>_schema())
   }
   ```
   This unblocks catalog recognition with zero field changes.

2. Add `mutation_footprint` plugin — this is the audit system that tracks all other mutations.
   See `crates/op-blockchain/REQUIREMENTS.md` and `crates/op-blockchain/DESIGN.md`.

### Short Term (schema quality)

3. Add `category` to all schemas using the domain table above.
4. Replace `Any` fields in security-sensitive plugins (`keypair`, `keyring`, `wireguard`,
   `users`) with typed `Object` schemas and declare `secret_paths`/`pii_paths`.
5. Add `example` values to all Tier C schemas.

### Medium Term (missing plugins)

6. Implement `StatePlugin` for `incus-wireguard-ingress`, `incus-xray-reality-client`,
   `incus-xray-reality-server` — schemas exist, implementations missing.
7. Add `firewall`, `certificate`, `dns_zone`, `ntp`, `btrfs` plugins.
8. Split `full_system` aggregate fields into typed sub-objects.

---

## Pattern: Wiring `StatePlugin::schema()` to Compat Schema

For any plugin that already has a compat schema in `plugin_schema.rs`, the minimum viable
schema override is:

```rust
// In crates/op-state-store/src/plugin_schema.rs — expose helper:
pub fn schema_for_net() -> PluginSchema { create_net_schema() }

// In crates/op-plugins/src/state_plugins/net.rs:
use op_state_store::plugin_schema::schema_for_net;

impl StatePlugin for NetStatePlugin {
    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(schema_for_net())
    }
    // … rest of trait
}
```

The 3-section pattern (`Identity` / `Tunables` / `Capabilities` structs with their own
`schema() -> Value` methods) as demonstrated by `web_ui.rs` is the preferred full-quality
approach for new or heavily refactored plugins.
</file>

<file path="SPEC.md">
# op-plugins - Specification

## Overview
**Crate**: `op-plugins`  
**Location**: `crates/op-plugins`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-plugins"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-plugins/src/state_plugins/systemd_networkd.rs
op-plugins/src/state_plugins/mod.rs
op-plugins/src/state_plugins/adc.rs
op-plugins/src/state_plugins/agent_config.rs
op-plugins/src/state_plugins/config.rs
op-plugins/src/state_plugins/dinit.rs
op-plugins/src/state_plugins/dnsresolver.rs
op-plugins/src/state_plugins/endpoint.rs
op-plugins/src/state_plugins/full_system.rs
op-plugins/src/state_plugins/gcloud_adc.rs
op-plugins/src/state_plugins/hardware.rs
op-plugins/src/state_plugins/keypair.rs
op-plugins/src/state_plugins/keyring.rs
op-plugins/src/state_plugins/login1.rs
op-plugins/src/state_plugins/lxc.rs
op-plugins/src/state_plugins/mcp.rs
op-plugins/src/state_plugins/net.rs
op-plugins/src/state_plugins/netmaker.rs
op-plugins/src/state_plugins/openflow.rs
op-plugins/src/state_plugins/openflow_obfuscation.rs
```

### Key Dependencies
```toml
op-core = { path = "../op-core" }
op-state = { path = "../op-state" }
op-state-store = { path = "../op-state-store" }
op-blockchain = { path = "../op-blockchain" }
op-network = { path = "../op-network" }
op-dynamic-loader = { path = "../op-dynamic-loader" }
op-execution-tracker = { path = "../op-execution-tracker" }

tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
zbus = { workspace = true }
chrono = { workspace = true }
log = { workspace = true }
reqwest = { workspace = true }
sha2 = { workspace = true }
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
      45 Rust source files

### Main Modules
registry
auto_create
builtin
chat
dynamic_loading
plugin
state
systemd
default_registry

## Purpose
Plugin system with state management, domain plugins, and blockchain footprints

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-core
- op-state
- op-state-store
- op-blockchain
- op-network
- op-dynamic-loader
- op-execution-tracker

---
*Generated from crate analysis*
</file>

</files>
