//! OCI container plugin — schema-driven lifecycle for Docker/OCI containers
//! managed by incus that need host-side bootstrap (loopback, netns init).
//!
//! OCI containers in incus with no NIC device boot with lo DOWN, preventing
//! services from binding to 127.0.0.1. This plugin declares which containers
//! need loopback bring-up, and the `rovs_commands` plugin handles the OVS
//! control-plane work.
//! D-Bus method executes it.
//!
//! THE PLUGIN IS THE SCHEMA — if a container is declared here with
//! loopback_required=true, the daemon will bring up lo inside its netns.
//!
//! D-Bus methods: https://docs.docker.com/engine/api/v1.45/

use anyhow::Result;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema, ReadOnlyCondition};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

// ── Schema definition — THE PLUGIN IS THE SCHEMA ─────────────────────────────

fn oci_container_fields() -> HashMap<String, FieldSchema> {
    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Incus instance name (must match hostname inside container)".to_string(),
            default: None,
            example: Some(json!("netmaker")),
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        },
    );
    fields.insert(
        "image".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "OCI image reference (e.g. docker:gravitl/netmaker:v1.5.1)".to_string(),
            default: None,
            example: Some(json!("docker:gravitl/netmaker:v1.5.1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "loopback_required".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether BringUpLoopback must run inside this container's netns before services start. Required for OCI containers with no NIC.".to_string(),
            default: Some(json!(false)),
            example: Some(json!(true)),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "entrypoint_override".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional wrapper command prepended before the OCI entrypoint. Used for lo-up and other pre-service hooks.".to_string(),
            default: None,
            example: Some(json!("/usr/local/bin/lo-up-wrapper.sh")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "privileged".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the container runs in privileged mode".to_string(),
            default: Some(json!(false)),
            example: Some(json!(true)),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "env".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Environment variables for the container".to_string(),
            default: Some(json!({})),
            example: Some(json!({"SERVER_HOST": "129.153.134.63", "API_PORT": "8081"})),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "sockets".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Proxy devices: unix socket listeners on host -> TCP inside container".to_string(),
            default: Some(json!([])),
            example: Some(json!([
                {"id": "api-sock", "listen": "unix:/run/netmaker/api.sock", "connect": "tcp:127.0.0.1:8081"}
            ])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "volumes".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Disk device mounts (storage volumes)".to_string(),
            default: Some(json!([])),
            example: Some(json!([
                {"id": "nm-sqldata", "path": "/root/data", "source": "nm-sqldata"}
            ])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "port_attach".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "OVS port attach config for this container (bridge, port_name, ip_addrs, gateway, routes). If present, ovs-attach calls AttachPort after BringUpLoopback.".to_string(),
            default: None,
            example: Some(json!({
                "bridge": "ovsbr0",
                "iface_name": "gbr_xray",
                "ip_addrs": ["10.200.0.1/30", "10.0.0.2/24"],
                "gateway": "10.200.0.2",
                "routes": ["10.0.0.1 via 10.200.0.2"]
            })),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields
}

/// Input struct for pulling an OCI image.
/// See: https://docs.docker.com/engine/api/v1.45/
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PullImageInput {
    /// Image reference (e.g., docker:gravitl/netmaker:v1.5.1)
    pub image: String,
}

/// Input struct for running a container from an OCI image.
/// See: https://docs.docker.com/engine/api/v1.45/
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunContainerInput {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub loopback_required: bool,
    #[serde(default)]
    pub privileged: bool,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateContainerInput {
    pub id: String,
    pub bundle_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartContainerInput {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KillContainerInput {
    pub id: String,
    pub signal: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteContainerInput {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetContainerStateInput {
    pub id: String,
}

pub(crate) fn oci_schema() -> PluginSchema {
    PluginSchema::builder("oci")
        .category("service")
        .version("1.0.0")
        .category("container")
        .description("OCI container lifecycle — schema-driven loopback bring-up, netns init, and port attach for incus-managed Docker/OCI containers")
        .dependency("incus")
        .dependency("rovs_commands")
        .array_field(
            "containers",
            FieldType::Object(oci_container_fields()),
            true,
            "Declared OCI containers with lifecycle config",
        )
        .example(json!({
            "containers": [
                {
                    "name": "netmaker",
                    "image": "docker:gravitl/netmaker:v1.5.1",
                    "loopback_required": true,
                    "privileged": true,
                    "env": {
                        "SERVER_HOST": "129.153.134.63",
                        "API_PORT": "8081",
                        "DATABASE": "sqlite"
                    },
                    "sockets": [
                        {"id": "api-sock", "listen": "unix:/run/netmaker/api.sock", "connect": "tcp:127.0.0.1:8081"}
                    ],
                    "volumes": [
                        {"id": "nm-sqldata", "path": "/root/data", "source": "nm-sqldata"}
                    ]
                },
                {
                    "name": "netmaker-mq",
                    "image": "docker:eclipse-mosquitto:2.0.15-openssl",
                    "loopback_required": true,
                    "sockets": [
                        {"id": "mqtt-sock", "listen": "unix:/run/netmaker/mq.sock", "connect": "tcp:127.0.0.1:1883"},
                        {"id": "mqtts-sock", "listen": "unix:/run/netmaker/mqtts.sock", "connect": "tcp:127.0.0.1:8883"}
                    ]
                },
                {
                    "name": "netmaker-ui",
                    "image": "docker:gravitl/netmaker-ui:v1.5.1",
                    "loopback_required": true,
                    "sockets": [
                        {"id": "ui-sock", "listen": "unix:/run/netmaker/ui.sock", "connect": "tcp:127.0.0.1:80"}
                    ]
                }
            ]
        }))
        .method(super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<PullImageInput, super::plugin_scaffold_helpers::AckOutput>(
            "pull_image",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.container.oci.image.pull@v1",
            "mut.container.oci.image.pull@v1",
        ))
        .method(super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<RunContainerInput, super::plugin_scaffold_helpers::AckOutput>(
            "run_container",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.container.oci.run@v1",
            "mut.container.oci.run@v1",
        ))
        .method(super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<CreateContainerInput, super::plugin_scaffold_helpers::AckOutput>(
            "create",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.container.oci.create@v1",
            "mut.container.oci.create@v1",
        ))
        .method(super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<StartContainerInput, super::plugin_scaffold_helpers::AckOutput>(
            "start",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.container.oci.start@v1",
            "mut.container.oci.start@v1",
        ))
        .method(super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<KillContainerInput, super::plugin_scaffold_helpers::AckOutput>(
            "kill",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.container.oci.kill@v1",
            "mut.container.oci.kill@v1",
        ))
        .method(super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<DeleteContainerInput, super::plugin_scaffold_helpers::AckOutput>(
            "delete",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.container.oci.delete@v1",
            "mut.container.oci.delete@v1",
        ))
        .method(super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<GetContainerStateInput, super::plugin_scaffold_helpers::AckOutput>(
            "state",
            op_state_store::SideEffect::Read,
            true,
            "cap.container.oci.state.get@v1",
            "obs.container.oci.state.get@v1",
        ))
        .capability(op_state_store::CapabilityDecl {
            id: "cap.container.oci.image.pull@v1".to_string(),
            description: "Grants: pull_image.".to_string(),
        })
        .capability(op_state_store::CapabilityDecl {
            id: "cap.container.oci.run@v1".to_string(),
            description: "Grants: run_container.".to_string(),
        })
        .capability(op_state_store::CapabilityDecl {
            id: "cap.container.oci.create@v1".to_string(),
            description: "Grants: create.".to_string(),
        })
        .capability(op_state_store::CapabilityDecl {
            id: "cap.container.oci.start@v1".to_string(),
            description: "Grants: start.".to_string(),
        })
        .capability(op_state_store::CapabilityDecl {
            id: "cap.container.oci.kill@v1".to_string(),
            description: "Grants: kill.".to_string(),
        })
        .capability(op_state_store::CapabilityDecl {
            id: "cap.container.oci.delete@v1".to_string(),
            description: "Grants: delete.".to_string(),
        })
        .capability(op_state_store::CapabilityDecl {
            id: "cap.container.oci.state.get@v1".to_string(),
            description: "Grants: state.".to_string(),
        })
        .build()
}

// ── Plugin implementation ─────────────────────────────────────────────────────

pub struct OciPlugin;

impl Default for OciPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl OciPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl StatePlugin for OciPlugin {
    fn name(&self) -> &str {
        "oci"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(oci_schema())
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
        let state = simd_json::json!(null);
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
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

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("oci", |_ctx| std::sync::Arc::new(OciPlugin::new()))
}
