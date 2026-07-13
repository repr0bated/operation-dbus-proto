use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

use super::plugin_scaffold_helpers::{
    method_decl_from_schemars, method_decl_from_schemars_with_output, AckOutput, EmptyInput,
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
}

impl Default for XrayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            socket_port: "gbr_xray".to_string(),
            config_path: "/etc/xray/config.json".to_string(),
        }
    }
}

// ── Typed Xray core configuration (matches xtls.github.io/en/config/) ───────────

/// Full Xray-core configuration object. This mirrors the top-level JSON structure
/// documented at https://xtls.github.io/en/config/ and lets the plugin validate,
/// store, and emit an Xray config instead of only pointing at an opaque file path.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.schema@v1"))]
pub struct XrayCoreConfig {
    /// Log configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.log@v1"))]
    pub log: Option<XrayLogConfig>,
    /// Built-in DNS server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.dns@v1"))]
    pub dns: Option<XrayDnsConfig>,
    /// Inbound listeners.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.inbounds@v1"))]
    pub inbounds: Vec<XrayInbound>,
    /// Outbound exits.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.outbounds@v1"))]
    pub outbounds: Vec<XrayOutbound>,
    /// Routing rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.routing@v1"))]
    pub routing: Option<XrayRouting>,
    /// Per-user connection and buffer policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.policy@v1"))]
    pub policy: Option<XrayPolicy>,
    /// Traffic statistics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.stats@v1"))]
    pub stats: Option<XrayStats>,
    /// Xray-core version compatibility range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core.version@v1"))]
    pub version: Option<XrayVersion>,
}

impl Default for XrayCoreConfig {
    fn default() -> Self {
        Self {
            log: Some(XrayLogConfig::default()),
            dns: None,
            inbounds: Vec::new(),
            outbounds: vec![XrayOutbound::direct()],
            routing: Some(XrayRouting::default()),
            policy: None,
            stats: None,
            version: None,
        }
    }
}

impl XrayCoreConfig {
    /// Validate the core config and return a list of errors. Empty list means valid.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.inbounds.is_empty() {
            errors.push("xray config requires at least one inbound".to_string());
        }
        if self.outbounds.is_empty() {
            errors.push("xray config requires at least one outbound".to_string());
        }

        let mut tags: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (idx, inbound) in self.inbounds.iter().enumerate() {
            if inbound.tag.is_empty() {
                errors.push(format!("inbound[{idx}]: missing tag"));
            }
            if inbound.protocol.is_empty() {
                errors.push(format!("inbound[{}]: missing protocol", inbound.tag));
            }
            if !tags.insert(inbound.tag.clone()) {
                errors.push(format!("duplicate tag: {}", inbound.tag));
            }
            if let Some(reason) = xhttp_only_bind_violation(inbound) {
                errors.push(format!("inbound[{}]: {reason}", inbound.tag));
            }
        }
        for (idx, outbound) in self.outbounds.iter().enumerate() {
            if outbound.tag.is_empty() {
                errors.push(format!("outbound[{idx}]: missing tag"));
            }
            if outbound.protocol.is_empty() {
                errors.push(format!("outbound[{}]: missing protocol", outbound.tag));
            }
            if !tags.insert(outbound.tag.clone()) {
                errors.push(format!("duplicate tag: {}", outbound.tag));
            }
        }

        if let Some(routing) = &self.routing {
            for (idx, rule) in routing.rules.iter().enumerate() {
                if rule.outbound_tag.is_empty() {
                    errors.push(format!("routing.rule[{idx}]: missing outboundTag"));
                } else if !tags.contains(&rule.outbound_tag) {
                    errors.push(format!(
                        "routing.rule[{idx}]: outboundTag '{}' does not exist",
                        rule.outbound_tag
                    ));
                }
            }
        }

        errors
    }
}

/// Listen addresses that carry identity injection (the WG peer pubkey stamped
/// as an `xhttp` custom header — see `[[project_oracle_decoy_relay_asbuilt]]`
/// / WISHLIST OD-32) and therefore MUST use `xhttp` transport: xray's native
/// `grpc` transport has no header-injection field, but `xhttpSettings.extra.headers`
/// does. Any inbound bound to one of these addresses that isn't `xhttp` is a
/// schema violation, not just a misconfiguration — it silently drops identity.
const XHTTP_ONLY_BIND_ADDRESSES: &[&str] = &["10.0.0.1", "10.0.0.2"];

