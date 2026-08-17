//! `blockchain` StatePlugin — exposes the live streaming-blockchain audit chain
//! (`op-blockchain`) through the `org.opdbus.v1.plugins` surface, reachable over
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

use super::plugin_scaffold_helpers::schema_from_state;
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

// ── Audit-trail query surface (accountability-audit-trail spec) ──────────────
//
// These types are declared at module scope (not inside `blockchain_schema()`
// like the older output structs) because `op-grpc-bridge`'s MutationEngine
// imports them to implement the `query_events` / `verify_chain` dispatch arm.
// The same `EventChain` that the gRPC `EventChainService` serves is read here,
// so the D-Bus/MCP path and the gRPC path never disagree.

/// Input for `query_events` — paginated audit-trail query.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, Default)]
#[schemars(extend("x-oscal-subid" = "sch.service.blockchain.query-events-input@v1"))]
pub struct QueryEventsInput {
    /// Filter by plugin_id. Absent or empty means all plugins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    /// Return events with `event_id >= from_event_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_event_id: Option<u64>,
    /// Return events with `event_id <= to_event_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_event_id: Option<u64>,
    /// Max events to return. Default 50, clamped to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Filter by decision: `allow`, `deny`, or absent for all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}

/// One audit event, flattened for transport across the plugin surface.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.blockchain.audit-event-record@v1"))]
pub struct AuditEventRecord {
    pub event_id: u64,
    pub event_hash: String,
    pub prev_hash: String,
    /// ISO 8601 / RFC 3339 timestamp.
    pub timestamp: String,
    pub actor_id: String,
    pub capability_id: String,
    pub plugin_id: String,
    pub method_name: String,
    pub operation_type: String,
    pub target: String,
    pub tags_touched: Vec<String>,
    pub decision: String,
    pub input_patch_hash: String,
    pub result_effective_hash: String,
}

/// Output for `query_events`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.blockchain.query-events-output@v1"))]
pub struct QueryEventsOutput {
    /// The page of events, oldest-first within the requested range.
    pub events: Vec<AuditEventRecord>,
    /// True when more events match the filter beyond this page.
    pub has_more: bool,
    /// Total number of events currently in the chain.
    pub total_in_chain: u64,
}

/// Input for `verify_chain` — hash-chain integrity check.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, Default)]
#[schemars(extend("x-oscal-subid" = "sch.service.blockchain.verify-chain-input@v1"))]
pub struct VerifyChainInput {
    /// Verify from this event_id. Absent or 0 means from genesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_event_id: Option<u64>,
    /// Verify to this event_id. Absent or 0 means to the latest event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_event_id: Option<u64>,
}

/// Output for `verify_chain`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.blockchain.verify-chain-output@v1"))]
pub struct VerifyChainOutput {
    /// True when every event in range hashes correctly and links to its predecessor.
    pub valid: bool,
    /// Number of events checked.
    pub events_verified: u64,
    /// One entry per detected integrity violation. Empty when `valid` is true.
    pub errors: Vec<String>,
}

/// Blockchain plugin state schema.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.blockchain.schema@v1"))]
#[schemars(extend("x-oscal-category" = "service"))]
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
    let root = serde_json::to_value(schemars::schema_for!(BlockchainState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "blockchain",
        "1.0.0",
        "Streaming blockchain audit chain — snapshots, DR current-state, retention/interval",
        &root,
    );

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListSnapshotsOutput {
        pub snapshots: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetSnapshotOutput {
        pub snapshot: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetCurrentStateOutput {
        pub state: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetStatsOutput {
        pub stats: serde_json::Value,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "create_snapshot".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "create_snapshot",
            SideEffect::Mutation,
            false,
            "blockchain.invoke",
            "mut.service.blockchain.snapshot.create@v1",
        ),
    );
    schema.methods.insert(
        "list_snapshots".to_string(),
        method_decl_from_schemars_with_output::<(), ListSnapshotsOutput>(
            "list_snapshots",
            SideEffect::Read,
            true,
            "blockchain.read",
            "obs.service.blockchain.snapshot.list@v1",
        ),
    );
    schema.methods.insert(
        "get_snapshot".to_string(),
        method_decl_from_schemars_with_output::<(), GetSnapshotOutput>(
            "get_snapshot",
            SideEffect::Read,
            true,
            "blockchain.read",
            "obs.service.blockchain.snapshot.get@v1",
        ),
    );
    schema.methods.insert(
        "rollback".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "rollback",
            SideEffect::Mutation,
            true,
            "blockchain.invoke",
            "mut.service.blockchain.rollback@v1",
        ),
    );
    schema.methods.insert(
        "get_current_state".to_string(),
        method_decl_from_schemars_with_output::<(), GetCurrentStateOutput>(
            "get_current_state",
            SideEffect::Read,
            true,
            "blockchain.read",
            "obs.service.blockchain.state.get@v1",
        ),
    );
    schema.methods.insert(
        "set_retention".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "set_retention",
            SideEffect::Mutation,
            false,
            "blockchain.invoke",
            "mut.service.blockchain.retention.set@v1",
        ),
    );
    schema.methods.insert(
        "get_stats".to_string(),
        method_decl_from_schemars_with_output::<(), GetStatsOutput>(
            "get_stats",
            SideEffect::Read,
            true,
            "blockchain.read",
            "obs.service.blockchain.stats@v1",
        ),
    );

    // Audit-trail query surface. Dispatched by MutationEngine's "blockchain"
    // arm — these two are wired; the seven above are not (see the spec's
    // scope boundary in .kiro/specs/accountability-audit-trail).
    schema.methods.insert(
        "query_events".to_string(),
        method_decl_from_schemars_with_output::<QueryEventsInput, QueryEventsOutput>(
            "query_events",
            SideEffect::Read,
            true,
            "blockchain.read",
            "obs.service.blockchain.events.query@v1",
        ),
    );
    schema.methods.insert(
        "verify_chain".to_string(),
        method_decl_from_schemars_with_output::<VerifyChainInput, VerifyChainOutput>(
            "verify_chain",
            SideEffect::Read,
            true,
            "blockchain.read",
            "obs.service.blockchain.chain.verify@v1",
        ),
    );

    schema.capabilities.insert(
        "blockchain.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "blockchain.read".to_string(),
            description: "Grants: list_snapshots, get_snapshot, get_current_state, get_stats, query_events, verify_chain.".to_string(),
        },
    );
    schema.capabilities.insert(
        "blockchain.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "blockchain.invoke".to_string(),
            description: "Grants: create_snapshot, rollback, set_retention.".to_string(),
        },
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("blockchain", |_ctx| std::sync::Arc::new(BlockchainPlugin::new()))
}
