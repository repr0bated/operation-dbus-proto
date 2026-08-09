use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateSubvolumeInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteSubvolumeInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotInput {
    pub source: String,
    pub destination: String,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSubvolumesInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScrubStartInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScrubStatusInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BalanceStartInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BalanceStatusInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FilesystemDfInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FilesystemUsageInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeviceAddInput {
    pub device: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeviceRemoveInput {
    pub device: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.btrfs.schema@v1"))]
#[schemars(extend("x-oscal-category" = "storage"))]
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
    let root = serde_json::to_value(schemars::schema_for!(BtrfsState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "btrfs",
        "1.0.0",
        "Btrfs filesystem — subvolumes, snapshots, send/receive, DR",
        &root,
    );
    schema.category = "infrastructure".to_string();
    if let Ok(defaults) = simd_json::serde::to_owned_value(BtrfsPlugin::current_state()) {
        super::schemars_adapter::apply_state_defaults(&mut schema, &defaults);
    }

    schema.methods.insert(
        "create_subvolume".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            CreateSubvolumeInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "create_subvolume",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.storage.btrfs.subvolume.create@v1",
            "mut.storage.btrfs.subvolume.create@v1",
        ),
    );
    schema.methods.insert(
        "delete_subvolume".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            DeleteSubvolumeInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "delete_subvolume",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.storage.btrfs.subvolume.delete@v1",
            "mut.storage.btrfs.subvolume.delete@v1",
        ),
    );
    schema.methods.insert(
        "snapshot".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SnapshotInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "snapshot",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.storage.btrfs.snapshot.create@v1",
            "mut.storage.btrfs.snapshot.create@v1",
        ),
    );
    schema.methods.insert(
        "list_subvolumes".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ListSubvolumesInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "list_subvolumes",
            op_state_store::SideEffect::Read,
            true,
            "cap.storage.btrfs.subvolume.list@v1",
            "obs.storage.btrfs.subvolume.list@v1",
        ),
    );
    schema.methods.insert(
        "scrub_start".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ScrubStartInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "scrub_start",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.storage.btrfs.scrub.start@v1",
            "mut.storage.btrfs.scrub.start@v1",
        ),
    );
    schema.methods.insert(
        "scrub_status".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ScrubStatusInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "scrub_status",
            op_state_store::SideEffect::Read,
            true,
            "cap.storage.btrfs.scrub.status@v1",
            "obs.storage.btrfs.scrub.status@v1",
        ),
    );
    schema.methods.insert(
        "balance_start".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            BalanceStartInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "balance_start",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.storage.btrfs.balance.start@v1",
            "mut.storage.btrfs.balance.start@v1",
        ),
    );
    schema.methods.insert(
        "balance_status".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            BalanceStatusInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "balance_status",
            op_state_store::SideEffect::Read,
            true,
            "cap.storage.btrfs.balance.status@v1",
            "obs.storage.btrfs.balance.status@v1",
        ),
    );
    schema.methods.insert(
        "filesystem_df".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            FilesystemDfInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "filesystem_df",
            op_state_store::SideEffect::Read,
            true,
            "cap.storage.btrfs.filesystem.df@v1",
            "obs.storage.btrfs.filesystem.df@v1",
        ),
    );
    schema.methods.insert(
        "filesystem_usage".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            FilesystemUsageInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "filesystem_usage",
            op_state_store::SideEffect::Read,
            true,
            "cap.storage.btrfs.filesystem.usage@v1",
            "obs.storage.btrfs.filesystem.usage@v1",
        ),
    );
    schema.methods.insert(
        "device_add".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            DeviceAddInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "device_add",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.storage.btrfs.device.add@v1",
            "mut.storage.btrfs.device.add@v1",
        ),
    );
    schema.methods.insert(
        "device_remove".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            DeviceRemoveInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "device_remove",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.storage.btrfs.device.remove@v1",
            "mut.storage.btrfs.device.remove@v1",
        ),
    );
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("btrfs", |_ctx| std::sync::Arc::new(BtrfsPlugin::new()))
}

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.btrfs-plugin.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `flag.root.dry_run`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.btrfs-plugin.dry-run@v1"))]
        pub dry_run: Option<String>,

        /// Discovered from Repomix path `flag.root.help`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.btrfs-plugin.help@v1"))]
        pub help: Option<String>,

        /// Discovered from Repomix path `flag.root.log`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.btrfs-plugin.log@v1"))]
        pub log: Option<String>,

        /// Discovered from Repomix path `flag.root.quiet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.btrfs-plugin.quiet@v1"))]
        pub quiet: Option<String>,

        /// Discovered from Repomix path `flag.root.verbose`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.btrfs-plugin.verbose@v1"))]
        pub verbose: Option<String>,

        /// Discovered from Repomix path `flag.root.version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.btrfs-plugin.version@v1"))]
        pub version: Option<String>,

    }

    /// Metadata needed when promoting a generated typed method into `schema.methods`.
    pub struct MethodCandidate {
        pub name: &'static str,
        pub side_effect: &'static str,
        pub idempotent: bool,
        pub required_capability: &'static str,
        pub subid: &'static str,
        pub repomix_path: &'static str,
        pub command: &'static [&'static str],
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[
    ];

    /// Promote every generated method into the sealed plugin schema.
    pub(super) fn register_methods(schema: &mut op_state_store::PluginSchema) {
        use super::super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    }
}

// Promotion checklist (Fable contract):
// 1. Move owned fields into the plugin State struct with concrete Rust types.
// 2. Replace method placeholders with dedicated typed Input/Output fields.
// 3. Register with method_decl_from_schemars_with_output and correct SideEffect.
// 4. Register every subid, implement dispatch, and add schema/subid tests.
// 5. Re-run op-plugin-lint; only then replace the original plugin file.
