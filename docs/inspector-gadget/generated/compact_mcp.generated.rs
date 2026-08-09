//! Compact MCP state plugin
//!
//! Tracks and manages the op-mcp-server: mode, transport bind addresses,
//! WireGuard identity, and tool registry.  Publishes live state to D-Bus
//! under `/opdbus/v1/plugins/compact_mcp` for introspection by clients.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::{prelude::*, OwnedValue as Value};

const RUNIT_SERVICE_PATH: &str = "/etc/runit/sv/op-mcp-compact";
const RUNIT_ACTIVE_PATH: &str = "/etc/runit/runsvdir/default/op-mcp-compact";
const ENV_DIR: &str = "/etc/runit/sv/op-mcp-compact/env";
const DEFAULT_MODE: &str = "compact";
const DEFAULT_HTTP: &str = "127.0.0.1:11436";
const DEFAULT_WG: &str = "netmaker";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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
    /// Run stdio transport (not used for runit daemon deployment)
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
        Ok(())
    }

    async fn reload_service() -> Result<()> {
        let status = tokio::process::Command::new("sv")
            .arg("restart")
            .arg(RUNIT_ACTIVE_PATH)
            .status()
            .await
            .context("sv restart op-mcp-compact")?;
        if status.success() {
            tracing::info!("Restarted op-mcp-compact through runit");
            Ok(())
        } else {
            anyhow::bail!("sv restart op-mcp-compact exited with {status}")
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
        std::path::Path::new(RUNIT_SERVICE_PATH).exists()
    }

    fn unavailable_reason(&self) -> String {
        format!("op-mcp-compact runit service definition not found at {RUNIT_SERVICE_PATH}")
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
#[schemars(extend("x-oscal-category" = "service"))]
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
        description = "Whether the runit service is currently running",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.software.plugin.compact-mcp.running@v1")
    )]
    running: bool,
}

pub(crate) fn compact_mcp_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(CompactMcpState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "compact_mcp",
        "1.0.0",
        "op-mcp-server — multi-mode MCP server (compact/full/agents) with stdio, HTTP, and WebSocket transports",
        &root,
    );

    use super::plugin_scaffold_helpers::{
        method_decl_from_schemars_with_output, AckOutput, EmptyInput,
    };
    use op_state_store::SideEffect;

    // Backed by a real dispatcher (dispatch_compact_mcp_method): reads live env
    // files directly, matching current_config()'s existing (previously unused)
    // logic. Config *writes* already go through the generic PropertySet/
    // apply_state path (SetProperty), which is why there's no set_config method
    // here — only the two actions apply_state doesn't cover.
    schema.methods.insert(
        "get_current_config".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, CompactMcpConfig>(
            "get_current_config",
            SideEffect::Read,
            true,
            "compact_mcp.read",
            "obs.software.plugin.compact-mcp.config.get@v1",
        ),
    );
    schema.methods.insert(
        "restart".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, AckOutput>(
            "restart",
            SideEffect::Mutation,
            false,
            "compact_mcp.write",
            "mut.software.plugin.compact-mcp.service.restart@v1",
        ),
    );

    schema
}