fn xhttp_only_bind_violation(inbound: &XrayInbound) -> Option<String> {
    let listen = inbound.listen.as_deref()?;
    if !XHTTP_ONLY_BIND_ADDRESSES.contains(&listen) {
        return None;
    }
    let network = inbound
        .stream_settings
        .as_ref()
        .and_then(|s| s.get("network"))
        .and_then(|v| v.as_str());
    match network {
        Some("xhttp") => None,
        Some(other) => Some(format!(
            "listens on {listen}, which is identity-injection-only and requires \
             streamSettings.network = \"xhttp\" (found \"{other}\")"
        )),
        None => Some(format!(
            "listens on {listen}, which is identity-injection-only and requires \
             streamSettings.network = \"xhttp\" (network not set)"
        )),
    }
}

/// Xray log configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.log@v1"))]
pub struct XrayLogConfig {
    /// Log level: debug, info, warning, error, none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loglevel: Option<String>,
    /// Access log path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    /// Error log path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Xray DNS configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.dns@v1"))]
pub struct XrayDnsConfig {
    /// DNS server addresses.
    #[serde(default)]
    pub servers: Vec<String>,
    /// Tag for this DNS object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

/// One Xray inbound listener.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.inbound@v1"))]
pub struct XrayInbound {
    /// Tag for this inbound (must be unique).
    pub tag: String,
    /// Port number.
    pub port: u16,
    /// Listen address (defaults to 0.0.0.0 if omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    /// Protocol: vless, vmess, trojan, shadowsocks, etc.
    pub protocol: String,
    /// Protocol-specific settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    /// Stream / transport settings.
    #[serde(
        default,
        rename = "streamSettings",
        skip_serializing_if = "Option::is_none"
    )]
    pub stream_settings: Option<serde_json::Value>,
    /// Sniffing settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sniffing: Option<serde_json::Value>,
}

/// One Xray outbound exit.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.outbound@v1"))]
pub struct XrayOutbound {
    /// Tag for this outbound (must be unique).
    pub tag: String,
    /// Protocol: freedom, blackhole, dns, vless, vmess, trojan, etc.
    pub protocol: String,
    /// Protocol-specific settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    /// Stream / transport settings.
    #[serde(
        default,
        rename = "streamSettings",
        skip_serializing_if = "Option::is_none"
    )]
    pub stream_settings: Option<serde_json::Value>,
    /// Mux / proxy settings.
    #[serde(
        default,
        rename = "proxySettings",
        skip_serializing_if = "Option::is_none"
    )]
    pub proxy_settings: Option<serde_json::Value>,
}

impl XrayOutbound {
    fn direct() -> Self {
        Self {
            tag: "direct".to_string(),
            protocol: "freedom".to_string(),
            settings: None,
            stream_settings: None,
            proxy_settings: None,
        }
    }
}

/// Xray routing configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.routing@v1"))]
pub struct XrayRouting {
    /// Domain resolution strategy: AsIs, IPIfNonMatch, IPOnDemand.
    #[serde(
        default,
        rename = "domainStrategy",
        skip_serializing_if = "Option::is_none"
    )]
    pub domain_strategy: Option<String>,
    /// Domain matcher: hybrid, linear, mph.
    #[serde(
        default,
        rename = "domainMatcher",
        skip_serializing_if = "Option::is_none"
    )]
    pub domain_matcher: Option<String>,
    /// Routing rules.
    #[serde(default)]
    pub rules: Vec<XrayRule>,
}

/// One Xray routing rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.rule@v1"))]
pub struct XrayRule {
    /// Rule type: field.
    #[serde(rename = "type")]
    pub rule_type: String,
    /// Inbound tags this rule matches.
    #[serde(
        default,
        rename = "inboundTag",
        skip_serializing_if = "Option::is_none"
    )]
    pub inbound_tag: Option<Vec<String>>,
    /// Outbound tag to send matching traffic to.
    #[serde(rename = "outboundTag")]
    pub outbound_tag: String,
    /// Domain matchers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    /// IP matchers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<Vec<String>>,
    /// Port matcher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
}

/// Xray policy configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.policy@v1"))]
pub struct XrayPolicy {
    /// Per-user level policies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<serde_json::Value>,
    /// Global system policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<serde_json::Value>,
}

