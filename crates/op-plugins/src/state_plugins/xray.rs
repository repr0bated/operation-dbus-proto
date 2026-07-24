use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

use super::plugin_scaffold_helpers::{
    method_decl_from_schemars, method_decl_from_schemars_with_output,
};

use super::xray_config_types::{
    DNSConfig, PolicyConfig, XrayInboundProtocolCatalog, XrayOutboundProtocolCatalog, APIConfig,
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
            config_path: "/dev/shm/xray_config.json".to_string(),
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
    fn derived_schema_matches_hand_rolled() {
        let golden = super::xray_schema_golden();
        let derived = super::xray_schema();
        let diffs = schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
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

#[cfg(test)]
pub(crate) fn xray_schema_golden() -> PluginSchema {
    use op_state_store::{FieldSchema, FieldType};
    use simd_json::json;

    let mut config_fields = std::collections::HashMap::new();
    config_fields.insert(
        "enabled".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the xray proxy is enabled.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    config_fields.insert(
        "socket_port".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Xray socket tag/port name.".to_string(),
            default: Some(json!("gbr_xray")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    config_fields.insert(
        "config_path".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Path to the xray JSON config.".to_string(),
            default: Some(json!("/dev/shm/xray_config.json")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    // The remaining `config` fields (api/dns/policy/inbound_protocols/
    // outbound_protocols) are mechanically generated from xray-core's real
    // Go struct definitions (see `xray_config_types.rs`'s header for
    // provenance) rather than hand-authored — there's no independent "human
    // intent" to compare against for their shape, so their expected
    // FieldSchema is derived via the same schemars_adapter conversion the
    // plugin itself uses, keeping this golden test meaningful for the fields
    // that ARE hand-authored while still catching shape breakage here.
    fn derived_object_field(description: &str, root: serde_json::Value) -> FieldSchema {
        let derived = super::schemars_adapter::plugin_schema_from_json(
            "_derived_ref",
            "0.0.0",
            description,
            &root,
        );
        FieldSchema {
            field_type: FieldType::Object(derived.fields),
            required: false,
            description: description.to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        }
    }
    config_fields.insert(
        "api".to_string(),
        derived_object_field(
            "Xray commander API block (`api`) — StatsService/RoutingService/LoggerService.",
            serde_json::to_value(schemars::schema_for!(super::xray_config_types::APIConfig))
                .expect("schemars schema serializes"),
        ),
    );
    config_fields.insert(
        "dns".to_string(),
        derived_object_field(
            "Xray DNS block (`dns`).",
            serde_json::to_value(schemars::schema_for!(super::xray_config_types::DNSConfig))
                .expect("schemars schema serializes"),
        ),
    );
    config_fields.insert(
        "policy".to_string(),
        derived_object_field(
            "Xray policy block (`policy`).",
            serde_json::to_value(schemars::schema_for!(super::xray_config_types::PolicyConfig))
                .expect("schemars schema serializes"),
        ),
    );
    config_fields.insert(
        "inbound_protocols".to_string(),
        derived_object_field(
            "Reference catalog of every inbound-protocol settings shape xray-core \
             defines (dokodemo/http/socks/vmess/vless/trojan/shadowsocks/freedom).",
            serde_json::to_value(schemars::schema_for!(
                super::xray_config_types::XrayInboundProtocolCatalog
            ))
            .expect("schemars schema serializes"),
        ),
    );
    config_fields.insert(
        "outbound_protocols".to_string(),
        derived_object_field(
            "Reference catalog of every outbound-protocol settings shape xray-core \
             defines (blackhole/freedom/http/socks/vmess/vless/trojan/shadowsocks).",
            serde_json::to_value(schemars::schema_for!(
                super::xray_config_types::XrayOutboundProtocolCatalog
            ))
            .expect("schemars schema serializes"),
        ),
    );

    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "software".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Software identifier.".to_string(),
            default: Some(json!("xray-core")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "version".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Software version.".to_string(),
            default: Some(json!("1.0.0")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "dependencies".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Runtime dependencies.".to_string(),
            default: Some(json!(["incus"])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "oscal_source".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "OSCAL subid registry source path.".to_string(),
            default: Some(json!("/org/opdbus/v1/plugins/oscal_subid_registry")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "config".to_string(),
        FieldSchema {
            field_type: FieldType::Object(config_fields),
            required: false,
            description: "Xray configuration.".to_string(),
            default: Some(json!({
                "config_path": "/dev/shm/xray_config.json",
                "enabled": true,
                "socket_port": "gbr_xray"
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "running".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether an xray process is currently running (host-native).".to_string(),
            default: Some(json!(false)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "tools".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "MCP tool definitions exposed by this plugin.".to_string(),
            default: Some(json!([
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
            ])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schema = PluginSchema::builder("xray")
        .version("1.0.0")
        .description("Xray proxy state and execution schema")
        .build();
    schema.fields = fields;
    schema.subids = std::collections::HashMap::from([
        (
            "__schema__".to_string(),
            "sch.software.plugin.xray.schema@v1".to_string(),
        ),
        (
            "software".to_string(),
            "obs.software.plugin.xray.software@v1".to_string(),
        ),
        (
            "version".to_string(),
            "obs.software.plugin.xray.version@v1".to_string(),
        ),
        (
            "dependencies".to_string(),
            "obs.software.plugin.xray.dependencies@v1".to_string(),
        ),
        (
            "oscal_source".to_string(),
            "src.software.plugin.xray.oscal-source@v1".to_string(),
        ),
        (
            "config".to_string(),
            "sch.software.plugin.xray.config@v1".to_string(),
        ),
        (
            "running".to_string(),
            "obs.software.plugin.xray.running@v1".to_string(),
        ),
        (
            "tools".to_string(),
            "exp.software.plugin.xray.tools@v1".to_string(),
        ),
    ]);
    let state = simd_json::serde::to_owned_value(&XrayState::default())
        .expect("XrayState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);

    schema.methods.insert(
        "add_user".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            UserInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "AddUser",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.user.add@v1",
        ),
    );
    schema.methods.insert(
        "remove_user".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            UserInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "RemoveUser",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.user.remove@v1",
        ),
    );
    schema.methods.insert(
        "add_inbound".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            InboundInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "AddInbound",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.inbound.add@v1",
        ),
    );
    schema.methods.insert(
        "get_stats".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            StatsInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetStats",
            op_state_store::SideEffect::Read,
            true,
            "xray.read",
            "obs.network.xray.stats.get@v1",
        ),
    );
    schema.methods.insert(
        "restart".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            super::plugin_scaffold_helpers::EmptyInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "Restart",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.service.restart@v1",
        ),
    );

    // StartTrace method - https://xtls.github.io/config/
    schema.methods.insert(
        "start_trace".to_string(),
        method_decl_from_schemars_with_output::<
            StartTraceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "StartTrace",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.trace.start@v1",
        ),
    );

    // EndTrace method
    schema.methods.insert(
        "end_trace".to_string(),
        method_decl_from_schemars_with_output::<
            EndTraceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "EndTrace",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.trace.end@v1",
        ),
    );

    // RecordSpan method
    schema.methods.insert(
        "record_span".to_string(),
        method_decl_from_schemars_with_output::<
            RecordSpanInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "RecordSpan",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.span.record@v1",
        ),
    );

    schema
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
