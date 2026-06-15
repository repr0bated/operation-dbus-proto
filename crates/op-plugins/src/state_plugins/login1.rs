//! login1 plugin - read-only D-Bus snapshot for sessions/seats

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Login1State {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub uid: u32,
    pub user: String,
    pub seat: String,
    pub path: String,
}

pub struct Login1Plugin;

impl Login1Plugin {
    pub fn new() -> Self {
        Self
    }

    async fn connect_manager(&self) -> Result<Proxy<'static>> {
        let conn = Connection::system().await?;
        let p = Proxy::new(
            &conn,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await?;
        Ok(p)
    }
}

impl Default for Login1Plugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for Login1Plugin {
    fn name(&self) -> &str {
        "login1"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(
            PluginSchema::builder("login1")
                .version("1.0.0")
                .description("Runtime login sessions")
                .field(
                    "sessions",
                    FieldSchema {
                        field_type: FieldType::Any,
                        required: true,
                        description: "Active sessions".to_string(),
                        default: Some(json!([])),
                        example: None,
                        constraints: Vec::new(),
                        read_only: false,
                        read_only_when: None,
                    },
                )
                .build(),
        )
    }

    async fn query_current_state(&self) -> Result<Value> {
        let proxy = self.connect_manager().await?;
        // ListSessions -> a(sssso) per docs: (s, u, s, s, o)
        let raw: Vec<(String, u32, String, String, OwnedObjectPath)> =
            proxy.call("ListSessions", &()).await?;
        let sessions: Vec<SessionInfo> = raw
            .into_iter()
            .map(|(id, uid, user, seat, path)| SessionInfo {
                id,
                uid,
                user,
                seat,
                path: path.to_string(),
            })
            .collect();
        Ok(simd_json::serde::to_owned_value(Login1State { sessions })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let actions = if current != desired {
            vec![StateAction::Modify {
                resource: "login1".into(),
                changes: desired.clone(),
            }]
        } else {
            vec![]
        };
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec!["read-only".into()],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("login1-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }
}
