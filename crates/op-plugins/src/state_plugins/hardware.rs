use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Hardware inventory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.hardware.schema@v1"))]
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

/// Derived `hardware` schema from the `HardwareState` struct.
pub fn hardware_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(HardwareState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "hardware",
        "1.0.0",
        "Hardware inventory snapshot",
        &root,
    )
}

/// Frozen golden reference: the original hand-rolled schema, kept **test-only**
/// so `derived_schema_matches_hand_rolled` can prove the derived schema still
/// matches the contract this plugin shipped with. Production uses
/// [`hardware_schema`].
#[cfg(test)]
pub(crate) fn hardware_schema_golden() -> PluginSchema {
    let mut cpu_fields = HashMap::new();
    cpu_fields.insert(
        "model".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "CPU model name".to_string(),
            default: None,
            example: Some(json!("AMD Ryzen 9 7950X")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    cpu_fields.insert(
        "cores".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Number of CPU cores".to_string(),
            default: None,
            example: Some(json!(16)),
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );

    let mut memory_fields = HashMap::new();
    memory_fields.insert(
        "total_kb".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Total memory in kilobytes".to_string(),
            default: None,
            example: Some(json!(33554432)),
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    memory_fields.insert(
        "available_kb".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Available memory in kilobytes".to_string(),
            default: None,
            example: Some(json!(16777216)),
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );

    let mut disk_fields = HashMap::new();
    disk_fields.insert(
        "name".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Disk device name".to_string(),
            default: None,
            example: Some(json!("nvme0n1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    disk_fields.insert(
        "size_bytes".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Disk size in bytes".to_string(),
            default: None,
            example: Some(json!(1000204886016_i64)),
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    disk_fields.insert(
        "mountpoint".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Mount point, if any".to_string(),
            default: Some(json!(null)),
            example: Some(json!("/")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schema = PluginSchema::builder("hardware")
        .version("1.0.0")
        .description("Hardware inventory snapshot")
        .field(
            "cpu",
            FieldSchema {
                field_type: FieldType::Object(cpu_fields),
                required: false,
                description: "CPU information".to_string(),
                default: Some(json!({
                    "cores": 0,
                    "model": ""
                })),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "memory",
            FieldSchema {
                field_type: FieldType::Object(memory_fields),
                required: false,
                description: "Memory information".to_string(),
                default: Some(json!({
                    "available_kb": 0,
                    "total_kb": 0
                })),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "disks",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(disk_fields))),
                required: false,
                description: "Disk inventory".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .example(json!({
            "cpu": {
                "model": "AMD Ryzen 9 7950X",
                "cores": 16
            },
            "memory": {
                "total_kb": 33554432,
                "available_kb": 16777216
            },
            "disks": [
                {
                    "name": "nvme0n1",
                    "size_bytes": 1000204886016_i64,
                    "mountpoint": "/"
                }
            ]
        }))
        .build();
    schema.subids.insert(
        "__schema__".to_string(),
        "sch.software.plugin.hardware.schema@v1".to_string(),
    );
    schema.subids.insert(
        "cpu".to_string(),
        "exp.service.hardware.cpu.render@v1".to_string(),
    );
    schema.subids.insert(
        "memory".to_string(),
        "exp.service.hardware.memory.render@v1".to_string(),
    );
    schema.subids.insert(
        "disks".to_string(),
        "exp.service.hardware.disks.render@v1".to_string(),
    );
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schemars-derived schema must match the hand-rolled golden reference.
    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = hardware_schema_golden();
        let derived = hardware_schema();
        let diffs = crate::state_plugins::schemars_adapter::schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

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
