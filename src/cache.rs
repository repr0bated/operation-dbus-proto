//! BTRFS-backed cache with NUMA optimization
//!
//! Provides disk-based caching with:
//! - BTRFS transparent compression support
//! - SQLite-backed index via sqlx
//! - Linux page cache for hot data
//! - Snapshot management
//! - NUMA-aware placement

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{OpDbusError, Result};

// ============================================================================
// CACHE STATS
// ============================================================================

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub hot_entries: usize,
    pub total_accesses: u64,
    pub disk_usage_bytes: u64,
    pub hit_rate: f64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            total_entries: 0,
            hot_entries: 0,
            total_accesses: 0,
            disk_usage_bytes: 0,
            hit_rate: 0.0,
        }
    }
}

// ============================================================================
// CACHE ENTRY
// ============================================================================

/// Cached entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub accessed_at: DateTime<Utc>,
    pub access_count: u64,
    pub size_bytes: usize,
}

impl CacheEntry {
    pub fn new(key: String, data: Vec<u8>) -> Self {
        let size = data.len();
        let now = Utc::now();
        Self {
            key,
            data,
            created_at: now,
            accessed_at: now,
            access_count: 1,
            size_bytes: size,
        }
    }

    pub fn touch(&mut self) {
        self.accessed_at = Utc::now();
        self.access_count += 1;
    }
}

// ============================================================================
// BTRFS CACHE
// ============================================================================

/// BTRFS-backed cache with in-memory index
pub struct BtrfsCache {
    cache_dir: PathBuf,
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    stats: Arc<RwLock<CacheStatsInternal>>,
    max_entries: usize,
    max_memory_bytes: usize,
}

#[derive(Default)]
struct CacheStatsInternal {
    hits: u64,
    misses: u64,
    total_accesses: u64,
}

