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

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.oci.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.History`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.history@v1"))]
        pub history: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.History.field.Author`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.author@v1"))]
        pub author: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.History.field.Comment`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.comment@v1"))]
        pub comment: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.History.field.Created`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.created@v1"))]
        pub created: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.History.field.CreatedBy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.createdby@v1"))]
        pub createdby: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.History.field.EmptyLayer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.emptylayer@v1"))]
        pub emptylayer: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.Image.field.Config`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.config@v1"))]
        pub config: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.Image.field.Platform`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.platform@v1"))]
        pub platform: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.Image.field.RootFS`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rootfs@v1"))]
        pub rootfs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.imageconfig@v1"))]
        pub imageconfig: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.ArgsEscaped`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.argsescaped@v1"))]
        pub argsescaped: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.Cmd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cmd@v1"))]
        pub cmd: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.Entrypoint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.entrypoint@v1"))]
        pub entrypoint: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.ExposedPorts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.exposedports@v1"))]
        pub exposedports: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.Labels`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.labels@v1"))]
        pub labels: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.StopSignal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.stopsignal@v1"))]
        pub stopsignal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.User`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.user@v1"))]
        pub user: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.Volumes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.volumes@v1"))]
        pub volumes: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.ImageConfig.field.WorkingDir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.workingdir@v1"))]
        pub workingdir: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.RootFS.field.DiffIDs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.diffids@v1"))]
        pub diffids: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.config.struct.RootFS.field.Type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Descriptor.field.Annotations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.annotations@v1"))]
        pub annotations: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Descriptor.field.ArtifactType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.artifacttype@v1"))]
        pub artifacttype: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Descriptor.field.Data`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.data@v1"))]
        pub data: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Descriptor.field.Digest`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.digest@v1"))]
        pub digest: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Descriptor.field.MediaType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mediatype@v1"))]
        pub mediatype: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Descriptor.field.Size`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.size@v1"))]
        pub size: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Descriptor.field.URLs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.urls@v1"))]
        pub urls: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Platform.field.Architecture`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.architecture@v1"))]
        pub architecture: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Platform.field.OS`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.os@v1"))]
        pub os: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Platform.field.OSFeatures`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.osfeatures@v1"))]
        pub osfeatures: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Platform.field.OSVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.osversion@v1"))]
        pub osversion: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.descriptor.struct.Platform.field.Variant`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.variant@v1"))]
        pub variant: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.index.struct.Index.field.Manifests`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.manifests@v1"))]
        pub manifests: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.index.struct.Index.field.Subject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.subject@v1"))]
        pub subject: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.layout.struct.ImageLayout.field.Version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.version@v1"))]
        pub version: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.v1.manifest.struct.Manifest.field.Layers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.layers@v1"))]
        pub layers: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__image-spec.specs-go.versioned.struct.Versioned.field.SchemaVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schemaversion@v1"))]
        pub schemaversion: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.interface.Hook`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hook@v1"))]
        pub hook: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.interface.Hook.method.Run`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.run@v1"))]
        pub run: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Arg`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.arg@v1"))]
        pub arg: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Arg.field.Index`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.index@v1"))]
        pub index: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Arg.field.Op`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.op@v1"))]
        pub op: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Arg.field.Value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.value@v1"))]
        pub value: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Arg.field.ValueTwo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.valuetwo@v1"))]
        pub valuetwo: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cpuaffinity@v1"))]
        pub cpuaffinity: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.CreateContainer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.createcontainer@v1"))]
        pub createcontainer: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.CreateRuntime`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.createruntime@v1"))]
        pub createruntime: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.HookList`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hooklist@v1"))]
        pub hooklist: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.HookName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hookname@v1"))]
        pub hookname: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.Hooks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hooks@v1"))]
        pub hooks: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.Poststart`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.poststart@v1"))]
        pub poststart: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.Poststop`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.poststop@v1"))]
        pub poststop: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.Prestart`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.prestart@v1"))]
        pub prestart: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CPUAffinity.field.StartContainer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.startcontainer@v1"))]
        pub startcontainer: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Capabilities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.capabilities@v1"))]
        pub capabilities: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Capabilities.field.Ambient`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ambient@v1"))]
        pub ambient: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Capabilities.field.Bounding`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.bounding@v1"))]
        pub bounding: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Capabilities.field.Effective`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.effective@v1"))]
        pub effective: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Capabilities.field.Inheritable`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.inheritable@v1"))]
        pub inheritable: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Capabilities.field.Permitted`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.permitted@v1"))]
        pub permitted: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Command`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.command@v1"))]
        pub command: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Command.field.Args`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.args@v1"))]
        pub args: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Command.field.Dir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.dir@v1"))]
        pub dir: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Command.field.Timeout`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.timeout@v1"))]
        pub timeout: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.CommandHook`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.commandhook@v1"))]
        pub commandhook: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.AppArmorProfile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.apparmorprofile@v1"))]
        pub apparmorprofile: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Cgroups`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cgroups@v1"))]
        pub cgroups: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Devices`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.devices@v1"))]
        pub devices: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Domainname`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.domainname@v1"))]
        pub domainname: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.ExecCPUAffinity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.execcpuaffinity@v1"))]
        pub execcpuaffinity: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.GIDMappings`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.gidmappings@v1"))]
        pub gidmappings: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Hostname`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hostname@v1"))]
        pub hostname: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.IOPriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.iopriority@v1"))]
        pub iopriority: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.IntelRdt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.intelrdt@v1"))]
        pub intelrdt: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.MaskPaths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.maskpaths@v1"))]
        pub maskpaths: Option<Vec<String>>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.MemoryPolicy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.memorypolicy@v1"))]
        pub memorypolicy: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.MountLabel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mountlabel@v1"))]
        pub mountlabel: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Mounts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mounts@v1"))]
        pub mounts: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Namespaces`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.namespaces@v1"))]
        pub namespaces: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.NetDevices`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.netdevices@v1"))]
        pub netdevices: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Networks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.networks@v1"))]
        pub networks: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.NoNewKeyring`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.nonewkeyring@v1"))]
        pub nonewkeyring: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.NoNewPrivileges`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.nonewprivileges@v1"))]
        pub nonewprivileges: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.NoPivotRoot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.nopivotroot@v1"))]
        pub nopivotroot: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.OomScoreAdj`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.oomscoreadj@v1"))]
        pub oomscoreadj: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.ParentDeathSignal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.parentdeathsignal@v1"))]
        pub parentdeathsignal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Personality`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.personality@v1"))]
        pub personality: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.ProcessLabel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.processlabel@v1"))]
        pub processlabel: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.ReadonlyPaths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.readonlypaths@v1"))]
        pub readonlypaths: Option<Vec<String>>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Readonlyfs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.readonlyfs@v1"))]
        pub readonlyfs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Rlimits`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rlimits@v1"))]
        pub rlimits: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.RootPropagation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rootpropagation@v1"))]
        pub rootpropagation: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.RootlessCgroups`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rootlesscgroups@v1"))]
        pub rootlesscgroups: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.RootlessEUID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rootlesseuid@v1"))]
        pub rootlesseuid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Routes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.routes@v1"))]
        pub routes: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Scheduler`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.scheduler@v1"))]
        pub scheduler: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Seccomp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.seccomp@v1"))]
        pub seccomp: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Sysctl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.sysctl@v1"))]
        pub sysctl: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.TimeOffsets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.timeoffsets@v1"))]
        pub timeoffsets: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.UIDMappings`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.uidmappings@v1"))]
        pub uidmappings: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Config.field.Umask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.umask@v1"))]
        pub umask: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.FuncHook`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.funchook@v1"))]
        pub funchook: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.IDMap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.idmap@v1"))]
        pub idmap: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.IDMap.field.ContainerID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.containerid@v1"))]
        pub containerid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.IDMap.field.HostID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hostid@v1"))]
        pub hostid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Rlimit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rlimit@v1"))]
        pub rlimit: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Rlimit.field.Hard`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hard@v1"))]
        pub hard: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Rlimit.field.Soft`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.soft@v1"))]
        pub soft: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.Allow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.allow@v1"))]
        pub allow: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.Architectures`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.architectures@v1"))]
        pub architectures: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.DefaultAction`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.defaultaction@v1"))]
        pub defaultaction: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.DefaultErrnoRet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.defaulterrnoret@v1"))]
        pub defaulterrnoret: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.EqualTo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.equalto@v1"))]
        pub equalto: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.Errno`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.errno@v1"))]
        pub errno: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.Flags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.flags@v1"))]
        pub flags: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.GreaterThanOrEqualTo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.greaterthanorequalto@v1"))]
        pub greaterthanorequalto: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.Kill`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.kill@v1"))]
        pub kill: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.KillThread`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.killthread@v1"))]
        pub killthread: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.LessThanOrEqualTo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.lessthanorequalto@v1"))]
        pub lessthanorequalto: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.ListenerMetadata`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.listenermetadata@v1"))]
        pub listenermetadata: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.ListenerPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.listenerpath@v1"))]
        pub listenerpath: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.Log`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.log@v1"))]
        pub log: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.NotEqualTo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.notequalto@v1"))]
        pub notequalto: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Seccomp.field.Syscalls`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.syscalls@v1"))]
        pub syscalls: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Syscall`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.syscall@v1"))]
        pub syscall: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Syscall.field.Action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config.struct.Syscall.field.ErrnoRet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.errnoret@v1"))]
        pub errnoret: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.config_linux.struct.LinuxPersonality.field.Domain`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.domain@v1"))]
        pub domain: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.intelrdt.struct.IntelRdt.field.ClosID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.closid@v1"))]
        pub closid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.intelrdt.struct.IntelRdt.field.EnableMonitoring`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.enablemonitoring@v1"))]
        pub enablemonitoring: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.intelrdt.struct.IntelRdt.field.L3CacheSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.l3cacheschema@v1"))]
        pub l3cacheschema: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.intelrdt.struct.IntelRdt.field.MemBwSchema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.membwschema@v1"))]
        pub membwschema: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.intelrdt.struct.IntelRdt.field.Schemata`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schemata@v1"))]
        pub schemata: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.memorypolicy.struct.LinuxMemoryPolicy.field.Mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mode@v1"))]
        pub mode: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.memorypolicy.struct.LinuxMemoryPolicy.field.Nodes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.nodes@v1"))]
        pub nodes: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.ClearedFlags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.clearedflags@v1"))]
        pub clearedflags: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.Destination`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.destination@v1"))]
        pub destination: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.Device`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.device@v1"))]
        pub device: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.Extensions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.extensions@v1"))]
        pub extensions: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.IDMapping`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.idmapping@v1"))]
        pub idmapping: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.PropagationFlags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.propagationflags@v1"))]
        pub propagationflags: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.RecAttr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.recattr@v1"))]
        pub recattr: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.Mount.field.Relabel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.relabel@v1"))]
        pub relabel: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.MountIDMapping.field.Recursive`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.recursive@v1"))]
        pub recursive: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.mount_linux.struct.MountIDMapping.field.UserNSPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.usernspath@v1"))]
        pub usernspath: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.Address`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.address@v1"))]
        pub address: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.Bridge`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.bridge@v1"))]
        pub bridge: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.Gateway`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.gateway@v1"))]
        pub gateway: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.HairpinMode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hairpinmode@v1"))]
        pub hairpinmode: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.HostInterfaceName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hostinterfacename@v1"))]
        pub hostinterfacename: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.IPv6Address`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ipv6address@v1"))]
        pub ipv6address: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.IPv6Gateway`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ipv6gateway@v1"))]
        pub ipv6gateway: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.MacAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.macaddress@v1"))]
        pub macaddress: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.Mtu`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mtu@v1"))]
        pub mtu: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Network.field.TxQueueLen`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.txqueuelen@v1"))]
        pub txqueuelen: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runc.libcontainer.configs.network.struct.Route.field.InterfaceName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.interfacename@v1"))]
        pub interfacename: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Box`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.box-field@v1"))]
        pub box_field: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Box.field.Height`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.height@v1"))]
        pub height: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Box.field.Width`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.width@v1"))]
        pub width: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.CPUAffinity.field.Final`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.final-field@v1"))]
        pub final_field: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.CPUAffinity.field.Initial`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.initial@v1"))]
        pub initial: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSD`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.freebsd@v1"))]
        pub freebsd: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSD.field.Jail`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.jail@v1"))]
        pub jail: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.freebsddevice@v1"))]
        pub freebsddevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.freebsdjail@v1"))]
        pub freebsdjail: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.EnforceStatfs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.enforcestatfs@v1"))]
        pub enforcestatfs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.FreeBSDShareDisable`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.freebsdsharedisable@v1"))]
        pub freebsdsharedisable: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.FreeBSDShareInherit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.freebsdshareinherit@v1"))]
        pub freebsdshareinherit: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.FreeBSDShareNew`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.freebsdsharenew@v1"))]
        pub freebsdsharenew: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Host`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.host@v1"))]
        pub host: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Interface`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.interface@v1"))]
        pub interface: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Ip4`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ip4@v1"))]
        pub ip4: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Ip4Addr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ip4addr@v1"))]
        pub ip4addr: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Ip6`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ip6@v1"))]
        pub ip6: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Ip6Addr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ip6addr@v1"))]
        pub ip6addr: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Parent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.parent@v1"))]
        pub parent: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.SysVMsg`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.sysvmsg@v1"))]
        pub sysvmsg: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.SysVSem`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.sysvsem@v1"))]
        pub sysvsem: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.SysVShm`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.sysvshm@v1"))]
        pub sysvshm: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.Vnet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.vnet@v1"))]
        pub vnet: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJail.field.VnetInterfaces`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.vnetinterfaces@v1"))]
        pub vnetinterfaces: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.freebsdjailallow@v1"))]
        pub freebsdjailallow: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.Chflags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.chflags@v1"))]
        pub chflags: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.Mlock`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mlock@v1"))]
        pub mlock: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.Mount`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mount@v1"))]
        pub mount: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.Quotas`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.quotas@v1"))]
        pub quotas: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.RawSockets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rawsockets@v1"))]
        pub rawsockets: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.ReservedPorts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.reservedports@v1"))]
        pub reservedports: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.SetHostname`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.sethostname@v1"))]
        pub sethostname: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.SocketAf`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.socketaf@v1"))]
        pub socketaf: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.FreeBSDJailAllow.field.Suser`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.suser@v1"))]
        pub suser: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.HWConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hwconfig@v1"))]
        pub hwconfig: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.HWConfig.field.DeviceTree`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.devicetree@v1"))]
        pub devicetree: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.HWConfig.field.DtDevs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.dtdevs@v1"))]
        pub dtdevs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.HWConfig.field.IOMems`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.iomems@v1"))]
        pub iomems: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.HWConfig.field.Irqs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.irqs@v1"))]
        pub irqs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.HWConfig.field.Memory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.memory@v1"))]
        pub memory: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.HWConfig.field.VCPUs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.vcpus@v1"))]
        pub vcpus: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.IOMems.field.FirstGFN`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.firstgfn@v1"))]
        pub firstgfn: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.IOMems.field.FirstMFN`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.firstmfn@v1"))]
        pub firstmfn: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.IOMems.field.NrMFNs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.nrmfns@v1"))]
        pub nrmfns: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Linux`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linux@v1"))]
        pub linux: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Linux.field.CgroupsPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cgroupspath@v1"))]
        pub cgroupspath: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Linux.field.MaskedPaths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.maskedpaths@v1"))]
        pub maskedpaths: Option<Vec<String>>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Linux.field.Resources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.resources@v1"))]
        pub resources: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Linux.field.RootfsPropagation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rootfspropagation@v1"))]
        pub rootfspropagation: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxblockio@v1"))]
        pub linuxblockio: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO.field.LeafWeight`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.leafweight@v1"))]
        pub leafweight: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO.field.ThrottleReadBpsDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.throttlereadbpsdevice@v1"))]
        pub throttlereadbpsdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO.field.ThrottleReadIOPSDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.throttlereadiopsdevice@v1"))]
        pub throttlereadiopsdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO.field.ThrottleWriteBpsDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.throttlewritebpsdevice@v1"))]
        pub throttlewritebpsdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO.field.ThrottleWriteIOPSDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.throttlewriteiopsdevice@v1"))]
        pub throttlewriteiopsdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO.field.Weight`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.weight@v1"))]
        pub weight: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIO.field.WeightDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.weightdevice@v1"))]
        pub weightdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIODevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxblockiodevice@v1"))]
        pub linuxblockiodevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIODevice.field.Major`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.major@v1"))]
        pub major: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxBlockIODevice.field.Minor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.minor@v1"))]
        pub minor: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxcpu@v1"))]
        pub linuxcpu: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.Burst`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.burst@v1"))]
        pub burst: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.Cpus`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cpus@v1"))]
        pub cpus: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.Idle`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.idle@v1"))]
        pub idle: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.Mems`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mems@v1"))]
        pub mems: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.Period`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.period@v1"))]
        pub period: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.Quota`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.quota@v1"))]
        pub quota: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.RealtimePeriod`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.realtimeperiod@v1"))]
        pub realtimeperiod: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.RealtimeRuntime`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.realtimeruntime@v1"))]
        pub realtimeruntime: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCPU.field.Shares`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.shares@v1"))]
        pub shares: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxCapabilities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxcapabilities@v1"))]
        pub linuxcapabilities: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxdevice@v1"))]
        pub linuxdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDevice.field.FileMode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.filemode@v1"))]
        pub filemode: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDevice.field.GID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.gid@v1"))]
        pub gid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDevice.field.UID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.uid@v1"))]
        pub uid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDeviceCgroup`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxdevicecgroup@v1"))]
        pub linuxdevicecgroup: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDeviceCgroup.field.Access`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.access@v1"))]
        pub access: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDeviceCgroup.field.PerLinux`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.perlinux@v1"))]
        pub perlinux: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxDeviceCgroup.field.PerLinux32`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.perlinux32@v1"))]
        pub perlinux32: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxHugepageLimit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxhugepagelimit@v1"))]
        pub linuxhugepagelimit: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxHugepageLimit.field.Limit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.limit@v1"))]
        pub limit: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxHugepageLimit.field.Pagesize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.pagesize@v1"))]
        pub pagesize: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIDMapping`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxidmapping@v1"))]
        pub linuxidmapping: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIOPriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxiopriority@v1"))]
        pub linuxiopriority: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIOPriority.field.Class`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.class@v1"))]
        pub class: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIOPriority.field.IOPRIO_CLASS_BE`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ioprio-class-be@v1"))]
        pub ioprio_class_be: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIOPriority.field.IOPRIO_CLASS_IDLE`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ioprio-class-idle@v1"))]
        pub ioprio_class_idle: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIOPriority.field.IOPRIO_CLASS_RT`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ioprio-class-rt@v1"))]
        pub ioprio_class_rt: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIOPriority.field.Priority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.priority@v1"))]
        pub priority: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxIntelRdt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxintelrdt@v1"))]
        pub linuxintelrdt: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxInterfacePriority`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxinterfacepriority@v1"))]
        pub linuxinterfacepriority: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxmemory@v1"))]
        pub linuxmemory: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.CheckBeforeUpdate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.checkbeforeupdate@v1"))]
        pub checkbeforeupdate: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.DisableOOMKiller`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.disableoomkiller@v1"))]
        pub disableoomkiller: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.Kernel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.kernel@v1"))]
        pub kernel: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.KernelTCP`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.kerneltcp@v1"))]
        pub kerneltcp: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.Reservation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.reservation@v1"))]
        pub reservation: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.Swap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.swap@v1"))]
        pub swap: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.Swappiness`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.swappiness@v1"))]
        pub swappiness: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemory.field.UseHierarchy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.usehierarchy@v1"))]
        pub usehierarchy: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxMemoryPolicy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxmemorypolicy@v1"))]
        pub linuxmemorypolicy: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxnamespace@v1"))]
        pub linuxnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.CgroupNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cgroupnamespace@v1"))]
        pub cgroupnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.IPCNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ipcnamespace@v1"))]
        pub ipcnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.MountNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mountnamespace@v1"))]
        pub mountnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.NetworkNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.networknamespace@v1"))]
        pub networknamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.PIDNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.pidnamespace@v1"))]
        pub pidnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.TimeNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.timenamespace@v1"))]
        pub timenamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.UTSNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.utsnamespace@v1"))]
        pub utsnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNamespace.field.UserNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.usernamespace@v1"))]
        pub usernamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNetDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxnetdevice@v1"))]
        pub linuxnetdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNetwork`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxnetwork@v1"))]
        pub linuxnetwork: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNetwork.field.ClassID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.classid@v1"))]
        pub classid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxNetwork.field.Priorities`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.priorities@v1"))]
        pub priorities: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxPersonality`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxpersonality@v1"))]
        pub linuxpersonality: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxPids`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxpids@v1"))]
        pub linuxpids: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxRdma`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxrdma@v1"))]
        pub linuxrdma: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxRdma.field.HcaHandles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hcahandles@v1"))]
        pub hcahandles: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxRdma.field.HcaObjects`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hcaobjects@v1"))]
        pub hcaobjects: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxresources@v1"))]
        pub linuxresources: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources.field.BlockIO`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.blockio@v1"))]
        pub blockio: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources.field.CPU`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cpu@v1"))]
        pub cpu: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources.field.HugepageLimits`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hugepagelimits@v1"))]
        pub hugepagelimits: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources.field.Network`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.network@v1"))]
        pub network: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources.field.Pids`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.pids@v1"))]
        pub pids: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources.field.Rdma`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rdma@v1"))]
        pub rdma: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxResources.field.Unified`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.unified@v1"))]
        pub unified: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxseccomp@v1"))]
        pub linuxseccomp: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActAllow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.actallow@v1"))]
        pub actallow: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActErrno`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.acterrno@v1"))]
        pub acterrno: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActKill`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.actkill@v1"))]
        pub actkill: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActKillProcess`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.actkillprocess@v1"))]
        pub actkillprocess: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActKillThread`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.actkillthread@v1"))]
        pub actkillthread: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActLog`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.actlog@v1"))]
        pub actlog: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActNotify`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.actnotify@v1"))]
        pub actnotify: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActTrace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.acttrace@v1"))]
        pub acttrace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ActTrap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.acttrap@v1"))]
        pub acttrap: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchAARCH64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archaarch64@v1"))]
        pub archaarch64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchARM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archarm@v1"))]
        pub archarm: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchLOONGARCH64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archloongarch64@v1"))]
        pub archloongarch64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchM68K`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archm68k@v1"))]
        pub archm68k: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchMIPS`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archmips@v1"))]
        pub archmips: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchMIPS64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archmips64@v1"))]
        pub archmips64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchMIPS64N32`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archmips64n32@v1"))]
        pub archmips64n32: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchMIPSEL`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archmipsel@v1"))]
        pub archmipsel: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchMIPSEL64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archmipsel64@v1"))]
        pub archmipsel64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchMIPSEL64N32`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archmipsel64n32@v1"))]
        pub archmipsel64n32: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchPARISC`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archparisc@v1"))]
        pub archparisc: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchPARISC64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archparisc64@v1"))]
        pub archparisc64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchPPC`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archppc@v1"))]
        pub archppc: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchPPC64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archppc64@v1"))]
        pub archppc64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchPPC64LE`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archppc64le@v1"))]
        pub archppc64le: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchRISCV64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archriscv64@v1"))]
        pub archriscv64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchS390`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archs390@v1"))]
        pub archs390: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchS390X`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archs390x@v1"))]
        pub archs390x: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchSH`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archsh@v1"))]
        pub archsh: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchSHEB`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archsheb@v1"))]
        pub archsheb: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchX32`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archx32@v1"))]
        pub archx32: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchX86`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archx86@v1"))]
        pub archx86: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.ArchX86_64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archx86-64@v1"))]
        pub archx86_64: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.LinuxSeccompFlagLog`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxseccompflaglog@v1"))]
        pub linuxseccompflaglog: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.LinuxSeccompFlagSpecAllow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxseccompflagspecallow@v1"))]
        pub linuxseccompflagspecallow: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.LinuxSeccompFlagWaitKillableRecv`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxseccompflagwaitkillablerecv@v1"))]
        pub linuxseccompflagwaitkillablerecv: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.OpEqualTo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.opequalto@v1"))]
        pub opequalto: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.OpGreaterEqual`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.opgreaterequal@v1"))]
        pub opgreaterequal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.OpGreaterThan`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.opgreaterthan@v1"))]
        pub opgreaterthan: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.OpLessEqual`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.oplessequal@v1"))]
        pub oplessequal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.OpLessThan`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.oplessthan@v1"))]
        pub oplessthan: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.OpMaskedEqual`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.opmaskedequal@v1"))]
        pub opmaskedequal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccomp.field.OpNotEqual`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.opnotequal@v1"))]
        pub opnotequal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSeccompArg`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxseccomparg@v1"))]
        pub linuxseccomparg: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSyscall`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxsyscall@v1"))]
        pub linuxsyscall: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxSyscall.field.Names`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.names@v1"))]
        pub names: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxThrottleDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxthrottledevice@v1"))]
        pub linuxthrottledevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxThrottleDevice.field.Rate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.rate@v1"))]
        pub rate: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxTimeOffset`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxtimeoffset@v1"))]
        pub linuxtimeoffset: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxTimeOffset.field.Nanosecs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.nanosecs@v1"))]
        pub nanosecs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxTimeOffset.field.Secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.secs@v1"))]
        pub secs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.LinuxWeightDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linuxweightdevice@v1"))]
        pub linuxweightdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Mount.field.Options`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.options@v1"))]
        pub options: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.POSIXRlimit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.posixrlimit@v1"))]
        pub posixrlimit: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Process`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.process@v1"))]
        pub process: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Process.field.CommandLine`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.commandline@v1"))]
        pub commandline: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Process.field.ConsoleSize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.consolesize@v1"))]
        pub consolesize: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Process.field.Cwd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cwd@v1"))]
        pub cwd: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Process.field.SelinuxLabel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.selinuxlabel@v1"))]
        pub selinuxlabel: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Process.field.Terminal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.terminal@v1"))]
        pub terminal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Root`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.root@v1"))]
        pub root: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Root.field.Readonly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.readonly@v1"))]
        pub readonly: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Scheduler.field.Deadline`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.deadline@v1"))]
        pub deadline: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Scheduler.field.Nice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.nice@v1"))]
        pub nice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Scheduler.field.Policy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.policy@v1"))]
        pub policy: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Scheduler.field.Runtime`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.runtime@v1"))]
        pub runtime: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Solaris`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.solaris@v1"))]
        pub solaris: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Solaris.field.Anet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.anet@v1"))]
        pub anet: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Solaris.field.CappedCPU`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cappedcpu@v1"))]
        pub cappedcpu: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Solaris.field.CappedMemory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cappedmemory@v1"))]
        pub cappedmemory: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Solaris.field.LimitPriv`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.limitpriv@v1"))]
        pub limitpriv: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Solaris.field.MaxShmMemory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.maxshmmemory@v1"))]
        pub maxshmmemory: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Solaris.field.Milestone`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.milestone@v1"))]
        pub milestone: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisAnet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.solarisanet@v1"))]
        pub solarisanet: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisAnet.field.Allowedaddr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.allowedaddr@v1"))]
        pub allowedaddr: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisAnet.field.Configallowedaddr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.configallowedaddr@v1"))]
        pub configallowedaddr: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisAnet.field.Defrouter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.defrouter@v1"))]
        pub defrouter: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisAnet.field.Linkname`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linkname@v1"))]
        pub linkname: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisAnet.field.Linkprotection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.linkprotection@v1"))]
        pub linkprotection: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisAnet.field.Lowerlink`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.lowerlink@v1"))]
        pub lowerlink: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisCappedCPU`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.solariscappedcpu@v1"))]
        pub solariscappedcpu: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisCappedCPU.field.Ncpus`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ncpus@v1"))]
        pub ncpus: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisCappedMemory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.solariscappedmemory@v1"))]
        pub solariscappedmemory: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.SolarisCappedMemory.field.Physical`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.physical@v1"))]
        pub physical: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Spec`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.spec@v1"))]
        pub spec: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Spec.field.VM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.vm@v1"))]
        pub vm: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Spec.field.Windows`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windows@v1"))]
        pub windows: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Spec.field.ZOS`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.zos@v1"))]
        pub zos: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.User.field.AdditionalGids`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.additionalgids@v1"))]
        pub additionalgids: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.User.field.Username`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.username@v1"))]
        pub username: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.VM.field.Hypervisor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hypervisor@v1"))]
        pub hypervisor: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.VMHypervisor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.vmhypervisor@v1"))]
        pub vmhypervisor: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.VMImage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.vmimage@v1"))]
        pub vmimage: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.VMImage.field.Format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.format@v1"))]
        pub format: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.VMKernel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.vmkernel@v1"))]
        pub vmkernel: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.VMKernel.field.InitRD`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.initrd@v1"))]
        pub initrd: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Windows.field.CredentialSpec`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.credentialspec@v1"))]
        pub credentialspec: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Windows.field.HyperV`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.hyperv@v1"))]
        pub hyperv: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Windows.field.IgnoreFlushesDuringBoot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ignoreflushesduringboot@v1"))]
        pub ignoreflushesduringboot: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Windows.field.LayerFolders`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.layerfolders@v1"))]
        pub layerfolders: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.Windows.field.Servicing`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.servicing@v1"))]
        pub servicing: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsCPUGroupAffinity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowscpugroupaffinity@v1"))]
        pub windowscpugroupaffinity: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsCPUGroupAffinity.field.Group`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.group@v1"))]
        pub group: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsCPUGroupAffinity.field.Mask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mask@v1"))]
        pub mask: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsCPUResources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowscpuresources@v1"))]
        pub windowscpuresources: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsCPUResources.field.Affinity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.affinity@v1"))]
        pub affinity: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsCPUResources.field.Count`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.count@v1"))]
        pub count: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsCPUResources.field.Maximum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.maximum@v1"))]
        pub maximum: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowsdevice@v1"))]
        pub windowsdevice: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsDevice.field.IDType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.idtype@v1"))]
        pub idtype: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsHyperV`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowshyperv@v1"))]
        pub windowshyperv: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsHyperV.field.UtilityVMPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.utilityvmpath@v1"))]
        pub utilityvmpath: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsMemoryResources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowsmemoryresources@v1"))]
        pub windowsmemoryresources: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsNetwork`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowsnetwork@v1"))]
        pub windowsnetwork: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsNetwork.field.AllowUnqualifiedDNSQuery`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.allowunqualifieddnsquery@v1"))]
        pub allowunqualifieddnsquery: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsNetwork.field.DNSSearchList`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.dnssearchlist@v1"))]
        pub dnssearchlist: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsNetwork.field.EndpointList`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.endpointlist@v1"))]
        pub endpointlist: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsNetwork.field.NetworkSharedContainerName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.networksharedcontainername@v1"))]
        pub networksharedcontainername: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsResources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowsresources@v1"))]
        pub windowsresources: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsResources.field.Storage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.storage@v1"))]
        pub storage: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsStorageResources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.windowsstorageresources@v1"))]
        pub windowsstorageresources: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsStorageResources.field.Bps`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.bps@v1"))]
        pub bps: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsStorageResources.field.Iops`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.iops@v1"))]
        pub iops: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.WindowsStorageResources.field.SandboxSize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.sandboxsize@v1"))]
        pub sandboxsize: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.zosnamespace@v1"))]
        pub zosnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolBind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolbind@v1"))]
        pub mpolbind: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolDefault`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpoldefault@v1"))]
        pub mpoldefault: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolFNumaBalancing`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolfnumabalancing@v1"))]
        pub mpolfnumabalancing: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolFRelativeNodes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolfrelativenodes@v1"))]
        pub mpolfrelativenodes: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolFStaticNodes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolfstaticnodes@v1"))]
        pub mpolfstaticnodes: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolInterleave`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolinterleave@v1"))]
        pub mpolinterleave: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolLocal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpollocal@v1"))]
        pub mpollocal: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolPreferred`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolpreferred@v1"))]
        pub mpolpreferred: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolPreferredMany`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolpreferredmany@v1"))]
        pub mpolpreferredmany: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.MpolWeightedInterleave`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mpolweightedinterleave@v1"))]
        pub mpolweightedinterleave: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedBatch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedbatch@v1"))]
        pub schedbatch: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedDeadline`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.scheddeadline@v1"))]
        pub scheddeadline: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFIFO`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedfifo@v1"))]
        pub schedfifo: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFlagDLOverrun`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedflagdloverrun@v1"))]
        pub schedflagdloverrun: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFlagKeepParams`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedflagkeepparams@v1"))]
        pub schedflagkeepparams: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFlagKeepPolicy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedflagkeeppolicy@v1"))]
        pub schedflagkeeppolicy: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFlagReclaim`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedflagreclaim@v1"))]
        pub schedflagreclaim: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFlagResetOnFork`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedflagresetonfork@v1"))]
        pub schedflagresetonfork: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFlagUtilClampMax`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedflagutilclampmax@v1"))]
        pub schedflagutilclampmax: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedFlagUtilClampMin`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedflagutilclampmin@v1"))]
        pub schedflagutilclampmin: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedISO`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schediso@v1"))]
        pub schediso: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedIdle`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedidle@v1"))]
        pub schedidle: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedOther`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedother@v1"))]
        pub schedother: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.SchedRR`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.schedrr@v1"))]
        pub schedrr: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.ZOSIPCNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.zosipcnamespace@v1"))]
        pub zosipcnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.ZOSMountNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.zosmountnamespace@v1"))]
        pub zosmountnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.ZOSPIDNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.zospidnamespace@v1"))]
        pub zospidnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.config.struct.ZOSNamespace.field.ZOSUTSNamespace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.zosutsnamespace@v1"))]
        pub zosutsnamespace: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Apparmor.field.Enabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.enabled@v1"))]
        pub enabled: Option<bool>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Cgroup.field.Systemd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.systemd@v1"))]
        pub systemd: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Cgroup.field.SystemdUser`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.systemduser@v1"))]
        pub systemduser: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Cgroup.field.V1`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.v1@v1"))]
        pub v1: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Cgroup.field.V2`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.v2@v1"))]
        pub v2: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Features.field.MountOptions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mountoptions@v1"))]
        pub mountoptions: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Features.field.OCIVersionMax`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ociversionmax@v1"))]
        pub ociversionmax: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Features.field.OCIVersionMin`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.ociversionmin@v1"))]
        pub ociversionmin: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Features.field.PotentiallyUnsafeConfigAnnotations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.potentiallyunsafeconfigannotations@v1"))]
        pub potentiallyunsafeconfigannotations: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.IntelRdt.field.Monitoring`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.monitoring@v1"))]
        pub monitoring: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Linux.field.Apparmor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.apparmor@v1"))]
        pub apparmor: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Linux.field.Cgroup`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.cgroup@v1"))]
        pub cgroup: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Linux.field.MountExtensions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.mountextensions@v1"))]
        pub mountextensions: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Linux.field.Selinux`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.selinux@v1"))]
        pub selinux: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.MemoryPolicy.field.Modes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.modes@v1"))]
        pub modes: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Seccomp.field.Actions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.actions@v1"))]
        pub actions: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Seccomp.field.Archs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.archs@v1"))]
        pub archs: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Seccomp.field.KnownFlags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.knownflags@v1"))]
        pub knownflags: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Seccomp.field.Operators`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.operators@v1"))]
        pub operators: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.features.features.struct.Seccomp.field.SupportedFlags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.supportedflags@v1"))]
        pub supportedflags: Option<u64>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.state.struct.ContainerProcessState.field.Fds`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.fds@v1"))]
        pub fds: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.state.struct.ContainerProcessState.field.Metadata`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.metadata@v1"))]
        pub metadata: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.state.struct.ContainerProcessState.field.Pid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.pid@v1"))]
        pub pid: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.state.struct.ContainerProcessState.field.State`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.state@v1"))]
        pub state: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.state.struct.State.field.Bundle`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.bundle@v1"))]
        pub bundle: Option<String>,

        /// Discovered from Repomix path `go.opencontainers__runtime-spec.specs-go.state.struct.State.field.SeccompFdName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oci.seccompfdname@v1"))]
        pub seccompfdname: Option<String>,

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
