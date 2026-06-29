use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;

use simd_json::OwnedValue as Value;

use super::plugin_schema_defs::{any_field, method_decl_from_schemars, simple_schema};
use op_state_store::PluginSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersState {
    pub users: Vec<UserConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub username: String,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub groups: Vec<String>,
    pub shell: Option<String>,
    pub present: bool,
}

/// Input struct for CreateUser method.
/// See: https://www.freedesktop.org/wiki/Software/systemd/
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateUserInput {
    /// Username to create
    pub username: String,
    /// User ID (optional, auto-assigned if not provided)
    pub uid: Option<u32>,
    /// Primary group ID (optional)
    pub gid: Option<u32>,
    /// Additional groups
    #[serde(default)]
    pub groups: Vec<String>,
    /// Login shell
    pub shell: Option<String>,
}

/// Input struct for DeleteUser method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteUserInput {
    /// Username to delete
    pub username: String,
    /// Remove home directory
    #[serde(default)]
    pub remove_home: bool,
}

/// Input struct for ModifyUser method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModifyUserInput {
    /// Username to modify
    pub username: String,
    /// New login shell
    pub shell: Option<String>,
    /// New primary group
    pub gid: Option<u32>,
    /// Groups to add
    #[serde(default)]
    pub add_groups: Vec<String>,
    /// Groups to remove
    #[serde(default)]
    pub remove_groups: Vec<String>,
}

/// Input struct for ListUsers method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListUsersInput {
    /// Filter by group (optional)
    pub group: Option<String>,
}

pub struct UsersPlugin;

impl Default for UsersPlugin {
    fn default() -> Self {
        Self
    }
}

impl UsersPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for UsersPlugin {
    fn name(&self) -> &str {
        "users"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(users_schema())
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "unknown".to_string(),
                desired_hash: "unknown".to_string(),
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
            state_snapshot: Value::null(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
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

pub(crate) fn users_schema() -> PluginSchema {
    let mut schema = simple_schema(
        "users",
        "User account declarations",
        &[],
        vec![("users", any_field(true, "Users list", Some(json!([]))))],
    );

    // CreateUser method - https://www.freedesktop.org/wiki/Software/systemd/
    schema.methods.insert(
        "create_user".to_string(),
        method_decl_from_schemars::<CreateUserInput>(
            "CreateUser",
            op_state_store::SideEffect::Mutation,
            false,
            "users.write",
            "mut.software.plugin.users.create@v1",
        ),
    );

    // DeleteUser method
    schema.methods.insert(
        "delete_user".to_string(),
        method_decl_from_schemars::<DeleteUserInput>(
            "DeleteUser",
            op_state_store::SideEffect::Mutation,
            false,
            "users.write",
            "mut.software.plugin.users.delete@v1",
        ),
    );

    // ModifyUser method
    schema.methods.insert(
        "modify_user".to_string(),
        method_decl_from_schemars::<ModifyUserInput>(
            "ModifyUser",
            op_state_store::SideEffect::Mutation,
            false,
            "users.write",
            "mut.software.plugin.users.modify@v1",
        ),
    );

    // ListUsers method
    schema.methods.insert(
        "list_users".to_string(),
        method_decl_from_schemars::<ListUsersInput>(
            "ListUsers",
            op_state_store::SideEffect::Read,
            true,
            "users.read",
            "obs.software.plugin.users.list@v1",
        ),
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("users", |_ctx| std::sync::Arc::new(UsersPlugin::new()))
}
