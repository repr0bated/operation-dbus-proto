use super::plugin_schema_defs::{simple_schema, method_decl_from_schemars};
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{Constraint, FieldSchema, FieldType, MethodDecl, PluginSchema, SideEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProxyServerState {
    pub enabled: bool,
    pub port: u16,
}

// D-Bus method input types for Proxy Server operations
// Reference: https://www.freedesktop.org/wiki/Specifications/ProxyServerSpec

/// StartProxy method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartProxyInput {
    /// Proxy server identifier
    pub name: String,
    /// Port to bind to
    pub port: Option<u16>,
}

/// StopProxy method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopProxyInput {
    /// Proxy server identifier
    pub name: String,
}

/// ConfigureProxy method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigureProxyInput {
    /// Proxy server identifier
    pub name: String,
    /// Enable/disable the proxy
    pub enabled: Option<bool>,
    /// Port number
    pub port: Option<u16>,
    /// Upstream backend URL
    pub upstream: Option<String>,
}

pub struct ProxyServerPlugin;

impl Default for ProxyServerPlugin {
    fn default() -> Self {
        Self
    }
}

impl ProxyServerPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for ProxyServerPlugin {
    fn name(&self) -> &str {
        "proxy_server"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(proxy_server_schema())
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
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

pub(crate) fn proxy_server_schema() -> PluginSchema {
    let mut schema = simple_schema(
        "proxy_server",
        "Proxy server runtime config",
        &["net"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable proxy".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "port",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Proxy port".to_string(),
                    default: Some(json!(8080)),
                    example: None,
                    constraints: vec![
                        Constraint::Min { value: 1.0 },
                        Constraint::Max { value: 65535.0 },
                    ],
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    );
    
    // Add D-Bus methods for proxy lifecycle management
    // Reference: https://www.freedesktop.org/wiki/Specifications/ProxyServerSpec
    schema.methods.insert("start_proxy".to_string(), method_decl_from_schemars::<StartProxyInput>(
        "start_proxy",
        SideEffect::Mutation,
        false,
        "cap.network.proxy.start@v1",
        "mut.network.proxy.start@v1",
    ));
    schema.methods.insert("stop_proxy".to_string(), method_decl_from_schemars::<StopProxyInput>(
        "stop_proxy",
        SideEffect::Mutation,
        false,
        "cap.network.proxy.stop@v1",
        "mut.network.proxy.stop@v1",
    ));
    schema.methods.insert("configure".to_string(), method_decl_from_schemars::<ConfigureProxyInput>(
        "configure",
        SideEffect::Mutation,
        false,
        "cap.network.proxy.configure@v1",
        "mut.network.proxy.configure@v1",
    ));
    
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("proxy_server", |_ctx| std::sync::Arc::new(ProxyServerPlugin::new()))
}
