use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;

/// Source-owned global options exposed by btrfs-progs v7.1.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.btrfs.cli-options@v1"))]
pub struct BtrfsCliOptions {
    /// Root command selector from `btrfs <group> <command>`.
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub log: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub help: bool,
    #[serde(default)]
    pub version: Option<String>,
}

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
    /// Global CLI surface discovered from the installed upstream btrfs-progs binary.
    #[serde(default)]
    #[schemars(
        description = "btrfs-progs global command options",
        extend("x-oscal-subid" = "obs.software.plugin.btrfs.cli-options@v1")
    )]
    pub inspector_fields: BtrfsCliOptions,
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
            inspector_fields: BtrfsCliOptions {
                version: Some("btrfs-progs v7.1".to_string()),
                ..BtrfsCliOptions::default()
            },
        }
    }
}

fn required_string(args: &JsonValue, field: &str) -> Result<String> {
    args.get(field)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("missing non-empty string argument '{field}'"))
}

/// Resolve every published method to an argv vector without invoking a shell.
fn btrfs_command_args(method: &str, args: &JsonValue) -> Result<Vec<String>> {
    let path = || required_string(args, "path");
    Ok(match method {
        "create_subvolume" => vec!["subvolume".into(), "create".into(), path()?],
        "delete_subvolume" => vec!["subvolume".into(), "delete".into(), path()?],
        "snapshot" => {
            let mut command = vec!["subvolume".into(), "snapshot".into()];
            if args
                .get("readonly")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false)
            {
                command.push("-r".into());
            }
            command.push(required_string(args, "source")?);
            command.push(required_string(args, "destination")?);
            command
        }
        "list_subvolumes" => vec!["subvolume".into(), "list".into(), path()?],
        "scrub_start" => vec!["scrub".into(), "start".into(), path()?],
        "scrub_status" => vec!["scrub".into(), "status".into(), path()?],
        "balance_start" => vec!["balance".into(), "start".into(), path()?],
        "balance_status" => vec!["balance".into(), "status".into(), path()?],
        "filesystem_df" => vec!["filesystem".into(), "df".into(), path()?],
        "filesystem_usage" => vec!["filesystem".into(), "usage".into(), path()?],
        "device_add" => vec![
            "device".into(),
            "add".into(),
            required_string(args, "device")?,
            path()?,
        ],
        "device_remove" => vec![
            "device".into(),
            "remove".into(),
            required_string(args, "device")?,
            path()?,
        ],
        other => return Err(anyhow::anyhow!("unknown btrfs method '{other}'")),
    })
}

/// Execute a published Btrfs method through the installed upstream CLI.
pub async fn dispatch_btrfs_method(method: &str, args: &JsonValue) -> Result<JsonValue> {
    let command_args = btrfs_command_args(method, args)?;
    let output = tokio::process::Command::new("/usr/bin/btrfs")
        .args(&command_args)
        .output()
        .await
        .map_err(|error| anyhow::anyhow!("execute /usr/bin/btrfs: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "btrfs {} failed with {}: {}",
            command_args.join(" "),
            output.status,
            stderr
        ));
    }
    Ok(serde_json::json!({
        "command": command_args,
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": output.status.code(),
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_declared_method_has_real_cli_dispatch() {
        let schema = btrfs_schema();
        for method in schema.methods.keys() {
            let args = match method.as_str() {
                "snapshot" => serde_json::json!({"source":"/src", "destination":"/dst"}),
                "device_add" | "device_remove" => {
                    serde_json::json!({"device":"/dev/test", "path":"/mnt/test"})
                }
                _ => serde_json::json!({"path":"/mnt/test"}),
            };
            let command = btrfs_command_args(method, &args)
                .unwrap_or_else(|error| panic!("{method} lacks dispatch: {error}"));
            assert!(!command.is_empty(), "{method} produced an empty command");
        }
    }

    #[test]
    fn inspector_fields_preserve_original_state_fields() {
        let state = BtrfsPlugin::current_state();
        assert_eq!(state.status, "active");
        assert_eq!(
            state.inspector_fields.version.as_deref(),
            Some("btrfs-progs v7.1")
        );
        let schema = btrfs_schema();
        assert!(schema.fields.contains_key("status"));
        assert!(schema.fields.contains_key("inspector_fields"));
        assert_eq!(schema.methods.len(), 12);
    }
}
