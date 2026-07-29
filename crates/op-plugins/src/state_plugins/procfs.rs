//! Procfs state plugin.
//!
//! This turns procfs-derived host state into a `PluginSchema`-backed plugin so
//! JSON rendering, D-Bus projection, and tool generation all consume the same
//! schema authority instead of ad-hoc `/proc` tools.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::path::Path;
use tokio::fs;

/// Runtime procfs state, derived from `/proc` files.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(
    extend("x-oscal-subid" = "sch.software.plugin.procfs.schema@v1"),
    extend("x-immutable-paths" = [
        "/memory",
        "/loadavg",
        "/uptime",
        "/cpuinfo",
        "/stat",
        "/net_dev",
        "/mounts",
        "/kernel",
        "/vmstat",
        "/diskstats"
    ])
)]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct ProcfsState {
    /// Parsed /proc/meminfo values.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/meminfo values.",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.memory.query@v1")
    )]
    pub memory: std::collections::BTreeMap<String, u64>,
    /// Parsed /proc/loadavg values.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/loadavg values.",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.loadavg.query@v1")
    )]
    pub loadavg: LoadAvg,
    /// Parsed /proc/uptime values.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/uptime values.",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.uptime.query@v1")
    )]
    pub uptime: Uptime,
    /// Parsed CPU inventory from /proc/cpuinfo.
    #[serde(default)]
    #[schemars(
        description = "Parsed CPU inventory from /proc/cpuinfo.",
        with = "serde_json::Value",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.cpuinfo.query@v1")
    )]
    pub cpuinfo: Value,
    /// Parsed /proc/stat values.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/stat values.",
        with = "serde_json::Value",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.stat.query@v1")
    )]
    pub stat: Value,
    /// Parsed /proc/net/dev counters.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/net/dev counters.",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.net-dev.query@v1")
    )]
    pub net_dev: NetDev,
    /// Parsed /proc/mounts entries.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/mounts entries.",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.mounts.query@v1")
    )]
    pub mounts: Vec<MountEntry>,
    /// Kernel version from /proc/version.
    #[serde(default)]
    #[schemars(
        description = "Kernel version from /proc/version.",
        with = "serde_json::Value",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.kernel.query@v1")
    )]
    pub kernel: Value,
    /// Parsed /proc/vmstat values.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/vmstat values.",
        with = "serde_json::Value",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.vmstat.query@v1")
    )]
    pub vmstat: Value,
    /// Parsed /proc/diskstats rows.
    #[serde(default)]
    #[schemars(
        description = "Parsed /proc/diskstats rows.",
        with = "serde_json::Value",
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.service.procfs.diskstats.query@v1")
    )]
    pub diskstats: Value,
}

pub struct ProcfsPlugin;

impl ProcfsPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcfsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for ProcfsPlugin {
    fn name(&self) -> &str {
        "procfs"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        Path::new("/proc").exists()
    }

    fn unavailable_reason(&self) -> String {
        "/proc is not mounted".to_string()
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(procfs_schema())
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![StateAction::NoOp {
                resource: "procfs".to_string(),
            }],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: Vec::new(),
            errors: Vec::new(),
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("procfs-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!(null),
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
            atomic_operations: true,
        }
    }
}

/// Procfs schema derived from the typed [`ProcfsState`] struct via schemars.
pub(crate) fn procfs_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(ProcfsState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "procfs",
        "1.0.0",
        "Read-only procfs host state projected through PluginSchema.",
        &root,
    );

    schema.methods.insert(
        "list_processes".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            super::plugin_scaffold_helpers::EmptyInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "ListProcesses",
            op_state_store::SideEffect::Read,
            true,
            "procfs.read",
            "obs.service.procfs.process.list@v1",
        ),
    );
    schema.methods.insert(
        "get_process_info".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ProcessInfoInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetProcessInfo",
            op_state_store::SideEffect::Read,
            true,
            "procfs.read",
            "obs.service.procfs.process.get@v1",
        ),
    );
    schema.methods.insert(
        "get_meminfo".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            super::plugin_scaffold_helpers::EmptyInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetMeminfo",
            op_state_store::SideEffect::Read,
            true,
            "procfs.read",
            "obs.service.procfs.meminfo.get@v1",
        ),
    );
    schema.methods.insert(
        "get_cpuinfo".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            super::plugin_scaffold_helpers::EmptyInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetCpuinfo",
            op_state_store::SideEffect::Read,
            true,
            "procfs.read",
            "obs.service.procfs.cpuinfo.get@v1",
        ),
    );
    schema.methods.insert(
        "get_loadavg".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            super::plugin_scaffold_helpers::EmptyInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetLoadavg",
            op_state_store::SideEffect::Read,
            true,
            "procfs.read",
            "obs.service.procfs.loadavg.get@v1",
        ),
    );
    schema.methods.insert(
        "get_uptime".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            super::plugin_scaffold_helpers::EmptyInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetUptime",
            op_state_store::SideEffect::Read,
            true,
            "procfs.read",
            "obs.service.procfs.uptime.get@v1",
        ),
    );
    schema.methods.insert(
        "get_net_dev".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            super::plugin_scaffold_helpers::EmptyInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "GetNetDev",
            op_state_store::SideEffect::Read,
            true,
            "procfs.read",
            "obs.service.procfs.net-dev.get@v1",
        ),
    );

    schema
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProcessInfoInput {
    pub pid: i32,
}

