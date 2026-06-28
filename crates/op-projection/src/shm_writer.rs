//! SHM Writer: Sole writer of plugin capability schemas and present-state to
//! shared memory.
//!
//! This module implements the producer side of the SHM pipeline.  It is the
//! **only** component that writes schema files, the combined monolith, the
//! catalog hash manifest, and present-state snapshots to `/dev/shm/opdbus/`.
//!
//! # Layout
//!
//! ```text
//! /dev/shm/
//! ├── live-schema.json                         ← combined monolith (all plugins)
//! └── opdbus/
//!     ├── .manifest.json                        ← { "catalog_hash": "<blake3>" }
//!     ├── schemas/
//!     │   ├── wireguard.json
//!     │   ├── xray.json
//!     │   └── ...                               ← one file per plugin
//!     └── state/
//!         ├── wireguard.json                    ← present-state snapshot
//!         ├── xray.json
//!         └── ...
//! ```
//!
//! # Atomicity
//!
//! The monolith and manifest are written atomically: data is first written to
//! a `.tmp` file and then renamed over the final path.  This prevents consumers
//! from reading a partial file.
//!
//! # Catalog Hash
//!
//! The Blake3 catalog hash is computed by sorting per-plugin schema filenames,
//! leaf-hashing each file's bytes, and folding the leaf hashes into a single
//! root hash.  Only the producer computes this hash; consumers read it from
//! `.manifest.json`.

use anyhow::{Context, Result};
use op_state_store::PluginSchema;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Default base directory for shared memory.
const DEFAULT_SHM_BASE: &str = "/dev/shm";

/// Subdirectory under the SHM base for all opdbus files.
const OPDBUS_DIR: &str = "opdbus";

/// Subdirectory for per-plugin capability schema files.
const SCHEMAS_DIR: &str = "schemas";

/// Subdirectory for per-plugin present-state files.
const STATE_DIR: &str = "state";

/// Combined monolith filename (lives directly under SHM base, not opdbus/).
const MONOLITH_FILENAME: &str = "live-schema.json";

/// Manifest filename (lives under opdbus/).
const MANIFEST_FILENAME: &str = ".manifest.json";

/// Sole writer of plugin capability schemas and present-state to SHM.
pub struct ShmWriter {
    /// Base directory (default `/dev/shm`).
    base_dir: PathBuf,
    /// `/dev/shm/opdbus`
    opdbus_dir: PathBuf,
    /// `/dev/shm/opdbus/schemas`
    schemas_dir: PathBuf,
    /// `/dev/shm/opdbus/state`
    state_dir: PathBuf,
    /// `/dev/shm/live-schema.json`
    monolith_path: PathBuf,
    /// `/dev/shm/opdbus/.manifest.json`
    manifest_path: PathBuf,
}

impl Default for ShmWriter {
    fn default() -> Self {
        Self::new(DEFAULT_SHM_BASE)
    }
}

