use super::plugin_schema_defs::{any_field, simple_schema};
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareState {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CpuInfo {
    pub model: String,
    pub cores: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryInfo {
    pub total_kb: u64,
    pub available_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub size_bytes: u64,
    pub mountpoint: Option<String>,
}

pub struct HardwarePlugin;

impl Default for HardwarePlugin {
    fn default() -> Self {
        Self
    }
}

impl HardwarePlugin {
    pub fn new() -> Self {
        Self
    }

    async fn get_cpu_info() -> CpuInfo {
        let content = tokio::fs::read_to_string("/proc/cpuinfo")
            .await
            .unwrap_or_default();
        let mut model = "Unknown".to_string();
        let mut cores = 0;

        for line in content.lines() {
            if line.starts_with("model name") {
                if let Some(val) = line.split(':').nth(1) {
                    if model == "Unknown" {
                        model = val.trim().to_string();
                    }
                }
                cores += 1;
            }
        }

        // Fallback for cores if using processor count
        if cores == 0 {
            cores = content
                .lines()
                .filter(|l| l.starts_with("processor"))
                .count();
        }

        CpuInfo { model, cores }
    }

    async fn get_memory_info() -> MemoryInfo {
        let content = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .unwrap_or_default();
        let mut total = 0;
        let mut available = 0;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    total = val.parse().unwrap_or(0);
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    available = val.parse().unwrap_or(0);
                }
            }
        }

        MemoryInfo {
            total_kb: total,
            available_kb: available,
        }
    }

    async fn get_disk_info() -> Vec<DiskInfo> {
        let mut disks = Vec::new();
        let sysfs = std::path::Path::new("/sys/block");
        if !sysfs.exists() {
            return disks;
        }

        let mut entries = match tokio::fs::read_dir(sysfs).await {
            Ok(e) => e,
            Err(_) => return disks,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip loop devices and ramdisks
            if name.starts_with("loop") || name.starts_with("ram") {
                continue;
            }

            let size_path = entry.path().join("size");
            let size_sectors = tokio::fs::read_to_string(&size_path)
                .await
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            let size_bytes = size_sectors * 512;

            let mountpoint = Self::read_mountpoint(&name).await;

            disks.push(DiskInfo {
                name,
                size_bytes,
                mountpoint,
            });
        }
        disks
    }

    async fn read_mountpoint(device: &str) -> Option<String> {
        let mounts = tokio::fs::read_to_string("/proc/mounts").await.ok()?;
        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let dev = parts[0];
                if dev.contains(device) {
                    return Some(parts[1].to_string());
                }
            }
        }
        None
    }
}

#[async_trait]
impl StatePlugin for HardwarePlugin {
    fn name(&self) -> &str {
        "hardware"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(hardware_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let cpu = Self::get_cpu_info().await;
        let memory = Self::get_memory_info().await;
        let disks = Self::get_disk_info().await;

        Ok(simd_json::serde::to_owned_value(HardwareState {
            cpu,
            memory,
            disks,
        })?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: Value::null(),
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

pub(crate) fn hardware_schema() -> PluginSchema {
    simple_schema(
        "hardware",
        "Hardware inventory snapshot",
        &[],
        vec![
            ("cpu", any_field(true, "CPU info", Some(json!({})))),
            ("memory", any_field(true, "Memory info", Some(json!({})))),
            ("disks", any_field(true, "Disk list", Some(json!([])))),
        ],
    )
}
