//! Streaming blockchain with BTRFS subvolumes for audit trail
//!
//! Architecture:
//! - timing_subvol: Immutable audit trail (append-only)
//! - vector_subvol: ML embeddings for semantic search
//! - state_subvol: Current system state for DR/reinstall

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{OpDbusError, Result};

// ============================================================================
// CHAIN BLOCK
// ============================================================================

/// A block in the execution blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainBlock {
    /// Block number
    pub block_num: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Previous block hash (for chaining)
    pub prev_hash: String,
    /// This block's hash
    pub hash: String,
    /// Tool/plugin name that executed
    pub tool_name: String,
    /// Content hash of input/output
    pub content_hash: String,
    /// Policy ID governing execution
    pub policy_id: String,
    /// Vector embedding (if computed)
    pub vector: Vec<f32>,
    /// Additional metadata
    pub metadata: Value,
}

impl ChainBlock {
    pub fn new(
        block_num: u64,
        prev_hash: String,
        tool_name: String,
        content_hash: String,
        policy_id: String,
    ) -> Self {
        let timestamp = Utc::now();
        let hash = Self::compute_hash(block_num, &prev_hash, &tool_name, &content_hash, &timestamp);
        
        Self {
            block_num,
            timestamp,
            prev_hash,
            hash,
            tool_name,
            content_hash,
            policy_id,
            vector: Vec::new(),
            metadata: Value::null(),
        }
    }

    pub fn with_vector(mut self, vector: Vec<f32>) -> Self {
        self.vector = vector;
        self
    }

    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    fn compute_hash(
        block_num: u64,
        prev_hash: &str,
        tool_name: &str,
        content_hash: &str,
        timestamp: &DateTime<Utc>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(block_num.to_le_bytes());
        hasher.update(prev_hash.as_bytes());
        hasher.update(tool_name.as_bytes());
        hasher.update(content_hash.as_bytes());
        hasher.update(timestamp.timestamp().to_le_bytes());
        hex::encode(hasher.finalize())
    }

    /// Alias for hash field (compatibility)
    pub fn block_hash(&self) -> &str {
        &self.hash
    }
}

// ============================================================================
// BLOCKCHAIN STREAM
// ============================================================================

/// Streaming blockchain with BTRFS subvolumes
pub struct BlockchainStream {
    base_path: PathBuf,
    block_counter: Arc<RwLock<u64>>,
    last_hash: Arc<RwLock<String>>,
    blocks: Arc<RwLock<Vec<ChainBlock>>>,
    max_memory_blocks: usize,
}

impl BlockchainStream {
    /// Create a new blockchain stream
    pub async fn new(base_path: PathBuf) -> Result<Self> {
        // Create directories
        tokio::fs::create_dir_all(&base_path).await
            .map_err(|e| OpDbusError::IoError(e))?;
        
        let timing_dir = base_path.join("timing");
        let vector_dir = base_path.join("vectors");
        let state_dir = base_path.join("state");
        let snapshots_dir = base_path.join("snapshots");

        tokio::fs::create_dir_all(&timing_dir).await.map_err(|e| OpDbusError::IoError(e))?;
        tokio::fs::create_dir_all(&vector_dir).await.map_err(|e| OpDbusError::IoError(e))?;
        tokio::fs::create_dir_all(&state_dir).await.map_err(|e| OpDbusError::IoError(e))?;
        tokio::fs::create_dir_all(&snapshots_dir).await.map_err(|e| OpDbusError::IoError(e))?;

        // Try to create BTRFS subvolumes (falls back silently)
        Self::try_create_subvolume(&timing_dir).await;
        Self::try_create_subvolume(&vector_dir).await;
        Self::try_create_subvolume(&state_dir).await;

        tracing::info!("Blockchain stream initialized at {:?}", base_path);

        Ok(Self {
            base_path,
            block_counter: Arc::new(RwLock::new(0)),
            last_hash: Arc::new(RwLock::new("genesis".to_string())),
            blocks: Arc::new(RwLock::new(Vec::new())),
            max_memory_blocks: 1000,
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
                // BTRFS not available, directory already created
                tracing::debug!("BTRFS subvolume creation skipped: {:?}", path);
            }
        }
    }

