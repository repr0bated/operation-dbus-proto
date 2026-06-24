//! Shared-memory projection layer for the projected plugin tree.
//!
//! The projected plugin tree lives in tmpfs (`/dev/shm/opdbus/projections/`).
//! The MutationEngine writes one file per plugin on every mutation (atomic
//! temp+rename). The projection D-Bus server reads 1:1 from these files on
//! every property access — zero-copy, memory-speed, no held cache.
//!
//! This module is the shared contract between the **writer** (op-grpc-bridge
//! MutationEngine, the single write door) and the **reader** (op-projection
//! D-Bus server). Both depend on op-core, so the path constants and I/O
//! helpers live here.

use simd_json::prelude::*;
use std::fs;
use std::io::Write;

/// Directory for per-plugin projection files (one JSON file per plugin).
pub const SHM_PROJECTION_DIR: &str = "/dev/shm/opdbus/projections";

/// Manifest carrying the monotonic `generation` counter. Written LAST as the
/// atomic commit point after each projection write. Consumers read it to
/// detect staleness (compare generation before/after a read).
pub const SHM_PROJECTION_MANIFEST: &str = "/dev/shm/opdbus/projections/.manifest.json";

/// Atomically publish `bytes` to a `/dev/shm` path via a sibling temp file +
/// rename, so readers see either the old or new content, never a torn write.
pub fn atomic_write_shm(path: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = format!("{}.tmp", path);
    {
        let mut file = fs::File::create(&tmp)
            .map_err(|e| anyhow::anyhow!("Cannot create {}: {}", tmp, e))?;
        file.write_all(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", tmp, e))?;
        file.sync_all()
            .map_err(|e| anyhow::anyhow!("Failed to sync {}: {}", tmp, e))?;
    }
    fs::rename(&tmp, path)
        .map_err(|e| anyhow::anyhow!("Failed to rename {} -> {}: {}", tmp, path, e))?;
    Ok(())
}

/// Sanitize a plugin id into a safe flat filename (no path separators or NUL).
fn safe_filename(plugin_id: &str) -> String {
    plugin_id
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == '\0' { '_' } else { c })
        .collect()
}

/// File path for a specific plugin's projection.
pub fn projection_file_path(plugin_id: &str) -> String {
    format!("{}/{}.json", SHM_PROJECTION_DIR, safe_filename(plugin_id))
}

/// Write a plugin's full projected state to shm and bump the manifest
/// generation.
///
/// Called by the MutationEngine on every mutation that changes a plugin's
/// state. `json_bytes` is the JSON serialization of the plugin's current state
/// (the mutation fold from `state_cache`).
pub fn write_projection(plugin_id: &str, json_bytes: &[u8]) -> anyhow::Result<()> {
    fs::create_dir_all(SHM_PROJECTION_DIR).map_err(|e| {
        anyhow::anyhow!("Cannot create projection dir {}: {}", SHM_PROJECTION_DIR, e)
    })?;

    let path = projection_file_path(plugin_id);
    atomic_write_shm(&path, json_bytes)?;

    // Bump manifest generation (atomic commit point, written last).
    let generation = read_manifest_generation().wrapping_add(1);
    let manifest = format!("{{\"generation\":{}}}", generation);
    atomic_write_shm(SHM_PROJECTION_MANIFEST, manifest.as_bytes())?;

    tracing::debug!(plugin_id, generation, "Projection written to shm");
    Ok(())
}

/// Read the raw bytes of a plugin's projection from shm.
pub fn read_projection_bytes(plugin_id: &str) -> Option<Vec<u8>> {
    fs::read(projection_file_path(plugin_id)).ok()
}

/// Read the current manifest `generation`, or 0 if no manifest exists yet.
pub fn read_manifest_generation() -> u64 {
    let mut bytes = match fs::read(SHM_PROJECTION_MANIFEST) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let value = match simd_json::from_slice::<simd_json::OwnedValue>(&mut bytes) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    match value.get("generation") {
        Some(g) => g.as_u64().unwrap_or(0),
        None => 0,
    }
}

/// List all plugin ids that currently have projection files in shm.
///
/// The manifest file (`.manifest.json`) is excluded. Plugin filenames are
/// unsanitized back to their original form (the safe-filename transform is
/// lossless for valid plugin ids).
pub fn list_projected_plugins() -> Vec<String> {
    let mut plugins = Vec::new();
    if let Ok(entries) = fs::read_dir(SHM_PROJECTION_DIR) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(plugin) = name.strip_suffix(".json") {
                    // Skip the manifest (`.manifest.json` → `.manifest`)
                    if !plugin.starts_with('.') {
                        plugins.push(plugin.to_string());
                    }
                }
            }
        }
    }
    plugins.sort();
    plugins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_filename_replaces_separators() {
        assert_eq!(safe_filename("wireguard"), "wireguard");
        assert_eq!(safe_filename("a/b"), "a_b");
        assert_eq!(safe_filename("a\\b"), "a_b");
    }

    #[test]
    fn projection_file_path_under_canonical_dir() {
        assert_eq!(
            projection_file_path("cognitive_mcp"),
            "/dev/shm/opdbus/projections/cognitive_mcp.json"
        );
    }
}