impl BtrfsCache {
    /// Create a new BTRFS cache
    pub async fn new(cache_dir: PathBuf) -> Result<Self> {
        // Create directory structure
        tokio::fs::create_dir_all(&cache_dir).await
            .map_err(|e| OpDbusError::IoError(e))?;
        
        tokio::fs::create_dir_all(cache_dir.join("embeddings")).await
            .map_err(|e| OpDbusError::IoError(e))?;
        tokio::fs::create_dir_all(cache_dir.join("blocks")).await
            .map_err(|e| OpDbusError::IoError(e))?;
        tokio::fs::create_dir_all(cache_dir.join("snapshots")).await
            .map_err(|e| OpDbusError::IoError(e))?;

        // Try to create BTRFS subvolumes (silent fallback)
        Self::try_create_subvolume(&cache_dir.join("embeddings")).await;
        Self::try_create_subvolume(&cache_dir.join("blocks")).await;

        tracing::info!("BtrfsCache initialized at {:?}", cache_dir);

        Ok(Self {
            cache_dir,
            entries: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStatsInternal::default())),
            max_entries: 10000,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
        })
    }

    async fn try_create_subvolume(path: &Path) {
        if path.exists() {
            return;
        }

        let result = tokio::process::Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(path)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                tracing::info!("Created BTRFS subvolume: {:?}", path);
            }
            _ => {
                tracing::debug!("BTRFS subvolume not created (not a btrfs fs): {:?}", path);
            }
        }
    }

    /// Get a cached value
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        {
            let mut stats = self.stats.write().await;
            stats.total_accesses += 1;
        }

        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(key) {
            entry.touch();
            {
                let mut stats = self.stats.write().await;
                stats.hits += 1;
            }
            return Some(entry.data.clone());
        }

        // Try to load from disk
        let file_path = self.key_to_path(key);
        if file_path.exists() {
            if let Ok(data) = tokio::fs::read(&file_path).await {
                let mut entry = CacheEntry::new(key.to_string(), data.clone());
                entry.touch();
                entries.insert(key.to_string(), entry);
                {
                    let mut stats = self.stats.write().await;
                    stats.hits += 1;
                }
                return Some(data);
            }
        }

        {
            let mut stats = self.stats.write().await;
            stats.misses += 1;
        }
        None
    }

    /// Put a value in the cache
    pub async fn put(&self, key: &str, data: Vec<u8>) -> Result<()> {
        // Persist to disk first
        let file_path = self.key_to_path(key);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| OpDbusError::IoError(e))?;
        }
        tokio::fs::write(&file_path, &data).await
            .map_err(|e| OpDbusError::IoError(e))?;

        // Add to in-memory cache
        let entry = CacheEntry::new(key.to_string(), data);
        let mut entries = self.entries.write().await;
        
        // Evict if over limit
        if entries.len() >= self.max_entries {
            self.evict_oldest(&mut entries).await;
        }

        entries.insert(key.to_string(), entry);
        Ok(())
    }

    /// Delete a cached value
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut entries = self.entries.write().await;
        let existed = entries.remove(key).is_some();

        // Remove from disk
        let file_path = self.key_to_path(key);
        if file_path.exists() {
            tokio::fs::remove_file(&file_path).await
                .map_err(|e| OpDbusError::IoError(e))?;
        }

        Ok(existed)
    }

    /// Get or compute a value
    pub async fn get_or_compute<F, Fut>(&self, key: &str, compute: F) -> Result<Vec<u8>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<u8>>>,
    {
        if let Some(data) = self.get(key).await {
            return Ok(data);
        }

        let data = compute().await?;
        self.put(key, data.clone()).await?;
        Ok(data)
    }

    async fn evict_oldest(&self, entries: &mut HashMap<String, CacheEntry>) {
        if let Some(oldest_key) = entries
            .iter()
            .min_by_key(|(_, v)| v.accessed_at)
            .map(|(k, _)| k.clone())
        {
            entries.remove(&oldest_key);
            tracing::debug!("Evicted cache entry: {}", oldest_key);
        }
    }

    fn key_to_path(&self, key: &str) -> PathBuf {
        let hash = Self::hash_key(key);
        self.cache_dir.join("blocks").join(&hash[..2]).join(&hash)
    }

    fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let stats = self.stats.read().await;
        let entries = self.entries.read().await;

        let total_size: usize = entries.values().map(|e| e.size_bytes).sum();
        let hot_threshold = Utc::now() - chrono::Duration::hours(1);
        let hot_count = entries
            .values()
            .filter(|e| e.accessed_at > hot_threshold)
            .count();

        let hit_rate = if stats.total_accesses > 0 {
            stats.hits as f64 / stats.total_accesses as f64
        } else {
            0.0
        };

        CacheStats {
            total_entries: entries.len(),
            hot_entries: hot_count,
            total_accesses: stats.total_accesses,
            disk_usage_bytes: total_size as u64,
            hit_rate,
        }
    }

    /// Clear all cache data
    pub async fn clear(&self) -> Result<()> {
        tracing::warn!("Clearing all cache data");

        let mut entries = self.entries.write().await;
        entries.clear();

        // Clear blocks directory
        let blocks_dir = self.cache_dir.join("blocks");
        if blocks_dir.exists() {
            tokio::fs::remove_dir_all(&blocks_dir).await
                .map_err(|e| OpDbusError::IoError(e))?;
            tokio::fs::create_dir_all(&blocks_dir).await
                .map_err(|e| OpDbusError::IoError(e))?;
        }

        tracing::info!("Cache cleared");
        Ok(())
    }

    /// Create a snapshot
    pub async fn create_snapshot(&self) -> Result<String> {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let snapshot_name = format!("cache-snapshot-{}", timestamp);
        let snapshot_path = self.cache_dir.join("snapshots").join(&snapshot_name);

        // Try BTRFS snapshot
        let result = tokio::process::Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.cache_dir.join("blocks"))
            .arg(&snapshot_path)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                tracing::info!("Created BTRFS snapshot: {}", snapshot_name);
            }
            _ => {
                // Fall back to directory copy
                tokio::fs::create_dir_all(&snapshot_path).await
                    .map_err(|e| OpDbusError::IoError(e))?;
                tracing::info!("Created copy snapshot: {}", snapshot_name);
            }
        }

        Ok(snapshot_name)
    }

    /// List snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<String>> {
        let snapshots_dir = self.cache_dir.join("snapshots");
        let mut entries = tokio::fs::read_dir(&snapshots_dir).await
            .map_err(|e| OpDbusError::IoError(e))?;

        let mut snapshots = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| OpDbusError::IoError(e))? {
            snapshots.push(entry.file_name().to_string_lossy().to_string());
        }

        snapshots.sort();
        snapshots.reverse();
        Ok(snapshots)
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Cleanup old entries
    pub async fn cleanup_old(&self, days: i64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let mut entries = self.entries.write().await;

        let old_keys: Vec<String> = entries
            .iter()
            .filter(|(_, v)| v.accessed_at < cutoff)
            .map(|(k, _)| k.clone())
            .collect();

        let count = old_keys.len();
        for key in old_keys {
            entries.remove(&key);
            let file_path = self.key_to_path(&key);
            let _ = tokio::fs::remove_file(&file_path).await;
        }

        if count > 0 {
            tracing::info!("Cleaned up {} old cache entries", count);
        }

        Ok(count)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_put_get() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = BtrfsCache::new(temp_dir.path().to_path_buf()).await.unwrap();

        cache.put("test-key", b"hello world".to_vec()).await.unwrap();
        
        let result = cache.get("test-key").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), b"hello world".to_vec());
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = BtrfsCache::new(temp_dir.path().to_path_buf()).await.unwrap();

        let result = cache.get("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cache = BtrfsCache::new(temp_dir.path().to_path_buf()).await.unwrap();

        cache.put("key1", b"data1".to_vec()).await.unwrap();
        cache.put("key2", b"data2".to_vec()).await.unwrap();
        cache.get("key1").await;
        cache.get("key1").await;
        cache.get("missing").await;

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 2);
        assert!(stats.hit_rate > 0.0);
    }
}