    /// Add a new block to the blockchain
    pub async fn add_block(
        &self,
        tool_name: &str,
        content_hash: &str,
        policy_id: &str,
    ) -> Result<ChainBlock> {
        let block_num = {
            let mut counter = self.block_counter.write().await;
            *counter += 1;
            *counter
        };

        let prev_hash = self.last_hash.read().await.clone();

        let block = ChainBlock::new(
            block_num,
            prev_hash,
            tool_name.to_string(),
            content_hash.to_string(),
            policy_id.to_string(),
        );

        // Update last hash
        *self.last_hash.write().await = block.hash.clone();

        // Store in memory
        {
            let mut blocks = self.blocks.write().await;
            blocks.push(block.clone());
            
            // Trim if over limit
            if blocks.len() > self.max_memory_blocks {
                blocks.remove(0);
            }
        }

        // Persist to disk
        self.persist_block(&block).await?;

        tracing::debug!("Added block {} with hash {}", block_num, &block.hash[..16]);

        Ok(block)
    }

    /// Add a block with vector embedding
    pub async fn add_block_with_vector(
        &self,
        tool_name: &str,
        content_hash: &str,
        policy_id: &str,
        vector: Vec<f32>,
    ) -> Result<ChainBlock> {
        let mut block = self.add_block(tool_name, content_hash, policy_id).await?;
        block = block.with_vector(vector.clone());

        // Persist vector separately
        self.persist_vector(block.block_num, &vector).await?;

        Ok(block)
    }

    async fn persist_block(&self, block: &ChainBlock) -> Result<()> {
        let timing_dir = self.base_path.join("timing");
        let file_path = timing_dir.join(format!("block-{:012}.json", block.block_num));
        
        let data = simd_json::to_string_pretty(block)
            .map_err(|e| OpDbusError::Serialization(e.to_string()))?;
        
        tokio::fs::write(&file_path, data).await
            .map_err(|e| OpDbusError::IoError(e))?;

        Ok(())
    }

    async fn persist_vector(&self, block_num: u64, vector: &[f32]) -> Result<()> {
        let vector_dir = self.base_path.join("vectors");
        let file_path = vector_dir.join(format!("vec-{:012}.bin", block_num));
        
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        
        tokio::fs::write(&file_path, bytes).await
            .map_err(|e| OpDbusError::IoError(e))?;

        Ok(())
    }

    /// Write state data (for disaster recovery)
    pub async fn write_state(&self, key: &str, value: &Value) -> Result<()> {
        let state_dir = self.base_path.join("state");
        let file_path = state_dir.join(format!("{}.json", key));
        
        let data = simd_json::to_string_pretty(value)
            .map_err(|e| OpDbusError::Serialization(e.to_string()))?;
        
        tokio::fs::write(&file_path, data).await
            .map_err(|e| OpDbusError::IoError(e))?;

        Ok(())
    }

    /// Write state data with validation against declared restorable_state_keys
    /// 
    /// This method checks if the key is declared in any plugin's restorable_state_keys.
    /// If not declared, it logs a warning (or blocks in strict mode).
    /// 
    /// declared_keys: All keys from all plugins' restorable_state_keys combined
    /// strict: If true, returns error for undeclared keys. If false, just warns.
    pub async fn write_state_validated(
        &self,
        key: &str,
        value: &Value,
        declared_keys: &[String],
        strict: bool,
    ) -> Result<()> {
        // Check if key is declared
        let key_declared = declared_keys.iter().any(|dk| {
            // Support prefix matching (e.g., "plugin/foo/*" matches "plugin/foo/bar")
            if dk.ends_with("/*") {
                key.starts_with(&dk[..dk.len()-2])
            } else {
                dk == key
            }
        });

        if !key_declared {
            let msg = format!(
                "State key '{}' is not declared in any plugin's restorable_state_keys. \
                This state may not be restored during disaster recovery!",
                key
            );

            if strict {
                tracing::error!("{}", msg);
                return Err(OpDbusError::ConfigError(msg));
            } else {
                tracing::warn!("{}", msg);
            }
        }

        // Proceed with write
        self.write_state(key, value).await
    }

