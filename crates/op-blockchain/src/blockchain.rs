//! Streaming blockchain with dual BTRFS subvolumes
//!
//! Architecture:
//! - timing_subvol: Immutable audit trail (append-only)
//! - vector_subvol: ML embeddings for semantic search
//! - state_subvol: Current system state for DR/reinstall

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Utc};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashSet};
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

/// The three subvolumes that make up a chain, snapshotted and replicated as a
/// set. Order is stable: it decides the aligned-counter scan order only.
pub const SNAPSHOT_LABELS: [&str; 3] = ["timing", "vectors", "state"];

/// Cap on a single flattened field's length in embedding text, so one large
/// payload cannot dominate a block's vector.
const MAX_EMBEDDING_FIELD_LEN: usize = 512;

/// Records the last aligned counter whose full snapshot triple reached the
/// replication target, i.e. the only valid `btrfs send -p` parent.
const REPLICATED_COUNTER_FILE: &str = ".replicated-counter";

/// One snapshot considered by the retention policy.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotEntry {
    name: String,
    created: DateTime<Utc>,
    /// Aligned counter parsed from the name; the only strictly increasing,
    /// per-snapshot identity available (btrfs snapshots inherit their source's
    /// mtime, so timestamps tie or even move backwards).
    counter: u64,
}

/// Counter suffix of a snapshot name, e.g. `SNP-vectors-000012` -> 12.
fn parse_snapshot_counter(name: &str, prefix: &str) -> Option<u64> {
    name.strip_prefix(prefix)?.strip_prefix('-')?.parse().ok()
}

/// Decide which snapshots of one family to keep.
///
/// Sorts newest-first and returns the keep set. Two rules exist independently of
/// the time buckets, because losing either snapshot breaks a chain guarantee:
///
/// - the highest counter is always kept: it is the latest recovery point and the
///   next incremental send's parent;
/// - the last successfully replicated counter is always kept: `btrfs send -p`
///   needs a parent that both sides still have.
///
/// Both matter because btrfs snapshots inherit the source subvolume's mtime, so
/// every snapshot of a rarely-written subvolume can share a single timestamp and
/// collapse into one bucket — which silently deleted the just-created member of
/// an aligned triple before this rule existed.
fn retain_snapshots(
    snapshots: &mut [SnapshotEntry],
    policy: &RetentionPolicy,
    now: DateTime<Utc>,
    replicated_parent: Option<u64>,
) -> HashSet<String> {
    use chrono::Duration;

    snapshots.sort_by_key(|entry| (Reverse(entry.created), Reverse(entry.counter)));

    let mut hourly: Vec<&str> = Vec::new();
    let mut daily: BTreeMap<String, &str> = BTreeMap::new();
    let mut weekly: BTreeMap<(i32, u32), &str> = BTreeMap::new();
    let mut quarterly: BTreeMap<String, &str> = BTreeMap::new();

    for entry in snapshots.iter() {
        let age = now.signed_duration_since(entry.created);
        if age <= Duration::hours(24) {
            hourly.push(&entry.name);
        } else if age <= Duration::days(30) {
            daily
                .entry(entry.created.format("%Y%m%d").to_string())
                .or_insert(&entry.name);
        } else if age <= Duration::weeks(12) {
            weekly
                .entry((entry.created.year(), entry.created.iso_week().week()))
                .or_insert(&entry.name);
        } else {
            let quarter = (entry.created.month() - 1) / 3 + 1;
            quarterly
                .entry(format!("{}-Q{}", entry.created.year(), quarter))
                .or_insert(&entry.name);
        }
    }

    let mut keep: HashSet<String> = HashSet::new();
    keep.extend(hourly.iter().take(policy.hourly).map(|n| n.to_string()));
    for (bucket, count) in [
        (daily, policy.daily),
        (
            weekly
                .into_iter()
                .map(|((year, week), name)| (format!("{year}-W{week:02}"), name))
                .collect(),
            policy.weekly,
        ),
        (quarterly, policy.quarterly),
    ] {
        // Buckets are keyed chronologically, so reversing takes the newest.
        keep.extend(
            bucket
                .into_values()
                .rev()
                .take(count)
                .map(|name| name.to_string()),
        );
    }

    if let Some(newest) = snapshots.iter().max_by_key(|entry| entry.counter) {
        keep.insert(newest.name.clone());
    }
    if let Some(parent) = replicated_parent {
        if let Some(entry) = snapshots.iter().find(|entry| entry.counter == parent) {
            keep.insert(entry.name.clone());
        }
    }

    keep
}

