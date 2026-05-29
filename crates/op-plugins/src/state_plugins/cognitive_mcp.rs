//! Cognitive MCP state plugin
//!
//! Tracks and manages the op-cognitive-mcp server: bind addresses, WireGuard
//! identity, tool registrations, and gRPC/HTTP health.  Publishes live state
//! to D-Bus under `/org/opdbus/v1/plugins/cognitive_mcp` so that
//! `register_plugin_projection_tools` can expose it as MCP tools.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};

const S6_SV_PATH: &str = "/run/service/op-cognitive-mcp";
const ENV_DIR: &str = "/etc/s6/sv/op-cognitive-mcp/env";
const DEFAULT_HTTP: &str = "0.0.0.0:3003";
const DEFAULT_GRPC: &str = "0.0.0.0:50052";
const DEFAULT_DB: &str = "/var/lib/op-dbus/cognitive.db";
const DEFAULT_WG: &str = "netmaker";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitiveMcpConfig {
    /// HTTP/SSE bind address (MCP protocol)
    #[serde(default = "default_http")]
    pub http: String,
    /// gRPC bind address (CognitiveToolService)
    #[serde(default = "default_grpc")]
    pub grpc: String,
    /// CozoDB database path
    #[serde(default = "default_db")]
    pub db_path: String,
    /// WireGuard interface for identity
    #[serde(default = "default_wg")]
    pub wg_interface: String,
    /// Whether the HTTP server is enabled
    #[serde(default = "default_true")]
    pub http_enabled: bool,
    /// Whether the gRPC server is enabled
    #[serde(default = "default_true")]
    pub grpc_enabled: bool,
    /// Whether D-Bus registration is enabled
    #[serde(default = "default_true")]
    pub dbus_enabled: bool,
}

fn default_http() -> String {
    DEFAULT_HTTP.into()
}
fn default_grpc() -> String {
    DEFAULT_GRPC.into()
}
fn default_db() -> String {
    DEFAULT_DB.into()
}
fn default_wg() -> String {
    DEFAULT_WG.into()
}
fn default_true() -> bool {
    true
}

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

pub struct CognitiveMcpPlugin;

impl CognitiveMcpPlugin {
    pub fn new() -> Self {
        Self
    }

    /// True if the s6 service is currently up.
    fn service_running() -> bool {
        // s6 creates /run/service/<name> when the service is supervised.
        // The `down` file means the service is intentionally stopped.
        let sv = std::path::Path::new(S6_SV_PATH);
        if !sv.exists() {
            return false;
        }
        !sv.join("down").exists()
    }

    /// Read a single env-dir variable.
    fn read_env(key: &str) -> Option<String> {
        std::fs::read_to_string(format!("{ENV_DIR}/{key}"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Build current config from env-dir overrides + defaults.
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

    /// Write a variable to the env-dir (creating the directory if needed).
    async fn write_env(key: &str, value: &str) -> Result<()> {
        tokio::fs::create_dir_all(ENV_DIR)
            .await
            .context("create env dir")?;
        tokio::fs::write(format!("{ENV_DIR}/{key}"), value)
            .await
            .with_context(|| format!("write env {key}"))
    }

    /// Signal s6 to reload the service after config change.
    async fn reload_service() -> Result<()> {
        let out = tokio::process::Command::new("s6-svc")
            .args(["-r", S6_SV_PATH])
            .output()
            .await
            .context("s6-svc -r op-cognitive-mcp")?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::warn!("s6-svc -r failed: {stderr}");
        }
        Ok(())
    }
}

impl Default for CognitiveMcpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn cognitive_mcp_plugin_schema() -> PluginSchema {
    PluginSchema::builder("cognitive_mcp")
        .version("1.0.0")
        .description("Cognitive MCP server — memory, NotebookLM bridge, gRPC CognitiveToolService")
        .field(
            "http",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "HTTP/SSE bind address for the MCP protocol endpoint".into(),
                default: Some(json!(DEFAULT_HTTP)),
                example: Some(json!("100.90.37.254:3003")),
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "grpc",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "gRPC bind address for the CognitiveToolService endpoint".into(),
                default: Some(json!(DEFAULT_GRPC)),
                example: Some(json!("100.90.37.254:50052")),
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "db_path",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "CozoDB database path for persistent memory storage".into(),
                default: Some(json!(DEFAULT_DB)),
                example: Some(json!("/var/lib/op-dbus/cognitive.db")),
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "wg_interface",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "WireGuard interface to read identity from".into(),
                default: Some(json!(DEFAULT_WG)),
                example: Some(json!("netmaker")),
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "http_enabled",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Enable the HTTP/SSE MCP transport".into(),
                default: Some(json!(true)),
                example: None,
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "grpc_enabled",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Enable the gRPC CognitiveToolService transport".into(),
                default: Some(json!(true)),
                example: None,
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "dbus_enabled",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Register on D-Bus as org.opdbus.CognitiveMcp".into(),
                default: Some(json!(true)),
                example: None,
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "running",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether the s6 service is currently running".into(),
                default: Some(json!(false)),
                example: None,
                constraints: vec![],
                read_only: true,
                read_only_when: None,
            },
        )
        .build()
}

#[async_trait]
impl StatePlugin for CognitiveMcpPlugin {
    fn name(&self) -> &str {
        "cognitive_mcp"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(cognitive_mcp_plugin_schema())
    }

    fn is_available(&self) -> bool {
        // Available if the s6 service definition exists.
        std::path::Path::new("/etc/s6/sv/op-cognitive-mcp").exists()
    }

    fn unavailable_reason(&self) -> String {
        "op-cognitive-mcp s6 service definition not found at /etc/s6/sv/op-cognitive-mcp".into()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut cfg = simd_json::serde::to_owned_value(Self::current_config())?;
        // Inject live running status.
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert(
                "running".into(),
                simd_json::OwnedValue::from(Self::service_running()),
            );
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
            if let StateAction::Modify {
                resource,
                changes: val,
            } = action
            {
                let result: Result<()> = match resource.as_str() {
                    "http" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_BIND", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("http must be a string"))
                        }
                    }
                    "grpc" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_GRPC_BIND", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("grpc must be a string"))
                        }
                    }
                    "db_path" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_DB_PATH", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("db_path must be a string"))
                        }
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
                    "http_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_HTTP_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(format!(
                                "{ENV_DIR}/COGNITIVE_MCP_HTTP_DISABLED"
                            ))
                            .await;
                        }
                        needs_reload = true;
                        Ok(())
                    }
                    "grpc_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_GRPC_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(format!(
                                "{ENV_DIR}/COGNITIVE_MCP_GRPC_DISABLED"
                            ))
                            .await;
                        }
                        needs_reload = true;
                        Ok(())
                    }
                    "dbus_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_DBUS_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(format!(
                                "{ENV_DIR}/COGNITIVE_MCP_DBUS_DISABLED"
                            ))
                            .await;
                        }
                        needs_reload = true;
                        Ok(())
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