async fn gather_procfs_state() -> ProcfsState {
    let (memory, loadavg, uptime, cpuinfo, stat, net_dev, mounts, kernel, vmstat, diskstats) = tokio::join!(
        gather_memory(),
        gather_loadavg(),
        gather_uptime(),
        gather_cpuinfo(),
        gather_stat(),
        gather_net_dev(),
        gather_mounts(),
        gather_kernel(),
        gather_vmstat(),
        gather_diskstats(),
    );

    ProcfsState {
        memory,
        loadavg,
        uptime,
        cpuinfo,
        stat,
        net_dev,
        mounts,
        kernel,
        vmstat,
        diskstats,
    }
}

async fn read_proc(path: &str) -> String {
    fs::read_to_string(Path::new("/proc").join(path))
        .await
        .unwrap_or_default()
}

/// Parsed `/proc/loadavg`. Sample: `7.02 4.40 3.68 16/2222 8032`.
///
/// Typed rather than a free-form map so the value is usable without a consumer
/// re-parsing it: `num_or_str` would type `2` as an integer and `2.82` as a
/// float for the same field depending on the instantaneous load.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LoadAvg {
    /// 1-minute load average.
    pub load1: f64,
    /// 5-minute load average.
    pub load5: f64,
    /// 15-minute load average.
    pub load15: f64,
    /// Runnable kernel entities — numerator of the `16/2222` field.
    pub procs_running: u64,
    /// Total kernel entities — denominator of the `16/2222` field.
    pub procs_total: u64,
}

/// Parsed `/proc/uptime`. Sample: `210649.82 3136532.75`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Uptime {
    /// Seconds since boot, centisecond resolution.
    pub uptime_secs: f64,
    /// Summed idle seconds across all cores. Exceeds `uptime_secs` on SMP —
    /// this is correct, not a parsing error.
    pub idle_secs: f64,
}

/// One interface row from `/proc/net/dev`. All counters are cumulative since boot.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetDevInterface {
    /// Interface name, e.g. `eth0`, `ovsbr0`.
    pub interface: String,
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errs: u64,
    pub rx_drop: u64,
    pub rx_fifo: u64,
    pub rx_frame: u64,
    pub rx_compressed: u64,
    pub rx_multicast: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errs: u64,
    pub tx_drop: u64,
    pub tx_fifo: u64,
    pub tx_colls: u64,
    pub tx_carrier: u64,
    pub tx_compressed: u64,
}

/// Parsed `/proc/net/dev`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetDev {
    pub interfaces: Vec<NetDevInterface>,
}

/// One row from `/proc/mounts`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MountEntry {
    /// Backing device or pseudo-source, e.g. `/dev/sda4`, `proc`.
    pub device: String,
    /// Where it is mounted, e.g. `/home`.
    pub mountpoint: String,
    /// Filesystem type, e.g. `btrfs`, `tmpfs`.
    pub fstype: String,
    /// Comma-separated mount options exactly as the kernel reports them.
    pub options: String,
}

/// Strip a trailing unit and parse the leading integer: `"32855784 kB"` → `32855784`.
fn kb_value(raw: &str) -> Option<u64> {
    raw.split_whitespace().next()?.parse::<u64>().ok()
}

/// Normalise a `/proc/meminfo` key to a snake_case field name, encoding the unit
/// in the name so no consumer has to parse `" kB"` out of a value:
/// `MemTotal` → `mem_total_kb`, `Active(anon)` → `active_anon_kb`.
fn meminfo_key(key: &str, has_kb_unit: bool) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    let mut prev_lower = false;
    for ch in key.chars() {
        match ch {
            '(' | '-' | ' ' => {
                out.push('_');
                prev_lower = false;
            }
            ')' => prev_lower = false,
            c if c.is_ascii_uppercase() => {
                if prev_lower {
                    out.push('_');
                }
                out.push(c.to_ascii_lowercase());
                prev_lower = false;
            }
            c => {
                out.push(c);
                prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            }
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if has_kb_unit {
        format!("{out}_kb")
    } else {
        out
    }
}

fn num_or_str(s: &str) -> Value {
    let t = s.trim();
    if let Ok(n) = t.parse::<i64>() {
        return Value::from(n);
    }
    if let Ok(f) = t.parse::<f64>() {
        return Value::from(f);
    }
    Value::from(t.to_string())
}

fn kv_file(content: &str) -> Value {
    let mut map = simd_json::owned::Object::new();
    for line in content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            map.insert(
                key.trim().replace(' ', "_").to_lowercase(),
                num_or_str(value),
            );
        }
    }
    Value::Object(Box::new(map))
}