/// Xray statistics configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.stats@v1"))]
pub struct XrayStats {
    /// Placeholder for stats object fields (mostly boolean toggles).
    #[serde(flatten, default)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Xray-core version compatibility range.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.version@v1"))]
pub struct XrayVersion {
    /// Minimum acceptable Xray-core version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<String>,
    /// Maximum acceptable Xray-core version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
}

// ── Method inputs / outputs ────────────────────────────────────────────────────

/// Input for setting the full Xray core configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct XrayConfigInput {
    pub config: XrayCoreConfig,
}

/// Output for reading the full Xray core configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct XrayConfigOutput {
    pub config: XrayCoreConfig,
}

/// Output for validating the Xray core configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct XrayValidationOutput {
    /// Whether the config is valid.
    pub valid: bool,
    /// Validation errors. Empty when valid.
    pub errors: Vec<String>,
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
    /// Full Xray-core configuration (inbounds, outbounds, routing, etc.).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.xray.core-config@v1"))]
    pub core_config: XrayCoreConfig,
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
            core_config: XrayCoreConfig::default(),
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

/// Is an xray process currently running on the host? (`pgrep -x xray`)
fn xray_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "xray"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
            core_config: XrayCoreConfig::default(),
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
        if self.config.enabled {
            // Reload (SIGHUP) so a running xray re-reads its config; if none is
            // running, the s6 supervisor (gbr-xray) is responsible for starting it.
            match std::process::Command::new("pkill")
                .args(["-HUP", "-x", "xray"])
                .status()
            {
                Ok(s) if s.success() => changes.push("xray reloaded (SIGHUP)".to_string()),
                Ok(_) => changes.push("xray not running; supervisor starts it".to_string()),
                Err(e) => errors.push(format!("xray reload failed: {e}")),
            }
        } else {
            match std::process::Command::new("pkill")
                .args(["-x", "xray"])
                .status()
            {
                Ok(_) => changes.push("xray stopped".to_string()),
                Err(e) => errors.push(format!("xray stop failed: {e}")),
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
    use op_state_store::{Constraint, FieldSchema, FieldType};
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
            default: Some(json!("/etc/xray/config.json")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
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
                "config_path": "/etc/xray/config.json",
                "enabled": true,
                "socket_port": "gbr_xray"
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    // ── core_config: XrayCoreConfig ──────────────────────────────────────────
    let mut log_fields = std::collections::HashMap::new();
    log_fields.insert(
        "loglevel".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Log level: debug, info, warning, error, none.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    log_fields.insert(
        "access".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Access log path.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    log_fields.insert(
        "error".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Error log path.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut dns_fields = std::collections::HashMap::new();
    dns_fields.insert(
        "servers".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "DNS server addresses.".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    dns_fields.insert(
        "tag".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Tag for this DNS object.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut inbound_fields = std::collections::HashMap::new();
    inbound_fields.insert(
        "tag".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Tag for this inbound (must be unique).".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    inbound_fields.insert(
        "port".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Port number.".to_string(),
            default: None,
            example: None,
            constraints: vec![
                Constraint::Min { value: 0.0 },
                Constraint::Max { value: 65535.0 },
            ],
            read_only: false,
            read_only_when: None,
        },
    );
    inbound_fields.insert(
        "listen".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Listen address (defaults to 0.0.0.0 if omitted).".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    inbound_fields.insert(
        "protocol".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Protocol: vless, vmess, trojan, shadowsocks, etc.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    inbound_fields.insert(
        "settings".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Protocol-specific settings.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    inbound_fields.insert(
        "streamSettings".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Stream / transport settings.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    inbound_fields.insert(
        "sniffing".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Sniffing settings.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut outbound_fields = std::collections::HashMap::new();
    outbound_fields.insert(
        "tag".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Tag for this outbound (must be unique).".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    outbound_fields.insert(
        "protocol".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Protocol: freedom, blackhole, dns, vless, vmess, trojan, etc."
                .to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    outbound_fields.insert(
        "settings".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Protocol-specific settings.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    outbound_fields.insert(
        "streamSettings".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Stream / transport settings.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    outbound_fields.insert(
        "proxySettings".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Mux / proxy settings.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut rule_fields = std::collections::HashMap::new();
    rule_fields.insert(
        "type".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Rule type: field.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    rule_fields.insert(
        "inboundTag".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Inbound tags this rule matches.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    rule_fields.insert(
        "outboundTag".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Outbound tag to send matching traffic to.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    rule_fields.insert(
        "domain".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Domain matchers.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    rule_fields.insert(
        "ip".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "IP matchers.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    rule_fields.insert(
        "port".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Port matcher.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut routing_fields = std::collections::HashMap::new();
    routing_fields.insert(
        "domainStrategy".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Domain resolution strategy: AsIs, IPIfNonMatch, IPOnDemand.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    routing_fields.insert(
        "domainMatcher".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Domain matcher: hybrid, linear, mph.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    routing_fields.insert(
        "rules".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::Object(rule_fields))),
            required: false,
            description: "Routing rules.".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut policy_fields = std::collections::HashMap::new();
    policy_fields.insert(
        "levels".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Per-user level policies.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    policy_fields.insert(
        "system".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Global system policy.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let stats_fields: std::collections::HashMap<String, FieldSchema> =
        std::collections::HashMap::new();

    let mut version_fields = std::collections::HashMap::new();
    version_fields.insert(
        "min".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Minimum acceptable Xray-core version.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    version_fields.insert(
        "max".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Maximum acceptable Xray-core version.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut core_config_fields = std::collections::HashMap::new();
    core_config_fields.insert(
        "log".to_string(),
        FieldSchema {
            field_type: FieldType::Object(log_fields),
            required: false,
            description: "Log configuration.".to_string(),
            default: Some(json!({})),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    core_config_fields.insert(
        "dns".to_string(),
        FieldSchema {
            field_type: FieldType::Object(dns_fields),
            required: false,
            description: "Built-in DNS server.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    core_config_fields.insert(
        "inbounds".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::Object(inbound_fields))),
            required: false,
            description: "Inbound listeners.".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    core_config_fields.insert(
        "outbounds".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::Object(outbound_fields))),
            required: false,
            description: "Outbound exits.".to_string(),
            default: Some(json!([{"tag": "direct", "protocol": "freedom"}])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    core_config_fields.insert(
        "routing".to_string(),
        FieldSchema {
            field_type: FieldType::Object(routing_fields),
            required: false,
            description: "Routing rules.".to_string(),
            default: Some(json!({"rules": []})),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    core_config_fields.insert(
        "policy".to_string(),
        FieldSchema {
            field_type: FieldType::Object(policy_fields),
            required: false,
            description: "Per-user connection and buffer policy.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    core_config_fields.insert(
        "stats".to_string(),
        FieldSchema {
            field_type: FieldType::Object(stats_fields),
            required: false,
            description: "Traffic statistics.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    core_config_fields.insert(
        "version".to_string(),
        FieldSchema {
            field_type: FieldType::Object(version_fields),
            required: false,
            description: "Xray-core version compatibility range.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    fields.insert(
        "core_config".to_string(),
        FieldSchema {
            field_type: FieldType::Object(core_config_fields),
            required: false,
            description: "Full Xray-core configuration (inbounds, outbounds, routing, etc.)."
                .to_string(),
            default: Some(json!({
                "log": {},
                "inbounds": [],
                "outbounds": [{"tag": "direct", "protocol": "freedom"}],
                "routing": {"rules": []}
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
            "core_config".to_string(),
            "sch.software.plugin.xray.core-config@v1".to_string(),
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

    // ── Config management methods (typed input/output) ────────────────────────
    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, XrayConfigOutput>(
            "GetConfig",
            op_state_store::SideEffect::Read,
            true,
            "xray.read",
            "obs.network.xray.config.get@v1",
        ),
    );
    schema.methods.insert(
        "set_config".to_string(),
        method_decl_from_schemars_with_output::<XrayConfigInput, AckOutput>(
            "SetConfig",
            op_state_store::SideEffect::Mutation,
            false,
            "xray.write",
            "mut.network.xray.config.set@v1",
        ),
    );
    schema.methods.insert(
        "validate_config".to_string(),
        method_decl_from_schemars_with_output::<XrayConfigInput, XrayValidationOutput>(
            "ValidateConfig",
            op_state_store::SideEffect::Read,
            true,
            "xray.read",
            "obs.network.xray.config.validate@v1",
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
