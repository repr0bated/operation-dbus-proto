//! Compact MCP state plugin
//!
//! Tracks and manages the op-mcp-server: mode, transport bind addresses,
//! WireGuard identity, and tool registry.  Publishes live state to D-Bus
//! under `/opdbus/v1/plugins/compact_mcp` so that
//! `register_plugin_projection_tools` can expose it as MCP tools.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::{json, prelude::*, OwnedValue as Value};
use zbus::{Connection, Proxy};

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
        // D-Bus only per AGENTS.md §4 - no subprocess fallbacks
        Self::reload_service_dbus().await
    }

    async fn reload_service_dbus() -> Result<()> {
        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        let proxy = Proxy::new(
            &conn,
            "opdbus.v1",
            "/opdbus/v1/s6/systemctl",
            "opdbus.v1.S6.Systemctl",
        )
        .await
        .context("Failed to create s6-systemctl D-Bus proxy")?;

        let (success, message): (bool, String) =
            proxy
                .call("reload", &("op-mcp-compact",))
                .await
                .context("Failed to call reload on s6-systemctl")?;

        if success {
            tracing::info!("Reloaded op-mcp-compact via D-Bus: {}", message);
            Ok(())
        } else {
            anyhow::bail!("s6-systemctl reload failed: {}", message)
        }
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
        Some(compact_mcp_schema())
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

pub(crate) fn compact_mcp_schema() -> PluginSchema {
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