/// Outcome of one [`StreamingBlockchain::replicate`] pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicationReport {
    /// Aligned snapshot counter that was sent.
    pub counter: u64,
    /// Parent counter used for the incremental send, or `None` for a full send.
    pub parent: Option<u64>,
    /// Snapshot names that landed on the remote.
    pub sent: Vec<String>,
    /// Snapshot names that failed, with their error text.
    pub failed: Vec<(String, String)>,
    /// Error from the post-receive hook, if one was configured and failed.
    /// Non-fatal: the data landed, only the derived index lagged.
    pub hook_error: Option<String>,
}

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

        // Resume from the highest block already on disk. Starting at 0 would
        // renumber from block 1 on every restart and overwrite existing timing
        // records — silent destruction of the one artifact that cannot be
        // regenerated.
        let resumed_from = highest_block_number(&timing_subvol).await?;

        info!(
            "Streaming blockchain initialized at {:?} with {} interval, resuming after block {}",
            base_path, snapshot_interval, resumed_from
        );

        Ok(Self {
            base_path,
            timing_subvol,
            vector_subvol,
            state_subvol,
            snapshot_interval,
            retention_policy: RetentionPolicy::from_env(),
            last_snapshot_time: Arc::new(RwLock::new(Instant::now())),
            block_counter: Arc::new(RwLock::new(resumed_from)),
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
            self.attach_vector(block_num, &event.vector).await?;
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

    // ── Vector projections ───────────────────────────────────────────────
    //
    // Vectors live in the chain, not only in the index. Because the vector
    // subvolume is snapshotted and sent alongside timing, a received replica
    // can rebuild an entire vector index from the stream alone — no
    // re-embedding, no embedding-provider dependency on the restore path.
    //
    // On-disk form is raw little-endian f32 (4096 bytes at 1024 dims), which
    // needs no parse to become an upsert payload.

    /// Path of the vector projection for a block, whether or not it exists.
    pub fn vector_path(&self, block_num: u64) -> PathBuf {
        self.vector_subvol.join(vector_file_name(block_num))
    }

    /// Write (or replace) the vector projection for an existing block.
    ///
    /// Rejects a block that has no timing record, so a vector can never
    /// reference a block that does not exist.
    pub async fn attach_vector(&self, block_num: u64, vector: &[f32]) -> Result<PathBuf> {
        anyhow::ensure!(
            !vector.is_empty(),
            "refusing to attach an empty vector to block {block_num}"
        );
        let timing_file = self.timing_path(block_num);
        anyhow::ensure!(
            tokio::fs::try_exists(&timing_file).await.unwrap_or(false),
            "no timing record for block {block_num} at {}",
            timing_file.display()
        );

        let vector_file = self.vector_path(block_num);
        let mut bytes = Vec::with_capacity(vector.len() * 4);
        for value in vector {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        tokio::fs::write(&vector_file, &bytes)
            .await
            .with_context(|| format!("failed to write vector to {}", vector_file.display()))?;

        debug!(
            "Attached {}-dim vector to block {}",
            vector.len(),
            block_num
        );
        Ok(vector_file)
    }

    /// Read a block's vector projection, or `None` when it has not landed yet.
    pub async fn read_vector(&self, block_num: u64) -> Result<Option<Vec<f32>>> {
        let path = self.vector_path(block_num);
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                Ok(Some(decode_vector(&bytes).with_context(|| {
                    format!("malformed vector file {}", path.display())
                })?))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => {
                Err(anyhow::Error::from(err).context(format!("failed to read {}", path.display())))
            }
        }
    }

    /// Every block on disk, ascending by block number.
    pub async fn blocks(&self) -> Result<Vec<ChainBlockRef>> {
        let mut blocks = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.timing_subvol)
            .await
            .with_context(|| {
                format!(
                    "failed to read timing subvolume {}",
                    self.timing_subvol.display()
                )
            })?;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(block_num) = parse_block_number(&name) else {
                continue;
            };
            let bytes = tokio::fs::read(entry.path()).await?;
            let has_vector = tokio::fs::try_exists(self.vector_path(block_num))
                .await
                .unwrap_or(false);
            match ChainBlockRef::from_timing_bytes(block_num, bytes, has_vector) {
                Ok(block) => blocks.push(block),
                Err(err) => warn!("skipping unparseable timing record {}: {}", name, err),
            }
        }

        blocks.sort_by_key(|block| block.block_num);
        Ok(blocks)
    }

    /// Blocks whose vector projection has not landed yet — the work queue for
    /// an embedding pass.
    pub async fn blocks_missing_vectors(&self) -> Result<Vec<ChainBlockRef>> {
        Ok(self
            .blocks()
            .await?
            .into_iter()
            .filter(|block| !block.has_vector)
            .collect())
    }

    fn timing_path(&self, block_num: u64) -> PathBuf {
        self.timing_subvol.join(timing_file_name(block_num))
    }

    /// Path of the timing subvolume (authoritative audit trail).
    pub fn timing_subvolume_path(&self) -> &Path {
        &self.timing_subvol
    }

    /// Path of the vector subvolume (embedding projections).
    pub fn vector_subvolume_path(&self) -> &Path {
        &self.vector_subvol
    }

    /// Path of the state subvolume (disaster-recovery state).
    pub fn state_subvolume_path(&self) -> &Path {
        &self.state_subvol
    }

    /// Snapshot all three subvolumes under one aligned counter.
    ///
    /// Timing, vectors and state are snapshotted together (`SNP-timing-000007`,
    /// `SNP-vectors-000007`, `SNP-state-000007`) because a replica that
    /// receives state without the matching timing and vectors cannot rebuild
    /// anything. `btrfs send` operates on one subvolume at a time, so each is
    /// its own read-only snapshot rather than a directory holding three.
    ///
    /// Returns the aligned counter; use [`Self::snapshot_name`] to address an
    /// individual subvolume's snapshot.
    pub async fn create_snapshot(&self) -> Result<String> {
        let counter = self.create_snapshot_aligned().await?;
        Ok(Self::snapshot_name("state", counter))
    }

    /// Same as [`Self::create_snapshot`] but returns the aligned counter, which
    /// is what replication needs in order to address the whole triple.
    pub async fn create_snapshot_aligned(&self) -> Result<u64> {
        let snapshot_dir = self.base_path.join("snapshots");
        let counter = self.next_aligned_snapshot_counter(&snapshot_dir).await?;

        for label in SNAPSHOT_LABELS {
            let source = self.subvolume_for_label(label);
            let name = Self::snapshot_name(label, counter);
            let snapshot_path = snapshot_dir.join(&name);
            self.snapshot_one(source, &snapshot_path, &name).await;
        }

        // Update last snapshot time
        *self.last_snapshot_time.write().await = Instant::now();

        // Prune old snapshots according to retention policy
        if let Err(e) = self.prune_snapshots().await {
            warn!("Failed to prune snapshots: {}", e);
        }

        Ok(counter)
    }

    /// Snapshot the chain and send it offsite, incrementally against the last
    /// successfully replicated counter.
    ///
    /// The parent counter is persisted locally (`.replicated-counter`), because
    /// `btrfs send -p` requires a parent that the *remote* already has; sending
    /// a delta against a snapshot the remote never received is rejected by
    /// `btrfs receive`. If the recorded parent's local snapshots are gone
    /// (pruned), this falls back to a full send.
    ///
    /// `on_receive` is an absolute program path on the remote, invoked as
    /// `<program> <counter>` once the whole triple has landed. That arrival is
    /// the trigger for the remote to re-point its working subvolumes and index
    /// the new vectors — no watcher, no polling loop on the replica.
    pub async fn replicate(
        &self,
        remote_host: &str,
        remote_path: &str,
        on_receive: Option<&str>,
    ) -> Result<ReplicationReport> {
        let counter = self.create_snapshot_aligned().await?;
        let parent = match self.read_replicated_counter().await? {
            Some(prev) if prev < counter && self.snapshot_triple_exists(prev).await => Some(prev),
            Some(prev) if prev >= counter => None,
            _ => None,
        };

        let mut sent = Vec::new();
        let mut failed = Vec::new();
        for label in SNAPSHOT_LABELS {
            let name = Self::snapshot_name(label, counter);
            let parent_name = parent.map(|prev| Self::snapshot_name(label, prev));
            match self
                .stream_to_remote_incremental(
                    &name,
                    parent_name.as_deref(),
                    remote_host,
                    remote_path,
                )
                .await
            {
                Ok(()) => sent.push(name),
                Err(err) => failed.push((name, format!("{err:#}"))),
            }
        }

        // Only advance the parent pointer when the whole triple landed;
        // otherwise the next incremental send would assume a parent the remote
        // is missing for at least one subvolume.
        let mut hook_error = None;
        if failed.is_empty() {
            self.write_replicated_counter(counter).await?;
            if let Some(program) = on_receive {
                if let Err(err) = self.run_receive_hook(remote_host, program, counter).await {
                    hook_error = Some(format!("{err:#}"));
                }
            }
        }

        Ok(ReplicationReport {
            counter,
            parent,
            sent,
            failed,
            hook_error,
        })
    }

    /// Invoke the remote post-receive program as `<program> <counter>`.
    /// Argv form, no remote shell string, and the program path is validated the
    /// same way as any other remote path.
    async fn run_receive_hook(&self, remote_host: &str, program: &str, counter: u64) -> Result<()> {
        validate_remote_host(remote_host).context("invalid remote host")?;
        validate_btrfs_path(Path::new(program)).context("invalid on-receive program path")?;

        let output = Command::new("ssh")
            .arg("--")
            .arg(remote_host)
            .arg(program)
            .arg(counter.to_string())
            .output()
            .await
            .context("failed to invoke the remote post-receive hook")?;

        if !output.status.success() {
            anyhow::bail!(
                "remote hook `{program} {counter}` failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        info!(
            "post-receive hook on {} reported: {}",
            remote_host,
            String::from_utf8_lossy(&output.stdout).trim()
        );
        Ok(())
    }

    async fn snapshot_triple_exists(&self, counter: u64) -> bool {
        let snapshot_dir = self.base_path.join("snapshots");
        for label in SNAPSHOT_LABELS {
            let path = snapshot_dir.join(Self::snapshot_name(label, counter));
            if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                return false;
            }
        }
        true
    }

    async fn read_replicated_counter(&self) -> Result<Option<u64>> {
        let path = self.base_path.join(REPLICATED_COUNTER_FILE);
        match tokio::fs::read_to_string(&path).await {
            Ok(text) => Ok(text.trim().parse().ok()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => {
                Err(anyhow::Error::from(err).context(format!("failed to read {}", path.display())))
            }
        }
    }

    async fn write_replicated_counter(&self, counter: u64) -> Result<()> {
        let path = self.base_path.join(REPLICATED_COUNTER_FILE);
        tokio::fs::write(&path, format!("{counter}\n"))
            .await
            .with_context(|| format!("failed to write {}", path.display()))
    }

    async fn snapshot_one(&self, source: &Path, snapshot_path: &Path, name: &str) {
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(source)
            .arg(snapshot_path)
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                info!("Created snapshot: {}", name);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                // Fall back to regular copy
                if stderr.contains("not a btrfs") {
                    debug!("BTRFS not available, using regular copy for snapshot");
                    if let Err(e) = tokio::fs::create_dir_all(snapshot_path).await {
                        warn!("Failed to create snapshot dir {}: {}", name, e);
                        return;
                    }
                    if let Err(e) = copy_dir_recursive(source, snapshot_path).await {
                        warn!("Failed to copy snapshot {}: {}", name, e);
                    }
                } else {
                    warn!("Snapshot failed: {}", stderr);
                }
            }
            Err(e) => {
                warn!("Failed to create snapshot: {}", e);
            }
        }
    }

    /// Name of the snapshot holding `label` (`timing` / `vectors` / `state`)
    /// at an aligned counter.
    pub fn snapshot_name(label: &str, counter: u64) -> String {
        format!("{}-{:06}", Self::snapshot_prefix(label), counter)
    }

    fn subvolume_for_label(&self, label: &str) -> &Path {
        match label {
            "timing" => &self.timing_subvol,
            "vectors" => &self.vector_subvol,
            _ => &self.state_subvol,
        }
    }

    /// One counter shared by all three subvolumes, so a snapshot triple is
    /// always addressable by a single number even if one member failed.
    async fn next_aligned_snapshot_counter(&self, snapshot_dir: &Path) -> Result<u64> {
        let mut next = 1u64;
        for label in SNAPSHOT_LABELS {
            let candidate = self
                .next_snapshot_counter(snapshot_dir, &Self::snapshot_prefix(label))
                .await?;
            next = next.max(candidate);
        }
        Ok(next)
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
        self.stream_to_remote_incremental(snapshot_name, None, remote_host, remote_path)
            .await
    }

    /// Incremental variant: sends only the delta against `parent_snapshot`
    /// (`btrfs send -p`), which is what makes offsite replication affordable
    /// and gives the receiving side a bounded set of new files to index.
    ///
    /// The parent must already exist on the remote, otherwise `btrfs receive`
    /// rejects the stream. Pass `None` for the first (full) send of a chain.
    pub async fn stream_to_remote_incremental(
        &self,
        snapshot_name: &str,
        parent_snapshot: Option<&str>,
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

        let parent_path = match parent_snapshot {
            Some(parent) => {
                let path = self.base_path.join("snapshots").join(parent);
                if !path.exists() {
                    anyhow::bail!("Parent snapshot not found: {}", parent);
                }
                validate_btrfs_path(&path).context("invalid parent snapshot path")?;
                Some(path)
            }
            None => None,
        };

        info!(
            "Streaming snapshot {} (parent {}) to {}:{}",
            snapshot_name,
            parent_snapshot.unwrap_or("none"),
            remote_host,
            remote_path
        );

        // ---- Argv-form two-process pipeline; no shell on the local side. ----
        let mut send = Command::new("btrfs");
        send.arg("send");
        if let Some(parent) = &parent_path {
            send.arg("-p").arg(parent);
        }
        let mut send_child = send
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
        let recv_child = Command::new("ssh")
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

    /// Prune old snapshots according to retention policy.
    ///
    /// Each subvolume family is pruned independently under the same policy, so
    /// the aligned triple ages out together.
    async fn prune_snapshots(&self) -> Result<()> {
        for label in SNAPSHOT_LABELS {
            self.prune_snapshot_family(&Self::snapshot_prefix(label))
                .await?;
        }
        Ok(())
    }

    async fn prune_snapshot_family(&self, prefix: &str) -> Result<()> {
        let snapshot_dir = self.base_path.join("snapshots");
        let mut entries = tokio::fs::read_dir(&snapshot_dir).await?;
        let mut snapshots: Vec<SnapshotEntry> = Vec::new();

        let name_prefix = format!("{}-", prefix);
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&name_prefix) {
                continue;
            }

            // btrfs's own creation time first: a snapshot inherits the source
            // subvolume's inode birth/mtime, so `metadata` reports every
            // snapshot of one subvolume as equally old and the whole policy
            // collapses to a single survivor. The inode fallback only matters
            // for the plain-directory copy path used off btrfs.
            let created = match crate::btrfs_delta::creation_time(&entry.path()).await {
                Ok(Some(created)) => Some(created),
                _ => {
                    let metadata = tokio::fs::metadata(entry.path()).await?;
                    metadata
                        .created()
                        .or_else(|_| metadata.modified())
                        .ok()
                        .and_then(system_time_to_utc)
                }
            };

            if let Some(created) = created {
                snapshots.push(SnapshotEntry {
                    counter: parse_snapshot_counter(&name, prefix).unwrap_or(0),
                    name,
                    created,
                });
            }
        }

        let keep = retain_snapshots(
            &mut snapshots,
            &self.retention_policy,
            Utc::now(),
            self.read_replicated_counter().await.unwrap_or(None),
        );

        // Delete old snapshots
        let mut deleted = 0;
        for SnapshotEntry { name, .. } in &snapshots {
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

    /// Snapshot name prefix per subvolume. `state` keeps its own env override
    /// for backwards compatibility with existing `SNP-state-*` snapshots.
    fn snapshot_prefix(label: &str) -> String {
        if label == "state" {
            Self::state_snapshot_prefix()
        } else {
            format!("SNP-{label}")
        }
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
// Block references and vector encoding
// ----------------------------------------------------------------------------

/// A block as it exists on disk: its authoritative timing record, plus whether
/// its vector projection has landed.
///
/// `data_fields` is the block's payload flattened to sorted dotted paths, which
/// is what makes [`Self::embedding_text`] deterministic — the on-disk JSON
/// object order is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainBlockRef {
    pub block_num: u64,
    pub hash: String,
    /// Plugin id (the `category` field of the block event).
    pub category: String,
    /// Method or operation name (the `action` field).
    pub action: String,
    pub timestamp: u64,
    pub data_fields: Vec<(String, String)>,
    pub has_vector: bool,
}

impl ChainBlockRef {
    fn from_timing_bytes(block_num: u64, mut bytes: Vec<u8>, has_vector: bool) -> Result<Self> {
        let value = simd_json::to_owned_value(&mut bytes)
            .map_err(|e| anyhow::anyhow!("invalid JSON in timing record: {e}"))?;

        let mut data_fields = Vec::new();
        if let Some(data) = owned_get(&value, "data") {
            flatten_sorted(data, "data", &mut data_fields);
        }

        Ok(Self {
            block_num,
            hash: owned_str(&value, "hash").unwrap_or_default(),
            category: owned_str(&value, "category").unwrap_or_default(),
            action: owned_str(&value, "action").unwrap_or_default(),
            timestamp: owned_u64(&value, "timestamp").unwrap_or_default(),
            data_fields,
            has_vector,
        })
    }

    /// Value of one flattened field, e.g. `data.metadata.event_hash`.
    pub fn field(&self, path: &str) -> Option<&str> {
        self.data_fields
            .iter()
            .find(|(key, _)| key == path)
            .map(|(_, value)| value.as_str())
    }

    /// Deterministic retrieval text for this block.
    ///
    /// Both the origin (which embeds) and any replica (which may re-embed
    /// after a policy change) must derive identical text from identical
    /// timing records, so this is the single renderer for block embeddings.
    pub fn embedding_text(&self) -> String {
        let mut lines = vec![
            format!("block: {}", self.block_num),
            format!("plugin: {}", self.category),
            format!("action: {}", self.action),
            format!("hash: {}", self.hash),
            format!("timestamp: {}", self.timestamp),
        ];
        for (key, value) in &self.data_fields {
            lines.push(format!("{key}: {value}"));
        }
        lines.join("\n")
    }
}

fn owned_get<'a>(value: &'a simd_json::OwnedValue, key: &str) -> Option<&'a simd_json::OwnedValue> {
    match value {
        simd_json::OwnedValue::Object(map) => map.get(key),
        _ => None,
    }
}

