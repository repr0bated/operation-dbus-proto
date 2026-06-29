//! `blockchain` StatePlugin — exposes the live streaming-blockchain audit chain
//! (`op-blockchain`) through the `org.dbus.v1.plugins` surface, reachable over
//! `PluginService.CallMethod`.
//!
//! This is the correct home for the capability the Lovable frontend mistakenly
//! called as a standalone `operation.blockchain.v1.BlockchainService` gRPC
//! package — there is no such proto and there must not be one. New backend
//! capabilities register here as plugins; the served gRPC surface stays the one
//! shared route-builder.
//!
//! Every read is live: the plugin's live state
//! opens the on-disk chain at `$OPDBUS_BLOCKCHAIN_PATH` and reports its real
//! snapshots, DR `current_state`, snapshot interval and retention policy.
//! [`create_checkpoint`](BlockchainPlugin::create_checkpoint) performs a real
//! `create_snapshot()` and `apply_state` writes the DR state through the chain.
//! Nothing here is mocked.

use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_blockchain::StreamingBlockchain;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::ValueAsContainer;
use simd_json::{json, OwnedValue as Value};
use std::path::PathBuf;

/// Default on-disk location of the streaming blockchain when
/// `$OPDBUS_BLOCKCHAIN_PATH` is unset.
const DEFAULT_BASE_PATH: &str = "/var/lib/opdbus/blockchain";

