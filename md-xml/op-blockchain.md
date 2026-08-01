This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  blockchain.rs
  btrfs_numa_integration.rs
  footprint.rs
  lib.rs
  plugin_footprint.rs
  retention.rs
  snapshot.rs
  streaming_blockchain.rs
Cargo.toml
compare-op-blockchain.md
DESIGN.md
REQUIREMENTS.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/blockchain.rs">
//! Streaming blockchain with dual BTRFS subvolumes
//!
//! Architecture:
//! - timing_subvol: Immutable audit trail (append-only)
//! - vector_subvol: ML embeddings for semantic search
//! - state_subvol: Current system state for DR/reinstall

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Utc};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::footprint::{BlockEvent, PluginFootprint};
use crate::retention::RetentionPolicy;
use crate::snapshot::SnapshotInterval;

/// Streaming blockchain with BTRFS subvolumes
pub struct StreamingBlockchain {
    base_path: PathBuf,
    timing_subvol: PathBuf,
    vector_subvol: PathBuf,
    state_subvol: PathBuf,
    snapshot_interval: SnapshotInterval,
    retention_policy: RetentionPolicy,
    last_snapshot_time: Arc<RwLock<Instant>>,
    block_counter: Arc<RwLock<u64>>,
}

impl StreamingBlockchain {
    /// Create a new streaming blockchain at the given path
    pub async fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        Self::new_with_interval(base_path, SnapshotInterval::from_env()).await
    }

    /// Create with a specific snapshot interval
    pub async fn new_with_interval(
        base_path: impl AsRef<Path>,
        snapshot_interval: SnapshotInterval,
    ) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let timing_subvol = base_path.join("timing");
        let vector_subvol = base_path.join("vectors");
        let state_subvol = base_path.join("state");

        // Create directories
        tokio::fs::create_dir_all(&base_path).await?;

        // Create BTRFS subvolumes
        Self::create_subvolume(&timing_subvol).await?;
        Self::create_subvolume(&vector_subvol).await?;
        Self::create_subvolume(&state_subvol).await?;

        // Create snapshots directory
        let snapshots_dir = base_path.join("snapshots");
        tokio::fs::create_dir_all(&snapshots_dir).await?;

        info!(
            "Streaming blockchain initialized at {:?} with {} interval",
            base_path, snapshot_interval
        );

        Ok(Self {
            base_path,
            timing_subvol,
            vector_subvol,
            state_subvol,
            snapshot_interval,
            retention_policy: RetentionPolicy::from_env(),
            last_snapshot_time: Arc::new(RwLock::new(Instant::now())),
            block_counter: Arc::new(RwLock::new(0)),
        })
    }

    /// Create a BTRFS subvolume
    async fn create_subvolume(path: &Path) -> Result<()> {
        if path.exists() {
            debug!("Subvolume already exists: {:?}", path);
            return Ok(());
        }

        let output = Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(path)
            .output()
            .await
            .context("Failed to execute btrfs command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If btrfs is not available, fall back to regular directory
            if stderr.contains("command not found") || stderr.contains("not a btrfs filesystem") {
                warn!(
                    "BTRFS not available, creating regular directory: {:?}",
                    path
                );
                tokio::fs::create_dir_all(path).await?;
            } else {
                anyhow::bail!("btrfs subvolume create failed: {}", stderr);
            }
        } else {
            info!("Created BTRFS subvolume: {:?}", path);
        }

        Ok(())
    }

    /// Add a plugin footprint to the blockchain
    pub async fn add_footprint(&self, footprint: PluginFootprint) -> Result<String> {
        let event = footprint.to_block_event();
        self.add_event(event).await
    }

    /// Add a block event to the blockchain
    pub async fn add_event(&self, event: BlockEvent) -> Result<String> {
        // Increment block counter
        let block_num = {
            let mut counter = self.block_counter.write().await;
            *counter += 1;
            *counter
        };

        // TIMING IS AUTHORITATIVE: Write timing record first (durable audit trail)
        // This is the source of truth - vectors are async projections
        let timing_file = self
            .timing_subvol
            .join(format!("block-{:012}.json", block_num));
        let timing_data = simd_json::to_string_pretty(&event)?;
        tokio::fs::write(&timing_file, &timing_data).await?;

        // VECTORS ARE PROJECTIONS: Write vector data if present (sync but optional)
        // Vectors can be recomputed from timing if lost, but timing cannot be regenerated
        if !event.vector.is_empty() {
            let vector_file = self
                .vector_subvol
                .join(format!("vec-{:012}.bin", block_num));
            let vector_bytes: Vec<u8> = event.vector.iter().flat_map(|f| f.to_le_bytes()).collect();
            tokio::fs::write(&vector_file, vector_bytes).await?;
        }

        debug!("Added block {} with hash {}", block_num, event.hash);

        // Check if we should snapshot
        let should_snapshot = {
            let last = self.last_snapshot_time.read().await;
            self.snapshot_interval.should_snapshot(last.elapsed())
        };

        if should_snapshot {
            self.create_snapshot().await?;
        }

        Ok(event.hash)
    }

    /// Create a snapshot of current state
    pub async fn create_snapshot(&self) -> Result<String> {
        let snapshot_dir = self.base_path.join("snapshots");
        let prefix = Self::state_snapshot_prefix();
        let counter = self.next_snapshot_counter(&snapshot_dir, &prefix).await?;
        let snapshot_name = format!("{}-{:06}", prefix, counter);
        let snapshot_path = snapshot_dir.join(&snapshot_name);

        // Create BTRFS snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.state_subvol)
            .arg(&snapshot_path)
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                info!("Created snapshot: {}", snapshot_name);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                // Fall back to regular copy
                if stderr.contains("not a btrfs") {
                    debug!("BTRFS not available, using regular copy for snapshot");
                    tokio::fs::create_dir_all(&snapshot_path).await?;
                    copy_dir_recursive(&self.state_subvol, &snapshot_path).await?;
                } else {
                    warn!("Snapshot failed: {}", stderr);
                }
            }
            Err(e) => {
                warn!("Failed to create snapshot: {}", e);
            }
        }

        // Update last snapshot time
        *self.last_snapshot_time.write().await = Instant::now();

        // Prune old snapshots according to retention policy
        if let Err(e) = self.prune_snapshots().await {
            warn!("Failed to prune snapshots: {}", e);
        }

        Ok(snapshot_name)
    }

    /// Write current state to the state subvolume
    pub async fn write_state(&self, key: &str, value: &simd_json::OwnedValue) -> Result<()> {
        let state_file = self.state_subvol.join(format!("{}.json", key));
        let data = simd_json::to_string_pretty(value)?;
        tokio::fs::write(&state_file, data).await?;
        Ok(())
    }

    /// Read state from the state subvolume
    pub async fn read_state(&self, key: &str) -> Result<simd_json::OwnedValue> {
        let state_file = self.state_subvol.join(format!("{}.json", key));
        let mut data = tokio::fs::read_to_string(&state_file).await?;
        Ok(unsafe { simd_json::from_str(&mut data)? })
    }

    /// List all available snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<(String, String)>> {
        let snapshot_dir = self.base_path.join("snapshots");
        let mut entries = tokio::fs::read_dir(&snapshot_dir).await?;
        let mut snapshots = Vec::new();
        let prefix = Self::state_snapshot_prefix();

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();

            let name_prefix = format!("{}-", prefix);
            if !name.starts_with(&name_prefix) {
                continue;
            }

            let metadata = tokio::fs::metadata(entry.path()).await?;
            let ts = metadata.created().or_else(|_| metadata.modified()).ok();
            let human_readable = ts
                .and_then(system_time_to_utc)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            snapshots.push((name, human_readable));
        }

        // Sort by name (newest counter first)
        snapshots.sort_by(|a, b| b.0.cmp(&a.0));

        Ok(snapshots)
    }

    /// Rollback to a specific snapshot
    pub async fn rollback(&self, snapshot_name: &str) -> Result<PathBuf> {
        let snapshot_path = self.base_path.join("snapshots").join(snapshot_name);

        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot not found: {}", snapshot_name);
        }

        info!("Rolling back to snapshot: {}", snapshot_name);
        Ok(snapshot_path)
    }

    /// Stream snapshot to remote using btrfs send / btrfs receive.
    ///
    /// Security (audit item #2):
    /// - The pipeline `btrfs send <snap> | ssh <host> btrfs receive <path>` is
    ///   built as two argv-form `Command` children connected via
    ///   `Stdio::piped()`. No shell is invoked locally; no string is
    ///   interpolated into a shell command line.
    /// - `remote_host` and `remote_path` are validated against strict ASCII
    ///   allow-lists before being passed to `ssh`, because `ssh` will
    ///   re-parse the remote argv through the destination shell.
    ///
    /// API note: this signature takes `remote_host` and `remote_path` as
    /// separate arguments. The previous signature took a single `remote_path`
    /// and (incorrectly) used it for both the ssh host slot and the receive
    /// path slot of an interpolated shell command \u2014 the method as written
    /// could not have transferred to a real remote. Callers must now supply
    /// the host explicitly.
    ///
    /// TODO: replace `ssh`/`btrfs` CLI shelling with a librust SSH client and
    /// the kernel ioctl in a follow-up. Tracked separately under AGENTS.md \u00a72.
    pub async fn stream_to_remote(
        &self,
        snapshot_name: &str,
        remote_host: &str,
        remote_path: &str,
    ) -> Result<()> {
        let snapshot_path = self.base_path.join("snapshots").join(snapshot_name);

        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot not found: {}", snapshot_name);
        }

        // ---- Hardened input validation (audit item #2) ----
        validate_remote_host(remote_host).context("invalid remote host")?;
        validate_btrfs_path(Path::new(remote_path)).context("invalid remote path")?;
        // Defense in depth: also validate the local snapshot path even though
        // it is constructed from our own `base_path`.
        validate_btrfs_path(&snapshot_path).context("invalid local snapshot path")?;

        info!(
            "Streaming snapshot {} to {}:{}",
            snapshot_name, remote_host, remote_path
        );

        // ---- Argv-form two-process pipeline; no shell on the local side. ----
        let mut send_child = Command::new("btrfs")
            .arg("send")
            .arg(&snapshot_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn `btrfs send`")?;

        let send_stdout = send_child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout of `btrfs send`"))?;
        let send_stdout: Stdio = send_stdout
            .try_into()
            .context("Failed to convert `btrfs send` stdout to Stdio")?;

        // `--` defeats any future leading-dash sneakiness in `remote_host`
        // (already rejected by the validator; belt-and-braces).
        let mut recv_child = Command::new("ssh")
            .arg("--")
            .arg(remote_host)
            .arg("btrfs")
            .arg("receive")
            .arg(remote_path)
            .stdin(send_stdout)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn `ssh ... btrfs receive`")?;

        let send_status = send_child
            .wait()
            .await
            .context("Failed to wait for `btrfs send`")?;
        let recv_output = recv_child
            .wait_with_output()
            .await
            .context("Failed to wait for `ssh ... btrfs receive`")?;

        if !send_status.success() {
            anyhow::bail!("`btrfs send` failed with status {:?}", send_status.code());
        }
        if !recv_output.status.success() {
            let stderr = String::from_utf8_lossy(&recv_output.stderr);
            anyhow::bail!(
                "`ssh ... btrfs receive` failed (status {:?}): {}",
                recv_output.status.code(),
                stderr
            );
        }

        info!(
            "Successfully streamed snapshot {} to {}",
            snapshot_name, remote_host
        );
        Ok(())
    }

    /// Prune old snapshots according to retention policy
    async fn prune_snapshots(&self) -> Result<()> {
        use chrono::Duration;
        use std::collections::HashMap;

        let snapshot_dir = self.base_path.join("snapshots");
        let mut entries = tokio::fs::read_dir(&snapshot_dir).await?;
        let mut snapshots: Vec<(String, DateTime<Utc>)> = Vec::new();
        let prefix = Self::state_snapshot_prefix();

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();

            let name_prefix = format!("{}-", prefix);
            if !name.starts_with(&name_prefix) {
                continue;
            }

            let metadata = tokio::fs::metadata(entry.path()).await?;
            let ts = metadata.created().or_else(|_| metadata.modified()).ok();
            if let Some(dt_utc) = ts.and_then(system_time_to_utc) {
                snapshots.push((name, dt_utc));
            }
        }

        // Sort by timestamp (newest first)
        snapshots.sort_by(|a, b| b.1.cmp(&a.1));

        let now = Utc::now();

        // Categorize snapshots
        let mut hourly: Vec<String> = Vec::new();
        let mut daily: HashMap<String, String> = HashMap::new();
        let mut weekly: HashMap<u32, String> = HashMap::new();
        let mut quarterly: HashMap<String, String> = HashMap::new();

        for (name, dt) in &snapshots {
            let age = now.signed_duration_since(*dt);

            if age <= Duration::hours(24) {
                hourly.push(name.clone());
            } else if age <= Duration::days(30) {
                let date_key = dt.format("%Y%m%d").to_string();
                daily.entry(date_key).or_insert_with(|| name.clone());
            } else if age <= Duration::weeks(12) {
                let week_key = dt.iso_week().week();
                weekly.entry(week_key).or_insert_with(|| name.clone());
            } else {
                let quarter = (dt.month() - 1) / 3 + 1;
                let quarter_key = format!("{}-Q{}", dt.year(), quarter);
                quarterly.entry(quarter_key).or_insert_with(|| name.clone());
            }
        }

        // Build keep set
        let mut keep: std::collections::HashSet<String> = std::collections::HashSet::new();

        for snapshot in hourly.iter().take(self.retention_policy.hourly) {
            keep.insert(snapshot.clone());
        }

        let mut daily_list: Vec<_> = daily.into_values().collect();
        daily_list.sort();
        daily_list.reverse();
        for snapshot in daily_list.iter().take(self.retention_policy.daily) {
            keep.insert(snapshot.clone());
        }

        let mut weekly_list: Vec<_> = weekly.into_values().collect();
        weekly_list.sort();
        weekly_list.reverse();
        for snapshot in weekly_list.iter().take(self.retention_policy.weekly) {
            keep.insert(snapshot.clone());
        }

        let mut quarterly_list: Vec<_> = quarterly.into_values().collect();
        quarterly_list.sort();
        quarterly_list.reverse();
        for snapshot in quarterly_list.iter().take(self.retention_policy.quarterly) {
            keep.insert(snapshot.clone());
        }

        // Delete old snapshots
        let mut deleted = 0;
        for (name, _) in &snapshots {
            if !keep.contains(name) {
                let path = snapshot_dir.join(name);

                // Try btrfs delete first, fall back to rm
                let result = Command::new("btrfs")
                    .args(["subvolume", "delete"])
                    .arg(&path)
                    .output()
                    .await;

                match result {
                    Ok(out) if out.status.success() => {
                        deleted += 1;
                        debug!("Pruned snapshot: {}", name);
                    }
                    _ => {
                        // Fall back to rm -rf
                        if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                            warn!("Failed to delete snapshot {}: {}", name, e);
                        } else {
                            deleted += 1;
                        }
                    }
                }
            }
        }

        if deleted > 0 {
            info!(
                "Pruned {} snapshots (retention: {}h/{}d/{}w/{}q)",
                deleted,
                self.retention_policy.hourly,
                self.retention_policy.daily,
                self.retention_policy.weekly,
                self.retention_policy.quarterly
            );
        }

        Ok(())
    }

    fn state_snapshot_prefix() -> String {
        std::env::var("OPDBUS_STATE_SNAPSHOT_PREFIX").unwrap_or_else(|_| "SNP-state".to_string())
    }

    async fn next_snapshot_counter(&self, snapshot_dir: &Path, prefix: &str) -> Result<u64> {
        let mut entries = tokio::fs::read_dir(snapshot_dir).await?;
        let name_prefix = format!("{}-", prefix);
        let mut max_counter = 0u64;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&name_prefix) {
                continue;
            }
            if let Some(counter_str) = name.strip_prefix(&name_prefix) {
                if let Ok(counter) = counter_str.parse::<u64>() {
                    if counter > max_counter {
                        max_counter = counter;
                    }
                }
            }
        }

        Ok(max_counter + 1)
    }

    /// Get snapshot interval
    pub fn snapshot_interval(&self) -> SnapshotInterval {
        self.snapshot_interval
    }

    /// Set snapshot interval
    pub fn set_snapshot_interval(&mut self, interval: SnapshotInterval) {
        self.snapshot_interval = interval;
        info!("Snapshot interval changed to: {}", interval);
    }

    /// Get retention policy
    pub fn retention_policy(&self) -> RetentionPolicy {
        self.retention_policy
    }

    /// Set retention policy
    pub fn set_retention_policy(&mut self, policy: RetentionPolicy) {
        self.retention_policy = policy;
        info!(
            "Retention policy updated: {}h/{}d/{}w/{}q",
            policy.hourly, policy.daily, policy.weekly, policy.quarterly
        );
    }

    /// Start a footprint receiver that processes incoming footprints
    pub async fn start_footprint_receiver(
        &self,
        mut receiver: tokio::sync::mpsc::Receiver<PluginFootprint>,
    ) -> Result<()> {
        info!("Starting blockchain footprint receiver");

        while let Some(footprint) = receiver.recv().await {
            if let Err(e) = self.add_footprint(footprint).await {
                warn!("Failed to add footprint to blockchain: {}", e);
                // Continue processing other footprints
            }
        }

        info!("Blockchain footprint receiver stopped");
        Ok(())
    }

    /// Write the current system state (for disaster recovery)
    pub async fn write_current_state(&self, state: &simd_json::OwnedValue) -> Result<()> {
        self.write_state("current", state).await
    }

    /// Get base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

