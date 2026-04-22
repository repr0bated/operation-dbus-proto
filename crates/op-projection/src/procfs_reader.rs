//! Procfs Reader: Reading from /proc.
//!
//! This module implements the `ProcfsReader` trait, scanning the `/proc`
//! directory and projecting standard entries into raw entities.

use crate::interfaces::{ProcfsReader, RawEntity, SourceReader};
use anyhow::Result;
use simd_json::json;
use std::fs;
use tracing::{debug, warn};

/// Reader that extracts state from the /proc filesystem.
#[derive(Debug, Clone)]
pub struct SystemProcfsReader {
    /// Source identifier
    source: String,
}

impl SystemProcfsReader {
    /// Creates a new SystemProcfsReader
    pub fn new() -> Self {
        Self {
            source: "procfs".to_string(),
        }
    }

    /// Helper to read a value from a proc file
    fn read_proc_value(&self, path: &str) -> Option<String> {
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }
}

impl Default for SystemProcfsReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for SystemProcfsReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        let mut entities = Vec::new();

        entities.extend(self.read_processes()?);

        if let Ok(memory) = self.read_memory() {
            entities.push(memory);
        }

        if let Ok(cpu) = self.read_cpu() {
            entities.push(cpu);
        }

        if let Ok(filesystems) = self.read_filesystems() {
            entities.push(filesystems);
        }

        if let Ok(network) = self.read_network() {
            entities.push(network);
        }

        Ok(entities)
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        match entity_id {
            "memory" => self.read_memory(),
            "cpu" => self.read_cpu(),
            "filesystems" => self.read_filesystems(),
            "network" => self.read_network(),
            _ => Err(anyhow::anyhow!("Unknown procfs entity: {}", entity_id)),
        }
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/proc").exists()
    }
}

impl ProcfsReader for SystemProcfsReader {
    fn read_processes(&self) -> Result<Vec<RawEntity>> {
        let mut processes = Vec::new();

        // Iterate over /proc/[0-9]*
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.chars().all(|c| c.is_ascii_digit()) {
                    let pid = name_str.to_string();
                    let comm_path = format!("/proc/{}/comm", pid);

                    if let Some(comm) = self.read_proc_value(&comm_path) {
                        processes.push(RawEntity {
                            entity_type: "system.process".to_string(),
                            entity_id: pid,
                            data: json!({ "name": comm }).into(),
                            source: self.source.clone(),
                        });
                    }
                }

                // Limit to 10 processes for now to avoid overwhelming
                if processes.len() >= 10 {
                    break;
                }
            }
        }

        Ok(processes)
    }

    fn read_memory(&self) -> Result<RawEntity> {
        debug!("Reading memory info from /proc/meminfo");

        let mut total_kb = 0;
        let mut free_kb = 0;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    total_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                } else if line.starts_with("MemFree:") {
                    free_kb = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                }
            }
        }

        Ok(RawEntity {
            entity_type: "system.memory".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "total_kb": total_kb, "free_kb": free_kb }).into(),
            source: self.source.clone(),
        })
    }

    fn read_cpu(&self) -> Result<RawEntity> {
        debug!("Reading CPU info from /proc/cpuinfo");

        let mut cores = 0;
        let mut model = "unknown".to_string();

        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if line.starts_with("processor") {
                    cores += 1;
                } else if line.starts_with("model name") && model == "unknown" {
                    model = line
                        .split(':')
                        .nth(1)
                        .unwrap_or("unknown")
                        .trim()
                        .to_string();
                }
            }
        }

        Ok(RawEntity {
            entity_type: "system.cpu".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "cores": cores, "model": model }).into(),
            source: self.source.clone(),
        })
    }

    fn read_filesystems(&self) -> Result<RawEntity> {
        debug!("Reading filesystems from /proc/filesystems");

        let mut fs_types = Vec::new();
        if let Ok(content) = fs::read_to_string("/proc/filesystems") {
            for line in content.lines() {
                fs_types.push(line.trim().to_string());
            }
        }

        Ok(RawEntity {
            entity_type: "system.filesystems".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "types": fs_types }).into(),
            source: self.source.clone(),
        })
    }

    fn read_network(&self) -> Result<RawEntity> {
        debug!("Reading network info from /proc/net/dev");

        let mut interfaces = Vec::new();
        if let Ok(content) = fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                if let Some(iface) = line.split(':').next() {
                    interfaces.push(iface.trim().to_string());
                }
            }
        }

        Ok(RawEntity {
            entity_type: "system.network".to_string(),
            entity_id: "current".to_string(),
            data: json!({ "interfaces": interfaces }).into(),
            source: self.source.clone(),
        })
    }
}
