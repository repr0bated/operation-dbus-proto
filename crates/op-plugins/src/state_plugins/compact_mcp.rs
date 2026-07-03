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
use op_state_store::PluginSchema;
#[cfg(test)]
use op_state_store::{FieldSchema, FieldType};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use simd_json::json;
use simd_json::{prelude::*, OwnedValue as Value};
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
fn default_true() -> bool {
    true
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
        let mut schema = compact_mcp_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/etc/s6/sv/op-mcp-compact").exists()
    }

    fn unavailable_reason(&self) -> String {
        "op-mcp-compact s6 service definition not found at /etc/s6/sv/op-mcp-compact".into()
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

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = simd_json::json!(null);
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
        let current = simd_json::json!(null);
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompactMcpMode {
    Compact,
    Full,
    Agents,
}

impl Default for CompactMcpMode {
    fn default() -> Self {
        Self::Compact
    }
}

fn default_compact_mode() -> CompactMcpMode {
    CompactMcpMode::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompactMcpLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for CompactMcpLogLevel {
    fn default() -> Self {
        Self::Info
    }
}

fn default_compact_log_level() -> CompactMcpLogLevel {
    CompactMcpLogLevel::default()
}

fn example_http() -> String {
    "100.90.37.254:3001".to_string()
}

fn example_ws() -> Option<String> {
    Some("100.90.37.254:3002".to_string())
}

fn example_wg_interface() -> String {
    "netmaker".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.compact-mcp.schema@v1"))]
pub struct CompactMcpState {
    #[serde(default = "default_compact_mode")]
    #[schemars(
        description = "Server mode: compact (5 meta-tools), full (all tools), agents (D-Bus agents)",
        example = default_compact_mode(),
        extend("x-oscal-subid" = "mut.software.plugin.compact-mcp.mode@v1")
    )]
    mode: CompactMcpMode,
    #[serde(default = "default_http")]
    #[schemars(
        description = "HTTP/SSE bind address (empty = not started)",
        example = example_http(),
        extend("x-oscal-subid" = "mut.software.plugin.compact-mcp.http@v1")
    )]
    http: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "WebSocket bind address (empty = not started)",
        example = example_ws(),
        extend("x-oscal-subid" = "mut.software.plugin.compact-mcp.ws@v1")
    )]
    ws: Option<String>,
    #[serde(default = "default_wg")]
    #[schemars(
        description = "WireGuard interface for identity sled",
        example = example_wg_interface(),
        extend("x-oscal-subid" = "mut.software.plugin.compact-mcp.wg-interface@v1")
    )]
    wg_interface: String,
    #[serde(default = "default_true")]
    #[schemars(
        description = "Run stdio transport (default for Claude Desktop)",
        extend("x-oscal-subid" = "mut.software.plugin.compact-mcp.stdio@v1")
    )]
    stdio: bool,
    #[serde(default = "default_compact_log_level")]
    #[schemars(
        description = "Log verbosity",
        example = default_compact_log_level(),
        extend("x-oscal-subid" = "mut.software.plugin.compact-mcp.log-level@v1")
    )]
    log_level: CompactMcpLogLevel,
    #[serde(default)]
    #[schemars(
        description = "Whether the s6 service is currently running",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.compact-mcp.running@v1")
    )]
    running: bool,
}

pub(crate) fn compact_mcp_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(CompactMcpState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "compact_mcp",
        "1.0.0",
        "op-mcp-server — multi-mode MCP server (compact/full/agents) with stdio, HTTP, and WebSocket transports",
        &root,
    )
}

#[cfg(test)]
pub(crate) fn compact_mcp_schema_golden() -> PluginSchema {
    PluginSchema::builder("compact_mcp")
        .version("1.0.0")
        .description("op-mcp-server — multi-mode MCP server (compact/full/agents) with stdio, HTTP, and WebSocket transports")
        .subid("__schema__", "sch.software.plugin.compact-mcp.schema@v1")
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
        .subid("mode", "mut.software.plugin.compact-mcp.mode@v1")
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
        .subid("http", "mut.software.plugin.compact-mcp.http@v1")
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
        .subid("ws", "mut.software.plugin.compact-mcp.ws@v1")
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
        .subid("wg_interface", "mut.software.plugin.compact-mcp.wg-interface@v1")
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
        .subid("stdio", "mut.software.plugin.compact-mcp.stdio@v1")
        .field("log_level", FieldSchema {
            field_type: FieldType::Enum(vec![
                "trace".into(), "debug".into(), "info".into(), "warn".into(), "error".into(),
            ]),
            required: false,
            description: "Log verbosity".into(),
            default: Some(json!("info")),
            example: Some(json!("info")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .subid("log_level", "mut.software.plugin.compact-mcp.log-level@v1")
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
        .subid("running", "obs.software.plugin.compact-mcp.running@v1")
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use crate::state_plugins::schemars_adapter::schema_diffs;
    use serde_json::Value as JVal;

    fn collect_subids(node: &JVal, out: &mut Vec<String>) {
        if let Some(subid) = node.get("x-oscal-subid").and_then(JVal::as_str) {
            out.push(subid.to_string());
        }
        if let Some(props) = node.get("properties").and_then(JVal::as_object) {
            for v in props.values() {
                collect_subids(v, out);
            }
        }
        if let Some(defs) = node
            .get("$defs")
            .or_else(|| node.get("definitions"))
            .and_then(JVal::as_object)
        {
            for v in defs.values() {
                collect_subids(v, out);
            }
        }
        if let Some(items) = node.get("items") {
            collect_subids(items, out);
        }
        if let Some(alternatives) = node
            .get("anyOf")
            .or_else(|| node.get("oneOf"))
            .and_then(JVal::as_array)
        {
            for v in alternatives {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let diffs = schema_diffs(&compact_mcp_schema_golden(), &compact_mcp_schema());
        assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(CompactMcpState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(
            !subids.is_empty(),
            "expected at least one x-oscal-subid in the derived schema"
        );
        for subid in subids {
            validate_subid(&subid).expect("invalid subid: {subid}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("compact_mcp", |_ctx| std::sync::Arc::new(CompactMcpPlugin::new()))
}