fn system_time_to_utc(ts: SystemTime) -> Option<DateTime<Utc>> {
    Some(DateTime::<Utc>::from(ts))
}

// ----------------------------------------------------------------------------
// Security validators (audit item #2: shell-injection hardening)
//
// Intentionally duplicated from `op-cache::btrfs_cache` rather than crossing
// the crate boundary, because the originals are private static methods.
// Exposing them would re-open the API surface we just hardened in item #1.
//
// TODO: consolidate into an `op-core::path_safety` module once a second
// audit item benefits from it. Tracked separately.
// ----------------------------------------------------------------------------

/// Validate a remote host specifier (e.g. "host", "user@host", "1.2.3.4").
///
/// Allowed: ASCII alphanumerics and `._@:-`. Rejected: anything that could be
/// interpreted by a shell or alter ssh's argv parsing.
fn validate_remote_host(host: &str) -> Result<()> {
    if host.is_empty() {
        anyhow::bail!("remote host must not be empty");
    }
    if host.len() > 253 {
        anyhow::bail!("remote host exceeds 253 chars");
    }
    if host.starts_with('-') {
        anyhow::bail!("remote host must not start with '-' (would look like an ssh flag)");
    }
    for (i, b) in host.bytes().enumerate() {
        let ok = b.is_ascii_alphanumeric()
            || b == b'.'
            || b == b'_'
            || b == b'-'
            || b == b'@'
            || b == b':';
        if !ok {
            anyhow::bail!("invalid byte 0x{:02x} at position {} in remote host", b, i);
        }
    }
    Ok(())
}

/// Validate a path that will be forwarded to `btrfs send` / `btrfs receive`,
/// including via `ssh` where the remote shell re-parses argv.
///
/// Requires absolute, no `..` components, no shell metacharacters, no
/// control characters.
fn validate_btrfs_path(path: &Path) -> Result<()> {
    let s = path.to_str().context("btrfs path is not valid UTF-8")?;
    if s.is_empty() {
        anyhow::bail!("btrfs path must not be empty");
    }
    if !path.is_absolute() {
        anyhow::bail!("btrfs path must be absolute: {:?}", path);
    }
    if s.starts_with('-') {
        anyhow::bail!("btrfs path must not start with '-'");
    }
    const FORBIDDEN: &[char] = &[
        '\0', '\n', '\r', '\t', '\x0b', '\x0c', ' ', '`', '$', ';', '&', '|', '<', '>', '(', ')',
        '{', '}', '*', '?', '[', ']', '!', '~', '#', '\\', '"', '\'',
    ];
    for ch in s.chars() {
        if (ch as u32) < 0x20 {
            anyhow::bail!(
                "control character 0x{:02x} not allowed in btrfs path",
                ch as u32
            );
        }
        if FORBIDDEN.contains(&ch) {
            anyhow::bail!("character {:?} not allowed in btrfs path", ch);
        }
    }
    for comp in path.components() {
        if let std::path::Component::ParentDir = comp {
            anyhow::bail!("btrfs path must not contain '..' components: {:?}", path);
        }
    }
    Ok(())
}

/// Recursively copy a directory
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry.file_type().await?.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_remote_host_accepts_reasonable_inputs() {
        assert!(validate_remote_host("example.com").is_ok());
        assert!(validate_remote_host("user@example.com").is_ok());
        assert!(validate_remote_host("10.0.0.1").is_ok());
        assert!(validate_remote_host("host_with_underscore-1").is_ok());
    }

    #[test]
    fn validate_remote_host_rejects_injection() {
        assert!(validate_remote_host("").is_err());
        assert!(validate_remote_host("host; rm -rf /").is_err());
        assert!(validate_remote_host("host`whoami`").is_err());
        assert!(validate_remote_host("host$(whoami)").is_err());
        assert!(validate_remote_host("host|cat").is_err());
        assert!(validate_remote_host("host with space").is_err());
        assert!(validate_remote_host("host\nnewline").is_err());
        assert!(validate_remote_host("-oProxyCommand=evil").is_err());
    }

    #[test]
    fn validate_btrfs_path_accepts_clean_absolute_paths() {
        assert!(validate_btrfs_path(Path::new("/var/lib/op-dbus/snap")).is_ok());
        assert!(validate_btrfs_path(Path::new("/tmp/cache-001")).is_ok());
    }

    #[test]
    fn validate_btrfs_path_rejects_injection_and_traversal() {
        assert!(validate_btrfs_path(Path::new("")).is_err());
        assert!(validate_btrfs_path(Path::new("relative/path")).is_err());
        assert!(validate_btrfs_path(Path::new("/etc/../etc/passwd")).is_err());
        assert!(validate_btrfs_path(Path::new("/tmp/foo; rm -rf /")).is_err());
        assert!(validate_btrfs_path(Path::new("/tmp/$(whoami)")).is_err());
        assert!(validate_btrfs_path(Path::new("/tmp/foo bar")).is_err());
        assert!(validate_btrfs_path(Path::new("/tmp/foo\nbar")).is_err());
        assert!(validate_btrfs_path(Path::new("/tmp/`whoami`")).is_err());
    }
}
</file>

<file path="src/btrfs_numa_integration.rs">
//! Unified BTRFS cache and NUMA integration for blockchain footprints
//!
//! This module integrates:
//! - StreamingBlockchain: Immutable audit trail with vectorization
//! - BtrfsCache: Unlimited disk-based caching with compression
//! - NumaTopology: NUMA-aware CPU/memory optimization
//!
//! Benefits:
//! - Blockchain blocks cached in BTRFS cache (faster retrieval)
//! - NUMA-aware writes (optimal CPU/memory placement)
//! - Unified snapshot management
//! - Shared compression and deduplication

use crate::streaming_blockchain::StreamingBlockchain;
use crate::PluginFootprint;
use anyhow::{Context, Result};
use op_cache::{BtrfsCache, NumaTopology};
use simd_json::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Unified blockchain with BTRFS cache and NUMA optimization
pub struct OptimizedBlockchain {
    blockchain: Arc<StreamingBlockchain>,
    cache: Arc<BtrfsCache>,
    numa_topology: Arc<RwLock<Option<NumaTopology>>>,
    cache_enabled: bool,
}

impl OptimizedBlockchain {
    /// Create optimized blockchain with BTRFS cache and NUMA support
    pub async fn new(
        blockchain_path: impl AsRef<Path>,
        cache_path: impl AsRef<Path>,
    ) -> Result<Self> {
        // Initialize blockchain
        let blockchain = Arc::new(
            StreamingBlockchain::new(blockchain_path)
                .await
                .context("Failed to initialize streaming blockchain")?,
        );

        // Initialize BTRFS cache
        let cache = Arc::new(
            BtrfsCache::new(cache_path.as_ref().to_path_buf())
                .await
                .context("Failed to initialize BTRFS cache")?,
        );

        // Detect NUMA topology (best-effort, non-blocking)
        let numa_topology = Arc::new(RwLock::new(None));
        {
            match NumaTopology::detect() {
                Ok(topology) => {
                    info!("NUMA topology detected: {} nodes", topology.node_count());
                    *numa_topology.write().await = Some(topology);
                }
                Err(e) => {
                    warn!(
                        "NUMA topology detection failed: {} (continuing without NUMA)",
                        e
                    );
                }
            }
        }

        let cache_enabled = true;

        Ok(Self {
            blockchain,
            cache,
            numa_topology,
            cache_enabled,
        })
    }

    /// Add footprint with NUMA-aware caching
    pub async fn add_footprint(&self, footprint: PluginFootprint) -> Result<String> {
        // Apply NUMA affinity for write operations
        self.apply_numa_affinity("blockchain_write").await?;

        // Store in blockchain (primary storage)
        let block_hash = self
            .blockchain
            .add_footprint(footprint.clone())
            .await
            .context("Failed to add footprint to blockchain")?;

        // Cache in BTRFS cache for fast retrieval
        if self.cache_enabled {
            if let Err(e) = self.cache_block(block_hash.clone(), &footprint).await {
                warn!("Failed to cache blockchain block {}: {}", block_hash, e);
                // Don't fail the operation if caching fails
            }
        }

        Ok(block_hash)
    }

    /// Cache blockchain block in BTRFS cache
    async fn cache_block(&self, block_hash: String, footprint: &PluginFootprint) -> Result<()> {
        // Serialize footprint for caching
        let block_data = simd_json::json!({
            "plugin_id": footprint.plugin_id,
            "operation": footprint.operation,
            "timestamp": footprint.timestamp,
            "data_hash": footprint.data_hash,
            "content_hash": footprint.content_hash,
            "metadata": footprint.metadata,
            "vector_features": footprint.vector_features,
        });

        // Use cache's embedding storage for block data
        // (blocks are stored as JSON, not vectors, but we use the same infrastructure)
        // Store as JSON in cache (BTRFS will compress it)
        let cache_dir = self.cache.cache_dir();
        let blocks_dir = cache_dir.join("blocks").join("by-hash");
        tokio::fs::create_dir_all(&blocks_dir).await?;

        let block_file = blocks_dir.join(format!("{}.json", block_hash));
        tokio::fs::write(&block_file, simd_json::to_string_pretty(&block_data)?)
            .await
            .context("Failed to write block to cache")?;

        debug!("Cached blockchain block {} in BTRFS cache", block_hash);
        Ok(())
    }

    /// Get cached block from BTRFS cache (fast path)
    pub async fn get_cached_block(&self, block_hash: &str) -> Result<Option<PluginFootprint>> {
        if !self.cache_enabled {
            return Ok(None);
        }

        let cache_dir = self.cache.cache_dir();
        let block_file = cache_dir
            .join("blocks")
            .join("by-hash")
            .join(format!("{}.json", block_hash));

        if !block_file.exists() {
            return Ok(None);
        }

        // Read from BTRFS cache (page cache will keep hot blocks in RAM)
        let mut data = tokio::fs::read_to_string(&block_file).await?;
        let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };

