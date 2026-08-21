//! Static Tree Reader — direct filesystem read from /dev/shm/opdbus/state/
//!
//! Replaces `projection_client.rs`. No D-Bus, no caching, no network.
//! Each function reads the atomic files that `write_projection()` maintains.
//!
//! # Layout
//! `write_projection()` stores one file per plugin:
//!   `/dev/shm/opdbus/state/<plugin_id>.json`
//! The file contains the plugin's full present-state JSON object. Consumers
//! navigate the returned object for the subkeys they need (e.g.
//! `state.get("model_routes")`).
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

/// Read a plugin's full present-state from the static tree.
///
/// Returns `None` if the file does not exist (state not yet mutated — correct per REQ-3.4).
pub fn read_plugin(plugin: &str) -> Option<Value> {
    let path = format!("{SHM_STATE_DIR}/{plugin}.json");
    read_json_file(&path)
}

/// Walk the entire state tree and aggregate all plugin state files.
///
/// Used by `GET /api/ui-model/state` (replaces the projection daemon dump).
/// Returns a map keyed by plugin_id (`.json` suffix stripped) with the
/// plugin's state as value. Dotfiles (e.g. `.manifest.json`) are skipped.
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
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            // Skip dotfiles (`.manifest.json`) and non-state files.
            if name.starts_with('.') {
                continue;
            }
            let Some(plugin) = name.strip_suffix(".json") else {
                continue;
            };
            if let Some(val) = read_json_file_path(&path) {
                tree.insert(plugin.to_string(), val);
            }
        }
    }
    tree
}

/// Read initial state from Btrfs seed volume (cold start).
///
/// The seed volume is produced externally at deploy time (Btrfs snapshot of
/// the state tree) — it has no relationship to `op-blockchain`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_all_from_path_lists_state_json_and_skips_dotfiles() {
        let dir = std::env::temp_dir().join(format!(
            "op-web-state-tree-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("tched_router.json"),
            br#"{"selected_model":"qwen"}"#,
        )
        .unwrap();
        fs::write(dir.join("system.memory.json"), br#"{"rss":1}"#).unwrap();
        fs::write(dir.join(".manifest.json"), br#"{"generation":3}"#).unwrap();
        fs::write(dir.join("notes.txt"), b"ignore").unwrap();

        let tree = read_all_from_path(dir.to_str().unwrap());
        // Drop the temp dir before asserting so a failure does not leak it.
        let _ = fs::remove_dir_all(&dir);

        assert!(tree.contains_key("tched_router"));
        assert!(tree.contains_key("system.memory"));
        assert!(!tree.contains_key(".manifest"));
        assert_eq!(tree.len(), 2);
    }
}