fn owned_str(value: &simd_json::OwnedValue, key: &str) -> Option<String> {
    match owned_get(value, key)? {
        simd_json::OwnedValue::String(s) => Some(s.clone()),
        other => Some(scalar_to_string(other)?),
    }
}

fn owned_u64(value: &simd_json::OwnedValue, key: &str) -> Option<u64> {
    match owned_get(value, key)? {
        simd_json::OwnedValue::Static(simd_json::StaticNode::U64(n)) => Some(*n),
        simd_json::OwnedValue::Static(simd_json::StaticNode::I64(n)) => u64::try_from(*n).ok(),
        _ => None,
    }
}

fn scalar_to_string(value: &simd_json::OwnedValue) -> Option<String> {
    use simd_json::StaticNode;
    match value {
        simd_json::OwnedValue::String(s) => Some(s.clone()),
        simd_json::OwnedValue::Static(StaticNode::U64(n)) => Some(n.to_string()),
        simd_json::OwnedValue::Static(StaticNode::I64(n)) => Some(n.to_string()),
        simd_json::OwnedValue::Static(StaticNode::F64(n)) => Some(n.to_string()),
        simd_json::OwnedValue::Static(StaticNode::Bool(b)) => Some(b.to_string()),
        simd_json::OwnedValue::Static(StaticNode::Null) => Some("null".to_string()),
        _ => None,
    }
}