/// Parsed `/proc/meminfo`, keyed by normalised field name with integer values.
///
/// A map rather than a fixed struct because the key set is kernel-config
/// dependent (hugepages, ZSWAP, CMA and others appear conditionally) — but every
/// value is a `u64`, so this is fully typed regardless of which keys a given
/// kernel emits.
async fn gather_memory() -> std::collections::BTreeMap<String, u64> {
    let raw = read_proc("meminfo").await;
    let mut map = std::collections::BTreeMap::new();
    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        let has_kb = value.ends_with("kB");
        if let Some(n) = kb_value(value) {
            map.insert(meminfo_key(key.trim(), has_kb), n);
        }
    }
    map
}

async fn gather_loadavg() -> LoadAvg {
    let raw = read_proc("loadavg").await;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let (procs_running, procs_total) = parts
        .get(3)
        .and_then(|v| v.split_once('/'))
        .map(|(r, t)| {
            (
                r.parse::<u64>().unwrap_or_default(),
                t.parse::<u64>().unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    LoadAvg {
        load1: parts
            .first()
            .and_then(|v| v.parse().ok())
            .unwrap_or_default(),
        load5: parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or_default(),
        load15: parts
            .get(2)
            .and_then(|v| v.parse().ok())
            .unwrap_or_default(),
        procs_running,
        procs_total,
    }
}

async fn gather_uptime() -> Uptime {
    let raw = read_proc("uptime").await;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    Uptime {
        uptime_secs: parts
            .first()
            .and_then(|v| v.parse().ok())
            .unwrap_or_default(),
        idle_secs: parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or_default(),
    }
}

async fn gather_cpuinfo() -> Value {
    let raw = read_proc("cpuinfo").await;
    let mut cpus: Vec<Value> = Vec::new();
    let mut cur = simd_json::owned::Object::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                cpus.push(Value::Object(Box::new(std::mem::take(&mut cur))));
            }
        } else if let Some((key, value)) = line.split_once(':') {
            cur.insert(
                key.trim().replace(' ', "_").to_lowercase(),
                num_or_str(value),
            );
        }
    }
    if !cur.is_empty() {
        cpus.push(Value::Object(Box::new(cur)));
    }

    let mut map = simd_json::owned::Object::new();
    map.insert("count".into(), Value::from(cpus.len() as i64));
    map.insert("cpus".into(), Value::Array(cpus));
    Value::Object(Box::new(map))
}

async fn gather_stat() -> Value {
    let raw = read_proc("stat").await;
    let mut map = simd_json::owned::Object::new();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once(' ') {
            map.insert(key.into(), num_or_str(value));
        }
    }
    Value::Object(Box::new(map))
}

async fn gather_net_dev() -> NetDev {
    let raw = read_proc("net/dev").await;
    let mut interfaces = Vec::new();
    for line in raw.lines().skip(2) {
        let Some((iface, stats)) = line.split_once(':') else {
            continue;
        };
        let n: Vec<u64> = stats
            .split_whitespace()
            .map(|v| v.parse::<u64>().unwrap_or_default())
            .collect();
        let at = |i: usize| n.get(i).copied().unwrap_or_default();
        interfaces.push(NetDevInterface {
            interface: iface.trim().to_string(),
            rx_bytes: at(0),
            rx_packets: at(1),
            rx_errs: at(2),
            rx_drop: at(3),
            rx_fifo: at(4),
            rx_frame: at(5),
            rx_compressed: at(6),
            rx_multicast: at(7),
            tx_bytes: at(8),
            tx_packets: at(9),
            tx_errs: at(10),
            tx_drop: at(11),
            tx_fifo: at(12),
            tx_colls: at(13),
            tx_carrier: at(14),
            tx_compressed: at(15),
        });
    }
    NetDev { interfaces }
}

async fn gather_mounts() -> Vec<MountEntry> {
    let raw = read_proc("mounts").await;
    raw.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            (parts.len() >= 4).then(|| MountEntry {
                device: parts[0].to_string(),
                mountpoint: parts[1].to_string(),
                fstype: parts[2].to_string(),
                options: parts[3].to_string(),
            })
        })
        .collect()
}

async fn gather_kernel() -> Value {
    let mut map = simd_json::owned::Object::new();
    map.insert(
        "version".into(),
        Value::from(read_proc("version").await.trim().to_string()),
    );
    Value::Object(Box::new(map))
}

