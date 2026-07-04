use schemars::JsonSchema;
// dnsresolver_plugin.rs
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::fs;

use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DnsState {
    /// DNS plugin state schema.
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.dnsresolver.schema@v1"))]
    pub version: u32,
    #[schemars(extend("x-oscal-subid" = "obs.software.dnsresolver.state.items@v1"))]
    pub items: Vec<DnsItem>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[schemars(extend("x-oscal-subid" = "obs.software.dnsresolver.mode.enforce@v1"))]
    Enforce,
    #[schemars(extend("x-oscal-subid" = "obs.software.dnsresolver.mode.observe-only@v1"))]
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DnsItem {
    /// DNS item identifier.
    #[schemars(extend("x-oscal-subid" = "exp.software.dnsresolver.item.id@v1"))]
    pub id: String,
    /// DNS mode.
    #[schemars(extend("x-oscal-subid" = "obs.software.dnsresolver.item.mode@v1"))]
    pub mode: Mode,
    /// DNS server list.
    #[schemars(extend("x-oscal-subid" = "mut.software.dnsresolver.item.servers@v1"))]
    pub servers: Vec<String>,
    /// DNS search domains.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "obs.software.dnsresolver.item.search@v1"))]
    pub search: Option<Vec<String>>,
    /// DNS options.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "obs.software.dnsresolver.item.options@v1"))]
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
        fs::rename(tmp_path, "/etc/resolv.conf").context("rename resolv.conf")?;
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

    fn schema(&self) -> Option<PluginSchema> {
        Some(dnsresolver_schema())
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
                    simd_json::to_string(&simd_json::json!({"items": cur_all})).unwrap_or_default()
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveHostnameInput {
    pub ifindex: i32,
    pub name: String,
    pub family: i32,
    pub flags: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveAddressInput {
    pub ifindex: i32,
    pub family: i32,
    pub address: Vec<u8>,
    pub flags: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveRecordInput {
    pub ifindex: i32,
    pub name: String,
    pub class: u16,
    pub record_type: u16,
    pub flags: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveServiceInput {
    pub ifindex: i32,
    pub name: String,
    pub family: i32,
    pub flags: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetLinkInput {
    pub ifindex: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkDNSInput {
    pub ifindex: i32,
    pub addresses: Vec<(i32, Vec<u8>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkDNSExInput {
    pub ifindex: i32,
    pub addresses: Vec<(i32, Vec<u8>, u16, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkDomainsInput {
    pub ifindex: i32,
    pub domains: Vec<(String, bool)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkDefaultRouteInput {
    pub ifindex: i32,
    pub enable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkLLMNRInput {
    pub ifindex: i32,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkMulticastDNSInput {
    pub ifindex: i32,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkDNSOverTLSInput {
    pub ifindex: i32,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkDNSSECInput {
    pub ifindex: i32,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetLinkDNSSECNegativeTrustAnchorsInput {
    pub ifindex: i32,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RevertLinkInput {
    pub ifindex: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RegisterServiceInput {
    pub id: String,
    pub name_template: String,
    pub type_: String,
    pub port: u16,
    pub priority: u16,
    pub weight: u16,
    pub txt: Vec<std::collections::BTreeMap<String, Vec<u8>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UnregisterServiceInput {
    pub service_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResetStatisticsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FlushCachesInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResetServerFeaturesInput {}

pub(crate) fn dnsresolver_schema() -> PluginSchema {
    let mut schema = PluginSchema::builder("dnsresolver")
        .version("1.0.0")
        .description("DNS resolver declaration state")
        .field(
            "version",
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Schema version".to_string(),
                default: Some(simd_json::json!(1)),
                example: None,
                constraints: vec![Constraint::Min { value: 1.0 }],
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "items",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Any)),
                required: true,
                description: "Resolver items".to_string(),
                default: Some(simd_json::json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .build();

    // org.freedesktop.resolve1.Manager methods
    // Spec: https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.resolve1.html
    schema.methods.insert(
        "resolve_hostname".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ResolveHostnameInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ResolveHostname",
            op_state_store::SideEffect::Read,
            true,
            "dnsresolver.read",
            "obs.software.dnsresolver.hostname.resolve@v1",
        ),
    );
    schema.methods.insert(
        "resolve_address".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ResolveAddressInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ResolveAddress",
            op_state_store::SideEffect::Read,
            true,
            "dnsresolver.read",
            "obs.software.dnsresolver.address.resolve@v1",
        ),
    );
    schema.methods.insert(
        "resolve_record".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ResolveRecordInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ResolveRecord",
            op_state_store::SideEffect::Read,
            true,
            "dnsresolver.read",
            "obs.software.dnsresolver.record.resolve@v1",
        ),
    );
    schema.methods.insert(
        "resolve_service".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ResolveServiceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ResolveService",
            op_state_store::SideEffect::Read,
            true,
            "dnsresolver.read",
            "obs.software.dnsresolver.service.resolve@v1",
        ),
    );
    schema.methods.insert(
        "get_link".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            GetLinkInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetLink",
            op_state_store::SideEffect::Read,
            true,
            "dnsresolver.read",
            "obs.software.dnsresolver.link.get@v1",
        ),
    );
    schema.methods.insert(
        "set_link_dns".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkDNSInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkDNS",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-dns@v1",
        ),
    );
    schema.methods.insert(
        "set_link_dns_ex".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkDNSExInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkDNSEx",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-dns-ex@v1",
        ),
    );
    schema.methods.insert(
        "set_link_domains".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkDomainsInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkDomains",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-domains@v1",
        ),
    );
    schema.methods.insert(
        "set_link_default_route".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkDefaultRouteInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkDefaultRoute",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-default-route@v1",
        ),
    );
    schema.methods.insert(
        "set_link_llmnr".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkLLMNRInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkLLMNR",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-llmnr@v1",
        ),
    );
    schema.methods.insert(
        "set_link_multicast_dns".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkMulticastDNSInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkMulticastDNS",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-mdns@v1",
        ),
    );
    schema.methods.insert(
        "set_link_dns_over_tls".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkDNSOverTLSInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkDNSOverTLS",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-dot@v1",
        ),
    );
    schema.methods.insert(
        "set_link_dnssec".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkDNSSECInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkDNSSEC",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-dnssec@v1",
        ),
    );
    schema.methods.insert(
        "set_link_dnssec_nta".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SetLinkDNSSECNegativeTrustAnchorsInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "SetLinkDNSSECNegativeTrustAnchors",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.set-dnssec-nta@v1",
        ),
    );
    schema.methods.insert(
        "revert_link".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            RevertLinkInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "RevertLink",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.link.revert@v1",
        ),
    );
    schema.methods.insert(
        "register_service".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            RegisterServiceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "RegisterService",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.service.register@v1",
        ),
    );
    schema.methods.insert(
        "unregister_service".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            UnregisterServiceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "UnregisterService",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.service.unregister@v1",
        ),
    );
    schema.methods.insert(
        "reset_statistics".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ResetStatisticsInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ResetStatistics",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.statistics.reset@v1",
        ),
    );
    schema.methods.insert(
        "flush_caches".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            FlushCachesInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "FlushCaches",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.caches.flush@v1",
        ),
    );
    schema.methods.insert(
        "reset_server_features".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ResetServerFeaturesInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ResetServerFeatures",
            op_state_store::SideEffect::Mutation,
            false,
            "dnsresolver.write",
            "mut.software.dnsresolver.server-features.reset@v1",
        ),
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("dnsresolver", |_ctx| std::sync::Arc::new(DnsResolverPlugin::new()))
}