/// Flatten a JSON value into sorted `dotted.path` / value pairs.
fn flatten_sorted(value: &simd_json::OwnedValue, prefix: &str, out: &mut Vec<(String, String)>) {
    match value {
        simd_json::OwnedValue::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(child) = map.get(key.as_str()) {
                    flatten_sorted(child, &format!("{prefix}.{key}"), out);
                }
            }
        }
        simd_json::OwnedValue::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                flatten_sorted(item, &format!("{prefix}[{index}]"), out);
            }
        }
        scalar => {
            if let Some(mut text) = scalar_to_string(scalar) {
                if text.len() > MAX_EMBEDDING_FIELD_LEN {
                    text.truncate(MAX_EMBEDDING_FIELD_LEN);
                    text.push('…');
                }
                out.push((prefix.to_string(), text));
            }
        }
    }
}

fn timing_file_name(block_num: u64) -> String {
    format!("block-{:012}.json", block_num)
}

fn vector_file_name(block_num: u64) -> String {
    format!("vec-{:012}.bin", block_num)
}

/// Block number from a timing file name, or `None` for anything else in the
/// subvolume.
pub fn parse_block_number(file_name: &str) -> Option<u64> {
    file_name
        .strip_prefix("block-")?
        .strip_suffix(".json")?
        .parse()
        .ok()
}

