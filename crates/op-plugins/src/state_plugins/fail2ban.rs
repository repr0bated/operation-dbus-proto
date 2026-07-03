use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.security.plugin.fail2ban.schema@v1"))]
pub struct Fail2banState {
    pub status: String,
    #[schemars(with = "serde_json::Value")]
    pub jails: JsonValue,
    #[schemars(with = "serde_json::Value")]
    pub bans: JsonValue,
    #[schemars(with = "serde_json::Value")]
    pub filters: JsonValue,
    #[schemars(with = "serde_json::Value")]
    pub actions: JsonValue,
    #[schemars(with = "serde_json::Value")]
    pub logs: JsonValue,
    #[schemars(with = "serde_json::Value")]
    pub config: JsonValue,
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
    let root = serde_json::to_value(schemars::schema_for!(Fail2banState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "fail2ban",
        "1.0.0",
        "Fail2ban intrusion prevention — jails, bans, filters, actions",
        &root,
    );
    schema.category = "security".to_string();
    if let Ok(defaults) = simd_json::serde::to_owned_value(Fail2banPlugin::current_state()) {
        super::schemars_adapter::apply_state_defaults(&mut schema, &defaults);
    }

    schema.methods.insert(
        "ban_ip".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<BanIpInput, super::plugin_scaffold_helpers::AckOutput>(
            "BanIp",
            op_state_store::SideEffect::Mutation,
            false,
            "fail2ban.write",
            "mut.security.fail2ban.ip.ban@v1",
        ),
    );
    schema.methods.insert(
        "unban_ip".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<BanIpInput, super::plugin_scaffold_helpers::AckOutput>(
            "UnbanIp",
            op_state_store::SideEffect::Mutation,
            false,
            "fail2ban.write",
            "mut.security.fail2ban.ip.unban@v1",
        ),
    );
    schema.methods.insert(
        "get_jail_status".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<JailInput, super::plugin_scaffold_helpers::AckOutput>(
            "GetJailStatus",
            op_state_store::SideEffect::Read,
            true,
            "fail2ban.read",
            "obs.security.fail2ban.jail.status@v1",
        ),
    );
    schema.methods.insert(
        "reload".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<super::plugin_scaffold_helpers::EmptyInput, super::plugin_scaffold_helpers::AckOutput>(
            "Reload",
            op_state_store::SideEffect::Mutation,
            false,
            "fail2ban.write",
            "mut.security.fail2ban.service.reload@v1",
        ),
    );

    schema
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JailInput {
    pub jail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BanIpInput {
    pub jail: String,
    pub ip: String,
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("fail2ban", |_ctx| std::sync::Arc::new(Fail2banPlugin::new()))
}
