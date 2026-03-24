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