/// Block number from a vector file name — the receive-side counterpart of
/// [`parse_block_number`].
pub fn parse_vector_block_number(file_name: &str) -> Option<u64> {
    file_name
        .strip_prefix("vec-")?
        .strip_suffix(".bin")?
        .parse()
        .ok()
}

/// Decode a raw little-endian f32 vector file.
pub fn decode_vector(bytes: &[u8]) -> Result<Vec<f32>> {
    anyhow::ensure!(
        !bytes.is_empty() && bytes.len().is_multiple_of(4),
        "vector file length {} is not a non-zero multiple of 4",
        bytes.len()
    );
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

/// Encode a vector to its on-disk little-endian f32 form.
pub fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Highest block number already written, so a restart appends instead of
/// renumbering over existing records.
async fn highest_block_number(timing_subvol: &Path) -> Result<u64> {
    let mut entries = match tokio::fs::read_dir(timing_subvol).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => {
            return Err(anyhow::Error::from(err)
                .context(format!("failed to read {}", timing_subvol.display())))
        }
    };

    let mut highest = 0u64;
    while let Some(entry) = entries.next_entry().await? {
        if let Some(block_num) = parse_block_number(&entry.file_name().to_string_lossy()) {
            highest = highest.max(block_num);
        }
    }
    Ok(highest)
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

    const TIMING_RECORD: &[u8] = br#"{
      "timestamp": 1786156636821,
      "category": "zeroclaw",
      "action": "Chat",
      "data": {
        "plugin_id": "zeroclaw",
        "operation": "Chat",
        "data_hash": "027ef6db",
        "metadata": { "event_hash": "97aadd58", "decision": "Allow", "event_id": 89 }
      },
      "hash": "027ef6db",
      "vector": []
    }"#;

    #[test]
    fn vector_round_trips_through_on_disk_encoding() {
        let original = vec![0.25f32, -0.5, 1.5, 0.0];
        let decoded = decode_vector(&encode_vector(&original)).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn rejects_truncated_or_empty_vector_files() {
        assert!(decode_vector(&[]).is_err(), "empty file is not a vector");
        assert!(
            decode_vector(&[1, 2, 3]).is_err(),
            "length must be a multiple of 4"
        );
    }

    #[test]
    fn parses_block_and_vector_file_names() {
        assert_eq!(parse_block_number("block-000000000042.json"), Some(42));
        assert_eq!(parse_vector_block_number("vec-000000000042.bin"), Some(42));
        assert_eq!(parse_block_number("vec-000000000042.bin"), None);
        assert_eq!(parse_block_number("notes.txt"), None);
    }

    #[test]
    fn block_ref_reads_timing_record_and_flattens_data() {
        let block = ChainBlockRef::from_timing_bytes(42, TIMING_RECORD.to_vec(), false).unwrap();

        assert_eq!(block.block_num, 42);
        assert_eq!(block.category, "zeroclaw");
        assert_eq!(block.action, "Chat");
        assert_eq!(block.timestamp, 1786156636821);
        assert_eq!(block.field("data.metadata.event_hash"), Some("97aadd58"));
        assert!(!block.has_vector);
    }

    #[test]
    fn embedding_text_is_deterministic_and_sorted() {
        let first = ChainBlockRef::from_timing_bytes(42, TIMING_RECORD.to_vec(), false).unwrap();
        let second = ChainBlockRef::from_timing_bytes(42, TIMING_RECORD.to_vec(), false).unwrap();
        assert_eq!(
            first.embedding_text(),
            second.embedding_text(),
            "identical timing records must render identical text so origin and \
             replica derive the same vector"
        );

        let keys: Vec<&String> = first.data_fields.iter().map(|(key, _)| key).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "flattened fields must be key-sorted");
        assert!(first
            .embedding_text()
            .starts_with("block: 42\nplugin: zeroclaw"));
    }

    #[test]
    fn long_fields_are_capped_in_embedding_text() {
        let long = "x".repeat(MAX_EMBEDDING_FIELD_LEN * 2);
        let record = format!(
            r#"{{"timestamp":1,"category":"c","action":"a","hash":"h","vector":[],"data":{{"big":"{long}"}}}}"#
        );
        let block = ChainBlockRef::from_timing_bytes(1, record.into_bytes(), true).unwrap();
        let value = block.field("data.big").unwrap();
        assert!(
            value.chars().count() <= MAX_EMBEDDING_FIELD_LEN + 1,
            "one oversized payload field must not dominate the vector"
        );
    }

    fn entry(name: &str, created: DateTime<Utc>, counter: u64) -> SnapshotEntry {
        SnapshotEntry {
            name: name.to_string(),
            created,
            counter,
        }
    }

    #[test]
    fn parses_counter_out_of_snapshot_names() {
        assert_eq!(
            parse_snapshot_counter("SNP-vectors-000012", "SNP-vectors"),
            Some(12)
        );
        assert_eq!(
            parse_snapshot_counter("SNP-state-000001", "SNP-vectors"),
            None
        );
        assert_eq!(
            parse_snapshot_counter("SNP-vectors-latest", "SNP-vectors"),
            None
        );
    }

    #[test]
    fn retention_never_prunes_the_newest_snapshot_on_a_timestamp_tie() {
        // A snapshot of a subvolume that is never written inherits the source's
        // mtime, so every member of the family shares one timestamp and lands in
        // the same bucket. Before this rule, the tie-break was read_dir order and
        // the freshly created snapshot could be deleted immediately after the
        // aligned triple was made, breaking `btrfs send` with "unable to resolve".
        let stale = Utc::now() - chrono::Duration::days(10);
        let mut snapshots = vec![
            entry("SNP-state-000001", stale, 1),
            entry("SNP-state-000002", stale, 2),
        ];
        let policy = RetentionPolicy {
            hourly: 0,
            daily: 1,
            weekly: 0,
            quarterly: 0,
        };

        let keep = retain_snapshots(&mut snapshots, &policy, Utc::now(), None);
        assert!(
            keep.contains("SNP-state-000002"),
            "the newest counter must always survive: {keep:?}"
        );
    }

    #[test]
    fn retention_keeps_the_replication_parent() {
        let long_ago = Utc::now() - chrono::Duration::days(400);
        let mut snapshots = vec![
            entry("SNP-vectors-000005", long_ago, 5),
            entry("SNP-vectors-000009", Utc::now(), 9),
        ];
        let policy = RetentionPolicy {
            hourly: 1,
            daily: 0,
            weekly: 0,
            quarterly: 0,
        };

        let keep = retain_snapshots(&mut snapshots, &policy, Utc::now(), Some(5));
        assert!(
            keep.contains("SNP-vectors-000005"),
            "`btrfs send -p` needs the parent both sides still have: {keep:?}"
        );
        assert!(keep.contains("SNP-vectors-000009"));
    }

    #[test]
    fn retention_still_prunes_beyond_the_policy() {
        let now = Utc::now();
        let mut snapshots = vec![
            entry("SNP-timing-000001", now - chrono::Duration::hours(1), 1),
            entry("SNP-timing-000002", now - chrono::Duration::hours(2), 2),
            entry("SNP-timing-000003", now - chrono::Duration::hours(3), 3),
        ];
        let policy = RetentionPolicy {
            hourly: 1,
            daily: 0,
            weekly: 0,
            quarterly: 0,
        };

        let keep = retain_snapshots(&mut snapshots, &policy, now, None);
        assert_eq!(
            keep.len(),
            2,
            "one hourly slot plus the mandatory newest counter: {keep:?}"
        );
        assert!(keep.contains("SNP-timing-000001"), "newest by time");
        assert!(keep.contains("SNP-timing-000003"), "highest counter");
    }

    #[test]
    fn snapshot_names_are_aligned_per_subvolume() {
        assert_eq!(
            StreamingBlockchain::snapshot_name("timing", 7),
            "SNP-timing-000007"
        );
        assert_eq!(
            StreamingBlockchain::snapshot_name("vectors", 7),
            "SNP-vectors-000007"
        );
        assert_eq!(
            StreamingBlockchain::snapshot_name("state", 7),
            "SNP-state-000007",
            "state keeps its historical prefix so existing snapshots still match"
        );
    }

    #[tokio::test]
    async fn resumes_block_numbering_from_disk() {
        let dir = std::env::temp_dir().join(format!("op-chain-test-{}", uuid::Uuid::new_v4()));
        let timing = dir.join("timing");
        tokio::fs::create_dir_all(&timing).await.unwrap();
        assert_eq!(highest_block_number(&timing).await.unwrap(), 0);

        tokio::fs::write(timing.join(timing_file_name(7)), b"{}")
            .await
            .unwrap();
        tokio::fs::write(timing.join(timing_file_name(3)), b"{}")
            .await
            .unwrap();
        tokio::fs::write(timing.join("unrelated.json"), b"{}")
            .await
            .unwrap();

        assert_eq!(
            highest_block_number(&timing).await.unwrap(),
            7,
            "a restart must append after the highest existing block, not overwrite from 1"
        );
        tokio::fs::remove_dir_all(&dir).await.ok();
    }

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