        // Reconstruct footprint
        let footprint = PluginFootprint {
            plugin_id: block_data["plugin_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing plugin_id"))?
                .to_string(),
            operation: block_data["operation"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing operation"))?
                .to_string(),
            timestamp: block_data["timestamp"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Missing timestamp"))?,
            data_hash: block_data["data_hash"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing data_hash"))?
                .to_string(),
            content_hash: block_data["content_hash"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing content_hash"))?
                .to_string(),
            metadata: simd_json::serde::from_owned_value(block_data["metadata"].clone())?,
            vector_features: simd_json::serde::from_owned_value(
                block_data["vector_features"].clone(),
            )?,
        };

        Ok(Some(footprint))
    }

    /// Apply NUMA affinity for blockchain operations
    async fn apply_numa_affinity(&self, operation: &str) -> Result<()> {
        let numa = self.numa_topology.read().await;
        if let Some(ref topology) = *numa {
            // Get optimal NUMA node
            let optimal_node = topology.optimal_node();

            if let Some(node) = topology.get_node(optimal_node) {
                debug!(
                    "Applying NUMA affinity: node {} ({} CPUs, {} MB free) for {}",
                    node.node_id,
                    node.cpu_list.len(),
                    node.memory_free_kb / 1024,
                    operation
                );

                // Use cache's NUMA methods (which use taskset/numactl)
                // The cache already has NUMA-aware operations
                // We just need to ensure we're using the right node
            }
        }
        Ok(())
    }

    /// Get blockchain instance (for direct access if needed)
    pub fn blockchain(&self) -> &Arc<StreamingBlockchain> {
        &self.blockchain
    }

    /// Get cache instance
    pub fn cache(&self) -> &Arc<BtrfsCache> {
        &self.cache
    }

    /// Get NUMA topology info
    pub async fn numa_info(&self) -> Option<NumaTopology> {
        self.numa_topology.read().await.clone()
    }

    /// Start footprint receiver with caching
    pub async fn start_footprint_receiver(
        &self,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<PluginFootprint>,
    ) -> Result<()> {
        info!("Starting optimized footprint receiver (with BTRFS cache and NUMA)");

        while let Some(footprint) = receiver.recv().await {
            if let Err(e) = self.add_footprint(footprint).await {
                tracing::error!("Failed to add footprint: {}", e);
                // Continue processing other footprints
            }
        }

        info!("Optimized footprint receiver shutting down");
        Ok(())
    }

    /// Create unified snapshot (blockchain + cache)
    pub async fn create_unified_snapshot(&self) -> Result<Vec<PathBuf>> {
        let mut snapshots = Vec::new();

        // Snapshot blockchain
        let blockchain_snapshot = self
            .blockchain
            .as_ref()
            .state_subvolume_path()
            .parent()
            .ok_or_else(|| anyhow::anyhow!("No parent path for blockchain"))?
            .join("snapshots")
            .join(format!(
                "blockchain-{}",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            ));

        // Use btrfs snapshot command
        let output = tokio::process::Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(self.blockchain.as_ref().state_subvolume_path())
            .arg(&blockchain_snapshot)
            .output()
            .await
            .context("Failed to create blockchain snapshot")?;

        if output.status.success() {
            snapshots.push(blockchain_snapshot);
            info!(
                "Created blockchain snapshot: {}",
                snapshots.last().unwrap().display()
            );
        }

        // Snapshot cache
        let cache_snapshot = self.cache.create_snapshot().await?;
        snapshots.push(cache_snapshot);
        info!(
            "Created cache snapshot: {}",
            snapshots.last().unwrap().display()
        );

        Ok(snapshots)
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> Result<op_cache::btrfs_cache::CacheStats> {
        self.cache.stats()
    }
}
</file>

<file path="src/footprint.rs">
//! Block events and plugin footprints for the streaming blockchain

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// A block event in the streaming blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub timestamp: u64,
    pub category: String,
    pub action: String,
    pub data: simd_json::OwnedValue,
    pub hash: String,
    pub vector: Vec<f32>,
}

impl BlockEvent {
    /// Create a new block event
    pub fn new(
        category: impl Into<String>,
        action: impl Into<String>,
        data: simd_json::OwnedValue,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        let category = category.into();
        let action = action.into();

        // Compute hash
        let hash_input = format!("{}:{}:{}:{}", timestamp, category, action, data);
        let mut hasher = Sha256::new();
        hasher.update(hash_input.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Self {
            timestamp,
            category,
            action,
            data,
            hash,
            vector: Vec::new(), // Empty vector, can be populated by ML
        }
    }

    /// Create with a pre-computed vector
    pub fn with_vector(mut self, vector: Vec<f32>) -> Self {
        self.vector = vector;
        self
    }
}

/// A plugin footprint representing a tracked operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFootprint {
    pub plugin_id: String,
    pub operation: String,
    pub timestamp: u64,
    pub data_hash: String,
    pub content_hash: String,
    pub metadata: HashMap<String, simd_json::OwnedValue>,
    pub vector_features: Vec<f32>,
}

impl PluginFootprint {
    /// Create a new plugin footprint
    pub fn new(
        plugin_id: impl Into<String>,
        operation: impl Into<String>,
        data: &simd_json::OwnedValue,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        // Hash the data
        let data_str = simd_json::to_string(data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(data_str.as_bytes());
        let data_hash = format!("{:x}", hasher.finalize());

        Self {
            plugin_id: plugin_id.into(),
            operation: operation.into(),
            timestamp,
            data_hash: data_hash.clone(),
            content_hash: data_hash,
            metadata: HashMap::new(),
            vector_features: Vec::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: simd_json::OwnedValue) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Convert to a BlockEvent
    /// NOTE: Vectors are dropped for plugin footprints - timing is authoritative,
    /// vectors are async projections handled separately via Voyage AI embedding pipeline
    pub fn to_block_event(&self) -> BlockEvent {
        let data = simd_json::json!({
            "plugin_id": self.plugin_id,
            "operation": self.operation,
            "data_hash": self.data_hash,
            "metadata": self.metadata
        });

        BlockEvent {
            timestamp: self.timestamp,
            category: self.plugin_id.clone(),
            action: self.operation.clone(),
            data,
            hash: self.data_hash.clone(),
            vector: Vec::new(), // Vectors are projections, not part of authoritative timing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_event_creation() {
        let event = BlockEvent::new("test", "create", simd_json::json!({"key": "value"}));

        assert!(!event.hash.is_empty());
        assert_eq!(event.category, "test");
        assert_eq!(event.action, "create");
    }

    #[test]
    fn test_plugin_footprint_creation() {
        let footprint = PluginFootprint::new(
            "systemd",
            "unit_started",
            &simd_json::json!({"unit": "nginx.service"}),
        );

        assert_eq!(footprint.plugin_id, "systemd");
        assert!(!footprint.data_hash.is_empty());
    }
}
</file>

<file path="src/lib.rs">
//! op-blockchain: Streaming blockchain with BTRFS subvolumes
//!
//! This crate provides:
//! - Streaming blockchain for audit trails
//! - Plugin footprints for change tracking
//! - Dual BTRFS subvolumes (timing/vectors/state)
//! - Automatic snapshots with configurable intervals
//! - Rolling retention policies
//! - btrfs send/receive for replication

pub mod blockchain;
pub mod btrfs_numa_integration;
pub mod footprint;
pub mod plugin_footprint;
pub mod retention;
pub mod snapshot;
pub mod streaming_blockchain;

// Re-export main types
pub use blockchain::StreamingBlockchain;
pub use footprint::{BlockEvent, PluginFootprint};
pub use retention::RetentionPolicy;
pub use snapshot::SnapshotInterval;

// Also export from plugin_footprint for compatibility
pub use plugin_footprint::PluginFootprint as LegacyPluginFootprint;

/// Prelude for convenient imports
pub mod prelude {
    pub use super::blockchain::StreamingBlockchain;
    pub use super::footprint::{BlockEvent, PluginFootprint};
    pub use super::retention::RetentionPolicy;
    pub use super::snapshot::SnapshotInterval;
}
</file>

<file path="src/plugin_footprint.rs">
//! Plugin footprint mechanism with hash for blockchain vectorization

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use simd_json::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFootprint {
    pub plugin_id: String,
    pub operation: String,
    pub timestamp: u64,
    pub data_hash: String,
    pub content_hash: String,
    pub metadata: HashMap<String, simd_json::OwnedValue>,
    pub vector_features: Vec<f32>,
}

impl PluginFootprint {
    #[allow(dead_code)]
    pub fn new(plugin_id: String, operation: String, metadata: simd_json::OwnedValue) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let data_str = simd_json::to_string(&metadata).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(data_str.as_bytes());
        let data_hash = format!("{:x}", hasher.finalize());

        let content = format!("{}:{}:{}", plugin_id, operation, timestamp);
        let mut content_hasher = Sha256::new();
        content_hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", content_hasher.finalize());

        let mut metadata_map = HashMap::new();
        if let simd_json::OwnedValue::Object(obj) = metadata {
            for (k, v) in obj.iter() {
                metadata_map.insert(k.clone(), v.clone());
            }
        }

        Self {
            plugin_id,
            operation,
            timestamp,
            data_hash,
            content_hash,
            metadata: metadata_map,
            vector_features: vec![0.0; 64], // Default 64-dimensional vector
        }
    }
}

pub struct FootprintGenerator {
    plugin_id: String,
}

impl FootprintGenerator {
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
        }
    }

    /// Create footprint upon plugin operation
    pub fn create_footprint(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<PluginFootprint> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| anyhow::anyhow!("System clock error: {}", e))?
            .as_secs();

        // Hash the data content
        let data_str =
            simd_json::to_string(data).context("Failed to serialize data for hashing")?;
        let data_hash = format!("{:x}", Sha256::digest(data_str.as_bytes()));

        // Hash the entire operation context
        let context = format!(
            "{}:{}:{}:{}",
            self.plugin_id, operation, timestamp, data_hash
        );
        let content_hash = format!("{:x}", Sha256::digest(context.as_bytes()));

        // Generate vector features for blockchain
        let vector_features = self.generate_vector_features(operation, data, &metadata)?;

        Ok(PluginFootprint {
            plugin_id: self.plugin_id.clone(),
            operation: operation.to_string(),
            timestamp,
            data_hash,
            content_hash,
            metadata: metadata.unwrap_or_default(),
            vector_features,
        })
    }

    /// Generate vector features for blockchain vectorization.
    /// Heuristic-based only — ML feature is not compiled in this crate.
    fn generate_vector_features(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: &Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<Vec<f32>> {
        self.generate_heuristic_features(operation, data, metadata)
    }

    /// Generate heuristic-based features (original method)
    fn generate_heuristic_features(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: &Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<Vec<f32>> {
        let mut features = Vec::with_capacity(64);

        // Plugin ID hash feature
        features.push(self.hash_string(&self.plugin_id) as f32 / u32::MAX as f32);

        // Operation type features
        let op_features = match operation {
            "create" => vec![1.0, 0.0, 0.0, 0.0],
            "update" => vec![0.0, 1.0, 0.0, 0.0],
            "delete" => vec![0.0, 0.0, 1.0, 0.0],
            "query" => vec![0.0, 0.0, 0.0, 1.0],
            _ => vec![0.5, 0.5, 0.5, 0.5],
        };
        features.extend(op_features);

        // Data structure features
        match data {
            simd_json::OwnedValue::Object(obj) => {
                features.push(1.0); // is_object
                features.push(obj.len() as f32 / 100.0); // normalized size

                // Key diversity (unique first chars)
                let unique_chars: std::collections::HashSet<char> =
                    obj.keys().filter_map(|k| k.chars().next()).collect();
                features.push(unique_chars.len() as f32 / 26.0);

                // Value type distribution
                let mut string_count = 0;
                let mut number_count = 0;
                let mut bool_count = 0;
                let mut null_count = 0;

                for value in obj.values() {
                    if value.is_str() {
                        string_count += 1;
                    } else if value.is_i64() || value.is_u64() || value.is_f64() {
                        number_count += 1;
                    } else if value.is_bool() {
                        bool_count += 1;
                    } else if value.is_null() {
                        null_count += 1;
                    }
                }

                let total = obj.len() as f32;
                features.push(string_count as f32 / total);
                features.push(number_count as f32 / total);
                features.push(bool_count as f32 / total);
                features.push(null_count as f32 / total);
            }
            simd_json::OwnedValue::Array(arr) => {
                features.push(0.0); // not_object
                features.push(arr.len() as f32 / 100.0);
                features.extend(vec![0.0; 6]); // padding
            }
            simd_json::OwnedValue::String(s) => {
                features.push(0.0); // not_object
                features.push(s.len() as f32 / 1000.0);
                features.push(self.hash_string(s) as f32 / u32::MAX as f32);
                features.extend(vec![0.0; 5]); // padding
            }
            _ => {
                features.extend(vec![0.0; 8]); // padding for other types
            }
        }

        // Metadata features
        if let Some(meta) = metadata {
            features.push(meta.len() as f32 / 50.0);

            // Common metadata keys
            let common_keys = ["user", "host", "process", "version", "source"];
            for key in &common_keys {
                features.push(if meta.contains_key(*key) { 1.0 } else { 0.0 });
            }
        } else {
            features.extend(vec![0.0; 6]); // no metadata
        }

        // Temporal features (time-based patterns)
        let hour = (self.get_current_timestamp() / 3600) % 24;
        let day_of_week = (self.get_current_timestamp() / 86400) % 7;
        features.push(hour as f32 / 24.0);
        features.push(day_of_week as f32 / 7.0);

        // Pad to fixed size
        features.resize(64, 0.0);
        Ok(features)
    }

    fn hash_string(&self, s: &str) -> u32 {
        s.bytes()
            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))
    }

    fn get_current_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Plugin trait with footprint mechanism
#[allow(dead_code)]
pub trait FootprintPlugin {
    fn plugin_id(&self) -> &str;

    /// Create footprint and send to blockchain
    fn create_and_record_footprint(
        &self,
        operation: &str,
        data: &simd_json::OwnedValue,
        metadata: Option<HashMap<String, simd_json::OwnedValue>>,
    ) -> Result<PluginFootprint> {
        let generator = FootprintGenerator::new(self.plugin_id());
        let footprint = generator.create_footprint(operation, data, metadata)?;

        // Send to blockchain for vectorization
        self.send_to_blockchain(&footprint)?;

        Ok(footprint)
    }

    /// Send footprint to blockchain (implemented by each plugin)
    fn send_to_blockchain(&self, footprint: &PluginFootprint) -> Result<()>;
}

/// Network plugin with footprint
#[allow(dead_code)]
pub struct NetworkPlugin {
    footprint_gen: FootprintGenerator,
    blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
}

impl NetworkPlugin {
    #[allow(dead_code)]
    pub fn new(blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>) -> Self {
        Self {
            footprint_gen: FootprintGenerator::new("network"),
            blockchain_sender,
        }
    }

    #[allow(dead_code)]
    pub async fn interface_created(
        &self,
        interface: &str,
        config: simd_json::OwnedValue,
    ) -> Result<()> {
        let mut metadata = HashMap::new();
        metadata.insert(
            "interface".to_string(),
            simd_json::OwnedValue::String(interface.to_string()),
        );
        metadata.insert(
            "host".to_string(),
            simd_json::OwnedValue::String(gethostname::gethostname().to_string_lossy().to_string()),
        );

        let footprint = self
            .footprint_gen
            .create_footprint("create", &config, Some(metadata))?;
        self.blockchain_sender.send(footprint)?;
        Ok(())
    }
}

impl FootprintPlugin for NetworkPlugin {
    fn plugin_id(&self) -> &str {
        "network"
    }

    fn send_to_blockchain(&self, footprint: &PluginFootprint) -> Result<()> {
        self.blockchain_sender.send(footprint.clone())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_footprint_generation() {
        let generator = FootprintGenerator::new("test_plugin");
        let data = simd_json::json!({
            "interface": "eth0",
            "ip": "192.168.1.100",
            "status": "active"
        });

        let footprint = generator.create_footprint("create", &data, None).unwrap();

        assert_eq!(footprint.plugin_id, "test_plugin");
        assert_eq!(footprint.operation, "create");
        assert!(!footprint.data_hash.is_empty());
        assert!(!footprint.content_hash.is_empty());
        assert_eq!(footprint.vector_features.len(), 64);
    }

    #[test]
    fn test_vector_features() {
        let generator = FootprintGenerator::new("test");
        let data = simd_json::json!({"key": "value"});

        let footprint = generator.create_footprint("create", &data, None).unwrap();

        // Should have create operation features
        assert_eq!(footprint.vector_features[1], 1.0); // create = [1,0,0,0]
        assert_eq!(footprint.vector_features[2], 0.0);
        assert_eq!(footprint.vector_features[3], 0.0);
        assert_eq!(footprint.vector_features[4], 0.0);

        // Should have object features
        assert_eq!(footprint.vector_features[5], 1.0); // is_object
    }
}
</file>

<file path="src/retention.rs">
//! Snapshot retention policies with rolling windows

use anyhow::Result;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;

/// Snapshot retention policy with rolling windows
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Keep last N hourly snapshots
    pub hourly: usize,
    /// Keep last N daily snapshots
    pub daily: usize,
    /// Keep last N weekly snapshots
    pub weekly: usize,
    /// Keep last N quarterly snapshots
    pub quarterly: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            hourly: 5,
            daily: 5,
            weekly: 5,
            quarterly: 5,
        }
    }
}

