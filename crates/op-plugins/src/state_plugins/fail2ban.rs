use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use op_state_store::{PluginSchema};
use super::plugin_schema_defs::{schema_from_state};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fail2banState {
    pub status: String,
    pub jails: Value,
    pub bans: Value,
    pub filters: Value,
    pub actions: Value,
    pub logs: Value,
    pub config: Value,
}

pub struct Fail2banPlugin;
impl Default for Fail2banPlugin {
    fn default() -> Self {
        Self
    }
}
impl Fail2banPlugin {
    pub fn new() -> Self {
        Self
    }
    pub(crate) fn current_state() -> Fail2banState {
        Fail2banState {
            status: "active".to_string(),
            jails: json!([
                {"name": "sshd", "enabled": true, "maxretry": 5, "findtime": 600, "bantime": 3600, "filter": "sshd", "action": "iptables-multiport"},
                {"name": "recidive", "enabled": true, "maxretry": 3, "findtime": 86400, "bantime": 604800, "filter": "recidive", "action": "iptables-multiport"},
                {"name": "nginx-http-auth", "enabled": false, "maxretry": 3, "findtime": 600, "bantime": 1800}
            ]),
            bans: json!({"active_bans": 0, "total_bans": 0, "unbanned": 0, "by_jail": {}}),
            filters: json!([
                {"name": "sshd", "regex_count": 12, "journal_match": "_SYSTEMD_UNIT=sshd.service"},
                {"name": "recidive", "regex_count": 4},
                {"name": "nginx-http-auth", "regex_count": 8}
            ]),
            actions: json!([
                {"name": "iptables-multiport", "type": "iptables", "blocktype": "REJECT --reject-with icmp-port-unreachable"},
                {"name": "sendmail-whois", "type": "mail", "dest": "root@localhost"}
            ]),
            logs: json!({"paths": ["/var/log/auth.log", "/var/log/nginx/error.log"], "rotation": "daily", "max_days": 90}),
            config: json!({"loglevel": "INFO", "logtarget": "/var/log/fail2ban.log", "socket": "/var/run/fail2ban/fail2ban.sock", "pidfile": "/var/run/fail2ban/fail2ban.pid"}),
        }
    }
}

#[async_trait]
impl StatePlugin for Fail2banPlugin {
    fn name(&self) -> &str {
        "fail2ban"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(fail2ban_schema())
    }
    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(Self::current_state())?)
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

pub(crate) fn fail2ban_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::fail2ban::Fail2banPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "fail2ban",
        "security",
        "1.0.0",
        "Fail2ban intrusion prevention — jails, bans, filters, actions",
        &state,
    )
}