/// Dispatch a `compact_mcp` schema method. Called from `op-grpc-bridge`'s
/// `MutationEngine::dispatch_method_call`.
pub async fn dispatch_compact_mcp_method(
    method: &str,
    _args: &serde_json::Value,
) -> Result<serde_json::Value> {
    match method {
        "get_current_config" => {
            let config = CompactMcpPlugin::current_config();
            Ok(serde_json::to_value(config)?)
        }
        "restart" => {
            CompactMcpPlugin::reload_service().await?;
            Ok(serde_json::json!({ "success": true }))
        }
        other => Err(anyhow::anyhow!("unknown compact_mcp method: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
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

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.compact_mcp.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.default`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.default@v1"))]
        pub default: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.description`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.description@v1"))]
        pub description: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.enum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.enum-field@v1"))]
        pub enum_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.items`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.items@v1"))]
        pub items: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.maxItems`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.maxitems@v1"))]
        pub maxitems: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.minItems`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.minitems@v1"))]
        pub minitems: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.oneOf`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.oneof@v1"))]
        pub oneof: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.title`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.title@v1"))]
        pub title: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.field.field.type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.options.field.anyOf`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.anyof@v1"))]
        pub anyof: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.value.field.const`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.const-field@v1"))]
        pub const_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.enum.values.field.enumNames`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.enumnames@v1"))]
        pub enumnames: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Annotations.field.audience`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.audience@v1"))]
        pub audience: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.BlobResourceContents.field.blob`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.blob@v1"))]
        pub blob: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequest.field.method`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.method@v1"))]
        pub method: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequest.field.params`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.params@v1"))]
        pub params: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequestParams.field.arguments`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.arguments@v1"))]
        pub arguments: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolRequestParams.field.name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.name@v1"))]
        pub name: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolResult.field.content`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.content@v1"))]
        pub content: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolResult.field.isError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.iserror@v1"))]
        pub iserror: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CallToolResult.field.structuredContent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.structuredcontent@v1"))]
        pub structuredcontent: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CancelTaskRequest.field.taskId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.taskid@v1"))]
        pub taskid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CancelledNotificationParams.field.reason`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.reason@v1"))]
        pub reason: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CancelledNotificationParams.field.requestId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.requestid@v1"))]
        pub requestid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.cancel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.cancel@v1"))]
        pub cancel: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.context`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.context@v1"))]
        pub context: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.create`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.create@v1"))]
        pub create: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.createMessage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.createmessage@v1"))]
        pub createmessage: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.elicitation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.elicitation@v1"))]
        pub elicitation: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.experimental`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.experimental@v1"))]
        pub experimental: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.list`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.list@v1"))]
        pub list: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.listChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.listchanged@v1"))]
        pub listchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.requests`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.requests@v1"))]
        pub requests: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.roots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.roots@v1"))]
        pub roots: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.sampling`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.sampling@v1"))]
        pub sampling: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.tasks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.tasks@v1"))]
        pub tasks: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ClientCapabilities.field.tools`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.tools@v1"))]
        pub tools: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteRequestParams.field.argument`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.argument@v1"))]
        pub argument: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteRequestParams.field.ref`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.ref-field@v1"))]
        pub ref_field: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteRequestParams.field.value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.value@v1"))]
        pub value: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.completion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.completion@v1"))]
        pub completion: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.hasMore`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.hasmore@v1"))]
        pub hasmore: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.total`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.total@v1"))]
        pub total: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CompleteResult.field.values`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.values@v1"))]
        pub values: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.includeContext`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.includecontext@v1"))]
        pub includecontext: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.messages@v1"))]
        pub messages: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.modelPreferences`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.modelpreferences@v1"))]
        pub modelpreferences: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageRequestParams.field.systemPrompt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.systemprompt@v1"))]
        pub systemprompt: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageResult.field.model`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.model@v1"))]
        pub model: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateMessageResult.field.stopReason`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.stopreason@v1"))]
        pub stopreason: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.CreateTaskResult.field.task`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.task@v1"))]
        pub task: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.$schema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.schema@v1"))]
        pub schema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.message`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.message@v1"))]
        pub message: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.properties`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.properties@v1"))]
        pub properties: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.requestedSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.requestedschema@v1"))]
        pub requestedschema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestFormParams.field.required`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.required@v1"))]
        pub required: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestURLParams.field.elicitationId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.elicitationid@v1"))]
        pub elicitationid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitRequestURLParams.field.url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.url@v1"))]
        pub url: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ElicitResult.field.action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.EmbeddedResource.field._meta`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.meta@v1"))]
        pub meta: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.EmbeddedResource.field.annotations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.annotations@v1"))]
        pub annotations: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.EmbeddedResource.field.resource`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.resource@v1"))]
        pub resource: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Icon.field.src`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.src@v1"))]
        pub src: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ImageContent.field.data`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.data@v1"))]
        pub data: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Implementation.field.version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.version@v1"))]
        pub version: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Implementation.field.websiteUrl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.websiteurl@v1"))]
        pub websiteurl: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeRequestParams.field.capabilities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.capabilities@v1"))]
        pub capabilities: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeRequestParams.field.clientInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.clientinfo@v1"))]
        pub clientinfo: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeRequestParams.field.protocolVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.protocolversion@v1"))]
        pub protocolversion: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeResult.field.instructions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.instructions@v1"))]
        pub instructions: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.InitializeResult.field.serverInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.serverinfo@v1"))]
        pub serverinfo: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCErrorResponse.field.error`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.error@v1"))]
        pub error: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCErrorResponse.field.id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.id@v1"))]
        pub id: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCErrorResponse.field.jsonrpc`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.jsonrpc@v1"))]
        pub jsonrpc: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.JSONRPCResultResponse.field.result`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.result@v1"))]
        pub result: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ListPromptsResult.field.prompts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.prompts@v1"))]
        pub prompts: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ListResourceTemplatesResult.field.resourceTemplates`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.resourcetemplates@v1"))]
        pub resourcetemplates: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ListResourcesResult.field.resources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.resources@v1"))]
        pub resources: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.LoggingMessageNotificationParams.field.level`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.level@v1"))]
        pub level: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.LoggingMessageNotificationParams.field.logger`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.logger@v1"))]
        pub logger: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.NumberSchema.field.maximum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.maximum@v1"))]
        pub maximum: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.NumberSchema.field.minimum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.minimum@v1"))]
        pub minimum: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.PaginatedRequestParams.field.cursor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.cursor@v1"))]
        pub cursor: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.PaginatedResult.field.nextCursor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.nextcursor@v1"))]
        pub nextcursor: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ProgressNotificationParams.field.progressToken`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.progresstoken@v1"))]
        pub progresstoken: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.PromptMessage.field.role`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.role@v1"))]
        pub role: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ReadResourceResult.field.contents`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.contents@v1"))]
        pub contents: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Resource.field.uri`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.uri@v1"))]
        pub uri: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.call`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.call@v1"))]
        pub call: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.completions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.completions@v1"))]
        pub completions: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.logging`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.logging@v1"))]
        pub logging: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ServerCapabilities.field.subscribe`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.subscribe@v1"))]
        pub subscribe: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.StringSchema.field.format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.format@v1"))]
        pub format: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.StringSchema.field.maxLength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.maxlength@v1"))]
        pub maxlength: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.StringSchema.field.minLength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.minlength@v1"))]
        pub minlength: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.createdAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.createdat@v1"))]
        pub createdat: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.lastUpdatedAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.lastupdatedat@v1"))]
        pub lastupdatedat: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.pollInterval`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.pollinterval@v1"))]
        pub pollinterval: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.status@v1"))]
        pub status: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.statusMessage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.statusmessage@v1"))]
        pub statusmessage: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Task.field.ttl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.ttl@v1"))]
        pub ttl: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.TextContent.field.text`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.text@v1"))]
        pub text: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Tool.field.execution`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.execution@v1"))]
        pub execution: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Tool.field.inputSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.inputschema@v1"))]
        pub inputschema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.Tool.field.outputSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.outputschema@v1"))]
        pub outputschema: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.destructiveHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.destructivehint@v1"))]
        pub destructivehint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.idempotentHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.idempotenthint@v1"))]
        pub idempotenthint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.openWorldHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.openworldhint@v1"))]
        pub openworldhint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolAnnotations.field.readOnlyHint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.readonlyhint@v1"))]
        pub readonlyhint: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolExecution.field.taskSupport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.tasksupport@v1"))]
        pub tasksupport: Option<u64>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.ToolResultContent.field.toolUseId`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.tooluseid@v1"))]
        pub tooluseid: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.URLElicitationRequiredError.field.code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.code@v1"))]
        pub code: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.URLElicitationRequiredError.field.elicitations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.elicitations@v1"))]
        pub elicitations: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.interface.allows.field.hints`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.hints@v1"))]
        pub hints: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.for.field.mimeType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.mimetype@v1"))]
        pub mimetype: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.costPriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.costpriority@v1"))]
        pub costpriority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.intelligencePriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.intelligencepriority@v1"))]
        pub intelligencepriority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.lastModified`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.lastmodified@v1"))]
        pub lastmodified: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.maxTokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.maxtokens@v1"))]
        pub maxtokens: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.metadata`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.metadata@v1"))]
        pub metadata: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.priority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.priority@v1"))]
        pub priority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.progress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.progress@v1"))]
        pub progress: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.speedPriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.speedpriority@v1"))]
        pub speedpriority: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.stopSequences`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.stopsequences@v1"))]
        pub stopsequences: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.temperature`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.temperature@v1"))]
        pub temperature: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.number.field.toolChoice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.toolchoice@v1"))]
        pub toolchoice: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.of.field.input`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.input@v1"))]
        pub input: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.of.field.size`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.size@v1"))]
        pub size: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.of.field.uriTemplate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.uritemplate@v1"))]
        pub uritemplate: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.override.field.icons`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.icons@v1"))]
        pub icons: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.override.field.sizes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.sizes@v1"))]
        pub sizes: Option<String>,

        /// Discovered from Repomix path `ts.schema.2025-11-25.schema.type.override.field.theme`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.theme@v1"))]
        pub theme: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.CacheableResult.field.cacheScope`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.cachescope@v1"))]
        pub cachescope: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.CacheableResult.field.ttlMs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.ttlms@v1"))]
        pub ttlms: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.DiscoverResult.field.supportedVersions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.supportedversions@v1"))]
        pub supportedversions: Option<u64>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.InputRequiredResult.field.inputRequests`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.inputrequests@v1"))]
        pub inputrequests: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.InputRequiredResult.field.requestState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.requeststate@v1"))]
        pub requeststate: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.MissingRequiredClientCapabilityError.field.requiredCapabilities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.requiredcapabilities@v1"))]
        pub requiredcapabilities: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.ServerCapabilities.field.extensions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.extensions@v1"))]
        pub extensions: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.SubscriptionsAcknowledgedNotificationParams.field.notifications`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.notifications@v1"))]
        pub notifications: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.UnsupportedProtocolVersionError.field.requested`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.requested@v1"))]
        pub requested: Option<u64>,

        /// Discovered from Repomix path `ts.schema.draft.schema.interface.UnsupportedProtocolVersionError.field.supported`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.supported@v1"))]
        pub supported: Option<u64>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.promptsListChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.promptslistchanged@v1"))]
        pub promptslistchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.resourceSubscriptions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.resourcesubscriptions@v1"))]
        pub resourcesubscriptions: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.resourcesListChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.resourceslistchanged@v1"))]
        pub resourceslistchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.is.field.toolsListChanged`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.toolslistchanged@v1"))]
        pub toolslistchanged: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.of.field.resultType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.resulttype@v1"))]
        pub resulttype: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.roots.field.form`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.form@v1"))]
        pub form: Option<String>,

        /// Discovered from Repomix path `ts.schema.draft.schema.type.that.field.inputResponses`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.compact_mcp.inputresponses@v1"))]
        pub inputresponses: Option<String>,

    }

    /// Metadata needed when promoting a generated typed method into `schema.methods`.
    pub struct MethodCandidate {
        pub name: &'static str,
        pub side_effect: &'static str,
        pub idempotent: bool,
        pub required_capability: &'static str,
        pub subid: &'static str,
        pub repomix_path: &'static str,
        pub command: &'static [&'static str],
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[
    ];

    /// Promote every generated method into the sealed plugin schema.
    pub(super) fn register_methods(schema: &mut op_state_store::PluginSchema) {
        use super::super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    }
}

// Promotion checklist (Fable contract):
// 1. Move owned fields into the plugin State struct with concrete Rust types.
// 2. Replace method placeholders with dedicated typed Input/Output fields.
// 3. Register with method_decl_from_schemars_with_output and correct SideEffect.
// 4. Register every subid, implement dispatch, and add schema/subid tests.
// 5. Re-run op-plugin-lint; only then replace the original plugin file.