/// Key under which the disaster-recovery "current state" blob is persisted in
/// the state subvolume (`StreamingBlockchain::write_current_state` writes here).
const CURRENT_STATE_KEY: &str = "current";

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RetentionView {
    /// Hourly retention count.
    #[schemars(extend("x-oscal-subid" = "obs.software.blockchain.retention.hourly@v1"))]
    pub hourly: usize,
    /// Daily retention count.
    #[schemars(extend("x-oscal-subid" = "obs.software.blockchain.retention.daily@v1"))]
    pub daily: usize,
    /// Weekly retention count.
    #[schemars(extend("x-oscal-subid" = "obs.software.blockchain.retention.weekly@v1"))]
    pub weekly: usize,
    /// Quarterly retention count.
    #[schemars(extend("x-oscal-subid" = "obs.software.blockchain.retention.quarterly@v1"))]
    pub quarterly: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SnapshotEntry {
    /// Snapshot name.
    #[schemars(extend("x-oscal-subid" = "exp.software.blockchain.snapshot.name@v1"))]
    pub name: String,
    /// Snapshot creation timestamp.
    #[schemars(extend("x-oscal-subid" = "exp.software.blockchain.snapshot.created@v1"))]
    pub created: String,
}

/// Blockchain plugin state schema.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.blockchain.schema@v1"))]
pub struct BlockchainState {
    /// `active` once the chain exists on disk, `uninitialized` before first write.
    #[schemars(extend("x-oscal-subid" = "obs.software.blockchain.status@v1"))]
    pub status: String,
    /// Blockchain base path.
    #[schemars(extend("x-oscal-subid" = "exp.software.blockchain.base-path@v1"))]
    pub base_path: String,
    /// Blockchain snapshot interval.
    #[schemars(extend("x-oscal-subid" = "mut.service.blockchain.snapshot-interval@v1"))]
    pub snapshot_interval: String,
    /// Blockchain retention policy.
    #[schemars(extend("x-oscal-subid" = "obs.service.blockchain.retention@v1"))]
    pub retention: RetentionView,
    /// Number of snapshots.
    #[schemars(extend("x-oscal-subid" = "exp.software.blockchain.snapshot-count@v1"))]
    pub snapshot_count: usize,
    /// List of snapshots.
    #[schemars(extend("x-oscal-subid" = "exp.software.blockchain.snapshots@v1"))]
    pub snapshots: Vec<SnapshotEntry>,
    /// Current blockchain state.
    #[schemars(skip)]
    #[schemars(extend("x-oscal-subid" = "obs.software.blockchain.current-state@v1"))]
    pub current_state: Value,
}

pub struct BlockchainPlugin {
    base_path: PathBuf,
}

impl Default for BlockchainPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockchainPlugin {
    pub fn new() -> Self {
        let base_path = std::env::var("OPDBUS_BLOCKCHAIN_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_BASE_PATH));
        Self { base_path }
    }

    /// Open the on-disk chain. `StreamingBlockchain::new` is idempotent: it only
    /// (re)creates subvolumes that do not already exist, so opening an existing
    /// chain is non-destructive. Callers that must not create the chain should
    /// gate on `base_path.exists()` first (see [`read_live`]).
    async fn open(&self) -> Result<StreamingBlockchain> {
        StreamingBlockchain::new(&self.base_path).await
    }

    /// Read the live chain state. If the chain has not been initialized on disk
    /// yet, report an honest `uninitialized` state rather than creating it as a
    /// side effect of a read.
    async fn read_live(&self) -> Result<BlockchainState> {
        if !self.base_path.exists() {
            return Ok(BlockchainState {
                status: "uninitialized".to_string(),
                base_path: self.base_path.display().to_string(),
                snapshot_interval: String::new(),
                retention: RetentionView {
                    hourly: 0,
                    daily: 0,
                    weekly: 0,
                    quarterly: 0,
                },
                snapshot_count: 0,
                snapshots: Vec::new(),
                current_state: json!({}),
            });
        }

        let chain = self.open().await?;

        let snapshots: Vec<SnapshotEntry> = chain
            .list_snapshots()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(name, created)| SnapshotEntry { name, created })
            .collect();

        let current_state = chain
            .read_state(CURRENT_STATE_KEY)
            .await
            .unwrap_or_else(|_| json!({}));

        let retention = chain.retention_policy();

        Ok(BlockchainState {
            status: "active".to_string(),
            base_path: chain.base_path().display().to_string(),
            snapshot_interval: chain.snapshot_interval().to_string(),
            retention: RetentionView {
                hourly: retention.hourly,
                daily: retention.daily,
                weekly: retention.weekly,
                quarterly: retention.quarterly,
            },
            snapshot_count: snapshots.len(),
            snapshots,
            current_state,
        })
    }

    /// Synchronous shape exemplar used only to derive the [`PluginSchema`]. The
    /// schema describes the *shape* of the plugin's live state output (field
    /// names + types); the data path itself is always live.
    fn schema_exemplar() -> BlockchainState {
        BlockchainState {
            status: "active".to_string(),
            base_path: DEFAULT_BASE_PATH.to_string(),
            snapshot_interval: "every-15-minutes".to_string(),
            retention: RetentionView {
                hourly: 24,
                daily: 7,
                weekly: 4,
                quarterly: 4,
            },
            snapshot_count: 0,
            snapshots: vec![SnapshotEntry {
                name: "state-000001".to_string(),
                created: "1970-01-01 00:00:00 UTC".to_string(),
            }],
            current_state: json!({}),
        }
    }
}

#[async_trait]
impl StatePlugin for BlockchainPlugin {
    fn name(&self) -> &str {
        "blockchain"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(blockchain_schema())
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        // The only writable projection of an append-only audit chain is its DR
        // `current_state` blob. Emit a real Modify when the desired state
        // differs, otherwise a NoOp.
        let actions = if current == desired {
            vec![StateAction::NoOp {
                resource: "current_state".to_string(),
            }]
        } else {
            vec![StateAction::Modify {
                resource: "current_state".to_string(),
                changes: desired.clone(),
            }]
        };
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Modify { resource, changes } if resource == "current_state" => {
                    // Persist the DR current-state blob through the real chain.
                    let state_blob = changes
                        .as_object()
                        .and_then(|o| o.get("current_state").cloned())
                        .unwrap_or_else(|| changes.clone());
                    match self.open().await {
                        Ok(chain) => match chain.write_current_state(&state_blob).await {
                            Ok(()) => changes_applied.push(format!("wrote {}", resource)),
                            Err(e) => errors.push(format!("write_current_state failed: {}", e)),
                        },
                        Err(e) => errors.push(format!("open chain failed: {}", e)),
                    }
                }
                StateAction::NoOp { .. } => {}
                other => errors.push(format!("unsupported blockchain action: {:?}", other)),
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = simd_json::serde::to_owned_value(self.read_live().await?)?;
        Ok(&current == desired)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        // A real BTRFS (or copy-fallback) snapshot of the state subvolume.
        let chain = self.open().await?;
        let snapshot_name = chain.create_snapshot().await?;
        let state_snapshot = chain
            .read_state(CURRENT_STATE_KEY)
            .await
            .unwrap_or_else(|_| json!({}));
        Ok(Checkpoint {
            id: snapshot_name,
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        // Resolve and validate the snapshot the checkpoint points at.
        let chain = self.open().await?;
        let path = chain.rollback(&checkpoint.id).await?;
        tracing::info!(snapshot = %checkpoint.id, path = %path.display(), "blockchain rollback resolved");
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

pub(crate) fn blockchain_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(BlockchainPlugin::schema_exemplar())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "blockchain",
        "infrastructure",
        "1.0.0",
        "Streaming blockchain audit chain — snapshots, DR current-state, retention/interval",
        &state,
    )
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("blockchain", |_ctx| std::sync::Arc::new(BlockchainPlugin::new()))
}
