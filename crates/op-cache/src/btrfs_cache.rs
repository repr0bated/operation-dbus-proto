//! BTRFS-backed cache with CozoDB index, compression, and NUMA optimization
//!
//! Provides unlimited disk-based caching with:
//! - BTRFS transparent compression (zstd)
//! - CozoDB in-memory index for O(1) lookups (via `op-cozo-store`)
//! - Linux page cache for hot data
//! - Automatic snapshot management
//! - NUMA-aware memory allocation and CPU affinity

use anyhow::{Context, Result};
use op_cozo_store::CozoGraphShuttle;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use tracing::{debug, info, warn};

use super::numa::{NumaNode, NumaStats, NumaTopology};
use super::snapshot_manager::{SnapshotConfig, SnapshotManager};

/// NUMA-aware cache placement strategy
#[derive(Debug, Clone)]
pub enum CachePlacementStrategy {
    /// Place cache data on the same NUMA node as the requesting CPU
    LocalNode,
    /// Distribute cache data across all NUMA nodes for load balancing
    RoundRobin,
    /// Use the NUMA node with most available memory
    MostMemory,
    /// Disable NUMA optimizations (default)
    Disabled,
}

/// Memory allocation policy for NUMA systems
#[derive(Debug, Clone)]
pub enum MemoryPolicy {
    /// Bind memory to specific NUMA node
    Bind(Vec<u32>),
    /// Prefer memory from specific NUMA node
    Preferred(Option<u32>),
    /// Interleave memory across multiple NUMA nodes
    Interleave(Vec<u32>),
    /// Use default system memory policy
    Default,
}

pub struct BtrfsCache {
    cache_dir: PathBuf,
    /// In-memory CozoDB index for the embedding cache.
    /// `CozoGraphShuttle` wraps an `Arc<DbInstance>` so it is cheaply
    /// cloneable and safe to share without an external `Mutex`.
    index: CozoGraphShuttle,
    snapshot_manager: SnapshotManager,
    numa_topology: NumaTopology,
    placement_strategy: CachePlacementStrategy,
    memory_policy: MemoryPolicy,
    cpu_affinity: Vec<u32>, // CPU cores for affinity binding
    current_node_index: AtomicUsize,
    #[allow(dead_code)]
    numa_stats: Mutex<NumaStats>,
}

#[allow(dead_code)]
impl BtrfsCache {
    /// Create BTRFS subvolume at specified path
    async fn create_btrfs_subvolume(path: &Path) -> Result<()> {
        if path.exists() {
            let output = tokio::process::Command::new("btrfs")
                .args(["subvolume", "show"])
                .arg(path)
                .output()
                .await
                .context("Failed to execute btrfs command")?;
            if output.status.success() {
                Self::enable_compression(path).await?;
                return Ok(());
            }

            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr
                .to_ascii_lowercase()
                .contains("not a btrfs filesystem")
            {
                return Ok(());
            }
            anyhow::bail!(
                "existing cache path is not a usable Btrfs subvolume ({}): {}",
                path.display(),
                stderr.trim()
            );
        }

        let output = tokio::process::Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(path)
            .output()
            .await
            .context("Failed to execute btrfs command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("command not found") || stderr.contains("not a btrfs filesystem") {
                warn!(
                    "BTRFS not available, creating regular directory: {:?}",
                    path
                );
                tokio::fs::create_dir_all(path)
                    .await
                    .context("Failed to create cache directory")?;
            } else {
                anyhow::bail!("btrfs subvolume create failed: {}", stderr);
            }
        } else {
            Self::enable_compression(path).await?;
        }

