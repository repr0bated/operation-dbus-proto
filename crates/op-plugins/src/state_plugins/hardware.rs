use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// Hardware inventory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.hardware.schema@v1"))]
#[schemars(extend("x-oscal-category" = "hardware"))]
pub struct HardwareState {
    /// CPU information.
    #[serde(default)]
    #[schemars(
        description = "CPU information",
        extend("x-oscal-subid" = "exp.service.hardware.cpu.render@v1")
    )]
    pub cpu: CpuInfo,

    /// Memory information.
    #[serde(default)]
    #[schemars(
        description = "Memory information",
        extend("x-oscal-subid" = "exp.service.hardware.memory.render@v1")
    )]
    pub memory: MemoryInfo,

    /// Disk inventory.
    #[serde(default)]
    #[schemars(
        description = "Disk inventory",
        extend("x-oscal-subid" = "exp.service.hardware.disks.render@v1")
    )]
    pub disks: Vec<DiskInfo>,
}

/// CPU information.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct CpuInfo {
    /// CPU model name.
    #[schemars(
        description = "CPU model name",
        example = &"AMD Ryzen 9 7950X",
        extend("x-oscal-subid" = "exp.service.hardware.cpu.model.declare@v1")
    )]
    pub model: String,

    /// Number of CPU cores.
    #[schemars(
        description = "Number of CPU cores",
        example = 16,
        extend("x-oscal-subid" = "exp.service.hardware.cpu.cores.declare@v1")
    )]
    pub cores: usize,
}

/// Memory information.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct MemoryInfo {
    /// Total memory in kilobytes.
    #[schemars(
        description = "Total memory in kilobytes",
        example = 33554432,
        extend("x-oscal-subid" = "exp.service.hardware.memory.total-kb.declare@v1")
    )]
    pub total_kb: u64,

    /// Available memory in kilobytes.
    #[schemars(
        description = "Available memory in kilobytes",
        example = 16777216,
        extend("x-oscal-subid" = "exp.service.hardware.memory.available-kb.declare@v1")
    )]
    pub available_kb: u64,
}

/// Disk information.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiskInfo {
    /// Disk device name.
    #[schemars(
        description = "Disk device name",
        example = &"nvme0n1",
        extend("x-oscal-subid" = "exp.service.hardware.disk.name.declare@v1")
    )]
    pub name: String,

    /// Disk size in bytes.
    #[schemars(
        description = "Disk size in bytes",
        example = 1000204886016_i64,
        extend("x-oscal-subid" = "exp.service.hardware.disk.size-bytes.declare@v1")
    )]
    pub size_bytes: u64,

    /// Mount point, if any.
    #[serde(default)]
    #[schemars(
        description = "Mount point, if any",
        example = &"/",
        extend("x-oscal-subid" = "exp.service.hardware.disk.mountpoint.declare@v1")
    )]
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

/// Derived `hardware` schema from the `HardwareState` struct.
pub fn hardware_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(HardwareState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "hardware",
        "1.0.0",
        "Hardware inventory snapshot",
        &root,
    );

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListDevicesOutput {
        pub devices: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetDeviceOutput {
        pub device: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetStatsOutput {
        pub stats: serde_json::Value,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "list_devices".to_string(),
        method_decl_from_schemars_with_output::<(), ListDevicesOutput>(
            "list_devices",
            SideEffect::Read,
            true,
            "hardware.read",
            "obs.service.hardware.device.list@v1",
        ),
    );
    schema.methods.insert(
        "get_device".to_string(),
        method_decl_from_schemars_with_output::<(), GetDeviceOutput>(
            "get_device",
            SideEffect::Read,
            true,
            "hardware.read",
            "obs.service.hardware.device.get@v1",
        ),
    );
    schema.methods.insert(
        "scan_hardware".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "scan_hardware",
            SideEffect::Mutation,
            false,
            "hardware.invoke",
            "mut.service.hardware.scan@v1",
        ),
    );
    schema.methods.insert(
        "get_stats".to_string(),
        method_decl_from_schemars_with_output::<(), GetStatsOutput>(
            "get_stats",
            SideEffect::Read,
            true,
            "hardware.read",
            "obs.service.hardware.stats@v1",
        ),
    );
    schema.methods.insert(
        "refresh_inventory".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "refresh_inventory",
            SideEffect::Mutation,
            false,
            "hardware.invoke",
            "mut.service.hardware.refresh@v1",
        ),
    );

    schema.capabilities.insert(
        "hardware.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "hardware.read".to_string(),
            description: "Grants: list_devices, get_device, get_stats.".to_string(),
        },
    );
    schema.capabilities.insert(
        "hardware.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "hardware.invoke".to_string(),
            description: "Grants: scan_hardware, refresh_inventory.".to_string(),
        },
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `x-oscal-subid` annotation in the derived schema must be a valid
    /// OSCAL subid according to the canonical taxonomy.
    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(HardwareState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            crate::state_plugins::common::oscal::validate_subid(&subid)
                .unwrap_or_else(|e| panic!("invalid subid {subid}: {e}"));
        }
    }

    fn collect_subids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let serde_json::Value::Object(map) = value {
            if let Some(subid) = map.get("x-oscal-subid").and_then(|v| v.as_str()) {
                out.push(subid.to_string());
            }
            for v in map.values() {
                collect_subids(v, out);
            }
        } else if let serde_json::Value::Array(arr) = value {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("hardware", |_ctx| std::sync::Arc::new(HardwarePlugin::new()))
}
