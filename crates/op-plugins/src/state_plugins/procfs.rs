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
use simd_json::OwnedValue as Value;
use std::path::Path;
use tokio::fs;

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
        Some(super::plugin_schema_defs::procfs_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(gather_procfs_state().await)
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
            state_snapshot: self.query_current_state().await?,
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

async fn gather_procfs_state() -> Value {
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

    let mut map = simd_json::owned::Object::new();
    map.insert("memory".into(), memory);
    map.insert("loadavg".into(), loadavg);
    map.insert("uptime".into(), uptime);
    map.insert("cpuinfo".into(), cpuinfo);
    map.insert("stat".into(), stat);
    map.insert("net_dev".into(), net_dev);
    map.insert("mounts".into(), mounts);
    map.insert("kernel".into(), kernel);
    map.insert("vmstat".into(), vmstat);
    map.insert("diskstats".into(), diskstats);
    Value::Object(Box::new(map))
}

async fn read_proc(path: &str) -> String {
    fs::read_to_string(Path::new("/proc").join(path))
        .await
        .unwrap_or_default()
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

async fn gather_memory() -> Value {
    kv_file(&read_proc("meminfo").await)
}

async fn gather_loadavg() -> Value {
    let raw = read_proc("loadavg").await;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let mut map = simd_json::owned::Object::new();
    if let Some(v) = parts.first() {
        map.insert("load1".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(1) {
        map.insert("load5".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(2) {
        map.insert("load15".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(3) {
        if let Some((running, total)) = v.split_once('/') {
            map.insert("procs_running".into(), num_or_str(running));
            map.insert("procs_total".into(), num_or_str(total));
        }
    }
    Value::Object(Box::new(map))
}

async fn gather_uptime() -> Value {
    let raw = read_proc("uptime").await;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let mut map = simd_json::owned::Object::new();
    if let Some(v) = parts.first() {
        map.insert("uptime_secs".into(), num_or_str(v));
    }
    if let Some(v) = parts.get(1) {
        map.insert("idle_secs".into(), num_or_str(v));
    }
    Value::Object(Box::new(map))
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

async fn gather_net_dev() -> Value {
    let raw = read_proc("net/dev").await;
    let mut interfaces = Vec::new();
    for line in raw.lines().skip(2) {
        if let Some((iface, stats)) = line.split_once(':') {
            let parts: Vec<&str> = stats.split_whitespace().collect();
            let labels = [
                "rx_bytes",
                "rx_packets",
                "rx_errs",
                "rx_drop",
                "rx_fifo",
                "rx_frame",
                "rx_compressed",
                "rx_multicast",
                "tx_bytes",
                "tx_packets",
                "tx_errs",
                "tx_drop",
                "tx_fifo",
                "tx_colls",
                "tx_carrier",
                "tx_compressed",
            ];
            let mut map = simd_json::owned::Object::new();
            map.insert("interface".into(), Value::from(iface.trim().to_string()));
            for (idx, label) in labels.iter().enumerate() {
                if let Some(value) = parts.get(idx) {
                    map.insert((*label).into(), num_or_str(value));
                }
            }
            interfaces.push(Value::Object(Box::new(map)));
        }
    }
    let mut map = simd_json::owned::Object::new();
    map.insert("interfaces".into(), Value::Array(interfaces));
    Value::Object(Box::new(map))
}

async fn gather_mounts() -> Value {
    let raw = read_proc("mounts").await;
    let mut mounts = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let mut map = simd_json::owned::Object::new();
            map.insert("device".into(), Value::from(parts[0].to_string()));
            map.insert("mountpoint".into(), Value::from(parts[1].to_string()));
            map.insert("fstype".into(), Value::from(parts[2].to_string()));
            map.insert("options".into(), Value::from(parts[3].to_string()));
            mounts.push(Value::Object(Box::new(map)));
        }
    }
    Value::Array(mounts)
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
