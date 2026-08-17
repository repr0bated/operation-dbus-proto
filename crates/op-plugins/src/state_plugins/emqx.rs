//! EMQX present-state plugin — GB.Emqx.
//!
//! Present-state for the live NIC-less NetMaker broker: sockets, ExHook target,
//! registered hook names. ExHook RPCs stay on HookProvider
//! (`emqx.exhook.v2` on `container.sock`). NetMaker reads this payload; it does
//! not own EMQX. MQTT is payload, not a plugin. Inspector Gadget on
//! `/srv/git/emqx/emqx.xml` produced messages, not plugin methods.

use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
use super::unix_socket::SHARED_CONTAINER_SOCKET;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{PluginSchema, SideEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;
use std::path::Path;

const PLUGIN_NAME: &str = "emqx";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "net";
const PLUGIN_DESCRIPTION: &str =
    "EMQX present-state for the NIC-less NetMaker container: broker.sock, api.sock, container.sock, ExHook target on :8090";
const PLUGIN_DISPLAY_NAME: &str = "GB.Emqx";

pub const CONTAINER_NAME: &str = "NetMaker";
pub const BROKER_SOFTWARE: &str = "emqx";
pub const BROKER_TYPE: &str = "emqx";
pub const BROKER_SOCKET: &str = "/run/ghostbridge/NetMaker/broker.sock";
pub const API_SOCKET: &str = "/run/ghostbridge/NetMaker/api.sock";
pub const HOST_BROKER_ALIAS: &str = "/run/ghostbridge/netmaker-broker.sock";
pub const EXHOOK_PROTO: &str = "emqx.exhook.v2";
pub const EXHOOK_SERVICE: &str = "emqx.exhook.v2.HookProvider";
pub const EXHOOK_TARGET: &str = "unix:/run/ghostbridge/container.sock";
pub const MQTT_WS_URL: &str = "ws://100.69.0.1:8090/mqtt";
pub const MQTT_TCP: &str = "127.0.0.1:1883";
pub const DASHBOARD_URL: &str = "http://127.0.0.1:18083";

/// HookSpec.name values from `exhook.proto`. RPCs stay on HookProvider.
const REGISTERED_HOOKS: &[&str] = &[
    "client.connect",
    "client.connack",
    "client.connected",
    "client.disconnected",
    "client.authenticate",
    "client.authorize",
    "client.subscribe",
    "client.unsubscribe",
    "session.created",
    "session.subscribed",
    "session.unsubscribed",
    "session.resumed",
    "session.discarded",
    "session.takenover",
    "session.terminated",
    "message.publish",
    "message.delivered",
    "message.acked",
    "message.dropped",
];

fn socket_present(path: &str) -> bool {
    Path::new(path).is_file() || Path::new(path).exists()
}

fn typed_input<T: serde::de::DeserializeOwned>(args: &JsonValue) -> Result<T> {
    serde_json::from_value(if args.is_null() {
        JsonValue::Object(Default::default())
    } else {
        args.clone()
    })
    .map_err(|err| anyhow::anyhow!("emqx method input must match the typed schema: {err}"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EmqxSocket {
    /// Socket identity tag.
    #[serde(default)]
    pub name: String,
    /// Filesystem path of the published Unix socket.
    #[serde(default)]
    pub path: String,
    /// Whether the path exists on the host now.
    #[serde(default)]
    pub present: bool,
    /// Why this socket exists.
    #[serde(default)]
    pub purpose: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EmqxExhook {
    /// ExHook proto package. Live is `emqx.exhook.v2`.
    #[serde(default)]
    pub proto: String,
    /// HookProvider service name EMQX calls.
    #[serde(default)]
    pub service: String,
    /// One gRPC door. Not a second TCP listener.
    #[serde(default)]
    pub target: String,
    /// Whether live `exhook.servers` has been pointed at HookProvider.
    #[serde(default)]
    pub servers_configured: bool,
    /// HookSpec.name values advertised on OnProviderLoaded.
    #[serde(default)]
    pub registered_hooks: Vec<String>,
}

/// Plugin-wide OSCAL lives here: one schema subid + category.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.emqx.schema@v1"))]
#[schemars(extend("x-oscal-category" = "network"))]
#[schemars(extend("x-immutable-paths" = ["software", "broker_type", "container_name", "nic"]))]
pub struct EmqxState {
    /// Authoritative broker process. Live is EMQX.
    #[serde(default)]
    pub software: String,
    /// Broker implementation. Live is `emqx`, never Mosquitto.
    #[serde(default)]
    pub broker_type: String,
    /// Plugin/schema version, not the EMQX package version.
    #[serde(default)]
    pub version: String,
    /// Incus container that runs EMQX + NetMaker.
    #[serde(default)]
    pub container_name: String,
    /// Containers have no NIC by design.
    #[serde(default)]
    pub nic: bool,
    /// Shared gRPC door bind-mounted into the container.
    #[serde(default)]
    pub container_socket: String,
    /// EMQX MQTT/WS payload socket.
    #[serde(default)]
    pub broker_socket: String,
    /// NetMaker REST API socket.
    #[serde(default)]
    pub api_socket: String,
    /// Loopback MQTT TCP listener inside the container.
    #[serde(default)]
    pub mqtt_tcp: String,
    /// Mesh MQTT websocket URL.
    #[serde(default)]
    pub mqtt_ws: String,
    /// EMQX dashboard URL. Not a control-plane door.
    #[serde(default)]
    pub dashboard: String,
    /// Published Unix sockets (container / broker / api).
    #[serde(default)]
    pub sockets: Vec<EmqxSocket>,
    /// ExHook tap that EMQX should call on the one gRPC door.
    #[serde(default)]
    pub exhook: EmqxExhook,
}

impl EmqxState {
    pub fn observed() -> Self {
        Self {
            software: BROKER_SOFTWARE.to_string(),
            broker_type: BROKER_TYPE.to_string(),
            version: PLUGIN_VERSION.to_string(),
            container_name: CONTAINER_NAME.to_string(),
            nic: false,
            container_socket: SHARED_CONTAINER_SOCKET.to_string(),
            broker_socket: BROKER_SOCKET.to_string(),
            api_socket: API_SOCKET.to_string(),
            mqtt_tcp: MQTT_TCP.to_string(),
            mqtt_ws: MQTT_WS_URL.to_string(),
            dashboard: DASHBOARD_URL.to_string(),
            sockets: vec![
                EmqxSocket {
                    name: "container".to_string(),
                    path: SHARED_CONTAINER_SOCKET.to_string(),
                    present: socket_present(SHARED_CONTAINER_SOCKET),
                    purpose: "one gRPC door / HookProvider".to_string(),
                },
                EmqxSocket {
                    name: "broker".to_string(),
                    path: BROKER_SOCKET.to_string(),
                    present: socket_present(BROKER_SOCKET),
                    purpose: "EMQX MQTT/WS payload".to_string(),
                },
                EmqxSocket {
                    name: "api".to_string(),
                    path: API_SOCKET.to_string(),
                    present: socket_present(API_SOCKET),
                    purpose: "NetMaker REST via UDS relay".to_string(),
                },
                EmqxSocket {
                    name: "broker-alias".to_string(),
                    path: HOST_BROKER_ALIAS.to_string(),
                    present: socket_present(HOST_BROKER_ALIAS),
                    purpose: "host alias for broker.sock".to_string(),
                },
            ],
            exhook: EmqxExhook {
                proto: EXHOOK_PROTO.to_string(),
                service: EXHOOK_SERVICE.to_string(),
                target: EXHOOK_TARGET.to_string(),
                servers_configured: false,
                registered_hooks: REGISTERED_HOOKS.iter().map(|s| (*s).to_string()).collect(),
            },
        }
    }
}

pub struct EmqxPlugin;

impl EmqxPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn current_state() -> EmqxState {
        EmqxState::observed()
    }
}

impl Default for EmqxPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GetStatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetStatusOutput {
    pub software: String,
    pub broker_type: String,
    pub container_name: String,
    pub nic: bool,
    pub container_socket: String,
    pub broker_socket: String,
    pub api_socket: String,
    pub broker_socket_present: bool,
    pub api_socket_present: bool,
    pub container_socket_present: bool,
    pub exhook_service: String,
    pub exhook_target: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListSocketsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSocketsOutput {
    pub sockets: Vec<EmqxSocket>,
}

/// Typed present-state methods only. ExHook RPCs stay on HookProvider.
pub async fn dispatch_emqx_method(method: &str, args: &JsonValue) -> Result<JsonValue> {
    match method {
        "get_status" => {
            let _input: GetStatusInput = typed_input(args)?;
            let state = EmqxPlugin::current_state();
            Ok(serde_json::to_value(GetStatusOutput {
                software: state.software,
                broker_type: state.broker_type,
                container_name: state.container_name,
                nic: state.nic,
                container_socket: state.container_socket,
                broker_socket: state.broker_socket,
                api_socket: state.api_socket,
                broker_socket_present: socket_present(BROKER_SOCKET),
                api_socket_present: socket_present(API_SOCKET),
                container_socket_present: socket_present(SHARED_CONTAINER_SOCKET),
                exhook_service: state.exhook.service,
                exhook_target: state.exhook.target,
            })?)
        }
        "list_sockets" => {
            let _input: ListSocketsInput = typed_input(args)?;
            Ok(serde_json::to_value(ListSocketsOutput {
                sockets: EmqxPlugin::current_state().sockets,
            })?)
        }
        other => anyhow::bail!("unknown emqx method: {other}"),
    }
}

#[async_trait]
impl StatePlugin for EmqxPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(emqx_schema())
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: PLUGIN_NAME.to_string(),
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
            id: format!("{PLUGIN_NAME}-checkpoint"),
            plugin: PLUGIN_NAME.to_string(),
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
            atomic_operations: true,
        }
    }
}

pub(crate) fn emqx_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(EmqxState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());
    if let Ok(defaults) = simd_json::serde::to_owned_value(EmqxPlugin::current_state()) {
        super::schemars_adapter::apply_state_defaults(&mut schema, &defaults);
        schema.example = Some(defaults);
    }
    schema.methods.insert(
        "get_status".to_string(),
        method_decl_from_schemars_with_output::<GetStatusInput, GetStatusOutput>(
            "get_status",
            SideEffect::Read,
            true,
            "cap.network.emqx.status.get@v1",
            "obs.network.emqx.status.get@v1",
        ),
    );
    schema.methods.insert(
        "list_sockets".to_string(),
        method_decl_from_schemars_with_output::<ListSocketsInput, ListSocketsOutput>(
            "list_sockets",
            SideEffect::Read,
            true,
            "cap.network.emqx.sockets.list@v1",
            "obs.network.emqx.sockets.list@v1",
        ),
    );
    schema.capabilities.insert(
        "cap.network.emqx.status.get@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.network.emqx.status.get@v1".to_string(),
            description: "Grants: get_status.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.network.emqx.sockets.list@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.network.emqx.sockets.list@v1".to_string(),
            description: "Grants: list_sockets.".to_string(),
        },
    );

    schema
}

inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(EmqxPlugin::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::prelude::*;

    #[test]
    fn schema_is_present_state_only() {
        let schema = emqx_schema();
        assert_eq!(schema.name, "emqx");
        assert!(schema.methods.contains_key("get_status"));
        assert!(schema.methods.contains_key("list_sockets"));
        assert!(!schema.methods.contains_key("on_provider_loaded"));
        assert!(!schema.methods.contains_key("record_hook"));
        assert_eq!(
            schema.subids.get("__schema__").map(String::as_str),
            Some("sch.software.plugin.emqx.schema@v1")
        );
        let example = schema.example.expect("emqx schema seeds present-state");
        let blob = simd_json::serde::to_owned_value(example).expect("example value");
        assert_eq!(blob.get("software").and_then(|v| v.as_str()), Some("emqx"));
        assert_eq!(
            blob.get("broker_socket").and_then(|v| v.as_str()),
            Some(BROKER_SOCKET)
        );
    }

    #[tokio::test]
    async fn dispatch_typed_status() {
        let result = dispatch_emqx_method("get_status", &serde_json::json!({}))
            .await
            .expect("typed get_status");
        assert_eq!(
            result.get("broker_type").and_then(|v| v.as_str()),
            Some("emqx")
        );
    }
}
