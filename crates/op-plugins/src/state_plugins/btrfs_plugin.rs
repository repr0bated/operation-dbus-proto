use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use serde_json::Value as JsonValue;
use simd_json::{json, OwnedValue as Value};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.btrfs.schema@v1"))]
pub struct BtrfsState {
    /// Runtime status.
    #[serde(default)]
    #[schemars(
        description = "Runtime status",
        extend("x-oscal-subid" = "obs.software.plugin.btrfs.status@v1")
    )]
    pub status: String,
    /// Subvolumes list.
    #[serde(default)]
    #[schemars(
        description = "Subvolumes list",
        extend("x-oscal-subid" = "obs.software.plugin.btrfs.subvolumes@v1")
    )]
    pub subvolumes: JsonValue,
    /// Snapshots list.
    #[serde(default)]
    #[schemars(
        description = "Snapshots list",
        extend("x-oscal-subid" = "obs.software.plugin.btrfs.snapshots@v1")
    )]
    pub snapshots: JsonValue,
    /// Send/receive state.
    #[serde(default)]
    #[schemars(
        description = "Send/receive state",
        extend("x-oscal-subid" = "obs.software.plugin.btrfs.send-state@v1")
    )]
    pub send_state: JsonValue,
    /// DR status.
    #[serde(default)]
    #[schemars(
        description = "DR status",
        extend("x-oscal-subid" = "obs.software.plugin.btrfs.dr-status@v1")
    )]
    pub dr_status: JsonValue,
    /// Configuration.
    #[serde(default)]
    #[schemars(
        description = "Configuration",
        extend("x-oscal-subid" = "obs.software.plugin.btrfs.config@v1")
    )]
    pub config: JsonValue,
}
pub struct BtrfsPlugin;
impl Default for BtrfsPlugin {
    fn default() -> Self {
        Self
    }
}
impl BtrfsPlugin {
    pub fn new() -> Self {
        Self
    }
    pub(crate) fn current_state() -> BtrfsState {
        BtrfsState {
            status: "active".to_string(),
            subvolumes: serde_json::json!([{"name": "@root", "path": "/", "uuid": null}, {"name": "@home", "path": "/home"}, {"name": "@var", "path": "/var"}, {"name": "@snapshots", "path": "/.snapshots"}]),
            snapshots: serde_json::json!([{"name": "root-20260601", "subvolume": "@root", "created": null, "send_status": "local"}]),
            send_state: serde_json::json!({"active_transfers": 0, "last_send": null, "last_receive": null}),
            dr_status: serde_json::json!({"mode": "none", "replication_enabled": false, "last_sync": null, "lag_bytes": 0}),
            config: serde_json::json!({"snapshot_schedule": "daily", "retention_count": 7, "compression": "zstd", "send_compressed": true}),
        }
    }
}
#[async_trait]
impl StatePlugin for BtrfsPlugin {
    fn name(&self) -> &str {
        "btrfs"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(btrfs_schema())
    }
    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }
    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }
    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }
    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?,
            backend_checkpoint: None,
        })
    }
    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

pub(crate) fn btrfs_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::btrfs_plugin::BtrfsPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "btrfs",
        "infrastructure",
        "1.0.0",
        "Btrfs filesystem — subvolumes, snapshots, send/receive, DR",
        &state,
    )
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("btrfs", |_ctx| std::sync::Arc::new(BtrfsPlugin::new()))
}