impl RetentionPolicy {
    /// Create a new retention policy with explicit values
    pub fn new(hourly: usize, daily: usize, weekly: usize, quarterly: usize) -> Self {
        Self {
            hourly,
            daily,
            weekly,
            quarterly,
        }
    }

    /// Create a minimal retention policy (for testing)
    pub fn minimal() -> Self {
        Self {
            hourly: 2,
            daily: 2,
            weekly: 2,
            quarterly: 2,
        }
    }

    /// Create a comprehensive retention policy (for production)
    pub fn comprehensive() -> Self {
        Self {
            hourly: 24,
            daily: 30,
            weekly: 12,
            quarterly: 8,
        }
    }

    /// Parse from environment variables or use defaults
    pub fn from_env() -> Self {
        Self {
            hourly: std::env::var("OPDBUS_RETAIN_HOURLY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            daily: std::env::var("OPDBUS_RETAIN_DAILY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            weekly: std::env::var("OPDBUS_RETAIN_WEEKLY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            quarterly: std::env::var("OPDBUS_RETAIN_QUARTERLY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
        }
    }

    /// Load from JSON value (for config files)
    pub fn from_json(value: &simd_json::OwnedValue) -> Result<Self> {
        Ok(Self {
            hourly: value.get("hourly").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
            daily: value.get("daily").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
            weekly: value.get("weekly").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
            quarterly: value.get("quarterly").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
        })
    }

    /// Total maximum snapshots that could be retained
    pub fn max_snapshots(&self) -> usize {
        self.hourly + self.daily + self.weekly + self.quarterly
    }

    /// Builder-style methods
    pub fn with_hourly(mut self, count: usize) -> Self {
        self.hourly = count;
        self
    }

    pub fn with_daily(mut self, count: usize) -> Self {
        self.daily = count;
        self
    }

    pub fn with_weekly(mut self, count: usize) -> Self {
        self.weekly = count;
        self
    }

    pub fn with_quarterly(mut self, count: usize) -> Self {
        self.quarterly = count;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = RetentionPolicy::default();
        assert_eq!(policy.hourly, 5);
        assert_eq!(policy.daily, 5);
        assert_eq!(policy.weekly, 5);
        assert_eq!(policy.quarterly, 5);
    }

    #[test]
    fn test_from_json() {
        let json = simd_json::json!({
            "hourly": 10,
            "daily": 7,
            "weekly": 4,
            "quarterly": 2
        });

        let policy = RetentionPolicy::from_json(&json).unwrap();
        assert_eq!(policy.hourly, 10);
        assert_eq!(policy.daily, 7);
    }
}
</file>

<file path="src/snapshot.rs">
//! Snapshot interval configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::warn;

/// Snapshot interval options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SnapshotInterval {
    /// Snapshot after every operation
    PerOperation,
    /// Snapshot every minute
    EveryMinute,
    /// Snapshot every 5 minutes
    Every5Minutes,
    /// Snapshot every 15 minutes
    #[default]
    Every15Minutes,
    /// Snapshot every 30 minutes
    Every30Minutes,
    /// Snapshot every hour
    Hourly,
    /// Snapshot every day
    Daily,
    /// Snapshot every week
    Weekly,
}

impl SnapshotInterval {
    /// Parse from environment variable
    pub fn from_env() -> Self {
        match std::env::var("OPDBUS_SNAPSHOT_INTERVAL")
            .unwrap_or_else(|_| "every-15-minutes".to_string())
            .to_lowercase()
            .as_str()
        {
            "per-op" | "per-operation" | "per_operation" => SnapshotInterval::PerOperation,
            "every-minute" | "1-minute" | "1min" => SnapshotInterval::EveryMinute,
            "every-5-minutes" | "5-minutes" | "5min" => SnapshotInterval::Every5Minutes,
            "every-15-minutes" | "15-minutes" | "15min" => SnapshotInterval::Every15Minutes,
            "every-30-minutes" | "30-minutes" | "30min" => SnapshotInterval::Every30Minutes,
            "hourly" | "1-hour" | "1h" => SnapshotInterval::Hourly,
            "daily" | "1-day" | "1d" => SnapshotInterval::Daily,
            "weekly" | "1-week" | "1w" => SnapshotInterval::Weekly,
            _ => {
                warn!("Invalid OPDBUS_SNAPSHOT_INTERVAL, defaulting to every-15-minutes");
                SnapshotInterval::Every15Minutes
            }
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "per-op" | "per-operation" | "per_operation" => Some(SnapshotInterval::PerOperation),
            "every-minute" | "1-minute" | "1min" | "minute" => Some(SnapshotInterval::EveryMinute),
            "every-5-minutes" | "5-minutes" | "5min" => Some(SnapshotInterval::Every5Minutes),
            "every-15-minutes" | "15-minutes" | "15min" => Some(SnapshotInterval::Every15Minutes),
            "every-30-minutes" | "30-minutes" | "30min" => Some(SnapshotInterval::Every30Minutes),
            "hourly" | "1-hour" | "1h" | "hour" => Some(SnapshotInterval::Hourly),
            "daily" | "1-day" | "1d" | "day" => Some(SnapshotInterval::Daily),
            "weekly" | "1-week" | "1w" | "week" => Some(SnapshotInterval::Weekly),
            _ => None,
        }
    }

    /// Get the duration for this interval
    /// Returns None for PerOperation (snapshot on every change)
    pub fn as_duration(&self) -> Option<Duration> {
        match self {
            SnapshotInterval::PerOperation => None,
            SnapshotInterval::EveryMinute => Some(Duration::from_secs(60)),
            SnapshotInterval::Every5Minutes => Some(Duration::from_secs(5 * 60)),
            SnapshotInterval::Every15Minutes => Some(Duration::from_secs(15 * 60)),
            SnapshotInterval::Every30Minutes => Some(Duration::from_secs(30 * 60)),
            SnapshotInterval::Hourly => Some(Duration::from_secs(60 * 60)),
            SnapshotInterval::Daily => Some(Duration::from_secs(24 * 60 * 60)),
            SnapshotInterval::Weekly => Some(Duration::from_secs(7 * 24 * 60 * 60)),
        }
    }

    /// Check if enough time has passed since the last snapshot
    pub fn should_snapshot(&self, elapsed: Duration) -> bool {
        match self.as_duration() {
            None => true, // PerOperation always snapshots
            Some(interval) => elapsed >= interval,
        }
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            SnapshotInterval::PerOperation => "after every operation",
            SnapshotInterval::EveryMinute => "every minute",
            SnapshotInterval::Every5Minutes => "every 5 minutes",
            SnapshotInterval::Every15Minutes => "every 15 minutes",
            SnapshotInterval::Every30Minutes => "every 30 minutes",
            SnapshotInterval::Hourly => "every hour",
            SnapshotInterval::Daily => "every day",
            SnapshotInterval::Weekly => "every week",
        }
    }
}

impl std::fmt::Display for SnapshotInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration() {
        assert!(SnapshotInterval::PerOperation.as_duration().is_none());
        assert_eq!(
            SnapshotInterval::EveryMinute.as_duration(),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            SnapshotInterval::Hourly.as_duration(),
            Some(Duration::from_secs(3600))
        );
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            SnapshotInterval::parse("hourly"),
            Some(SnapshotInterval::Hourly)
        );
        assert_eq!(
            SnapshotInterval::parse("15min"),
            Some(SnapshotInterval::Every15Minutes)
        );
        assert_eq!(SnapshotInterval::parse("invalid"), None);
    }
}
</file>

<file path="src/streaming_blockchain.rs">
#![allow(unused_imports)]
//! Streaming blockchain with vectorization and dual btrfs subvolumes
//!
//! This module provides a streaming blockchain implementation that:
//! 1. Automatically generates hashed footprints for all object modifications
//! 2. Stores timing and vector data in separate btrfs subvolumes
//! 3. Creates snapshots for each block
//! 4. Streams vector data to remote vector databases via btrfs send/receive

use crate::PluginFootprint;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockEvent {
    pub timestamp: u64,
    pub category: String,
    pub action: String,
    pub data: simd_json::OwnedValue,
    pub hash: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SnapshotInterval {
    PerOperation,
    EveryMinute,
    Every5Minutes,
    #[default]
    Every15Minutes,
    Every30Minutes,
    Hourly,
    Daily,
    Weekly,
}

/// Snapshot retention policy with rolling windows
#[derive(Debug, Clone, Copy)]
pub struct RetentionPolicy {
    pub hourly: usize,    // Keep last N hourly snapshots
    pub daily: usize,     // Keep last N daily snapshots
    pub weekly: usize,    // Keep last N weekly snapshots
    pub quarterly: usize, // Keep last N quarterly snapshots
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            hourly: 5,
            daily: 5,
            weekly: 5,
            quarterly: 5,
        }
    }
}

impl RetentionPolicy {
    /// Create a new retention policy with explicit values
    pub fn new(hourly: usize, daily: usize, weekly: usize, quarterly: usize) -> Self {
        Self {
            hourly,
            daily,
            weekly,
            quarterly,
        }
    }

    /// Parse from environment variables or use defaults
    pub fn from_env() -> Self {
        Self {
            hourly: std::env::var("OPDBUS_RETAIN_HOURLY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            daily: std::env::var("OPDBUS_RETAIN_DAILY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            weekly: std::env::var("OPDBUS_RETAIN_WEEKLY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            quarterly: std::env::var("OPDBUS_RETAIN_QUARTERLY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
        }
    }

    /// Load from JSON value (for config files)
    pub fn from_json(value: &simd_json::OwnedValue) -> Result<Self> {
        Ok(Self {
            hourly: value.get("hourly").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
            daily: value.get("daily").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
            weekly: value.get("weekly").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
            quarterly: value.get("quarterly").and_then(|v| v.as_u64()).unwrap_or(5) as usize,
        })
    }

    /// Set hourly retention count
    pub fn set_hourly(&mut self, count: usize) {
        self.hourly = count;
    }

    /// Set daily retention count
    pub fn set_daily(&mut self, count: usize) {
        self.daily = count;
    }

    /// Set weekly retention count
    pub fn set_weekly(&mut self, count: usize) {
        self.weekly = count;
    }

    /// Set quarterly retention count
    pub fn set_quarterly(&mut self, count: usize) {
        self.quarterly = count;
    }
}

impl SnapshotInterval {
    /// Parse from environment variable or string
    pub fn from_env() -> Self {
        match std::env::var("OPDBUS_SNAPSHOT_INTERVAL")
            .unwrap_or_else(|_| "every-15-minutes".to_string())
            .to_lowercase()
            .as_str()
        {
            "per-op" | "per-operation" | "per_operation" => SnapshotInterval::PerOperation,
            "every-minute" | "1-minute" | "1min" => SnapshotInterval::EveryMinute,
            "every-5-minutes" | "5-minutes" | "5min" => SnapshotInterval::Every5Minutes,
            "every-15-minutes" | "15-minutes" | "15min" => SnapshotInterval::Every15Minutes,
            "every-30-minutes" | "30-minutes" | "30min" => SnapshotInterval::Every30Minutes,
            "hourly" | "1-hour" | "1h" => SnapshotInterval::Hourly,
            "daily" | "1-day" | "1d" => SnapshotInterval::Daily,
            "weekly" | "1-week" | "1w" => SnapshotInterval::Weekly,
            _ => {
                warn!("Invalid OPDBUS_SNAPSHOT_INTERVAL, defaulting to every-15-minutes");
                SnapshotInterval::Every15Minutes
            }
        }
    }
}

pub struct StreamingBlockchain {
    base_path: PathBuf,
    timing_subvol: PathBuf, // Audit trail (immutable history)
    vector_subvol: PathBuf, // ML embeddings
    state_subvol: PathBuf,  // Current system state (for DR/reinstall)
    snapshot_interval: SnapshotInterval,
    retention_policy: RetentionPolicy,
    last_snapshot_time: Arc<RwLock<Instant>>,
}

impl StreamingBlockchain {
    pub async fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        Self::new_with_interval(base_path, SnapshotInterval::from_env()).await
    }

    pub async fn new_with_interval(
        base_path: impl AsRef<Path>,
        snapshot_interval: SnapshotInterval,
    ) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let timing_subvol = base_path.join("timing");
        let vector_subvol = base_path.join("vectors");
        let state_subvol = base_path.join("state");

        tokio::fs::create_dir_all(&base_path).await?;
        Self::create_subvolume(&timing_subvol).await?;
        Self::create_subvolume(&vector_subvol).await?;
        Self::create_subvolume(&state_subvol).await?;

        Ok(Self {
            base_path,
            timing_subvol,
            vector_subvol,
            state_subvol,
            snapshot_interval,
            retention_policy: RetentionPolicy::from_env(),
            last_snapshot_time: Arc::new(RwLock::new(Instant::now())),
        })
    }

