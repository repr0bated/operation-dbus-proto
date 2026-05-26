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
            anyhow::bail!(
                "`btrfs send` failed with status {:?}",
                send_status.code()
            );
        }
        if !recv_output.status.success() {
            let stderr = String::from_utf8_lossy(&recv_output.stderr);
            anyhow::bail!(
                "`ssh ... btrfs receive` failed (status {:?}): {}",
                recv_output.status.code(),
                stderr
            );
        }

        info!("Successfully streamed snapshot {} to {}", snapshot_name, remote_host);
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
            anyhow::bail!(
                "invalid byte 0x{:02x} at position {} in remote host",
                b,
                i
            );
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
    let s = path
        .to_str()
        .context("btrfs path is not valid UTF-8")?;
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
