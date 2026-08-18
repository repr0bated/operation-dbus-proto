use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{CapabilityDecl, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

use super::plugin_scaffold_helpers::{
    method_decl_from_schemars, method_decl_from_schemars_with_output,
};

use super::xray_config_types::{
    APIConfig, DNSConfig, PolicyConfig, XrayInboundProtocolCatalog, XrayOutboundProtocolCatalog,
};

/// Xray proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.config.schema@v1"))]
pub struct XrayConfig {
    /// Whether the xray proxy is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.xray.config.enabled@v1"))]
    pub enabled: bool,
    /// Xray socket tag/port name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.xray.config.socket-port@v1"))]
    pub socket_port: String,
    /// Path to the xray JSON config.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.xray.config.config-path@v1"))]
    pub config_path: String,
    /// Xray commander API block (`api`) — StatsService/RoutingService/LoggerService.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.config.api@v1"))]
    pub api: Option<APIConfig>,
    /// Xray DNS block (`dns`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.config.dns@v1"))]
    pub dns: Option<DNSConfig>,
    /// Xray policy block (`policy`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.config.policy@v1"))]
    pub policy: Option<PolicyConfig>,
    /// Reference catalog of every inbound-protocol settings shape xray-core defines (dokodemo/http/socks/vmess/vless/trojan/shadowsocks/freedom).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.config.inbound-protocols@v1"))]
    pub inbound_protocols: XrayInboundProtocolCatalog,
    /// Reference catalog of every outbound-protocol settings shape xray-core defines (blackhole/freedom/http/socks/vmess/vless/trojan/shadowsocks).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.config.outbound-protocols@v1"))]
    pub outbound_protocols: XrayOutboundProtocolCatalog,
}

impl Default for XrayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            socket_port: "gbr_xray".to_string(),
            config_path: "/etc/xray/xray_config.json".to_string(),
            api: None,
            dns: None,
            policy: None,
            inbound_protocols: XrayInboundProtocolCatalog::default(),
            outbound_protocols: XrayOutboundProtocolCatalog::default(),
        }
    }
}

/// Runtime state of the xray plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.schema@v1"))]
#[schemars(extend("x-oscal-category" = "network"))]
pub struct XrayState {
    /// Software identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.xray.software@v1"))]
    pub software: String,
    /// Software version.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.xray.version@v1"))]
    pub version: String,
    /// Runtime dependencies.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.xray.dependencies@v1"))]
    pub dependencies: Vec<String>,
    /// OSCAL subid registry source path.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.plugin.xray.oscal-source@v1"))]
    pub oscal_source: Option<String>,
    /// Xray configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.config@v1"))]
    pub config: XrayConfig,
    /// Whether an xray process is currently running (host-native).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.xray.running@v1"))]
    pub running: bool,
    /// MCP tool definitions exposed by this plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.xray.tools@v1"))]
    pub tools: serde_json::Value,
}

impl Default for XrayState {
    fn default() -> Self {
        Self {
            software: "xray-core".to_string(),
            version: "1.0.0".to_string(),
            dependencies: vec!["incus".to_string()],
            oscal_source: Some("/org/opdbus/v1/plugins/oscal_subid_registry".to_string()),
            config: XrayConfig::default(),
            running: false,
            tools: serde_json::json!([
                {
                    "name": "xray.run",
                    "description": "Run the Xray daemon",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "config": {
                                "type": "string",
                                "description": "The path to the config file"
                            }
                        },
                        "required": ["config"]
                    }
                }
            ]),
        }
    }
}

/// Find running `xray` process PIDs by scanning `/proc` directly — no
/// `pgrep`/`pkill` subprocess spawning (CLAUDE.md: no `Command::new(...)`
/// subprocesses in plugin/service code).
fn find_xray_pids() -> Vec<nix::unistd::Pid> {
    let mut pids = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return pids;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let comm_path = entry.path().join("comm");
        if let Ok(comm) = std::fs::read_to_string(&comm_path) {
            if comm.trim() == "xray" {
                pids.push(nix::unistd::Pid::from_raw(pid));
            }
        }
    }
    pids
}