    async fn create_subvolume(path: &Path) -> Result<()> {
        if !path.exists() {
            let output = Command::new("btrfs")
                .args(["subvolume", "create"])
                .arg(path)
                .output()
                .await
                .context("Failed to execute btrfs command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("btrfs subvolume create failed: {}", stderr);
            }
        }
        Ok(())
    }

    pub async fn add_footprint(&self, footprint: PluginFootprint) -> Result<String> {
        let data = simd_json::json!({
            "plugin_id": footprint.plugin_id,
            "operation": footprint.operation,
            "data_hash": footprint.data_hash,
            "metadata": footprint.metadata
        });

        let event = BlockEvent {
            timestamp: footprint.timestamp,
            category: footprint.plugin_id.clone(),
            action: footprint.operation.clone(),
            data,
            hash: footprint.content_hash.clone(),
            vector: footprint.vector_features,
        };

        let timing_file = self.timing_subvol.join(format!("{}.json", event.hash));
        let timing_data = simd_json::json!({
            "timestamp": event.timestamp,
            "category": event.category,
            "action": event.action,
            "hash": event.hash,
            "data": event.data,
            "plugin_footprint": true
        });
        tokio::fs::write(&timing_file, simd_json::to_string_pretty(&timing_data)?).await?;

        let vector_file = self.vector_subvol.join(format!("{}.vec", event.hash));
        let vector_data = simd_json::json!({
            "hash": event.hash,
            "vector": event.vector,
            "metadata": {
                "category": event.category,
                "action": event.action,
                "timestamp": event.timestamp,
                "plugin_id": footprint.plugin_id,
                "data_hash": footprint.data_hash
            }
        });
        tokio::fs::write(&vector_file, simd_json::to_string(&vector_data)?).await?;

        // Only create snapshot if interval requires it
        self.create_snapshot_if_needed(&event.hash).await?;
        info!("Plugin footprint added with hash: {}", event.hash);
        Ok(event.hash)
    }

    /// Add multiple footprints in batch (for bulk operations)
    pub async fn add_footprints_batch(
        &self,
        footprints: Vec<PluginFootprint>,
    ) -> Result<Vec<String>> {
        let mut hashes = Vec::new();

        for footprint in footprints {
            let hash = self.add_footprint(footprint).await?;
            hashes.push(hash);
        }

        // Create a batch snapshot after processing all footprints
        if !hashes.is_empty() {
            let batch_hash = format!("batch-{}", hashes.len());
            self.create_snapshot(&batch_hash).await?;
            info!("Created batch snapshot for {} footprints", hashes.len());
        }

        Ok(hashes)
    }

    /// Update current system state (for disaster recovery / reinstall)
    /// This writes the CURRENT state to state/current.json
    /// Called after every apply_state() to keep DR state up-to-date
    pub async fn update_current_state(&self, state: &simd_json::OwnedValue) -> Result<()> {
        let current_state_file = self.state_subvol.join("current.json");

        // Write atomically: write to temp file, then rename
        let temp_file = self.state_subvol.join(".current.json.tmp");
        tokio::fs::write(&temp_file, simd_json::to_string_pretty(state)?).await?;
        tokio::fs::rename(&temp_file, &current_state_file).await?;

        debug!("Updated current state for disaster recovery");
        Ok(())
    }

    /// Update per-plugin state (optional, for granular DR)
    pub async fn update_plugin_state(
        &self,
        plugin_name: &str,
        state: &simd_json::OwnedValue,
    ) -> Result<()> {
        let plugins_dir = self.state_subvol.join("plugins");
        tokio::fs::create_dir_all(&plugins_dir).await?;

        let plugin_file = plugins_dir.join(format!("{}.json", plugin_name));
        let temp_file = plugins_dir.join(format!(".{}.json.tmp", plugin_name));

        tokio::fs::write(&temp_file, simd_json::to_string_pretty(state)?).await?;
        tokio::fs::rename(&temp_file, &plugin_file).await?;

        debug!("Updated state for plugin: {}", plugin_name);
        Ok(())
    }

    /// Read current system state (for DR recovery)
    pub async fn read_current_state(&self) -> Result<simd_json::OwnedValue> {
        let current_state_file = self.state_subvol.join("current.json");
        let mut content = tokio::fs::read_to_string(&current_state_file).await?;
        Ok(unsafe { simd_json::from_str(&mut content)? })
    }

    /// Get path to state subvolume (for btrfs send/receive)
    pub fn state_subvolume_path(&self) -> &Path {
        &self.state_subvol
    }

    pub async fn start_footprint_receiver(
        &self,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<PluginFootprint>,
    ) -> Result<()> {
        info!("Starting plugin footprint receiver");

        while let Some(footprint) = receiver.recv().await {
            if let Err(e) = self.add_footprint(footprint).await {
                tracing::error!("Failed to add plugin footprint: {}", e);
                // Continue processing other footprints instead of failing completely
            }
        }

        info!("Plugin footprint receiver shutting down");
        Ok(())
    }

    /// Create snapshot only if the time interval has elapsed
    async fn create_snapshot_if_needed(&self, block_hash: &str) -> Result<()> {
        match self.snapshot_interval {
            SnapshotInterval::PerOperation => {
                // Always create snapshot (original behavior)
                self.create_snapshot(block_hash).await
            }
            SnapshotInterval::EveryMinute => {
                let now = Instant::now();
                let last_snapshot = *self.last_snapshot_time.read().await;

                if now.duration_since(last_snapshot) >= Duration::from_secs(60) {
                    // 1 minute has passed
                    self.create_snapshot(block_hash).await?;
                    *self.last_snapshot_time.write().await = now;
                }
                Ok(())
            }
            SnapshotInterval::Every5Minutes => {
                let now = Instant::now();
                let last_snapshot = *self.last_snapshot_time.read().await;

                if now.duration_since(last_snapshot) >= Duration::from_secs(300) {
                    // 5 minutes have passed
                    self.create_snapshot(block_hash).await?;
                    *self.last_snapshot_time.write().await = now;
                }
                Ok(())
            }
            SnapshotInterval::Every15Minutes => {
                let now = Instant::now();
                let last_snapshot = *self.last_snapshot_time.read().await;

                if now.duration_since(last_snapshot) >= Duration::from_secs(900) {
                    // 15 minutes have passed
                    self.create_snapshot(block_hash).await?;
                    *self.last_snapshot_time.write().await = now;
                }
                Ok(())
            }
            SnapshotInterval::Every30Minutes => {
                let now = Instant::now();
                let last_snapshot = *self.last_snapshot_time.read().await;

                if now.duration_since(last_snapshot) >= Duration::from_secs(1800) {
                    // 30 minutes have passed
                    self.create_snapshot(block_hash).await?;
                    *self.last_snapshot_time.write().await = now;
                }
                Ok(())
            }
            SnapshotInterval::Hourly => {
                let now = Instant::now();
                let last_snapshot = *self.last_snapshot_time.read().await;

                if now.duration_since(last_snapshot) >= Duration::from_secs(3600) {
                    // 1 hour has passed
                    self.create_snapshot(block_hash).await?;
                    *self.last_snapshot_time.write().await = now;
                }
                Ok(())
            }
            SnapshotInterval::Daily => {
                let now = Instant::now();
                let last_snapshot = *self.last_snapshot_time.read().await;

                if now.duration_since(last_snapshot) >= Duration::from_secs(86400) {
                    // 24 hours have passed
                    self.create_snapshot(block_hash).await?;
                    *self.last_snapshot_time.write().await = now;
                }
                Ok(())
            }
            SnapshotInterval::Weekly => {
                let now = Instant::now();
                let last_snapshot = *self.last_snapshot_time.read().await;

                if now.duration_since(last_snapshot) >= Duration::from_secs(604800) {
                    // 7 days have passed
                    self.create_snapshot(block_hash).await?;
                    *self.last_snapshot_time.write().await = now;
                }
                Ok(())
            }
        }
    }

    async fn create_snapshot(&self, block_hash: &str) -> Result<()> {
        let snapshot_dir = self.base_path.join("snapshots");
        tokio::fs::create_dir_all(&snapshot_dir).await?;

        let state_prefix = self.state_snapshot_prefix();
        let state_counter = self
            .next_state_snapshot_counter(&snapshot_dir, &state_prefix)
            .await?;
        let state_snapshot_name = format!("{}-{:06}", state_prefix, state_counter);

        // Snapshot timing (audit trail - indexed by block hash)
        let timing_snapshot = snapshot_dir.join(format!("timing-{}", block_hash));
        let timing_result = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.timing_subvol)
            .arg(&timing_snapshot)
            .output()
            .await
            .context("Failed to create timing snapshot")?;

        if !timing_result.status.success() {
            warn!(
                "Failed to create timing snapshot: {}",
                String::from_utf8_lossy(&timing_result.stderr)
            );
        }

