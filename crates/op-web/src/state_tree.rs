//! Static Tree Reader — direct filesystem read from /dev/shm/opdbus/state/
//!
//! Replaces `projection_client.rs`. No D-Bus, no caching, no network.
//! Each function reads the atomic files that `write_projection()` maintains.
//!
//! # No Polling
//! This module is a pure reader. It does NOT poll, cache, or hold a connection.
//! Consumers that need reactivity subscribe to the `Updated` signal on
//! `org.opdbus.v1.PluginV1` and re-read the relevant shm path on receipt.

use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::Path;

/// Base path for the SHM state tree written by `write_projection()`.
const SHM_STATE_DIR: &str = "/dev/shm/opdbus/state";

/// Read a single plugin's entire state from the static tree.
///
/// Returns `None` if the file does not exist (state not yet mutated — correct per REQ-3.4).
pub fn read_key(plugin: &str, _key: &str) -> Option<Value> {
    // write_projection stores one file per plugin_id (not per key):
    //   /dev/shm/opdbus/state/<plugin_id>
    // The file contains the full plugin state JSON.
    let path = format!("{SHM_STATE_DIR}/{plugin}");
    read_json_file(&path)
}

/// Read a plugin's full projection from the static tree.
///
/// Returns empty object if no state file exists.
pub fn read_plugin(plugin: &str) -> Option<Value> {
    let path = format!("{SHM_STATE_DIR}/{plugin}");
    read_json_file(&path)
}

/// Walk the entire state tree and aggregate all plugin state files.
///
/// Used by the dashboard whole-tree dump endpoint.
/// Returns an object keyed by plugin_id with the plugin's state as value.
pub fn read_all() -> HashMap<String, Value> {
    read_all_from_path(SHM_STATE_DIR)
}

/// Read all state from a given base directory.
///
/// Shared implementation for both live SHM reads and cold-start seed volume reads.
/// Called once at startup for seed volume, then push-only via Updated signal.
pub fn read_all_from_path(base: &str) -> HashMap<String, Value> {
    let mut tree = HashMap::new();
    let base_path = Path::new(base);
    if !base_path.exists() {
        return tree;
    }
    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(val) = read_json_file_path(&path) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    tree.insert(name, val);
                }
            }
        }
    }
    tree
}

/// Read initial state from Btrfs seed volume (cold start).
///
/// Returns empty map if snapshot is missing (first boot — correct per REQ-3.4).
/// Called once at startup, then push-only via Updated signal.
pub fn read_seed_volume() -> HashMap<String, Value> {
    let seed_path = "/var/lib/opdbus/snapshots/latest";
    if Path::new(seed_path).exists() {
        read_all_from_path(seed_path)
    } else {
        HashMap::new()
    }
}

/// Read and parse a JSON file, returning None on any failure.
fn read_json_file(path: &str) -> Option<Value> {
    read_json_file_path(Path::new(path))
}

/// Read and parse a JSON file by Path reference.
fn read_json_file_path(path: &Path) -> Option<Value> {
    let mut bytes = std::fs::read(path).ok()?;
    simd_json::to_owned_value(&mut bytes).ok()
}