    /// Read state data
    pub async fn read_state(&self, key: &str) -> Result<Value> {
        let state_dir = self.base_path.join("state");
        let file_path = state_dir.join(format!("{}.json", key));
        
        let mut data = tokio::fs::read_to_string(&file_path).await
            .map_err(|e| OpDbusError::IoError(e))?;
        
        unsafe { simd_json::from_str(&mut data) }
            .map_err(|e| OpDbusError::Serialization(e.to_string()))
    }

    /// Get recent blocks from memory
    pub async fn get_recent_blocks(&self, limit: usize) -> Vec<ChainBlock> {
        let blocks = self.blocks.read().await;
        blocks.as_slice().iter().rev().take(limit).cloned().collect()
    }

    /// Get block by number
    pub async fn get_block(&self, block_num: u64) -> Option<ChainBlock> {
        let blocks = self.blocks.read().await;
        blocks.as_slice().iter().find(|b| b.block_num == block_num).cloned()
    }

    /// Get current block count
    pub async fn block_count(&self) -> u64 {
        *self.block_counter.read().await
    }

    /// Get last block hash
    pub async fn last_hash(&self) -> String {
        self.last_hash.read().await.clone()
    }

    /// Create a snapshot
    pub async fn create_snapshot(&self) -> Result<String> {
        let snapshots_dir = self.base_path.join("snapshots");
        let state_dir = self.base_path.join("state");
        
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let snapshot_name = format!("snapshot-{}", timestamp);
        let snapshot_path = snapshots_dir.join(&snapshot_name);

        // Try BTRFS snapshot first
        let result = tokio::process::Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&state_dir)
            .arg(&snapshot_path)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                tracing::info!("Created BTRFS snapshot: {}", snapshot_name);
            }
            _ => {
                // Fall back to copy
                tokio::fs::create_dir_all(&snapshot_path).await
                    .map_err(|e| OpDbusError::IoError(e))?;
                
                Self::copy_dir(&state_dir, &snapshot_path).await?;
                tracing::info!("Created copy snapshot: {}", snapshot_name);
            }
        }

        Ok(snapshot_name)
    }

    async fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
        let mut entries = tokio::fs::read_dir(src).await
            .map_err(|e| OpDbusError::IoError(e))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| OpDbusError::IoError(e))? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if entry.file_type().await.map_err(|e| OpDbusError::IoError(e))?.is_dir() {
                tokio::fs::create_dir_all(&dst_path).await
                    .map_err(|e| OpDbusError::IoError(e))?;
                Box::pin(Self::copy_dir(&src_path, &dst_path)).await?;
            } else {
                tokio::fs::copy(&src_path, &dst_path).await
                    .map_err(|e| OpDbusError::IoError(e))?;
            }
        }

        Ok(())
    }

    /// List snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<String>> {
        let snapshots_dir = self.base_path.join("snapshots");
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

    /// Get base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_block_hash() {
        let block = ChainBlock::new(
            1,
            "genesis".to_string(),
            "test.tool".to_string(),
            "content123".to_string(),
            "default".to_string(),
        );

        assert_eq!(block.block_num, 1);
        assert!(!block.hash.is_empty());
        assert_eq!(block.prev_hash, "genesis");
    }

    #[tokio::test]
    async fn test_blockchain_stream() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let stream = BlockchainStream::new(temp_dir.path().to_path_buf()).await.unwrap();

        let block = stream.add_block("tool1", "hash1", "policy1").await.unwrap();
        assert_eq!(block.block_num, 1);
        assert_eq!(block.tool_name, "tool1");

        let block2 = stream.add_block("tool2", "hash2", "policy1").await.unwrap();
        assert_eq!(block2.block_num, 2);
        assert_eq!(block2.prev_hash, block.hash);
    }
}