async fn gather_vmstat() -> Value {
    let raw = read_proc("vmstat").await;
    let mut map = simd_json::owned::Object::new();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once(' ') {
            map.insert(key.into(), num_or_str(value));
        }
    }
    Value::Object(Box::new(map))
}

async fn gather_diskstats() -> Value {
    let raw = read_proc("diskstats").await;
    let rows = raw
        .lines()
        .map(|line| Value::from(line.to_string()))
        .collect::<Vec<_>>();
    Value::Array(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
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
        let root = serde_json::to_value(schemars::schema_for!(ProcfsState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("procfs", |_ctx| std::sync::Arc::new(ProcfsPlugin::new()))
}

#[cfg(test)]
mod typed_field_tests {
    use super::procfs_schema;
    use op_state_store::FieldType;

    /// Untyped fields are why procfs rendered as `cpuinfo: any`, generated ten
    /// `google.protobuf.Value`s, and materialised an all-null projection.
    #[test]
    fn retyped_fields_are_no_longer_any() {
        let schema = procfs_schema();
        for name in ["loadavg", "uptime", "memory", "net_dev", "mounts"] {
            let f = schema.fields.get(name).expect("field present");
            assert!(
                !matches!(f.field_type, FieldType::Any),
                "{name} is still FieldType::Any"
            );
        }
    }

    #[test]
    fn loadavg_exposes_its_real_subfields() {
        let schema = procfs_schema();
        let FieldType::Object(fields) = &schema.fields["loadavg"].field_type else {
            panic!(
                "loadavg should be an object, got {:?}",
                schema.fields["loadavg"].field_type
            );
        };
        for (name, expected) in [
            ("load1", FieldType::Float),
            ("load5", FieldType::Float),
            ("load15", FieldType::Float),
            ("procs_running", FieldType::Integer),
            ("procs_total", FieldType::Integer),
        ] {
            let got = &fields
                .get(name)
                .unwrap_or_else(|| panic!("{name} missing"))
                .field_type;
            assert_eq!(*got, expected, "{name} typed as {got:?}");
        }
    }

    #[test]
    fn uptime_is_two_floats() {
        let schema = procfs_schema();
        let FieldType::Object(fields) = &schema.fields["uptime"].field_type else {
            panic!("uptime should be an object");
        };
        assert_eq!(fields["uptime_secs"].field_type, FieldType::Float);
        assert_eq!(fields["idle_secs"].field_type, FieldType::Float);
    }

    #[test]
    fn meminfo_keys_normalise_and_carry_their_unit() {
        assert_eq!(super::meminfo_key("MemTotal", true), "mem_total_kb");
        assert_eq!(super::meminfo_key("Active(anon)", true), "active_anon_kb");
        assert_eq!(
            super::meminfo_key("HugePages_Total", false),
            "huge_pages_total"
        );
        assert_eq!(super::meminfo_key("SwapCached", true), "swap_cached_kb");
        // the unit is stripped from the value, not left for a consumer to parse
        assert_eq!(super::kb_value("32855784 kB"), Some(32855784));
        assert_eq!(super::kb_value("0"), Some(0));
    }
}

#[cfg(test)]
mod collection_field_tests {
    use super::procfs_schema;
    use op_state_store::FieldType;

    #[test]
    fn mounts_is_an_array_of_typed_rows() {
        let schema = procfs_schema();
        let FieldType::Array(item) = &schema.fields["mounts"].field_type else {
            panic!(
                "mounts should be an array, got {:?}",
                schema.fields["mounts"].field_type
            );
        };
        let FieldType::Object(cols) = item.as_ref() else {
            panic!("mount rows should be objects, got {item:?}");
        };
        for c in ["device", "mountpoint", "fstype", "options"] {
            assert_eq!(cols[c].field_type, FieldType::String, "{c}");
        }
    }

    #[test]
    fn net_dev_counters_are_integers_not_strings() {
        // num_or_str typed these by whatever the value happened to look like;
        // a counter that reads 0 must not be a different type from one at 10^9.
        let schema = procfs_schema();
        let FieldType::Object(top) = &schema.fields["net_dev"].field_type else {
            panic!("net_dev should be an object");
        };
        let FieldType::Array(item) = &top["interfaces"].field_type else {
            panic!("interfaces should be an array");
        };
        let FieldType::Object(cols) = item.as_ref() else {
            panic!("interface rows should be objects");
        };
        assert_eq!(cols["interface"].field_type, FieldType::String);
        for c in [
            "rx_bytes",
            "rx_packets",
            "rx_errs",
            "tx_bytes",
            "tx_packets",
            "tx_drop",
        ] {
            assert_eq!(cols[c].field_type, FieldType::Integer, "{c}");
        }
    }
}