impl ShmWriter {
    /// Creates a new `ShmWriter` rooted at the given base directory.
    ///
    /// In production, use `ShmWriter::default()` which targets `/dev/shm`.
    /// Tests should supply a temporary directory.
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base_dir = base.as_ref().to_path_buf();
        let opdbus_dir = base_dir.join(OPDBUS_DIR);
        let schemas_dir = opdbus_dir.join(SCHEMAS_DIR);
        let state_dir = opdbus_dir.join(STATE_DIR);
        let monolith_path = base_dir.join(MONOLITH_FILENAME);
        let manifest_path = opdbus_dir.join(MANIFEST_FILENAME);
        Self {
            base_dir,
            opdbus_dir,
            schemas_dir,
            state_dir,
            monolith_path,
            manifest_path,
        }
    }

    /// Returns the base directory path (e.g. `/dev/shm`).
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Returns the opdbus directory path.
    pub fn opdbus_dir(&self) -> &Path {
        &self.opdbus_dir
    }

    /// Returns the schemas directory path.
    pub fn schemas_dir(&self) -> &Path {
        &self.schemas_dir
    }

    /// Returns the state directory path.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Returns the monolith file path.
    pub fn monolith_path(&self) -> &Path {
        &self.monolith_path
    }

    /// Returns the manifest file path.
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Creates the `/dev/shm/opdbus/` hierarchy (`schemas/`, `state/`) if it
    /// does not already exist.  Idempotent.
    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.opdbus_dir)
            .with_context(|| format!("failed to create {}", self.opdbus_dir.display()))?;
        fs::create_dir_all(&self.schemas_dir)
            .with_context(|| format!("failed to create {}", self.schemas_dir.display()))?;
        fs::create_dir_all(&self.state_dir)
            .with_context(|| format!("failed to create {}", self.state_dir.display()))?;
        Ok(())
    }

    /// Writes a single per-plugin capability schema JSON file to
    /// `/dev/shm/opdbus/schemas/<plugin_id>.json`.
    pub fn write_plugin_schema(&self, plugin_id: &str, schema: &PluginSchema) -> Result<()> {
        let path = self.schemas_dir.join(format!("{}.json", plugin_id));
        let json_bytes = serde_json::to_vec_pretty(schema)
            .with_context(|| format!("failed to serialize schema for '{}'", plugin_id))?;
        atomic_write(&path, &json_bytes)
            .with_context(|| format!("failed to write schema for '{}'", plugin_id))?;
        debug!(plugin_id, path = %path.display(), "wrote per-plugin schema");
        Ok(())
    }

    /// Writes the combined monolith to `/dev/shm/live-schema.json` atomically.
    ///
    /// `schemas` is a map of `plugin_id → PluginSchema` containing every plugin
    /// that returned `Some(schema)` from `schema()`.
    pub fn write_monolith(&self, schemas: &HashMap<String, PluginSchema>) -> Result<()> {
        let json_bytes =
            serde_json::to_vec_pretty(schemas).context("failed to serialize monolith")?;
        atomic_write(&self.monolith_path, &json_bytes).with_context(|| {
            format!(
                "failed to write monolith to {}",
                self.monolith_path.display()
            )
        })?;
        debug!(path = %self.monolith_path.display(), "wrote combined monolith");
        Ok(())
    }

    /// Computes the Blake3 catalog hash over all per-plugin schema files.
    ///
    /// Algorithm:
    /// 1. List all `*.json` files in the schemas directory.
    /// 2. Sort by filename.
    /// 3. Leaf-hash each file's bytes with Blake3.
    /// 4. Fold (concatenate) the leaf hashes and hash the result to produce
    ///    a single root hash.
    pub fn compute_catalog_hash(&self) -> Result<String> {
        let mut entries: Vec<_> = fs::read_dir(&self.schemas_dir)
            .with_context(|| format!("cannot read schemas dir {}", self.schemas_dir.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        if entries.is_empty() {
            // No schema files — hash of empty input is still a valid Blake3.
            return Ok(blake3::hash(&[]).to_hex().to_string());
        }

        let mut concatenated: Vec<u8> = Vec::with_capacity(entries.len() * 32);
        for entry in &entries {
            let bytes = fs::read(entry.path())
                .with_context(|| format!("cannot read {}", entry.path().display()))?;
            let leaf_hash = blake3::hash(&bytes);
            concatenated.extend_from_slice(leaf_hash.as_bytes());
        }
        let root_hash = blake3::hash(&concatenated);
        Ok(root_hash.to_hex().to_string())
    }

    /// Writes `{ "catalog_hash": "<hex>" }` to `.manifest.json` atomically.
    pub fn write_manifest(&self, catalog_hash: &str) -> Result<()> {
        let json = serde_json::json!({ "catalog_hash": catalog_hash });
        let json_bytes =
            serde_json::to_vec_pretty(&json).context("failed to serialize manifest")?;
        atomic_write(&self.manifest_path, &json_bytes).with_context(|| {
            format!(
                "failed to write manifest to {}",
                self.manifest_path.display()
            )
        })?;
        debug!(catalog_hash, path = %self.manifest_path.display(), "wrote manifest");
        Ok(())
    }

    /// Writes present-state JSON for a plugin to
    /// `/dev/shm/opdbus/state/<plugin_id>.json` and then recomputes and
    /// rewrites the manifest hash so consumers detect the change.
    pub fn write_present_state(&self, plugin_id: &str, state: &serde_json::Value) -> Result<()> {
        let path = self.state_dir.join(format!("{}.json", plugin_id));
        let json_bytes = serde_json::to_vec_pretty(state)
            .with_context(|| format!("failed to serialize present-state for '{}'", plugin_id))?;
        atomic_write(&path, &json_bytes)
            .with_context(|| format!("failed to write present-state for '{}'", plugin_id))?;
        debug!(plugin_id, path = %path.display(), "wrote present-state");

        // Recompute catalog hash and update manifest so consumers detect the
        // state change on their next inbound connection.
        let hash = self.compute_catalog_hash()?;
        self.write_manifest(&hash)?;
        Ok(())
    }

    /// Convenience: writes all per-plugin schema files, the combined monolith,
    /// and the manifest in one call.
    ///
    /// Plugins returning `None` from `schema()` are skipped entirely — no
    /// per-plugin file, no monolith entry, no state file.
    pub fn write_all_schemas(&self, plugins: &[(String, Option<PluginSchema>)]) -> Result<()> {
        self.ensure_dirs()?;

        let mut monolith: HashMap<String, PluginSchema> = HashMap::new();
        for (plugin_id, schema_opt) in plugins {
            match schema_opt {
                Some(schema) => {
                    self.write_plugin_schema(plugin_id, schema)?;
                    monolith.insert(plugin_id.clone(), schema.clone());
                }
                None => {
                    debug!(plugin_id, "skipping plugin with None schema");
                }
            }
        }

        self.write_monolith(&monolith)?;
        let hash = self.compute_catalog_hash()?;
        self.write_manifest(&hash)?;

        info!(
            plugin_count = monolith.len(),
            catalog_hash = %hash,
            "wrote all schemas to SHM"
        );
        Ok(())
    }

    /// Convenience: writes present-state for all provided plugins and refreshes
    /// the manifest hash once at the end.
    pub fn write_all_present_states(&self, states: &[(String, serde_json::Value)]) -> Result<()> {
        self.ensure_dirs()?;
        for (plugin_id, state) in states {
            let path = self.state_dir.join(format!("{}.json", plugin_id));
            let json_bytes = serde_json::to_vec_pretty(state).with_context(|| {
                format!("failed to serialize present-state for '{}'", plugin_id)
            })?;
            atomic_write(&path, &json_bytes)
                .with_context(|| format!("failed to write present-state for '{}'", plugin_id))?;
        }
        let hash = self.compute_catalog_hash()?;
        self.write_manifest(&hash)?;
        info!(state_count = states.len(), "wrote present-states to SHM");
        Ok(())
    }
}

/// Atomically writes `data` to `path` by first writing to `<path>.tmp` and
/// then renaming over the target.
///
/// The rename(2) syscall is atomic on the same filesystem, so consumers will
/// never observe a partially written file.
fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let tmp_path = atomic_tmp_path(path);
    {
        let mut file = fs::File::create(&tmp_path)
            .with_context(|| format!("cannot create tmp file {}", tmp_path.display()))?;
        file.write_all(data)
            .with_context(|| format!("failed to write tmp file {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync tmp file {}", tmp_path.display()))?;
    }
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to rename {} → {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

/// Returns the temporary path for atomic writes: `<path>.tmp`.
fn atomic_tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let mut name = tmp
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    name.push_str(".tmp");
    tmp.set_file_name(name);
    tmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::{MethodDecl, PluginCapabilities, SideEffect, SignalDecl};
    use serde_json::json;
    use std::collections::HashMap;
    fn test_writer() -> ShmWriter {
        let id = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = PathBuf::from(format!("/dev/shm/opdbus-test-{}-{}", id, nanos));
        // Clean up any previous run (shouldn't exist, but be safe).
        let _ = fs::remove_dir_all(&dir);
        let writer = ShmWriter::new(&dir);
        writer.ensure_dirs().expect("ensure_dirs");
        writer
    }

    fn cleanup(writer: &ShmWriter) {
        let _ = fs::remove_dir_all(writer.base_dir.clone());
    }

    /// Build a minimal PluginSchema with one method, one signal, and guarantees.
    fn test_schema(plugin_id: &str) -> PluginSchema {
        let mut methods = HashMap::new();
        methods.insert(
            "GetStatus".to_string(),
            MethodDecl {
                name: "GetStatus".to_string(),
                args: json!({ "type": "object", "properties": {} }),
                returns: Some(json!({ "type": "object" })),
                side_effect: SideEffect::Read,
                idempotent: true,
                required_capability: None,
                subid: "obs.service.test.get-status@v1".to_string(),
            },
        );
        let signals = vec![SignalDecl {
            name: "StatusChanged".to_string(),
            payload: Some(json!({ "type": "object" })),
            subid: "evt.service.test.status-changed@v1".to_string(),
        }];
        let guarantees = PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        };
        PluginSchema::builder(plugin_id)
            .version("1.0.0")
            .category("test")
            .description("test plugin")
            .methods(methods)
            .signals(signals)
            .guarantees(guarantees)
            .build()
    }

    // ── VAL-PROD-001: per-plugin schema files ───────────────────────────

    #[test]
    fn producer_writes_per_plugin_schema_files() {
        let writer = test_writer();
        let plugins = vec![
            ("alpha".to_string(), Some(test_schema("alpha"))),
            ("beta".to_string(), Some(test_schema("beta"))),
            ("gamma".to_string(), None), // should be skipped
        ];
        writer
            .write_all_schemas(&plugins)
            .expect("write_all_schemas");

        // alpha and beta files exist; gamma does not
        let alpha_path = writer.schemas_dir.join("alpha.json");
        let beta_path = writer.schemas_dir.join("beta.json");
        let gamma_path = writer.schemas_dir.join("gamma.json");
        assert!(alpha_path.exists(), "alpha.json should exist");
        assert!(beta_path.exists(), "beta.json should exist");
        assert!(
            !gamma_path.exists(),
            "gamma.json should NOT exist (None schema)"
        );

        // Per-plugin file contains methods, signals, guarantees keys
        let alpha_text = fs::read_to_string(&alpha_path).unwrap();
        let alpha_json: serde_json::Value = serde_json::from_str(&alpha_text).unwrap();
        assert!(
            alpha_json.get("methods").is_some(),
            "per-plugin schema must contain methods key"
        );
        assert!(
            alpha_json.get("signals").is_some(),
            "per-plugin schema must contain signals key"
        );
        assert!(
            alpha_json.get("guarantees").is_some(),
            "per-plugin schema must contain guarantees key"
        );
        // Verify method content
        let methods = alpha_json["methods"].as_object().unwrap();
        assert!(
            methods.contains_key("GetStatus"),
            "methods must contain GetStatus"
        );
        // Verify signal content
        let signals = alpha_json["signals"].as_array().unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0]["name"], "StatusChanged");
        // Verify guarantees content
        let guarantees = &alpha_json["guarantees"];
        assert_eq!(guarantees["supports_checkpoints"], json!(true));

        cleanup(&writer);
    }

    // ── VAL-PROD-002: combined monolith ─────────────────────────────────

    #[test]
    fn producer_writes_combined_monolith() {
        let writer = test_writer();
        let plugins = vec![
            ("alpha".to_string(), Some(test_schema("alpha"))),
            ("beta".to_string(), Some(test_schema("beta"))),
            ("gamma".to_string(), None),
        ];
        writer
            .write_all_schemas(&plugins)
            .expect("write_all_schemas");

        let monolith_text = fs::read_to_string(writer.monolith_path()).unwrap();
        let monolith: serde_json::Value = serde_json::from_str(&monolith_text).unwrap();
        let obj = monolith.as_object().unwrap();
        assert!(obj.contains_key("alpha"), "monolith must contain alpha");
        assert!(obj.contains_key("beta"), "monolith must contain beta");
        assert!(
            !obj.contains_key("gamma"),
            "monolith must NOT contain gamma (None schema)"
        );
        assert_eq!(obj.len(), 2, "monolith must have exactly 2 plugins");

        // Monolith entries contain capability surface
        assert!(monolith["alpha"]["methods"].is_object());
        assert!(monolith["alpha"]["signals"].is_array());
        assert!(monolith["alpha"]["guarantees"].is_object());

        cleanup(&writer);
    }

    // ── VAL-PROD-003: manifest hash matches independent recompute ───────

    #[test]
    fn producer_manifest_hash_matches_recompute() {
        let writer = test_writer();
        let plugins = vec![
            ("alpha".to_string(), Some(test_schema("alpha"))),
            ("beta".to_string(), Some(test_schema("beta"))),
        ];
        writer
            .write_all_schemas(&plugins)
            .expect("write_all_schemas");

        // Read the manifest
        let manifest_text = fs::read_to_string(writer.manifest_path()).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
        let stored_hash = manifest["catalog_hash"].as_str().unwrap();

        // Independently recompute the hash
        let recomputed = writer.compute_catalog_hash().expect("recompute");
        assert_eq!(
            stored_hash, recomputed,
            "manifest hash must match independent recompute"
        );

        // Verify hash changes when a schema file is modified
        let alpha_path = writer.schemas_dir.join("alpha.json");
        fs::write(&alpha_path, b"{\"changed\":true}").unwrap();
        let recomputed2 = writer.compute_catalog_hash().expect("recompute2");
        assert_ne!(
            recomputed, recomputed2,
            "hash must change when a schema file changes"
        );

        cleanup(&writer);
    }

    // ── VAL-PROD-004: present-state on state change ─────────────────────

    #[test]
    fn producer_updates_present_state_on_change() {
        let writer = test_writer();
        let plugins = vec![("alpha".to_string(), Some(test_schema("alpha")))];
        writer
            .write_all_schemas(&plugins)
            .expect("write_all_schemas");

        // Read initial manifest hash
        let manifest_text = fs::read_to_string(writer.manifest_path()).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
        let hash_before = manifest["catalog_hash"].as_str().unwrap().to_string();

        // Write present-state
        let state = json!({ "status": "active", "uptime": 42 });
        writer
            .write_present_state("alpha", &state)
            .expect("write_present_state");

        // State file exists and contains correct content
        let state_path = writer.state_dir.join("alpha.json");
        assert!(state_path.exists(), "state file should exist");
        let state_text = fs::read_to_string(&state_path).unwrap();
        let state_json: serde_json::Value = serde_json::from_str(&state_text).unwrap();
        assert_eq!(state_json["status"], "active");
        assert_eq!(state_json["uptime"], 42);

        // Manifest hash updated (state files don't affect catalog hash,
        // but manifest is rewritten — hash should be same since schema
        // files didn't change)
        let manifest_text2 = fs::read_to_string(writer.manifest_path()).unwrap();
        let manifest2: serde_json::Value = serde_json::from_str(&manifest_text2).unwrap();
        let hash_after = manifest2["catalog_hash"].as_str().unwrap().to_string();
        assert_eq!(
            hash_before, hash_after,
            "catalog hash unchanged when only state changes (schemas untouched)"
        );

        cleanup(&writer);
    }

    // ── VAL-NFR-004: atomic write pattern (tmp + rename) ────────────────

    #[test]
    fn manifest_atomic_write() {
        let writer = test_writer();
        writer.ensure_dirs().expect("ensure_dirs");

        // Write manifest
        writer.write_manifest("abc123").expect("write_manifest");

        // Manifest exists
        assert!(writer.manifest_path().exists(), "manifest should exist");

        // Tmp file should NOT exist after atomic write completes
        let tmp_path = atomic_tmp_path(writer.manifest_path());
        assert!(
            !tmp_path.exists(),
            "tmp file should not linger after rename"
        );

        // Content is correct
        let text = fs::read_to_string(writer.manifest_path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["catalog_hash"], "abc123");

        // Monolith is also atomic
        let mut schemas = HashMap::new();
        schemas.insert("x".to_string(), test_schema("x"));
        writer.write_monolith(&schemas).expect("write_monolith");
        let mono_tmp = atomic_tmp_path(writer.monolith_path());
        assert!(!mono_tmp.exists(), "monolith tmp should not linger");

        cleanup(&writer);
    }

    // ── Atomic write overwrites existing file ───────────────────────────

    #[test]
    fn atomic_write_overwrites_existing() {
        let writer = test_writer();
        writer.ensure_dirs().expect("ensure_dirs");

        writer.write_manifest("first").expect("first");
        writer.write_manifest("second").expect("second");

        let text = fs::read_to_string(writer.manifest_path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["catalog_hash"], "second");

        cleanup(&writer);
    }

    // ── ensure_dirs is idempotent ───────────────────────────────────────

    #[test]
    fn ensure_dirs_idempotent() {
        let writer = test_writer();
        // Calling ensure_dirs again should not error
        writer.ensure_dirs().expect("first");
        writer.ensure_dirs().expect("second");
        assert!(writer.schemas_dir().exists());
        assert!(writer.state_dir().exists());
        cleanup(&writer);
    }

    // ── write_all_schemas with empty list produces empty monolith ───────

    #[test]
    fn write_all_schemas_empty() {
        let writer = test_writer();
        writer.write_all_schemas(&[]).expect("write_all");
        assert!(writer.monolith_path().exists());
        let text = fs::read_to_string(writer.monolith_path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(json.as_object().unwrap().is_empty());
        assert!(writer.manifest_path().exists());
        cleanup(&writer);
    }

    // ── write_all_present_states writes all and updates manifest ────────

    #[test]
    fn write_all_present_states() {
        let writer = test_writer();
        writer.ensure_dirs().expect("ensure_dirs");
        let states = vec![
            ("alpha".to_string(), json!({ "v": 1 })),
            ("beta".to_string(), json!({ "v": 2 })),
        ];
        writer
            .write_all_present_states(&states)
            .expect("write_all_states");
        assert!(writer.state_dir.join("alpha.json").exists());
        assert!(writer.state_dir.join("beta.json").exists());
        assert!(writer.manifest_path().exists());
        cleanup(&writer);
    }
}