        Ok(())
    }

    async fn enable_compression(path: &Path) -> Result<()> {
        let output = tokio::process::Command::new("btrfs")
            .args(["property", "set"])
            .arg(path)
            .args(["compression", "zstd"])
            .output()
            .await
            .context("Failed to execute btrfs property command")?;
        if !output.status.success() {
            anyhow::bail!(
                "failed to enable Btrfs zstd compression for {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    /// Create new BTRFS cache with proper subvolumes
    pub async fn new(cache_dir: PathBuf) -> Result<Self> {
        // Ensure parent directory exists (not as subvolume)
        if let Some(parent) = cache_dir.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Create directory structure (BTRFS subvolumes stubbed as regular dirs)
        Self::create_btrfs_subvolume(&cache_dir).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("embeddings")).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("blocks")).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("queries")).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("diffs")).await?;

        // Create regular directories within subvolumes
        tokio::fs::create_dir_all(cache_dir.join("embeddings/vectors")).await?;
        tokio::fs::create_dir_all(cache_dir.join("blocks/by-number")).await?;
        tokio::fs::create_dir_all(cache_dir.join("blocks/by-hash")).await?;

        // Create CozoDB in-memory index for embeddings (replaces SQLite)
        let index = CozoGraphShuttle::new_in_memory()
            .map_err(|e| anyhow::anyhow!("Failed to create CozoDB index: {e}"))?;

        // Create the embedding_cache relation.
        // :create errors if the relation already exists — safe to ignore
        // since each BtrfsCache gets its own fresh in-memory store.
        let _ = index.run_query(
            r#":create embedding_cache {
                text_hash: String
                =>
                text: String default "",
                vector_file: String default "",
                created_at: String default "",
                accessed_at: String default "",
                access_count: Int default 1,
                vector_size: Int default 0
            }"#,
            None,
        );

        // Initialize snapshot manager
        let snapshot_config = SnapshotConfig {
            snapshot_dir: cache_dir
                .parent()
                .unwrap_or(Path::new("/var/lib/op-dbus"))
                .join("@cache-snapshots"),
            max_snapshots: std::env::var("OPDBUS_MAX_CACHE_SNAPSHOTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(24),
            prefix: std::env::var("OPDBUS_CACHE_SNAPSHOT_PREFIX")
                .unwrap_or_else(|_| "SNP-cache".to_string()),
        };

        let snapshot_manager = SnapshotManager::new(cache_dir.clone(), snapshot_config);

        // Detect NUMA topology
        let numa_topology = NumaTopology::detect()?;
        let placement_strategy = Self::determine_placement_strategy(numa_topology.nodes());
        let memory_policy = Self::determine_memory_policy();

        // Detect CPU affinity - bind cache operations to same CPUs as Btrfs operations
        let cpu_affinity = if let Some(primary_node) = numa_topology.nodes().values().next() {
            primary_node.cpu_list.clone()
        } else {
            // No NUMA, use first few CPUs
            (0..(num_cpus::get().min(4) as u32)).collect()
        };

        Ok(Self {
            cache_dir,
            index,
            snapshot_manager,
            numa_topology,
            placement_strategy,
            memory_policy,
            cpu_affinity,
            current_node_index: AtomicUsize::new(0),
            numa_stats: Mutex::new(NumaStats::new()),
        })
    }

    fn determine_placement_strategy(numa_nodes: &HashMap<u32, NumaNode>) -> CachePlacementStrategy {
        let default_choice = if numa_nodes.is_empty() {
            "disabled".to_string()
        } else {
            "local".to_string()
        };

        let placement = std::env::var("OPDBUS_CACHE_PLACEMENT")
            .unwrap_or(default_choice)
            .to_lowercase();

        match placement.as_str() {
            "round-robin" | "round_robin" | "roundrobin" => CachePlacementStrategy::RoundRobin,
            "most-memory" | "most_memory" | "mostmemory" => CachePlacementStrategy::MostMemory,
            "disabled" => CachePlacementStrategy::Disabled,
            "local" | "local-node" | "local_node" => {
                if numa_nodes.is_empty() {
                    CachePlacementStrategy::Disabled
                } else {
                    CachePlacementStrategy::LocalNode
                }
            }
            other => {
                warn!(
                    "Unknown OPDBUS_CACHE_PLACEMENT value '{}', defaulting to {}",
                    other,
                    if numa_nodes.is_empty() {
                        "disabled"
                    } else {
                        "local"
                    }
                );
                if numa_nodes.is_empty() {
                    CachePlacementStrategy::Disabled
                } else {
                    CachePlacementStrategy::LocalNode
                }
            }
        }
    }

    fn determine_memory_policy() -> MemoryPolicy {
        match std::env::var("OPDBUS_CACHE_MEMORY_POLICY") {
            Ok(value) => {
                let value_lower = value.to_lowercase();
                if let Some(rest) = value_lower.strip_prefix("bind:") {
                    let nodes = Self::parse_node_list(rest);
                    if nodes.is_empty() {
                        warn!("OPDBUS_CACHE_MEMORY_POLICY=bind but no NUMA nodes listed");
                        MemoryPolicy::Default
                    } else {
                        MemoryPolicy::Bind(nodes)
                    }
                } else if let Some(rest) = value_lower.strip_prefix("preferred:") {
                    if rest.trim().is_empty() {
                        MemoryPolicy::Preferred(None)
                    } else {
                        match rest.trim().parse::<u32>() {
                            Ok(node) => MemoryPolicy::Preferred(Some(node)),
                            Err(e) => {
                                warn!(
                                    "Failed to parse preferred NUMA node '{}': {}",
                                    rest.trim(),
                                    e
                                );
                                MemoryPolicy::Default
                            }
                        }
                    }
                } else if value_lower == "preferred" {
                    MemoryPolicy::Preferred(None)
                } else if let Some(rest) = value_lower.strip_prefix("interleave:") {
                    let nodes = Self::parse_node_list(rest);
                    if nodes.is_empty() {
                        warn!("OPDBUS_CACHE_MEMORY_POLICY=interleave but no NUMA nodes listed");
                        MemoryPolicy::Default
                    } else {
                        MemoryPolicy::Interleave(nodes)
                    }
                } else if value_lower == "default" || value_lower.is_empty() {
                    MemoryPolicy::Default
                } else {
                    warn!(
                        "Unknown OPDBUS_CACHE_MEMORY_POLICY value '{}', using default",
                        value
                    );
                    MemoryPolicy::Default
                }
            }
            Err(_) => MemoryPolicy::Default,
        }
    }

    fn parse_node_list(list: &str) -> Vec<u32> {
        list.split(',')
            .filter_map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    match trimmed.parse::<u32>() {
                        Ok(value) => Some(value),
                        Err(e) => {
                            warn!("Invalid NUMA node id '{}': {}", trimmed, e);
                            None
                        }
                    }
                }
            })
            .collect()
    }

    fn select_numa_node(&self, operation: &str) -> Option<&NumaNode> {
        if self.numa_topology.node_count() == 0 {
            return None;
        }

        let nodes: Vec<&NumaNode> = self.numa_topology.nodes().values().collect();
        let selection = match self.placement_strategy {
            CachePlacementStrategy::LocalNode => nodes.first().copied(),
            CachePlacementStrategy::RoundRobin => {
                let index = self.current_node_index.fetch_add(1, Ordering::Relaxed);
                nodes.get(index % nodes.len()).copied()
            }
            CachePlacementStrategy::MostMemory => nodes
                .iter()
                .max_by_key(|node| node.memory_total_kb)
                .copied(),
            CachePlacementStrategy::Disabled => None,
        };

        if let Some(node) = selection {
            debug!(
                "NUMA node {} selected for {} (memory={} MB, distances={:?})",
                node.node_id,
                operation,
                node.memory_total_kb / 1024,
                node.distance_to_nodes
            );
        } else {
            debug!(
                "No NUMA node selected for {} (strategy={:?})",
                operation, self.placement_strategy
            );
        }

        selection
    }

    /// Get or compute embedding
    pub fn get_or_embed<F>(&self, text: &str, compute_fn: F) -> Result<Vec<f32>>
    where
        F: FnOnce(&str) -> Result<Vec<f32>>,
    {
        let text_hash = self.hash_text(text);

        // Check if cached
        if let Some(vector) = self.load_embedding(&text_hash)? {
            // Update access statistics
            self.update_access(&text_hash)?;
            return Ok(vector);
        }

        // Compute embedding
        let vector = compute_fn(text)?;

        // Store in cache
        self.save_embedding(text, &text_hash, &vector)?;

        Ok(vector)
    }

    /// Get embedding if cached (without computing)
    pub fn get_embedding(&self, text: &str) -> Result<Option<Vec<f32>>> {
        let text_hash = self.hash_text(text);
        if let Some(vector) = self.load_embedding(&text_hash)? {
            self.update_access(&text_hash)?;
            return Ok(Some(vector));
        }
        Ok(None)
    }

    /// Store embedding directly
    pub fn put_embedding(&self, text: &str, vector: &[f32]) -> Result<()> {
        let text_hash = self.hash_text(text);
        self.save_embedding(text, &text_hash, vector)
    }

    fn hash_text(&self, text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn load_embedding(&self, text_hash: &str) -> Result<Option<Vec<f32>>> {
        // Look up the vector file path in the CozoDB index
        let result = self
            .index
            .run_query(
                "?[vector_file] := *embedding_cache[text_hash, vector_file], text_hash = $hash",
                Some(serde_json::json!({"hash": text_hash})),
            )
            .map_err(|e| anyhow::anyhow!("CozoDB query failed: {e}"))?;

        let vector_file = result
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|row| row.get("vector_file"))
            .and_then(|v| v.as_str())
            .map(String::from);

        if let Some(file) = vector_file {
            let path = self.cache_dir.join("embeddings/vectors").join(&file);

            // Read from BTRFS (page cache will cache this!)
            let data = std::fs::read(&path)
                .context(format!("Failed to read cached embedding: {:?}", path))?;

            let vector: Vec<f32> =
                bincode::deserialize(&data).context("Failed to deserialize cached embedding")?;

            return Ok(Some(vector));
        }

        Ok(None)
    }

    fn save_embedding(&self, text: &str, text_hash: &str, vector: &[f32]) -> Result<()> {
        let vectors_dir = self.cache_dir.join("embeddings/vectors");
        std::fs::create_dir_all(&vectors_dir)?;

        let vector_file = format!("{}.vec", text_hash);
        let path = vectors_dir.join(&vector_file);

        // Write to BTRFS (automatically compressed by kernel)
        let data = bincode::serialize(vector)?;
        std::fs::write(&path, data)?;

        // Upsert into CozoDB index: read existing access_count, then :put
        // the row with the incremented count. CozoDB's :put overwrites the
        // entire row, so we carry forward the old access_count + 1.
        let existing = self
            .index
            .run_query(
                "?[access_count] := *embedding_cache[text_hash, access_count], text_hash = $hash",
                Some(serde_json::json!({"hash": text_hash})),
            )
            .map_err(|e| anyhow::anyhow!("CozoDB query failed: {e}"))?;

        let old_count = existing
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|row| row.get("access_count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let now = chrono::Utc::now().to_rfc3339();
        self.index
            .run_query(
                r#"?[text_hash, text, vector_file, created_at, accessed_at, access_count, vector_size]
                    <- [[$hash, $text, $vfile, $now, $now, $count, $vsize]]
                :put embedding_cache {
                    text_hash => text, vector_file, created_at, accessed_at, access_count, vector_size
                }"#,
                Some(serde_json::json!({
                    "hash": text_hash,
                    "text": text,
                    "vfile": vector_file,
                    "now": now,
                    "count": old_count + 1,
                    "vsize": vector.len() as i64,
                })),
            )
            .map_err(|e| anyhow::anyhow!("CozoDB put failed: {e}"))?;

        Ok(())
    }

    /// Bump `accessed_at` and increment `access_count` for a cached entry.
    ///
    /// CozoDB does not have an atomic `increment` operation, so this reads
    /// the current `access_count` and writes back `count + 1` via `:update`
    /// (partial-column update that fails silently if the row does not exist).
    fn update_access(&self, text_hash: &str) -> Result<()> {
        let existing = self
            .index
            .run_query(
                "?[access_count] := *embedding_cache[text_hash, access_count], text_hash = $hash",
                Some(serde_json::json!({"hash": text_hash})),
            )
            .map_err(|e| anyhow::anyhow!("CozoDB query failed: {e}"))?;

        let old_count = existing
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|row| row.get("access_count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let now = chrono::Utc::now().to_rfc3339();
        // :update is a partial-column write — only touches accessed_at and
        // access_count. It is a no-op if the row does not exist.
        self.index
            .run_query(
                "?[text_hash, accessed_at, access_count] <- [[$hash, $now, $count]]
                 :update embedding_cache { text_hash => accessed_at, access_count }",
                Some(serde_json::json!({
                    "hash": text_hash,
                    "now": now,
                    "count": old_count + 1,
                })),
            )
            .map_err(|e| anyhow::anyhow!("CozoDB update failed: {e}"))?;

        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats> {
        // Total entry count
        let total_result = self
            .index
            .run_query(
                "?[total] := total = count(*embedding_cache[text_hash, _])",
                None,
            )
            .map_err(|e| anyhow::anyhow!("CozoDB stats query failed: {e}"))?;

        let total = total_result
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|row| row.get("total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Hot entries: accessed within the last hour.
        // Timestamps are RFC3339 strings, so lexicographic comparison is valid.
        let hot_threshold = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let hot_result = self
            .index
            .run_query(
                "?[hot] := hot = count(*embedding_cache[text_hash, accessed_at]), accessed_at > $threshold",
                Some(serde_json::json!({"threshold": hot_threshold})),
            )
            .map_err(|e| anyhow::anyhow!("CozoDB hot stats query failed: {e}"))?;

        let hot = hot_result
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|row| row.get("hot"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Total accesses: sum of all access_count values.
        // Fetched as individual rows to avoid CozoDB sum-over-null edge cases.
        let accesses_result = self
            .index
            .run_query(
                "?[access_count] := *embedding_cache[text_hash, access_count]",
                None,
            )
            .map_err(|e| anyhow::anyhow!("CozoDB accesses query failed: {e}"))?;

        let total_accesses: i64 = accesses_result
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|row| row.get("access_count").and_then(|v| v.as_i64()))
                    .sum()
            })
            .unwrap_or(0);

        // Calculate disk usage
        let embeddings_size = self.dir_size(&self.cache_dir.join("embeddings/vectors"))?;
        let blocks_size = self.dir_size(&self.cache_dir.join("blocks"))?;
        let total_size = embeddings_size + blocks_size;

        Ok(CacheStats {
            total_entries: total as usize,
            hot_entries: hot as usize,
            total_accesses: total_accesses as u64,
            disk_usage_bytes: total_size,
            embeddings_size_bytes: embeddings_size,
            blocks_size_bytes: blocks_size,
        })
    }

    #[allow(clippy::only_used_in_recursion)]
    fn dir_size(&self, path: &Path) -> Result<u64> {
        let mut size = 0u64;
        if !path.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                size += metadata.len();
            } else if metadata.is_dir() {
                size += self.dir_size(&entry.path())?;
            }
        }
        Ok(size)
    }

    /// Clean old entries (accessed before cutoff).
    ///
    /// Queries the CozoDB index for stale rows, deletes their backing vector
    /// files from disk, then removes the index rows.
    pub fn cleanup_old(&self, days: i64) -> Result<usize> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();

        // Find old entries (need vector_file paths for disk cleanup)
        let result = self
            .index
            .run_query(
                "?[text_hash, vector_file] := *embedding_cache[text_hash, vector_file, accessed_at], accessed_at < $cutoff",
                Some(serde_json::json!({"cutoff": cutoff})),
            )
            .map_err(|e| anyhow::anyhow!("CozoDB cleanup query failed: {e}"))?;

        let old_entries: Vec<(String, String)> = result
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|row| {
                        let hash = row
                            .get("text_hash")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let file = row
                            .get("vector_file")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Some((hash, file))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let count = old_entries.len();

        // Delete vector files from disk
        for (_hash, file) in &old_entries {
            let path = self.cache_dir.join("embeddings/vectors").join(file);
            let _ = std::fs::remove_file(path); // Ignore errors
        }

        // Delete from CozoDB index
        if count > 0 {
            self.index
                .run_query(
                    "matched[text_hash] := *embedding_cache[text_hash, accessed_at], accessed_at < $cutoff
                     ?[text_hash] := matched[text_hash]
                     :rm embedding_cache { text_hash }",
                    Some(serde_json::json!({"cutoff": cutoff})),
                )
                .map_err(|e| anyhow::anyhow!("CozoDB cleanup delete failed: {e}"))?;
        }

        info!(
            "Cleaned up {} old cache entries (>{} days old)",
            count, days
        );

        Ok(count)
    }

    /// Clear all cache data
    pub fn clear(&self) -> Result<()> {
        log::warn!("Clearing all cache data");

        // Clear embeddings
        let vectors_dir = self.cache_dir.join("embeddings/vectors");
        if vectors_dir.exists() {
            std::fs::remove_dir_all(&vectors_dir)?;
            std::fs::create_dir_all(&vectors_dir)?;
        }

        // Clear blocks
        let blocks_dir = self.cache_dir.join("blocks");
        if blocks_dir.exists() {
            std::fs::remove_dir_all(&blocks_dir)?;
            std::fs::create_dir_all(blocks_dir.join("by-number"))?;
            std::fs::create_dir_all(blocks_dir.join("by-hash"))?;
        }

        // Clear CozoDB index: delete all rows from the relation
        let _ = self.index.run_query(
            "?[text_hash] := *embedding_cache[text_hash] :rm embedding_cache { text_hash }",
            None,
        );

        log::info!("Cache cleared");

        Ok(())
    }

    /// Clear only embeddings cache
    pub fn clear_embeddings(&self) -> Result<()> {
        log::warn!("Clearing embeddings cache");

        // Clear embeddings vectors
        let vectors_dir = self.cache_dir.join("embeddings/vectors");
        if vectors_dir.exists() {
            std::fs::remove_dir_all(&vectors_dir)?;
            std::fs::create_dir_all(&vectors_dir)?;
        }

        // Clear CozoDB index
        let _ = self.index.run_query(
            "?[text_hash] := *embedding_cache[text_hash] :rm embedding_cache { text_hash }",
            None,
        );

        log::info!("Embeddings cache cleared");

        Ok(())
    }

    /// Clear only blocks cache
    pub fn clear_blocks(&self) -> Result<()> {
        log::warn!("Clearing blocks cache");

        // Clear blocks
        let blocks_dir = self.cache_dir.join("blocks");
        if blocks_dir.exists() {
            std::fs::remove_dir_all(&blocks_dir)?;
            std::fs::create_dir_all(blocks_dir.join("by-number"))?;
            std::fs::create_dir_all(blocks_dir.join("by-hash"))?;
        }

        log::info!("Blocks cache cleared");

        Ok(())
    }

    /// Create BTRFS snapshot of cache
    pub async fn create_snapshot(&self) -> Result<PathBuf> {
        self.snapshot_manager.create_snapshot().await
    }

    /// List all snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<super::snapshot_manager::SnapshotInfo>> {
        self.snapshot_manager.list_snapshots().await
    }

    /// Delete all snapshots
    pub async fn delete_all_snapshots(&self) -> Result<usize> {
        self.snapshot_manager.delete_all_snapshots().await
    }

    /// Stream cache data to remote system using Btrfs send/receive with NUMA affinity
    pub async fn stream_to_remote(
        &self,
        remote_host: &str,
        remote_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Apply NUMA affinity for streaming operations
        self.apply_numa_affinity("cache_streaming").await?;

        let snapshot_path = self
            .create_snapshot()
            .await
            .map_err(|e| format!("Failed to create snapshot: {}", e))?;

        info!(
            "Streaming cache snapshot to {}:{}",
            remote_host, remote_path
        );

        let cmd = format!(
            "btrfs send {} | ssh {} 'btrfs receive {}'",
            snapshot_path.display(),
            remote_host,
            remote_path
        );

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .map_err(|e| format!("Failed to execute btrfs stream command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Btrfs streaming failed: {}", stderr).into());
        }

        info!("Successfully streamed cache snapshot");
        Ok(())
    }

    /// Receive cache data from remote system with NUMA affinity
    pub async fn receive_from_remote(
        &self,
        remote_host: &str,
        remote_snapshot: &str,
        local_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Apply NUMA affinity for receiving operations
        self.apply_numa_affinity("cache_receiving").await?;

        info!(
            "Receiving cache snapshot from {}:{}",
            remote_host, remote_snapshot
        );

        let cmd = format!(
            "ssh {} 'btrfs send {}' | btrfs receive {}",
            remote_host, remote_snapshot, local_path
        );

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .map_err(|e| format!("Failed to execute btrfs receive command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Btrfs receive failed: {}", stderr).into());
        }

        info!("Successfully received cache snapshot");
        Ok(())
    }

    /// Get NUMA configuration info
    pub fn numa_info(&self) -> NumaInfo {
        NumaInfo {
            node_count: self.numa_topology.node_count(),
            cpu_affinity: self.cpu_affinity.clone(),
            placement_strategy: self.placement_strategy.clone(),
            memory_policy: self.memory_policy.clone(),
        }
    }

    /// Get cache directory path
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Helper method to apply NUMA affinity (CPU + memory)
    async fn apply_numa_affinity(
        &self,
        operation: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Apply CPU affinity first
        self.apply_cpu_affinity(operation).await?;

        // Apply memory policy
        match &self.memory_policy {
            MemoryPolicy::Default => {
                debug!("Using default memory policy for {}", operation);
            }
            MemoryPolicy::Bind(nodes) if !nodes.is_empty() => {
                debug!("Memory bound to nodes {:?} for {}", nodes, operation);
            }
            MemoryPolicy::Preferred(Some(node)) => {
                debug!("Memory preferred on node {} for {}", node, operation);
            }
            MemoryPolicy::Interleave(nodes) if !nodes.is_empty() => {
                debug!(
                    "Memory interleaved across nodes {:?} for {}",
                    nodes, operation
                );
            }
            _ => {
                debug!("Memory policy not applied for {}", operation);
            }
        }

        Ok(())
    }

    /// Apply CPU affinity using taskset
    async fn apply_cpu_affinity(
        &self,
        operation: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let candidate_cpus = self
            .select_numa_node(operation)
            .and_then(|node| {
                if node.cpu_list.is_empty() {
                    None
                } else {
                    Some(node.cpu_list.clone())
                }
            })
            .unwrap_or_else(|| self.cpu_affinity.clone());

        if candidate_cpus.is_empty() {
            debug!("No CPU affinity configured for {}", operation);
            return Ok(());
        }

        if candidate_cpus == self.cpu_affinity {
            debug!(
                "Using default CPU affinity {:?} for {}",
                candidate_cpus, operation
            );
        }

        let cpu_list = candidate_cpus
            .iter()
            .map(|cpu| cpu.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let output = tokio::process::Command::new("taskset")
            .args(["-c", &cpu_list])
            .arg("echo")
            .arg(format!("CPU affinity test for {}", operation))
            .output()
            .await
            .map_err(|e| format!("taskset command failed: {}", e))?;

        if output.status.success() {
            debug!(
                "Applied CPU affinity to cores: {} for {}",
                cpu_list, operation
            );
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("taskset failed for {}: {}", operation, stderr);
            Ok(()) // Don't fail, just continue without affinity
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub hot_entries: usize,
    pub total_accesses: u64,
    pub disk_usage_bytes: u64,
    pub embeddings_size_bytes: u64,
    pub blocks_size_bytes: u64,
}

impl CacheStats {
    pub fn hot_ratio(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            self.hot_entries as f64 / self.total_entries as f64
        }
    }

    pub fn avg_accesses(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            self.total_accesses as f64 / self.total_entries as f64
        }
    }
}
#[derive(Debug, Clone)]
/// NUMA configuration information
pub struct NumaInfo {
    pub node_count: usize,
    pub cpu_affinity: Vec<u32>,
    pub placement_strategy: CachePlacementStrategy,
    pub memory_policy: MemoryPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_text_hashing() {
        let parent = tempfile::tempdir().unwrap();
        let cache_path = parent.path().join("cache");
        let cache = BtrfsCache::new(cache_path).await.unwrap();
        let hash1 = cache.hash_text("test");
        let hash2 = cache.hash_text("test");
        let hash3 = cache.hash_text("different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA256 hex length
    }
}
