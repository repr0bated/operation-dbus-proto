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