/// Is an xray process currently running on the host?
fn xray_running() -> bool {
    !find_xray_pids().is_empty()
}

pub struct XrayPlugin {
    config: XrayConfig,
}

impl XrayPlugin {
    pub fn new(config: XrayConfig) -> Self {
        Self { config }
    }
    pub(crate) fn current_state() -> XrayState {
        XrayState {
            software: "xray-core".to_string(),
            version: "1.0.0".to_string(),
            dependencies: vec!["incus".to_string()],
            oscal_source: Some("/org/opdbus/v1/plugins/oscal_subid_registry".to_string()),
            config: XrayConfig::default(),
            running: false,
            tools: serde_json::json!([
                {
                    "name": "xray.run",
                    "description": "Run the Xray daemon",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "config": {
                                "type": "string",
                                "description": "The path to the config file"
                            }
                        },
                        "required": ["config"]
                    }
                }
            ]),
        }
    }
}

#[async_trait]
impl StatePlugin for XrayPlugin {
    fn name(&self) -> &'static str {
        "xray"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        let mut schema = xray_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: op_state::DiffMetadata {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                current_hash: "".to_string(),
                desired_hash: "".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        // xray control point (the ONLY projectable tree: /org/opdbus/v1/plugins/xray).
        // Host-native lifecycle — no out-of-tree `opdbus.v1.Xray` daemon.
        let mut changes = Vec::new();
        let mut errors = Vec::new();
        let pids = find_xray_pids();
        if self.config.enabled {
            // Reload (SIGHUP) so a running xray re-reads its config; if none is
            // running, the container's own supervisor (systemd, inside the
            // `xray` container) is responsible for starting it.
            if pids.is_empty() {
                changes.push("xray not running; supervisor starts it".to_string());
            } else {
                for pid in pids {
                    match nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGHUP) {
                        Ok(()) => changes.push(format!("xray reloaded (SIGHUP, pid {pid})")),
                        Err(e) => errors.push(format!("xray reload failed (pid {pid}): {e}")),
                    }
                }
            }
        } else if pids.is_empty() {
            changes.push("xray already stopped".to_string());
        } else {
            for pid in pids {
                match nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM) {
                    Ok(()) => changes.push(format!("xray stopped (pid {pid})")),
                    Err(e) => errors.push(format!("xray stop failed (pid {pid}): {e}")),
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

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        Err(anyhow::anyhow!(
            "Checkpoints not supported by xray schema plugin"
        ))
    }

    async fn rollback(&self, _checkpoint: &op_state::Checkpoint) -> Result<()> {
        Err(anyhow::anyhow!(
            "Rollbacks not supported by xray schema plugin"
        ))
    }
}

/// Derived `xray` schema from the typed [`XrayState`] struct via schemars.
pub(crate) fn xray_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(XrayState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "xray",
        "1.0.0",
        "Xray proxy state and execution schema",
        &root,
    );
    let state = simd_json::serde::to_owned_value(&XrayState::default())
        .expect("XrayState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);

    use super::plugin_scaffold_helpers::EmptyInput;
    use op_state_store::SideEffect;

    // Backed by a real dispatcher (op-grpc-bridge's dispatch_xray_method):
    // xray-core StatsService.GetStats over the commander API socket.
    schema.methods.insert(
        "get_stats".to_string(),
        method_decl_from_schemars_with_output::<StatsInput, GetStatsOutput>(
            "get_stats",
            SideEffect::Read,
            true,
            "xray.read",
            "obs.software.plugin.xray.stats.get@v1",
        ),
    );
    // Backed by a real dispatcher: SIGHUP reload of the running xray process
    // (never a hard kill+respawn; the container's own supervisor owns that).
    schema.methods.insert(
        "restart".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, RestartOutput>(
            "restart",
            SideEffect::Mutation,
            false,
            "xray.write",
            "mut.software.plugin.xray.daemon.restart@v1",
        ),
    );
    // Declared for UI/gRPC discovery; dispatch_xray_method currently answers
    // these with "schema-declared but not yet implemented" until the
    // xray-core UsersService / routing-config write path is wired up.
    schema.methods.insert(
        "add_user".to_string(),
        method_decl_from_schemars::<UserInput>(
            "add_user",
            SideEffect::Mutation,
            false,
            "xray.write",
            "mut.software.plugin.xray.user.add@v1",
        ),
    );
    schema.methods.insert(
        "remove_user".to_string(),
        method_decl_from_schemars::<UserInput>(
            "remove_user",
            SideEffect::Mutation,
            false,
            "xray.write",
            "mut.software.plugin.xray.user.remove@v1",
        ),
    );
    schema.methods.insert(
        "add_inbound".to_string(),
        method_decl_from_schemars::<InboundInput>(
            "add_inbound",
            SideEffect::Mutation,
            false,
            "xray.write",
            "mut.software.plugin.xray.inbound.add@v1",
        ),
    );
    schema.methods.insert(
        "start_trace".to_string(),
        method_decl_from_schemars::<StartTraceInput>(
            "start_trace",
            SideEffect::Mutation,
            false,
            "xray.write",
            "mut.software.plugin.xray.trace.start@v1",
        ),
    );
    schema.methods.insert(
        "end_trace".to_string(),
        method_decl_from_schemars::<EndTraceInput>(
            "end_trace",
            SideEffect::Mutation,
            false,
            "xray.write",
            "mut.software.plugin.xray.trace.end@v1",
        ),
    );
    schema.methods.insert(
        "record_span".to_string(),
        method_decl_from_schemars::<RecordSpanInput>(
            "record_span",
            SideEffect::Mutation,
            false,
            "xray.write",
            "mut.software.plugin.xray.span.record@v1",
        ),
    );

    schema.capabilities.insert(
        "xray.read".to_string(),
        CapabilityDecl {
            id: "xray.read".to_string(),
            description: "Grants: get_stats.".to_string(),
        },
    );
    schema.capabilities.insert(
        "xray.write".to_string(),
        CapabilityDecl {
            id: "xray.write".to_string(),
            description: "Grants: restart, add_user, remove_user, add_inbound, start_trace, end_trace, record_span.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use crate::state_plugins::schemars_adapter::schema_diffs;
    use serde_json::Value as JVal;

    fn collect_subids(value: &JVal, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(JVal::String(subid)) = obj.get("x-oscal-subid") {
                out.push(subid.clone());
            }
            for v in obj.values() {
                collect_subids(v, out);
            }
        }
        if let Some(arr) = value.as_array() {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn all_subids_are_valid() {
        let root = serde_json::to_value(schemars::schema_for!(XrayState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}

/// Output for `get_stats`: a single xray-core StatsService counter.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetStatsOutput {
    pub name: String,
    pub value: i64,
}

/// Output for `restart`: PIDs that received the reload signal.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RestartOutput {
    pub reloaded_pids: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserInput {
    pub email: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InboundInput {
    pub tag: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StatsInput {
    pub name: String,
}

/// Input struct for StartTrace method.
/// See: https://xtls.github.io/config/
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StartTraceInput {
    /// Trace ID
    pub trace_id: String,
    /// Service name
    pub service: String,
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Input struct for EndTrace method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EndTraceInput {
    /// Trace ID to end
    pub trace_id: String,
    /// Final status
    pub status: Option<String>,
}

/// Input struct for RecordSpan method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecordSpanInput {
    /// Trace ID
    pub trace_id: String,
    /// Span name
    pub span_name: String,
    /// Start timestamp (unix epoch millis)
    pub start_ms: i64,
    /// End timestamp (unix epoch millis)
    pub end_ms: i64,
    /// Span metadata
    pub metadata: Option<serde_json::Value>,
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("xray", |_ctx| std::sync::Arc::new(XrayPlugin::new(XrayConfig::default())))
}