        // Snapshot vectors (ML embeddings - indexed by block hash)
        let vector_snapshot = snapshot_dir.join(format!("vectors-{}", block_hash));
        let vector_result = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.vector_subvol)
            .arg(&vector_snapshot)
            .output()
            .await
            .context("Failed to create vector snapshot")?;

        if !vector_result.status.success() {
            warn!(
                "Failed to create vector snapshot: {}",
                String::from_utf8_lossy(&vector_result.stderr)
            );
        }

        // Snapshot state (current system state - indexed by timestamp for DR)
        let state_snapshot = snapshot_dir.join(&state_snapshot_name);
        let state_result = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.state_subvol)
            .arg(&state_snapshot)
            .output()
            .await
            .context("Failed to create state snapshot")?;

        if !state_result.status.success() {
            warn!(
                "Failed to create state snapshot: {}",
                String::from_utf8_lossy(&state_result.stderr)
            );
        } else {
            debug!("Created state snapshot: {}", state_snapshot_name);

            // Prune old state snapshots according to retention policy
            if let Err(e) = self.prune_state_snapshots().await {
                warn!("Failed to prune old snapshots: {}", e);
            }
        }

        debug!("Created snapshots for block: {}", block_hash);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn stream_vectors(&self, block_hash: &str, remote: &str) -> Result<()> {
        let vector_snapshot = self
            .base_path
            .join("snapshots")
            .join(format!("vectors-{}", block_hash));

        info!("Streaming vectors for block {} to {}", block_hash, remote);

        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                "btrfs send {} | ssh {} 'btrfs receive /var/lib/blockchain/vectors/'",
                vector_snapshot.display(),
                remote
            ))
            .output()
            .await
            .context("Failed to stream vectors")?;

        if !output.status.success() {
            anyhow::bail!("Stream failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn stream_to_replicas(&self, block_hash: &str, replicas: &[String]) -> Result<()> {
        let vector_snapshot = self
            .base_path
            .join("snapshots")
            .join(format!("vectors-{}", block_hash));

        let mut tee_args = Vec::new();
        for replica in replicas {
            tee_args.push(format!(
                ">(ssh {} 'btrfs receive /var/lib/blockchain/vectors/')",
                replica
            ));
        }

        let cmd = format!(
            "btrfs send {} | tee {} > /dev/null",
            vector_snapshot.display(),
            tee_args.join(" ")
        );

        info!("Streaming to {} replicas", replicas.len());

        let output = Command::new("bash")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .context("Failed to stream to replicas")?;

        if !output.status.success() {
            anyhow::bail!(
                "Multi-replica stream failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Get current snapshot interval configuration
    pub fn snapshot_interval(&self) -> SnapshotInterval {
        self.snapshot_interval
    }

    /// Set snapshot interval (for runtime configuration)
    pub fn set_snapshot_interval(&mut self, interval: SnapshotInterval) {
        self.snapshot_interval = interval;
    }

    /// Get current retention policy
    pub fn retention_policy(&self) -> RetentionPolicy {
        self.retention_policy
    }

    /// Set retention policy (for runtime configuration)
    pub fn set_retention_policy(&mut self, policy: RetentionPolicy) {
        info!(
            "Updating retention policy: {}h/{}d/{}w/{}q",
            policy.hourly, policy.daily, policy.weekly, policy.quarterly
        );
        self.retention_policy = policy;
    }

    /// Update retention policy from JSON config
    pub fn update_retention_from_json(&mut self, value: &simd_json::OwnedValue) -> Result<()> {
        let policy = RetentionPolicy::from_json(value)?;
        self.set_retention_policy(policy);
        Ok(())
    }

    /// Prune state snapshots according to retention policy (rolling windows)
    async fn prune_state_snapshots(&self) -> Result<()> {
        use chrono::{DateTime, Datelike, Duration, Utc};
        use std::collections::HashMap;

        let snapshot_dir = self.base_path.join("snapshots");
        let state_prefix = self.state_snapshot_prefix();

        // List all state snapshots
        let mut entries = tokio::fs::read_dir(&snapshot_dir).await?;
        let mut snapshots: Vec<(String, DateTime<Utc>)> = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();

            // Only process state snapshots
            if !Self::is_state_snapshot_name(&name, &state_prefix) {
                continue;
            }

            let metadata = tokio::fs::metadata(entry.path()).await?;
            let ts = metadata.created().or_else(|_| metadata.modified()).ok();
            if let Some(dt_utc) = ts.map(DateTime::<Utc>::from) {
                snapshots.push((name, dt_utc));
            }
        }

        // Sort by timestamp (newest first)
        snapshots.sort_by(|a, b| b.1.cmp(&a.1));

        let now = Utc::now();

        // Categorize snapshots into retention buckets
        let mut hourly: Vec<String> = Vec::new();
        let mut daily: HashMap<String, String> = HashMap::new(); // date -> snapshot name
        let mut weekly: HashMap<u32, String> = HashMap::new(); // week number -> snapshot name
        let mut quarterly: HashMap<String, String> = HashMap::new(); // quarter -> snapshot name

        for (name, dt) in &snapshots {
            let age = now.signed_duration_since(*dt);

            // Hourly bucket: Last 24 hours, keep one per hour
            if age <= Duration::hours(24) {
                hourly.push(name.clone());
            }
            // Daily bucket: Keep one per day
            else if age <= Duration::days(30) {
                let date_key = dt.format("%Y%m%d").to_string();
                daily.entry(date_key).or_insert_with(|| name.clone());
            }
            // Weekly bucket: Keep one per week
            else if age <= Duration::weeks(12) {
                let week_key = dt.iso_week().week();
                weekly.entry(week_key).or_insert_with(|| name.clone());
            }
            // Quarterly bucket: Keep one per quarter
            else {
                let quarter = (dt.month() - 1) / 3 + 1;
                let quarter_key = format!("{}-Q{}", dt.year(), quarter);
                quarterly.entry(quarter_key).or_insert_with(|| name.clone());
            }
        }

        // Apply retention limits
        let mut keep_snapshots: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Keep last N hourly snapshots
        for snapshot in hourly.iter().take(self.retention_policy.hourly) {
            keep_snapshots.insert(snapshot.clone());
        }

        // Keep last N daily snapshots
        let mut daily_snapshots: Vec<_> = daily.into_values().collect();
        daily_snapshots.sort();
        daily_snapshots.reverse();
        for snapshot in daily_snapshots.iter().take(self.retention_policy.daily) {
            keep_snapshots.insert(snapshot.clone());
        }

        // Keep last N weekly snapshots
        let mut weekly_snapshots: Vec<_> = weekly.into_values().collect();
        weekly_snapshots.sort();
        weekly_snapshots.reverse();
        for snapshot in weekly_snapshots.iter().take(self.retention_policy.weekly) {
            keep_snapshots.insert(snapshot.clone());
        }

        // Keep last N quarterly snapshots
        let mut quarterly_snapshots: Vec<_> = quarterly.into_values().collect();
        quarterly_snapshots.sort();
        quarterly_snapshots.reverse();
        for snapshot in quarterly_snapshots
            .iter()
            .take(self.retention_policy.quarterly)
        {
            keep_snapshots.insert(snapshot.clone());
        }

        // Delete snapshots not in keep set
        let mut deleted_count = 0;
        for (name, _dt) in &snapshots {
            if !keep_snapshots.contains(name) {
                let snapshot_path = snapshot_dir.join(name);
                match Command::new("btrfs")
                    .args(["subvolume", "delete"])
                    .arg(&snapshot_path)
                    .output()
                    .await
                {
                    Ok(output) => {
                        if output.status.success() {
                            deleted_count += 1;
                            debug!("Pruned old snapshot: {}", name);
                        } else {
                            warn!(
                                "Failed to delete snapshot {}: {}",
                                name,
                                String::from_utf8_lossy(&output.stderr)
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Failed to execute btrfs delete for {}: {}", name, e);
                    }
                }
            }
        }

        if deleted_count > 0 {
            info!(
                "Pruned {} old state snapshots (retention: {}h/{}d/{}w/{}q)",
                deleted_count,
                self.retention_policy.hourly,
                self.retention_policy.daily,
                self.retention_policy.weekly,
                self.retention_policy.quarterly
            );
        }

        Ok(())
    }

    /// List all available state snapshots for rollback
    pub async fn list_state_snapshots(&self) -> Result<Vec<(String, String)>> {
        use chrono::{DateTime, Utc};

        let snapshot_dir = self.base_path.join("snapshots");
        let mut entries = tokio::fs::read_dir(&snapshot_dir).await?;
        let mut snapshots = Vec::new();
        let state_prefix = self.state_snapshot_prefix();

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();

            if !Self::is_state_snapshot_name(&name, &state_prefix) {
                continue;
            }

            let metadata = tokio::fs::metadata(entry.path()).await?;
            let ts = metadata.created().or_else(|_| metadata.modified()).ok();
            let human_readable = ts
                .map(DateTime::<Utc>::from)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            snapshots.push((name, human_readable));
        }

        // Sort by name (newest counter first)
        snapshots.sort_by(|a, b| b.0.cmp(&a.0));

        Ok(snapshots)
    }

    /// Rollback to a specific state snapshot
    pub async fn rollback_to_snapshot(&self, snapshot_name: &str) -> Result<PathBuf> {
        let snapshot_path = self.base_path.join("snapshots").join(snapshot_name);

        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot not found: {}", snapshot_name);
        }

        // Read the state from the snapshot
        let state_file = snapshot_path.join("current.json");
        if !state_file.exists() {
            anyhow::bail!("Snapshot does not contain current.json");
        }

        info!("Rolling back to snapshot: {}", snapshot_name);
        Ok(state_file)
    }

    fn state_snapshot_prefix(&self) -> String {
        std::env::var("OPDBUS_STATE_SNAPSHOT_PREFIX").unwrap_or_else(|_| "SNP-state".to_string())
    }

    async fn next_state_snapshot_counter(&self, snapshot_dir: &Path, prefix: &str) -> Result<u64> {
        let mut entries = tokio::fs::read_dir(snapshot_dir).await?;
        let name_prefix = format!("{}-", prefix);
        let mut max_counter = 0u64;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&name_prefix) {
                continue;
            }
            if let Some(counter_str) = name.strip_prefix(&name_prefix) {
                if let Ok(counter) = counter_str.parse::<u64>() {
                    if counter > max_counter {
                        max_counter = counter;
                    }
                }
            }
        }

        Ok(max_counter + 1)
    }

    fn is_state_snapshot_name(name: &str, prefix: &str) -> bool {
        if name.starts_with(&format!("{}-", prefix)) {
            return true;
        }
        name.starts_with("state-")
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-blockchain"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Streaming blockchain with BTRFS subvolumes for op-dbus-v2"

[dependencies]
op-core = { workspace = true }
op-cache = { path = "../op-cache" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
sha2 = { workspace = true }
gethostname = { workspace = true }

[features]
default = []
</file>

<file path="compare-op-blockchain.md">
# compare-op-blockchain

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 8 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 7 |
| Partial artifacts | 0 |
| Spec-listed source files | 8 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- Streaming blockchain with BTRFS subvolumes for op-dbus-v2
- Internal crate integrations: op-core, op-cache.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/streaming_blockchain.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/streaming_blockchain.rs |
| `src/snapshot.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/snapshot.rs |
| `src/retention.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/retention.rs |
| `src/plugin_footprint.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/plugin_footprint.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/footprint.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/footprint.rs |
| `src/btrfs_numa_integration.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/btrfs_numa_integration.rs |
| `src/blockchain.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/blockchain.rs |
| `root` | ✅ Present | root source group | src/blockchain.rs, src/btrfs_numa_integration.rs, src/footprint.rs, src/lib.rs, src/plugin_footprint.rs, src/retention.rs, src/snapshot.rs, src/streaming_blockchain.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| streaming_blockchain | ✅ Implemented | src/streaming_blockchain.rs | SPEC main module |
| snapshot | ✅ Implemented | src/snapshot.rs | SPEC main module |
| retention | ✅ Implemented | src/retention.rs | SPEC main module |
| plugin_footprint | ✅ Implemented | src/plugin_footprint.rs | SPEC main module |
| footprint | ✅ Implemented | src/footprint.rs | SPEC main module |
| btrfs_numa_integration | ✅ Implemented | src/btrfs_numa_integration.rs | SPEC main module |
| blockchain | ✅ Implemented | src/blockchain.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-cache` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - documented in SPEC
- `sha2` - documented in SPEC
- `gethostname` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: blockchain, btrfs_numa_integration, footprint, plugin_footprint, retention, snapshot, streaming_blockchain.
- Cargo feature flags: default, ml.
</file>

<file path="DESIGN.md">
# op-blockchain — Technical Design

**Crate**: `op-blockchain`  
**Scope**: `mutation_footprint` plugin implementation, schema design, blockchain persistence, chain integrity

See `REQUIREMENTS.md` for the acceptance criteria this design satisfies.

---

## Two Angles, One Plugin

The `mutation_footprint` plugin has two co-equal responsibilities that are inseparable:

1. **The plugin IS the schema.** `StatePlugin::schema()` returns the canonical `PluginSchema`
   that defines every field of a mutation footprint record. That schema is the single source of
   truth for what a blockchain block looks like.

2. **The schema IS the vectorization filter.** The `semantic_index.include_paths` and
   `privacy_index.redaction` sections of the generated contract document govern exactly which
   footprint fields the embedding worker ingests, and which are redacted from public payloads.
   The embedding worker calls `SchemaCatalog::get_copies("mutation_footprint")` — it never
   hardcodes field lists.

This follows the identical pattern to all other plugins in the system.

---

## Crate Placement

| Component | Crate | File |
|---|---|---|
| Plugin definition, schema, `StatePlugin` impl | `op-plugins` | `src/state_plugins/mutation_footprint.rs` |
| `PluginSchema`, `FieldSchema`, `FieldType`, `Constraint`, `SchemaRegistry` | `op-state-store` | `src/plugin_schema.rs` (unchanged) |
| `StreamingBlockchain`, `PluginFootprint`, `FootprintGenerator` | `op-blockchain` | (unchanged) |
| Mutation interception | `op-state` | `src/dbus_plugin_base.rs` — `record_state_transition` sends `MutationEvent` |
| Schema catalog index | `op-state-store` | `SchemaRegistry` — indexes the persisted schema |

---

## Plugin Structure — 3-Section Pattern

Following `web_ui.rs` and other plugins, the module is structured in three sections:

```
SECTION 1: Immutable Identity  — set once at registration, never changes
SECTION 2: Footprint Record    — the schema for each blockchain block (all readOnly)
SECTION 3: Capabilities        — what this plugin can do (read-only)
```

Because blockchain footprint records are **append-only and immutable once written**, the entire
`FootprintRecord` is `readOnly`. The `PluginSchema` is tagged `"immutable"` so
`to_json_schema()` adds `"readOnly": true` to every property automatically.

---

## Section 1 — Immutable Identity

```rust
pub struct MutationFootprintIdentity {
    pub name: String,        // const: "mutation_footprint"
    pub version: String,     // semver: "1.0.0"
    pub plugin_type: String, // const: "audit"
    pub driver: String,      // const: "op-blockchain"
}
```

JSON Schema (`$id: …/mutation-footprint/identity.json`):

```json
{
  "type": "object",
  "properties": {
    "name":        { "type": "string", "const": "mutation_footprint" },
    "version":     { "type": "string", "pattern": "^\\d+\\.\\d+\\.\\d+$" },
    "plugin_type": { "type": "string", "const": "audit" },
    "driver":      { "type": "string", "const": "op-blockchain" }
  },
  "required": ["name", "version", "plugin_type", "driver"],
  "additionalProperties": false
}
```

---

## Section 2 — Footprint Record Schema

This is what `StatePlugin::schema()` returns. Built with `PluginSchemaBuilder`, it becomes the
`tunable` section of the contract document. Tagged `"immutable"` → every field gets `readOnly: true`.

### Field Table

| Field | `FieldType` | Required | Constraints | Notes |
|---|---|---|---|---|
| `footprint_id` | `String` | ✅ | `Pattern(uuid-v4)` | Unique block identifier |
| `plugin_source` | `String` | ✅ | `Min(1)` | Originating plugin name |
| `operation_type` | `Enum(["create","update","delete","apply","rollback"])` | ✅ | — | The `ChangeOperation` kind |
| `old_state_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of pre-mutation state |
| `new_state_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of post-mutation state |
| `content_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of full payload — dedup key |
| `prev_block_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of preceding block; genesis = `"0"×64` |
| `timestamp_ms` | `Integer` | ✅ | `Min(0)` | Unix epoch in milliseconds |
| `block_num` | `Integer` | ✅ | `Min(1)` | Monotonically increasing sequence number |
| `actor` | `String` | ❌ | `Min(1)` | Principal that triggered the mutation — **PII** |
| `diff_summary` | `Object({})` | ❌ | — | Human-readable diff — **PII-capable** |
| `metadata` | `Object({})` | ❌ | — | Arbitrary plugin-supplied key-value context |

### Builder (Rust)

```rust
pub fn schema() -> PluginSchema {
    PluginSchema::builder("mutation_footprint")
        .version("1.0.0")
        .category("audit")
        .description("Immutable blockchain footprint records for all system mutations")
        .tag("immutable")        // → readOnly: true on every property in to_json_schema()
        .tag("append-only")
        .immutable_paths(&[
            "/footprint_id", "/plugin_source", "/operation_type",
            "/old_state_hash", "/new_state_hash", "/content_hash",
            "/prev_block_hash", "/timestamp_ms", "/block_num",
        ])
        .field("footprint_id", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "UUID v4 — unique identifier for this footprint block".into(),
            default: None,
            example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef1234567890")),
            constraints: vec![Constraint::Pattern {
                regex: r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$".into(),
            }],
            read_only: true,
            read_only_when: None,
        })
        .field("plugin_source", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Name of the state plugin that produced this mutation".into(),
            default: None,
            example: Some(json!("net")),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: true,
            read_only_when: None,
        })
        .field("operation_type", FieldSchema {
            field_type: FieldType::Enum(vec![
                "create".into(), "update".into(), "delete".into(),
                "apply".into(), "rollback".into(),
            ]),
            required: true,
            description: "The kind of mutation applied".into(),
            default: None,
            example: Some(json!("update")),
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .field("old_state_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 hex of the canonical JSON of the pre-mutation state".into(),
            default: None,
            example: Some(json!("e3b0c44298fc1c149afb...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("new_state_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 hex of the canonical JSON of the post-mutation state".into(),
            default: None,
            example: Some(json!("6b86b273ff34fce19d6...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("content_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 of the full footprint payload — dedup key and block identifier".into(),
            default: None,
            example: Some(json!("d4735e3a265e16eee03f...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("prev_block_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 of the preceding block; genesis block uses 64 zeros".into(),
            default: Some(json!("0000000000000000000000000000000000000000000000000000000000000000")),
            example: Some(json!("a665a45920422f9d417e...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("timestamp_ms", FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Unix epoch timestamp in milliseconds when the mutation occurred".into(),
            default: None,
            example: Some(json!(1700000000000i64)),
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: true,
            read_only_when: None,
        })
        .field("block_num", FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Monotonically increasing block sequence number within this chain".into(),
            default: None,
            example: Some(json!(42)),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: true,
            read_only_when: None,
        })
        .field("actor", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Principal or service that triggered the mutation (redacted from public payloads)".into(),
            default: None,
            example: Some(json!("admin@op-dbus")),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: true,
            read_only_when: None,
        })
        .field("diff_summary", FieldSchema {
            field_type: FieldType::Object(HashMap::new()),
            required: false,
            description: "Human-readable diff between old and new state (masked when source plugin marks data sensitive)".into(),
            default: None,
            example: None,
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .field("metadata", FieldSchema {
            field_type: FieldType::Object(HashMap::new()),
            required: false,
            description: "Arbitrary key-value context supplied by the source plugin".into(),
            default: Some(json!({})),
            example: None,
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .build()
}
```

---

## Contract Document Envelope

`PluginSchema::to_contract_json_schema()` wraps the above into the standard contract with all
required top-level sections. The `mutation_footprint` plugin customises two sections.

### `stub`
```json
{
  "system_id":     "<host UUID>",
  "source":        "mutation_footprint",
  "source_ref":    "op-blockchain/timing_subvol",
  "discovered_at": "<ISO-8601>"
}
```

### `meta`
```json
{
  "dependencies":        [],
  "include_in_recovery": true,
  "recovery_priority":   10,
  "category":            "audit",
  "sensitivity":         "internal",
  "tags":                ["immutable", "append-only"],
  "enabled":             true
}
```

`recovery_priority: 10` (high) — the audit trail should be restored before most other plugins.

### `semantic_index`

Governs which fields the embedding worker ingests. Worker reads this from
`SchemaCatalog::get_copies("mutation_footprint")` — no hardcoded field lists.

```json
{
  "include_paths": [
    "/tunable/footprint_id",
    "/tunable/plugin_source",
    "/tunable/operation_type",
    "/tunable/old_state_hash",
    "/tunable/new_state_hash",
    "/tunable/content_hash",
    "/tunable/prev_block_hash",
    "/tunable/timestamp_ms",
    "/tunable/block_num"
  ],
  "exclude_paths": ["/tunable/actor", "/tunable/diff_summary", "/stub/discovered_at"],
  "chunking": { "strategy": "json-path-group", "max_tokens": 256 },
  "redaction": { "enabled": true }
}
```

### `privacy_index`

`actor` and `diff_summary` do **not** match the auto-detected pii/secret name patterns
(`is_pii_field_name`, `is_secret_field_name` in `plugin_schema.rs`), so the plugin supplies
explicit redaction rules:

```json
{
  "redaction": {
    "rules": [
      { "path": "/tunable/actor",        "action": "drop", "reason": "PII — identifies the human operator" },
      { "path": "/tunable/diff_summary", "action": "mask", "reason": "May contain sensitive state from source plugin" }
    ],
    "default_action": "mask",
    "secret_paths":   [],
    "pii_paths":      ["/tunable/actor", "/tunable/diff_summary"],
    "hash_salt_ref":  "vault://op-dbus/privacy/hash-salt",
    "reversible":     false
  }
}
```

---

## Section 3 — Capabilities

```rust
pub struct MutationFootprintCapabilities {
    pub supports_rollback:     bool,  // false — footprints are immutable records
    pub supports_checkpoints:  bool,  // true  — chain head is a natural checkpoint
    pub supports_verification: bool,  // true  — chain integrity can be verified
    pub atomic_operations:     bool,  // true  — each block write is atomic
    pub append_only:           bool,  // true  — blocks are never modified after write
}
```

---

## `StatePlugin` Implementation

### Required trait methods

| Method | Implementation |
|---|---|
| `name()` | `"mutation_footprint"` |
| `version()` | `"1.0.0"` |
| `metadata()` | `PluginMetadata` with `category: "audit"`, no `dbus_services` |
| `schema()` | Returns `Some(schema())` from Section 2 |
| `is_available()` | `true` — always available |
| `query_current_state()` | Returns `{ block_num, chain_head_hash, last_timestamp_ms }` |
| `calculate_diff()` | No-op — footprints are generated by the worker, not diffed |
| `apply_state()` | No-op — state is applied by `MutationFootprintWorker` directly |
| `verify_state()` | Runs chain integrity verification (replays block file hashes) |
| `create_checkpoint()` | Snapshots the current chain head hash |
| `rollback()` | Not supported — footprints are immutable |
| `capabilities()` | Returns `MutationFootprintCapabilities` |

### Validation

`PluginSchema::validate()` is called on every `FootprintRecord` before it is submitted to
`StreamingBlockchain::add_footprint`. A record that fails validation is **rejected** — not
written to the chain. A `mutation_footprint.validation_failed` error span is emitted.

---

## Existing `PluginFootprint` vs. Schema Record

The current `PluginFootprint` struct in `plugin_footprint.rs` uses:
- `plugin_id`, `operation`, `timestamp` (secs), `data_hash`, `content_hash`, `metadata`, `vector_features`

The schema design introduces additional fields that must be added or mapped:
- `footprint_id` (UUID v4) — new; current struct uses no UUID
- `old_state_hash` / `new_state_hash` — new; current only has `data_hash`
- `prev_block_hash` — **critical missing field**; the chain-link property is not in the current struct
- `block_num` — new; not tracked in current struct
- `timestamp_ms` — current uses seconds (`u64`); must be converted to milliseconds
- `actor`, `diff_summary` — new optional PII fields

The `FootprintGenerator::create_footprint` method must be extended (or a new method added) to
accept old/new state, compute both hashes, chain to the previous block, and populate the full
schema-validated record.

---

## Chain Head Management

```rust
pub struct ChainHead {
    pub block_num:        u64,
    pub content_hash:     String,
    pub last_timestamp_ms: u64,
}

// Shared across the plugin worker
pub type ChainHeadState = Arc<RwLock<ChainHead>>;
```

On startup: read the last block file in `timing_subvol/` to restore `ChainHead` before
accepting new mutations.

Genesis: when no block files exist, use `block_num = 0` and
`content_hash = "0" × 64` as the initial `prev_block_hash`.

---

## Data Flow

```
Any plugin::apply_state()
  → record_state_transition(old, new, action)
    → MutationEvent { plugin_source, operation_type, old_state, new_state, actor }
      → MutationFootprintWorker  (async channel, non-blocking to caller)
        → FootprintGenerator::create_footprint_v2()
            computes old_state_hash, new_state_hash  (SHA-256, canonical JSON)
            reads ChainHead RwLock  →  prev_block_hash, block_num
            computes content_hash   (SHA-256 of all above)
            constructs FootprintRecord
            PluginSchema::validate(record)  → reject + span if invalid
            StreamingBlockchain::add_footprint()
              → timing_subvol/block-{N:012}.json  (atomic write)
              → vector_subvol/vec-{N:012}.bin      (if ml feature)
            updates ChainHead RwLock
            enqueues for EmbeddingWorker (non-blocking)

EmbeddingWorker
  → SchemaCatalog::get_copies("mutation_footprint")
  → reads semantic_index.include_paths  (no hardcoded lists)
  → omits privacy_index.pii_paths  (actor, diff_summary)
  → calls vector backend
  → writes vector_subvol
```

---

## Registration

The plugin must be added to `crates/op-plugins/src/default_registry.rs`:

```rust
// In default_auto_load()
vec![
    "mutation_footprint".to_string(),  // ← add before other plugins so audit trail starts first
    "mcp".to_string(),
    // …
]

// In load_plugin()
"mutation_footprint" => Arc::new(MutationFootprintPlugin::new(
    blockchain_path,      // path to op-blockchain storage dir
    chain_head_state,     // Arc<RwLock<ChainHead>>
    mutation_rx,          // Receiver<MutationEvent>
)),
```

The registration order matters: `mutation_footprint` should start before other plugins so the
chain is ready to receive events when other plugins run their first `apply_state`.
</file>

<file path="REQUIREMENTS.md">
# op-blockchain — Requirements

**Crate**: `op-blockchain`  
**Scope**: Mutation footprint capture, blockchain persistence, chain integrity, vectorization, and the `mutation_footprint` plugin

---

## Introduction

Every mutation applied to the system through the plugin + schema flow must produce a
cryptographically hashed footprint that is appended to the immutable blockchain audit trail
maintained by `op-blockchain`. The `mutation_footprint` plugin is the canonical plugin that
owns this audit trail. It registers once at startup, produces a `PluginCatalogDocument`
describing its schema, and receives mutation events from every other state plugin via a shared
async channel.

**Key architectural invariants:**

- The `mutation_footprint` plugin is the **single schema authority** for mutation audit records.
  No other component invents a competing audit record shape.
- Every mutation is captured as a `PluginFootprint` — containing the source plugin ID, operation
  type, SHA-256 hashes of the old and new state, a chained block hash linking to the previous
  footprint, and full metadata — before being appended to the `StreamingBlockchain`.
- The blockchain's `timing_subvol` is **append-only**. Once a footprint block is written it is
  never modified; only new blocks are added. Snapshots and retention policy control pruning.
- The plugin schema is the **ground truth** for validation, rendering, vectorization, and
  compliance queries. All downstream consumers resolve the audit record shape through
  `SchemaCatalog::get_copies("mutation_footprint")` — not hardcoded field lists.
- Mutations must not bypass the plugin + schema flow.

---

## 1. Canonical Plugin Document & Schema Flow

### Intent

The `mutation_footprint` plugin owns its canonical schema. That schema defines every field in an
audit record: identifiers, state hashes, chain linkage, actor context, and semantic/privacy tags.
The shared catalog indexes it so all projections read the same contract.

### Acceptance Criteria

1. WHEN `mutation_footprint` registers at startup THEN it persists a `PluginCatalogDocument`
   containing a full `PluginSchema` with these fields:
   - `footprint_id` — UUID v4, immutable, semantic
   - `plugin_source` — originating plugin name, semantic
   - `operation_type` — enum: `create | update | delete | apply | rollback`, semantic
   - `old_state_hash` — SHA-256 hex of pre-mutation state, semantic
   - `new_state_hash` — SHA-256 hex of post-mutation state, semantic
   - `content_hash` — SHA-256 of the full footprint payload, semantic (dedup key)
   - `prev_block_hash` — SHA-256 of the preceding block, semantic (chain link)
   - `block_num` — monotonically increasing sequence number, semantic
   - `timestamp_ms` — u64 milliseconds since epoch, semantic
   - `actor` — principal/user/service that triggered mutation, optional, PII-capable
   - `diff_summary` — JSON object with computed diff, optional, PII-capable
   - `metadata` — arbitrary plugin-supplied key-value pairs, optional

2. WHEN schema registration succeeds THEN `SchemaCatalog` indexes the schema and every consumer
   resolves audit record shape through the catalog, not hardcoded field lists.

3. WHEN schema fields change THEN the change is introduced in `mutation_footprint::schema()`
   first; all downstream projections follow automatically via catalog lookup.

4. WHEN `StatePlugin::schema()` is called THEN it returns `Some(PluginSchema)` — not `None`.
   The compat fallback in `plugin_schema.rs` does NOT satisfy this requirement.

---

## 2. Mutation Interception & Footprint Generation

### Intent

Every `apply_state` call on any registered state plugin must produce a footprint before the state
change is considered complete. The footprint captures before/after hashes and chains to the
previous block.

### Acceptance Criteria

1. WHEN any state plugin executes `apply_state` THEN the plugin runtime captures the old state
   (pre-apply) and new state (post-apply) and sends a `MutationEvent` to the
   `mutation_footprint` plugin's inbound channel before returning success to the caller.

2. WHEN the `MutationEvent` is received THEN the plugin:
   - Computes `old_state_hash = SHA-256(canonical_json(old_state))`
   - Computes `new_state_hash = SHA-256(canonical_json(new_state))`
   - Reads the last known `prev_block_hash` from the in-memory chain head (protected by a
     `RwLock`; genesis sentinel = `"0" × 64`)
   - Computes `content_hash = SHA-256(footprint_id || plugin_source || operation_type ||
     old_state_hash || new_state_hash || prev_block_hash || timestamp_ms)`
   - Constructs a `PluginFootprint` with all schema-required fields
   - Updates the chain head to `content_hash`

3. WHEN `content_hash` equals the chain head for an already-seen record THEN the footprint is a
   duplicate; it is logged and discarded without appending to the chain.

4. WHEN the originating plugin has `sensitive = true` in its metadata THEN `diff_summary` and
   `actor` are marked `pii_flagged = true` before persisting.

5. WHEN any step in footprint generation fails THEN the error is logged at `error` level with
   `plugin_source`, `operation_type`, and `footprint_id`; the mutation itself is not rolled
   back but the audit gap is surfaced via structured telemetry.

---

## 3. Blockchain Persistence & BTRFS Storage

### Intent

Footprints are written to the `StreamingBlockchain`'s immutable `timing_subvol` and optionally
to `vector_subvol` when semantic features are available. Snapshots and retention policy preserve
the audit history within configurable rolling windows.

### Acceptance Criteria

1. WHEN a footprint is ready THEN it is submitted to `StreamingBlockchain::add_footprint` which
   writes a JSON block file to `timing_subvol/block-{N:012}.json` atomically.

2. WHEN the `ml` cargo feature is enabled THEN `FootprintGenerator` uses transformer embeddings
   for `vector_features`; otherwise heuristic 64-dimensional features are used. The vector is
   stored in `vector_subvol`.

3. WHEN a snapshot interval elapses (configurable via `OPDBUS_SNAPSHOT_INTERVAL`, defaulting to
   `every-15-minutes`) THEN the blockchain creates a read-only BTRFS snapshot of `state_subvol`.
   The `timing_subvol` is never snapshotted-and-pruned; it is append-only.

4. WHEN the retention policy fires THEN old `state_subvol` snapshots are pruned according to
   `RetentionPolicy` (hourly/daily/weekly/quarterly windows configurable via env vars). Block
   files in `timing_subvol` are never pruned; they constitute the permanent audit ledger.

5. WHEN BTRFS is unavailable THEN the system falls back to regular directories and the audit
   trail continues with degraded snapshot capability. The footprint record logs the storage
   backend in use.

6. WHEN `btrfs send` / remote replication is triggered THEN the snapshot is streamed to the
   configured remote path, keeping an off-site copy of the audit trail.

---

## 4. Chain Integrity & Tamper Detection

### Intent

The chain-link property (`prev_block_hash`) makes the audit trail tamper-evident. Any inserted,
deleted, or modified block breaks the hash chain and is detectable by a verification pass.

### Acceptance Criteria

1. WHEN chain verification is requested THEN the system replays `timing_subvol/block-*.json`
   files in sequence, recomputes each `content_hash`, and confirms each block's
   `prev_block_hash` equals the preceding block's `content_hash`.

2. WHEN a broken link is found THEN verification reports the first block index where the chain
   breaks, the expected hash, and the stored hash.

3. WHEN genesis verification runs THEN the first block's `prev_block_hash` must equal the
   genesis sentinel value (default `"0" × 64`).

4. WHEN the chain head is queried THEN the plugin returns the `content_hash` of the most
   recently appended block without re-reading all block files.

5. WHEN the system restarts THEN `mutation_footprint` replays the last block file to restore the
   in-memory chain head before accepting new mutations.

---

## 5. Vectorization & Semantic Search

### Intent

Mutation footprints are optionally vectorized so operators can perform semantic similarity
queries across the audit trail (e.g., "find mutations similar to this network config change").

### Acceptance Criteria

1. WHEN the `ml` feature is active THEN the embedding worker resolves the `mutation_footprint`
   schema from `SchemaCatalog`, constructs embedding text from fields tagged `semantic`
   (excluding `pii`-flagged content), and calls the configured vector backend.

2. WHEN `pii_flagged = true` THEN `diff_summary` and `actor` are omitted from the embedding text.

3. WHEN vectorization completes THEN the vector is stored in `vector_subvol` alongside the block
   for semantic retrieval.

4. WHEN vectorization fails THEN the block is still committed to `timing_subvol`; the vector is
   queued for retry with exponential backoff. Audit integrity does not depend on vectorization.

5. WHEN a semantic search is issued THEN results include only schema-approved fields; no raw
   state payloads or PII-flagged content is surfaced.

---

## 6. Observability & Priority

### Intent

Mutation footprint recording must be low-latency and non-blocking relative to the plugin
mutation itself. Telemetry surfaces chain health, throughput, and any audit gaps.

### Acceptance Criteria

1. WHEN a footprint is appended THEN emit a `mutation_footprint.recorded` tracing span with
   attributes: `footprint_id`, `plugin_source`, `operation_type`, `content_hash`, `block_num`,
   `chain_valid` (bool), and write latency.

2. WHEN a duplicate footprint is dropped THEN emit a `mutation_footprint.deduped` event with
   `content_hash` and `plugin_source`.

3. WHEN chain verification fails THEN emit a `mutation_footprint.chain_broken` event at `error`
   level with `block_num`, `expected_hash`, and `actual_hash`.

4. WHEN the inbound mutation channel back-pressures THEN log a `warn` with current queue depth;
   do not drop mutations silently.

5. WHEN the mutation footprint worker is under load THEN it runs at lower priority than direct
   control-plane operations, but higher than schema footprint embedding. NUMA affinity for
   queue/storage is applied using `OptimizedBlockchain` where available.

---

## Implementation Checklist

Before the plugin is considered complete:

- [ ] `mutation_footprint` implements `StatePlugin::schema()` returning `Some(PluginSchema)` with
      all 12 fields (footprint_id, plugin_source, operation_type, old_state_hash, new_state_hash,
      content_hash, prev_block_hash, block_num, timestamp_ms, actor, diff_summary, metadata)
      with correct FieldType, constraints, and read_only flags.
- [ ] The schema is tagged `"immutable"` so `to_json_schema()` emits `readOnly: true` on every property.
- [ ] `actor` and `diff_summary` have explicit `privacy_index.redaction.rules` entries (path, action=drop/mask)
      because their names do not match the auto-PII detection patterns.
- [ ] The plugin is added to `DefaultPluginRegistry` and listed in `default_auto_load`.
- [ ] `PluginCatalog::register` persists the `PluginCatalogDocument` and indexes the schema into
      `SchemaCatalog` on startup.
- [ ] `op-state` intercepts `apply_state` and sends `MutationEvent` through a shared async channel
      to the mutation_footprint worker.
- [ ] The blockchain writer appends to `StreamingBlockchain::timing_subvol` using schema-defined
      fields and emits the tracing spans from Section 6.
- [ ] Chain verification (`verify_chain`) is callable independently of the write path.
- [ ] On restart, the chain head is restored from the last block file before new mutations are accepted.
- [ ] Vectorization worker reads semantic fields from `SchemaCatalog` — no hardcoded field lists.
</file>

<file path="SPEC.md">
# op-blockchain — Specification

**Crate**: `op-blockchain`  
**Location**: `crates/op-blockchain`  
**Purpose**: Streaming blockchain with BTRFS subvolumes for append-only mutation audit trails,
vectorized footprints, point-in-time snapshots, and tamper-evident chain integrity.

See `REQUIREMENTS.md` for what this crate must do and `DESIGN.md` for the implementation approach.

---

## Quick Reference

### Cargo.toml
```toml
[package]
name = "op-blockchain"
version.workspace = true
edition.workspace = true

[dependencies]
op-core    = { workspace = true }
op-cache   = { path = "../op-cache" }
tokio      = { workspace = true }
serde      = { workspace = true }
simd-json  = { workspace = true }
anyhow     = { workspace = true }
thiserror  = { workspace = true }
tracing    = { workspace = true }
chrono     = { workspace = true }
uuid       = { workspace = true }
sha2       = { workspace = true }
gethostname = { workspace = true }

[features]
default = []
ml      = []        # enables transformer-based vectorization via FootprintGenerator
```

### Source Structure
```
op-blockchain/src/
  lib.rs                      — crate root, re-exports
  blockchain.rs               — StreamingBlockchain, OptimizedBlockchain
  footprint.rs                — BlockEvent, PluginFootprint (current production struct)
  plugin_footprint.rs         — LegacyPluginFootprint, FootprintGenerator
  streaming_blockchain.rs     — StreamingBlockchain full implementation, SnapshotInterval
  retention.rs                — RetentionPolicy (hourly/daily/weekly/quarterly)
  snapshot.rs                 — SnapshotInterval enum and snapshot helpers
  btrfs_numa_integration.rs   — NUMA topology detection, OptimizedBlockchain wrapper
```

---

## Module Structure

### `blockchain` — Core Blockchain

- **`StreamingBlockchain`** — main struct managing three BTRFS subvolumes:
  - `timing_subvol` — append-only audit ledger (`block-{N:012}.json`)
  - `vector_subvol` — ML embedding vectors per block
  - `state_subvol` — current system state for disaster recovery (snapshotted)
- **`OptimizedBlockchain`** — NUMA-aware wrapper around `StreamingBlockchain` with BTRFS cache

Key methods:
- `new(base_path, snapshot_interval, retention_policy)` — initialize
- `add_footprint(footprint)` — append a new block (atomic write)
- `verify_chain()` — replay all blocks and check hash chain continuity
- `chain_head()` — return current head (`block_num`, `content_hash`)
- `trigger_snapshot()` — create read-only BTRFS snapshot of `state_subvol`
- `apply_retention()` — prune stale `state_subvol` snapshots per policy

### `footprint` — Block Types

- **`BlockEvent`** — timestamped event: `timestamp`, `category`, `action`, `data`, `hash`, `vector`
- **`PluginFootprint`** — production footprint record (see field gap analysis in DESIGN.md)

Current `PluginFootprint` fields:

| Field | Type | Notes |
|---|---|---|
| `plugin_id` | `String` | Source plugin name |
| `operation` | `String` | Operation string |
| `timestamp` | `u64` | Seconds since epoch (needs → ms) |
| `data_hash` | `String` | SHA-256 of data content |
| `content_hash` | `String` | SHA-256 of operation context |
| `metadata` | `HashMap<String, Value>` | Key-value context |
| `vector_features` | `Vec<f32>` | 64-dim heuristic or transformer embeddings |

**Missing vs. schema** (see DESIGN.md): `footprint_id` (UUID), `old_state_hash`,
`new_state_hash`, `prev_block_hash` (chain link), `block_num`, `actor`, `diff_summary`.

### `plugin_footprint` — FootprintGenerator

- **`LegacyPluginFootprint`** — older struct kept for compatibility
- **`FootprintGenerator`** — creates footprints from plugin operations:
  - `new(plugin_id)` — construct with plugin identity
  - `create_footprint(operation, data, metadata)` — heuristic features (64-dim)
  - When `ml` feature enabled: `generate_transformer_features()` uses `ModelManager::global()`

### `streaming_blockchain` — Storage Engine

- Implements the three-subvolume layout
- `SnapshotInterval` enum: `PerOperation` | `EveryMinute` | `Every5Minutes` | `Every15Minutes` |
  `Every30Minutes` | `Hourly` | `Daily` | `Weekly`
- Default interval: `Every15Minutes` (configurable via `OPDBUS_SNAPSHOT_INTERVAL`)
- BTRFS commands via `tokio::process::Command` (`btrfs subvol create/snapshot/delete`, `btrfs send`)
- Falls back to regular directories if BTRFS unavailable

### `retention` — Retention Policy

- **`RetentionPolicy`** — rolling windows for `state_subvol` snapshot pruning:
  - `hourly: usize` — keep last N hourly snapshots
  - `daily: usize` — keep last N daily snapshots
  - `weekly: usize` — keep last N weekly snapshots
  - `quarterly: usize` — keep last N quarterly snapshots
- Default: 5 for each window
- Configurable via env: `OPDBUS_RETAIN_HOURLY`, `OPDBUS_RETAIN_DAILY`, `OPDBUS_RETAIN_WEEKLY`, `OPDBUS_RETAIN_QUARTERLY`
- `timing_subvol` blocks are **never pruned** — permanent audit ledger

### `btrfs_numa_integration` — NUMA Support

- Detects NUMA topology from `/sys/devices/system/node/`
- `OptimizedBlockchain` assigns blockchain I/O to NUMA-local nodes
- Improves throughput on multi-socket systems for high-mutation workloads

---

## Storage Layout

```
{base_path}/
  timing_subvol/          ← append-only, never pruned
    block-000000000001.json
    block-000000000002.json
    …
  vector_subvol/          ← embedding vectors, one per block
    vec-000000000001.bin
    …
  state_subvol/           ← current system state, snapshotted
  snapshots/              ← read-only BTRFS snapshots of state_subvol
    snapshot-{ISO8601}/
    …
```

Block file format:
```json
{
  "footprint_id":    "uuid-v4",
  "plugin_source":   "net",
  "operation_type":  "update",
  "old_state_hash":  "sha256hex",
  "new_state_hash":  "sha256hex",
  "content_hash":    "sha256hex",
  "prev_block_hash": "sha256hex",
  "block_num":       42,
  "timestamp_ms":    1700000000000,
  "metadata":        {}
}
```

---

## Chain Integrity

Each block's `content_hash` is computed as:

```
SHA-256(footprint_id || plugin_source || operation_type ||
        old_state_hash || new_state_hash || prev_block_hash || timestamp_ms)
```

The chain is valid when every block's `prev_block_hash` equals the `content_hash` of the
immediately preceding block. The genesis block uses `"0" × 64` as `prev_block_hash`.

Verification (`verify_chain`): linear replay of `timing_subvol/block-*.json` in numeric order.
Any broken link produces a `mutation_footprint.chain_broken` error span.

---

## Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `OPDBUS_SNAPSHOT_INTERVAL` | `every-15-minutes` | Snapshot interval for `state_subvol` |
| `OPDBUS_RETAIN_HOURLY` | `5` | Hourly snapshot retention count |
| `OPDBUS_RETAIN_DAILY` | `5` | Daily snapshot retention count |
| `OPDBUS_RETAIN_WEEKLY` | `5` | Weekly snapshot retention count |
| `OPDBUS_RETAIN_QUARTERLY` | `5` | Quarterly snapshot retention count |

---

## Features

| Feature | Default | Effect |
|---|---|---|
| `default` | ✅ | Heuristic 64-dim vectorization |
| `ml` | ❌ | Transformer-based embeddings via `ModelManager::global()` |

---

## Related Crates

| Crate | Relationship |
|---|---|
| `op-cache` | BTRFS cache integration |
| `op-plugins` | Hosts `mutation_footprint` plugin that uses this crate |
| `op-state` | Sends `MutationEvent` to the footprint worker |
| `op-state-store` | `PluginSchema`, `SchemaCatalog` — schema for footprint records |
</file>

</files>
