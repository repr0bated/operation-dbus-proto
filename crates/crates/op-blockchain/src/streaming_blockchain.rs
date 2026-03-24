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

#[derive(Debug, Clone, Copy)]
pub enum SnapshotInterval {
    PerOperation,
    EveryMinute,
    Every5Minutes,
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

impl Default for SnapshotInterval {
    fn default() -> Self {
        SnapshotInterval::Every15Minutes // Default to every 15 minutes for production
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
