//! Zeroclaw route surface plugin — GB.Zeroclaw.
//!
//! Publishes the Antigravity-facing model and CLI routing contract through
//! `PluginSchema` so the UI can render provider/model controls from D-Bus.

use super::common::errors::ZeroclawError;
use super::common::llm_projection::{
    ConfigSchema, LlmProjection, LlmTool, ModelRoute, Provider, Router, StructuredOutput, UiSurface,
};
use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use simd_json::OwnedValue as Value;

// =============================================================================
// PLUGIN ENTRY: identity and typed schema seed
// =============================================================================

const PLUGIN_NAME: &str = "zeroclaw";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "llm";
const PLUGIN_DESCRIPTION: &str = "Zeroclaw schema/RPC-native model router for Antigravity UI, CLI providers, and structured JSON output";
const PLUGIN_DISPLAY_NAME: &str = "GB.Zeroclaw";

/// Transport layer metadata for the Zeroclaw plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-transport.schema@v1"))]
pub struct LlmTransport {
    /// D-Bus object path served by this plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw-transport.dbus-object@v1"))]
    pub dbus_object: String,
    /// gRPC upstream target.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw-transport.grpc-target@v1"))]
    pub grpc_target: String,
    /// Incus / WireGuard container target for xray routing. Kept as a
    /// published-schema field for backward compatibility, but zeroclaw's LLM
    /// transport now runs on the host (xray via the `gbr-xray` s6 service and
    /// the gRPC-bridge via `op-grpc-bridge-zeroclaw`); there is no per-service
    /// incus container. Defaults to the `"host"` sentinel.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw-transport.incus-container@v1"))]
    pub incus_container: String,
    /// Browser-facing surface description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw-transport.browser-surface@v1"))]
    pub browser_surface: String,
    /// REST aliases exposed by the bridge.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw-transport.rest-aliases@v1"))]
    pub rest_aliases: Vec<String>,
    /// Canonical OSCAL/subid mapping authority.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.zeroclaw-transport.policy-source@v1"))]
    pub policy_source: String,
}

/// Nested per-capability model assignments.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw.model-assignments.schema@v1"))]
pub struct ModelAssignments {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.ovs-routing@v1"))]
    pub ovs_routing: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.obfuscation@v1"))]
    pub obfuscation: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.vectorization@v1"))]
    pub vectorization: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.qdrant-retrieval@v1"))]
    pub qdrant_retrieval: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments.cozo-retrieval@v1"))]
    pub cozo_retrieval: String,
}

/// Configurable options RPC contract extracted from `operation.registration.v1`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.rpc.schema@v1"))]
pub struct ConfigurableOptionRpc {
    /// RPC name from RegistrationService.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.rpc.name@v1"))]
    pub name: String,
    /// Request message type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.rpc.request@v1"))]
    pub request_type: String,
    /// Response message type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.rpc.response@v1"))]
    pub response_type: String,
    /// Read or mutation side-effect classification.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.rpc.side-effect@v1"))]
    pub side_effect: String,
    /// Required capability for this RPC.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.rpc.capability@v1"))]
    pub capability_id: String,
    /// OSCAL operation taxonomy key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.rpc.subid@v1"))]
    pub subid: String,
}

/// Configurable options message contract extracted from registration.proto.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.message.schema@v1"))]
pub struct ConfigurableOptionMessage {
    /// Message type name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.message.name@v1"))]
    pub name: String,
    /// Message fields keyed by proto field name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.message.fields@v1"))]
    pub fields: Vec<ConfigurableOptionField>,
}

/// Field contract extracted from configurable options proto messages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.field.schema@v1"))]
pub struct ConfigurableOptionField {
    /// Field name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.field.name@v1"))]
    pub name: String,
    /// Proto type, including optional/repeated marker when applicable.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.field.proto-type@v1"))]
    pub proto_type: String,
    /// JSON/schema rendering type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.field.json-type@v1"))]
    pub json_type: String,
    /// True when the field contains sensitive material.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.field.sensitive@v1"))]
    pub sensitive: bool,
    /// True when this option is required by the source contract.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.field.required@v1"))]
    pub required: bool,
}

/// Registration error code contract.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.error-code.schema@v1"))]
pub struct ConfigurableOptionErrorCode {
    /// Symbolic proto enum name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.error-code.name@v1"))]
    pub name: String,
    /// Numeric proto value.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.error-code.value@v1"))]
    pub value: i32,
}

/// Magic-link and WireGuard registration service schema.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.registration-service.schema@v1"))]
pub struct RegistrationServiceSchema {
    /// Proto package name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.registration-service.package@v1"))]
    pub package: String,
    /// Owning proto source path.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.zeroclaw-options.registration-service.proto@v1"))]
    pub source_proto: String,
    /// Service name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.registration-service.name@v1"))]
    pub service: String,
    /// RPC methods exposed by the registration service.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.registration-service.rpcs@v1"))]
    pub rpcs: Vec<ConfigurableOptionRpc>,
    /// Message schemas used by those RPCs.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.registration-service.messages@v1"))]
    pub messages: Vec<ConfigurableOptionMessage>,
    /// Error enum values.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.registration-service.errors@v1"))]
    pub error_codes: Vec<ConfigurableOptionErrorCode>,
}

/// Identity chain declared for user container options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.identity-chain.schema@v1"))]
pub struct IdentityOptions {
    /// Hardware-bound seed accepted by the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.hardware.zeroclaw-options.identity.mac@v1"))]
    pub mac_address_key: String,
    /// Optional shared key accepted by the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.policy.zeroclaw-options.identity.psk@v1"))]
    pub pre_shared_key: String,
    /// WireGuard public key identity.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.zeroclaw-options.identity.wireguard-pubkey@v1"))]
    pub wireguard_pubkey: String,
    /// MCP bearer token derivation rule.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.identity.mcp-token@v1"))]
    pub mcp_token_derivation: String,
}

/// Namespace template declared for each user container.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.memory-namespace.schema@v1"))]
pub struct MemoryNamespaceOption {
    /// Namespace template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.memory-namespace.template@v1"))]
    pub namespace_template: String,
    /// Keys owned by this namespace.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.memory-namespace.keys@v1"))]
    pub keys: Vec<String>,
    /// Human-readable purpose.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.memory-namespace.purpose@v1"))]
    pub purpose: String,
    /// Identity key used to bind this namespace to the container/user identity.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.memory-namespace.identity-link@v1"))]
    pub identity_link_key: String,
}

/// User container option contract exposed through Zeroclaw.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.process-procedure.zeroclaw-options.user-container.schema@v1"))]
pub struct UserContainerOptions {
    /// Source script that currently materializes the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.process-procedure.zeroclaw-options.user-container.script@v1"))]
    pub source_script: String,
    /// User-visible container arguments.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.process-procedure.zeroclaw-options.user-container.arguments@v1"))]
    pub arguments: Vec<ConfigurableOptionField>,
    /// Incus container name template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.user-container.container-template@v1"))]
    pub container_id_template: String,
    /// Default Incus image.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.user-container.image@v1"))]
    pub image: String,
    /// Cognitive MCP endpoint used by the user container memory flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw-options.user-container.mcp-endpoint@v1"))]
    pub cognitive_mcp_endpoint: String,
    /// Feature flags declared for the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.zeroclaw-options.user-container.features@v1"))]
    pub feature_flags: Vec<String>,
}

/// Privacy rules for configurable options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.policy.zeroclaw-options.privacy.schema@v1"))]
pub struct PrivacyOptions {
    /// Whether email may be written to CozoDB.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.zeroclaw-options.privacy.email-storage@v1"))]
    pub email_storage_rule: String,
    /// Sensitive fields that must not be rendered casually.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.zeroclaw-options.privacy.sensitive-fields@v1"))]
    pub sensitive_fields: Vec<String>,
}

/// Complete configurable options schema surface owned by Zeroclaw.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.schema@v1"))]
pub struct ConfigurableOptions {
    /// Magic-link and WireGuard registration RPC schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw-options.registration-service@v1"))]
    pub registration_service: RegistrationServiceSchema,
    /// User container configurable option schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.process-procedure.zeroclaw-options.user-container@v1"))]
    pub user_container: UserContainerOptions,
    /// Identity chain schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.identity-chain@v1"))]
    pub identity_chain: IdentityOptions,
    /// Memory namespace templates.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw-options.memory-namespaces@v1"))]
    pub memory_namespaces: Vec<MemoryNamespaceOption>,
    /// Privacy handling rules.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.zeroclaw-options.privacy@v1"))]
    pub privacy_policy: PrivacyOptions,
}

/// Top-level Zeroclaw state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.zeroclaw.schema@v1"))]
pub struct ZeroclawState {
    /// Operational status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.zeroclaw.status@v1"))]
    pub status: String,
    /// Selected provider identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.selected-provider@v1"))]
    pub selected_provider: String,
    /// Selected model identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.selected-model@v1"))]
    pub selected_model: String,
    /// Per-capability model assignments.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.model-assignments@v1"))]
    pub model_assignments: ModelAssignments,
    /// Transport layer metadata.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.transport@v1"))]
    pub transport: LlmTransport,
    /// Configurable options schema: registration, user container options,
    /// identity chain, memory namespaces, and privacy rules.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.zeroclaw.options@v1"))]
    pub configurable_options: ConfigurableOptions,
    /// Shared LLM projection fields (flattened to the top level).
    #[serde(flatten)]
    #[schemars(extend("x-oscal-subid" = "sch.software.zeroclaw.llm-projection@v1"))]
    pub projection: LlmProjection,
}

/// Empty input for read-only ZeroClaw methods.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EmptyZeroclawInput {}

/// Input for resolving a model route by hint or model identifier.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveRouteInput {
    /// Route hint or model identifier.
    pub hint: String,
}

/// Input for selecting a provider.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetProviderInput {
    /// Provider identifier declared in the provider catalog.
    pub provider_id: String,
}

/// Input for selecting a model.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetModelInput {
    /// Model identifier declared in the model route catalog.
    pub model_id: String,
}

/// Output for the complete ZeroClaw state surface.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.state.result@v1"))]
pub struct GetStateOutput {
    /// Complete projected ZeroClaw state.
    pub state: ZeroclawState,
}

/// Output for the model route catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.model-routes.result@v1"))]
pub struct GetModelRoutesOutput {
    /// Declared model routes.
    pub model_routes: Vec<ModelRoute>,
}

/// Output for the provider catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.provider-catalog.result@v1"))]
pub struct GetProviderCatalogOutput {
    /// Declared providers.
    pub providers: Vec<Provider>,
}

/// Output for the declared tool catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.tools.result@v1"))]
pub struct GetToolsOutput {
    /// Declared LLM tools.
    pub tools: Vec<LlmTool>,
}

/// Output for the provider list accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.providers.result@v1"))]
pub struct ListProvidersOutput {
    pub providers: Vec<Provider>,
}

/// Output for the router accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.router.result@v1"))]
pub struct GetRouterOutput {
    pub router: Router,
}

/// Output for the config schema accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.config-schema.result@v1"))]
pub struct GetConfigSchemaOutput {
    pub config_schema: ConfigSchema,
}

/// Output for the UI surfaces accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.ui-surfaces.result@v1"))]
pub struct ListUiSurfacesOutput {
    pub ui_surfaces: Vec<UiSurface>,
}

/// Output for the structured output accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.structured-output.result@v1"))]
pub struct GetStructuredOutputOutput {
    pub structured_output: StructuredOutput,
}

/// Output for a resolved model route.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.zeroclaw.route.result@v1"))]
pub struct ResolveRouteOutput {
    /// Resolved route.
    pub route: ModelRoute,
}

/// Output for provider selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.selected-provider.result@v1"))]
pub struct SetProviderOutput {
    /// Selected provider identifier.
    pub selected_provider: String,
}

/// Output for model selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.zeroclaw.selected-model.result@v1"))]
pub struct SetModelOutput {
    /// Selected model identifier.
    pub selected_model: String,
}

/// Output for the model assignments accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.model-assignments.result@v1"))]
pub struct GetModelAssignmentsOutput {
    pub model_assignments: ModelAssignments,
}

/// Output for the visible Zeroclaw configurable options.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.options.result@v1"))]
pub struct GetConfigurableOptionsOutput {
    pub configurable_options: ConfigurableOptions,
}

/// Output for the workspace memory namespace option templates.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.zeroclaw.options.memory-namespaces.result@v1"))]
pub struct ListUserContainerMemoryNamespaceOptionsOutput {
    pub memory_namespaces: Vec<MemoryNamespaceOption>,
}

/// Input for setting an ovs routing model.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetOvsRoutingModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetOvsRoutingModelOutput {
    pub ovs_routing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetObfuscationModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetObfuscationModelOutput {
    pub obfuscation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetVectorizationModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetVectorizationModelOutput {
    pub vectorization: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetQdrantRetrievalModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetQdrantRetrievalModelOutput {
    pub qdrant_retrieval: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetCozoRetrievalModelInput {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetCozoRetrievalModelOutput {
    pub cozo_retrieval: String,
}

// =============================================================================
// PLUGIN BODY: D-Bus-backed behavior only
// =============================================================================

pub struct ZeroclawPlugin;

impl Default for ZeroclawPlugin {
    fn default() -> Self {
        Self
    }
}

impl ZeroclawPlugin {
    const DBUS_OBJECT: &'static str = "/org/opdbus/v1/plugins/zeroclaw";
    const OSCAL_SUBID_REGISTRY_OBJECT: &'static str = "/org/opdbus/v1/plugins/oscal_subid_registry";
    const DEFAULT_LOCAL_MODEL: &'static str = "gemma3:4b";

    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    pub fn current_state() -> ZeroclawState {
        let selected_provider = Self::env_or("LLM_PROVIDER", "ollama");
        let selected_model = Self::env_or("LLM_MODEL", Self::DEFAULT_LOCAL_MODEL);
        let local_model = selected_model.clone();
        let router_endpoint = Self::env_or("ZEROCLAW_ROUTER_ENDPOINT", "http://localhost:11434");
        let grpc_target = Self::env_or("ZEROCLAW_GRPC_TARGET", "http://10.200.0.2:50051");
        let grpc_target_for_provider = grpc_target.clone();

        ZeroclawState {
            status: "declared".to_string(),
            selected_provider,
            selected_model,
            model_assignments: ModelAssignments {
                ovs_routing: local_model.clone(),
                obfuscation: local_model.clone(),
                vectorization: "gemini-embedding-001".to_string(),
                qdrant_retrieval: "gemini-embedding-001".to_string(),
                cozo_retrieval: local_model.clone(),
            },
            transport: LlmTransport {
                dbus_object: Self::DBUS_OBJECT.to_string(),
                grpc_target: grpc_target_for_provider,
                incus_container: "host".to_string(),
                browser_surface: "gRPC-Web through op-web".to_string(),
                rest_aliases: vec![
                    "/api/zeroclaw/chat".to_string(),
                    "/api/llm/chat".to_string(),
                ],
                policy_source: Self::OSCAL_SUBID_REGISTRY_OBJECT.to_string(),
            },
            configurable_options: Self::configurable_options(),
            projection: LlmProjection {
                providers: vec![
                    Provider {
                        id: "factory".to_string(),
                        route: "factory".to_string(),
                        kind: "orchestrator".to_string(),
                        aliases: vec!["default".to_string(), "auto".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "codex".to_string(),
                        route: "openai-codex".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec!["openai_codex".to_string(), "codex".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "ollama".to_string(),
                        route: "ollama".to_string(),
                        kind: "local".to_string(),
                        aliases: vec![
                            "gemma".to_string(),
                            "gemma3".to_string(),
                            "gemma3:4b".to_string(),
                            "gemma4".to_string(),
                        ],
                        ..Default::default()
                    },
                    Provider {
                        id: "antigravity".to_string(),
                        route: "antigravity".to_string(),
                        kind: "orchestrator".to_string(),
                        aliases: vec!["google.antigravity".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "google".to_string(),
                        route: "gemini".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec!["gemini".to_string(), "google-gemini".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "opencode".to_string(),
                        route: "custom:opencode".to_string(),
                        kind: "cli".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "kilocode".to_string(),
                        route: "custom:kilocode".to_string(),
                        kind: "cli".to_string(),
                        aliases: vec!["kilo-code".to_string(), "kilo_code".to_string()],
                        ..Default::default()
                    },
                    Provider {
                        id: "openrouter".to_string(),
                        route: "openrouter".to_string(),
                        kind: "router".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "salad".to_string(),
                        route: "salad".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec!["salad-ai".to_string(), "salad_ai".to_string()],
                        endpoint: "https://ai.salad.cloud/v1".to_string(),
                        auth: "bearer".to_string(),
                        sdk: "openai-compatible".to_string(),
                        description: "Salad AI Gateway — managed OpenAI-compatible LLM API on SaladCloud GPUs.".to_string(),
                        ..Default::default()
                    },
                    Provider {
                        id: "anthropic".to_string(),
                        route: "anthropic".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "openai".to_string(),
                        route: "openai".to_string(),
                        kind: "provider".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "gemini-cli".to_string(),
                        route: "gemini-cli".to_string(),
                        kind: "cli".to_string(),
                        aliases: vec![],
                        ..Default::default()
                    },
                    Provider {
                        id: "oscal".to_string(),
                        route: "oscal".to_string(),
                        kind: "policy".to_string(),
                        aliases: vec![
                            "compliance".to_string(),
                            "subid".to_string(),
                            "nist".to_string(),
                        ],
                        source: Self::OSCAL_SUBID_REGISTRY_OBJECT.to_string(),
                        ..Default::default()
                    },
                ],
                router: Router {
                    provider: "ollama".to_string(),
                    model: local_model.clone(),
                    endpoint: router_endpoint,
                    scope: "all".to_string(),
                    role: "context_aware_allocator".to_string(),
                    emits: vec![
                        "provider".to_string(),
                        "model".to_string(),
                        "hint".to_string(),
                        "candidate_subids".to_string(),
                        "confidence".to_string(),
                        "thinking_budget".to_string(),
                        "reasoning_effort".to_string(),
                    ],
                    ..Default::default()
                },
                model_routes: vec![
                    ModelRoute {
                        hint: "code".to_string(),
                        provider: "openrouter".to_string(),
                        upstream_provider: "openrouter".to_string(),
                        transport: "direct".to_string(),
                        model: "anthropic/claude-sonnet-4.6".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Declared route; backend availability must be projected before execution.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "gemini".to_string(),
                        upstream_provider: "gemini".to_string(),
                        transport: "direct".to_string(),
                        model: "gemini-2.5-flash".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Declared route; backend availability must be projected before execution.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "local".to_string(),
                        provider: "ollama".to_string(),
                        upstream_provider: "ollama".to_string(),
                        transport: "loopback".to_string(),
                        model: local_model.clone(),
                        kind: "router".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: format!(
                            "{local_model} is the declared local classifier; unavailable until Ollama projects it."
                        ),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "auto".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "factory".to_string(),
                        transport: "auto".to_string(),
                        model: "auto".to_string(),
                        kind: "orchestrator".to_string(),
                        status: "declared".to_string(),
                        available: true,
                        status_reason: "Factory auto-router available - selects best provider based on query classification.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "code".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "openrouter".to_string(),
                        transport: "direct".to_string(),
                        model: "anthropic/claude-sonnet-4.6".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Factory code route -> openrouter/claude; requires backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "gemini".to_string(),
                        transport: "direct".to_string(),
                        model: "gemini-2.5-flash".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Factory fast route -> gemini/flash; requires backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "local".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "ollama".to_string(),
                        transport: "loopback".to_string(),
                        model: local_model.clone(),
                        kind: "router".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: format!(
                            "Factory local route -> ollama/{local_model}; requires backend projection."
                        ),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "reasoning".to_string(),
                        provider: "factory".to_string(),
                        upstream_provider: "antigravity".to_string(),
                        transport: "direct".to_string(),
                        model: "gemini-3-pro-preview".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Factory reasoning route; Gemma dynamically allocates thinking budget.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "compliance".to_string(),
                        provider: "oscal".to_string(),
                        upstream_provider: "oscal".to_string(),
                        transport: "dbus".to_string(),
                        model: "NIST_SP_800-53_Rev5".to_string(),
                        kind: "policy".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "OSCAL policy provider is declared; unavailable until oscal_subid_registry is projected.".to_string(),
                        source: Self::OSCAL_SUBID_REGISTRY_OBJECT.to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "reasoning".to_string(),
                        provider: "salad".to_string(),
                        upstream_provider: "salad".to_string(),
                        transport: "direct".to_string(),
                        model: "qwen3.6-35b-a3b".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Salad qwen3.6-35b-a3b; requires SALAD_API_KEY and backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "balanced".to_string(),
                        provider: "salad".to_string(),
                        upstream_provider: "salad".to_string(),
                        transport: "direct".to_string(),
                        model: "qwen3.6-27b".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Salad qwen3.6-27b; requires SALAD_API_KEY and backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                    ModelRoute {
                        hint: "fast".to_string(),
                        provider: "salad".to_string(),
                        upstream_provider: "salad".to_string(),
                        transport: "direct".to_string(),
                        model: "qwen3.5-9b".to_string(),
                        kind: "chat".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: "Salad qwen3.5-9b; requires SALAD_API_KEY and backend projection.".to_string(),
                        api_key: Some(JsonValue::Null),
                        ..Default::default()
                    },
                ],
                tools: vec![
                    LlmTool {
                        name: "zeroclaw.chat".to_string(),
                        description: "Send an Antigravity/Zeroclaw chat turn and return structured JSON when response_schema is present.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "message": {"type": "string"},
                                "provider": {"type": "string"},
                                "model": {"type": "string"},
                                "response_schema": {"type": "object"}
                            },
                            "required": ["message"]
                        }),
                        ..Default::default()
                    },
                    LlmTool {
                        name: "zeroclaw.models.list".to_string(),
                        description: "List cached or live models for a Zeroclaw provider.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {"provider": {"type": "string"}},
                            "required": []
                        }),
                        ..Default::default()
                    },
                ],
                config_schema: ConfigSchema {
                    source: "zeroclaw config schema".to_string(),
                    schema_crate: "schemars".to_string(),
                    native_type: "zeroclaw::config::schema::Config".to_string(),
                    status: "available_via_cli_or_gateway".to_string(),
                    ..Default::default()
                },
                ui_surfaces: vec![
                    UiSurface {
                        path: "/chat".to_string(),
                        name: "Antigravity Chat".to_string(),
                        schema: "zeroclaw".to_string(),
                    },
                    UiSurface {
                        path: "/grpc".to_string(),
                        name: "gRPC Diagnostics".to_string(),
                        schema: "plugin-service".to_string(),
                    },
                    UiSurface {
                        path: "/models".to_string(),
                        name: "Routable Models".to_string(),
                        schema: "zeroclaw.providers".to_string(),
                    },
                ],
                structured_output: StructuredOutput {
                    sdk: "google.antigravity".to_string(),
                    config_object: "LocalAgentConfig(response_schema=UiResponseSchema)".to_string(),
                    extractor: "response.structured_output()".to_string(),
                    response_schema: serde_json::json!({
                        "action": "",
                        "metadata": {},
                        "confidence_score": 0.0
                    }),
                    pydantic_source: vec![
                        "import pydantic".to_string(),
                        "class UiResponseSchema(pydantic.BaseModel):".to_string(),
                        "    action: str".to_string(),
                        "    metadata: dict[str, str]".to_string(),
                        "    confidence_score: float".to_string(),
                    ],
                    ui_renderer: "JsonRenderer".to_string(),
                    required: true,
                    ..Default::default()
                },
            },
        }
    }

    fn option_field(
        name: &str,
        proto_type: &str,
        json_type: &str,
        sensitive: bool,
        required: bool,
    ) -> ConfigurableOptionField {
        ConfigurableOptionField {
            name: name.to_string(),
            proto_type: proto_type.to_string(),
            json_type: json_type.to_string(),
            sensitive,
            required,
        }
    }

    fn option_message(
        name: &str,
        fields: Vec<ConfigurableOptionField>,
    ) -> ConfigurableOptionMessage {
        ConfigurableOptionMessage {
            name: name.to_string(),
            fields,
        }
    }

    fn option_rpc(
        name: &str,
        request_type: &str,
        response_type: &str,
        side_effect: &str,
        capability_id: &str,
        subid: &str,
    ) -> ConfigurableOptionRpc {
        ConfigurableOptionRpc {
            name: name.to_string(),
            request_type: request_type.to_string(),
            response_type: response_type.to_string(),
            side_effect: side_effect.to_string(),
            capability_id: capability_id.to_string(),
            subid: subid.to_string(),
        }
    }

    fn memory_namespace(
        namespace_template: &str,
        keys: &[&str],
        purpose: &str,
    ) -> MemoryNamespaceOption {
        MemoryNamespaceOption {
            namespace_template: namespace_template.to_string(),
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
            purpose: purpose.to_string(),
            identity_link_key: "container:{container_id}:identity.wireguard_pubkey".to_string(),
        }
    }

    fn configurable_options() -> ConfigurableOptions {
        ConfigurableOptions {
            registration_service: RegistrationServiceSchema {
                package: "operation.registration.v1".to_string(),
                source_proto: "crates/op-grpc-bridge/proto/registration.proto".to_string(),
                service: "RegistrationService".to_string(),
                rpcs: vec![
                    Self::option_rpc(
                        "SendMagicLink",
                        "SendMagicLinkRequest",
                        "SendMagicLinkResponse",
                        "Mutation",
                        "cap.software.zeroclaw.registration.magic-link.send@v1",
                        "mut.service.zeroclaw.registration.magic-link.send@v1",
                    ),
                    Self::option_rpc(
                        "VerifyMagicLink",
                        "VerifyMagicLinkRequest",
                        "VerifyMagicLinkResponse",
                        "Mutation",
                        "cap.software.zeroclaw.registration.magic-link.verify@v1",
                        "mut.service.zeroclaw.registration.magic-link.verify@v1",
                    ),
                    Self::option_rpc(
                        "RegisterUser",
                        "RegisterUserRequest",
                        "RegisterUserResponse",
                        "Mutation",
                        "cap.software.zeroclaw.registration.user.register@v1",
                        "mut.service.zeroclaw.registration.user.register@v1",
                    ),
                    Self::option_rpc(
                        "GetUserStatus",
                        "GetUserStatusRequest",
                        "GetUserStatusResponse",
                        "Read",
                        "cap.software.zeroclaw.registration.user-status.read@v1",
                        "obs.service.zeroclaw.registration.user-status.get@v1",
                    ),
                    Self::option_rpc(
                        "ListUsers",
                        "ListUsersRequest",
                        "ListUsersResponse",
                        "Read",
                        "cap.software.zeroclaw.registration.users.read@v1",
                        "obs.service.zeroclaw.registration.users.list@v1",
                    ),
                    Self::option_rpc(
                        "GetWireGuardConfig",
                        "GetWireGuardConfigRequest",
                        "GetWireGuardConfigResponse",
                        "Read",
                        "cap.software.zeroclaw.registration.wireguard-config.read@v1",
                        "obs.service.zeroclaw.registration.wireguard-config.get@v1",
                    ),
                    Self::option_rpc(
                        "AdminUserAction",
                        "AdminUserActionRequest",
                        "AdminUserActionResponse",
                        "Mutation",
                        "cap.software.zeroclaw.registration.admin-user-action.apply@v1",
                        "mut.service.zeroclaw.registration.admin-user-action.apply@v1",
                    ),
                ],
                messages: vec![
                    Self::option_message(
                        "SendMagicLinkRequest",
                        vec![
                            Self::option_field("email", "string", "string", false, true),
                            Self::option_field("domain", "string", "string", false, true),
                            Self::option_field("is_admin", "bool", "boolean", false, true),
                            Self::option_field("custom_message", "optional string", "string", false, false),
                        ],
                    ),
                    Self::option_message(
                        "SendMagicLinkResponse",
                        vec![
                            Self::option_field("success", "bool", "boolean", false, false),
                            Self::option_field("message", "string", "string", false, false),
                            Self::option_field("token", "optional string", "string", true, false),
                            Self::option_field(
                                "expires_at",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "VerifyMagicLinkRequest",
                        vec![
                            Self::option_field("token", "string", "string", true, true),
                            Self::option_field("domain", "string", "string", false, true),
                        ],
                    ),
                    Self::option_message(
                        "VerifyMagicLinkResponse",
                        vec![
                            Self::option_field("success", "bool", "boolean", false, false),
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field("email", "string", "string", false, false),
                            Self::option_field("wireguard_public_key", "string", "string", true, false),
                            Self::option_field("assigned_ip", "string", "string", false, false),
                            Self::option_field("wireguard_config", "string", "string", true, false),
                            Self::option_field("message", "string", "string", false, false),
                            Self::option_field("is_admin", "bool", "boolean", false, false),
                            Self::option_field(
                                "verified_at",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "RegisterUserRequest",
                        vec![
                            Self::option_field("email", "string", "string", false, true),
                            Self::option_field("wireguard_public_key", "string", "string", true, true),
                            Self::option_field("domain", "string", "string", false, true),
                            Self::option_field("is_admin", "bool", "boolean", false, true),
                            Self::option_field(
                                "metadata",
                                "optional google.protobuf.Struct",
                                "object",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "RegisterUserResponse",
                        vec![
                            Self::option_field("success", "bool", "boolean", false, false),
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field("message", "string", "string", false, false),
                            Self::option_field("assigned_ip", "string", "string", false, false),
                            Self::option_field("wireguard_config", "string", "string", true, false),
                            Self::option_field(
                                "registered_at",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "GetUserStatusRequest",
                        vec![
                            Self::option_field("email", "string", "string", false, false),
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field("domain", "string", "string", false, false),
                        ],
                    ),
                    Self::option_message(
                        "GetUserStatusResponse",
                        vec![
                            Self::option_field("registered", "bool", "boolean", false, false),
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field("email", "string", "string", false, false),
                            Self::option_field("email_verified", "bool", "boolean", false, false),
                            Self::option_field("wireguard_public_key", "string", "string", true, false),
                            Self::option_field("assigned_ip", "string", "string", false, false),
                            Self::option_field("is_admin", "bool", "boolean", false, false),
                            Self::option_field(
                                "registered_at",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                            Self::option_field(
                                "last_active",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "ListUsersRequest",
                        vec![
                            Self::option_field("limit", "uint32", "integer:uint32", false, false),
                            Self::option_field("offset", "uint32", "integer:uint32", false, false),
                            Self::option_field("include_admins_only", "bool", "boolean", false, false),
                            Self::option_field("domain_filter", "string", "string", false, false),
                        ],
                    ),
                    Self::option_message(
                        "ListUsersResponse",
                        vec![
                            Self::option_field("users", "repeated UserInfo", "array", false, false),
                            Self::option_field("total_count", "uint32", "integer:uint32", false, false),
                            Self::option_field("filtered_count", "uint32", "integer:uint32", false, false),
                        ],
                    ),
                    Self::option_message(
                        "UserInfo",
                        vec![
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field("email", "string", "string", false, false),
                            Self::option_field("email_verified", "bool", "boolean", false, false),
                            Self::option_field("wireguard_public_key", "string", "string", true, false),
                            Self::option_field("assigned_ip", "string", "string", false, false),
                            Self::option_field("is_admin", "bool", "boolean", false, false),
                            Self::option_field(
                                "registered_at",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                            Self::option_field(
                                "last_active",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                            Self::option_field(
                                "metadata",
                                "optional google.protobuf.Struct",
                                "object",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "GetWireGuardConfigRequest",
                        vec![
                            Self::option_field("email", "string", "string", false, false),
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field("domain", "string", "string", false, false),
                        ],
                    ),
                    Self::option_message(
                        "GetWireGuardConfigResponse",
                        vec![
                            Self::option_field("success", "bool", "boolean", false, false),
                            Self::option_field("wireguard_config", "string", "string", true, false),
                            Self::option_field("public_key", "string", "string", true, false),
                            Self::option_field("assigned_ip", "string", "string", false, false),
                            Self::option_field("message", "string", "string", false, false),
                            Self::option_field(
                                "generated_at",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "AdminUserActionRequest",
                        vec![
                            Self::option_field("action", "string", "string", false, true),
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field("email", "string", "string", false, false),
                            Self::option_field(
                                "parameters",
                                "optional google.protobuf.Struct",
                                "object",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "AdminUserActionResponse",
                        vec![
                            Self::option_field("success", "bool", "boolean", false, false),
                            Self::option_field("message", "string", "string", false, false),
                            Self::option_field("user_id", "string", "string", false, false),
                            Self::option_field(
                                "action_timestamp",
                                "google.protobuf.Timestamp",
                                "string:date-time",
                                false,
                                false,
                            ),
                        ],
                    ),
                    Self::option_message(
                        "RegistrationError",
                        vec![
                            Self::option_field("code", "int32", "integer:int32", false, false),
                            Self::option_field("message", "string", "string", false, false),
                            Self::option_field(
                                "details",
                                "optional google.protobuf.Struct",
                                "object",
                                false,
                                false,
                            ),
                        ],
                    ),
                ],
                error_codes: vec![
                    ConfigurableOptionErrorCode {
                        name: "REGISTRATION_ERROR_UNSPECIFIED".to_string(),
                        value: 0,
                    },
                    ConfigurableOptionErrorCode {
                        name: "REGISTRATION_ERROR_INVALID_EMAIL".to_string(),
                        value: 1,
                    },
                    ConfigurableOptionErrorCode {
                        name: "REGISTRATION_ERROR_INVALID_TOKEN".to_string(),
                        value: 2,
                    },
                    ConfigurableOptionErrorCode {
                        name: "REGISTRATION_ERROR_USER_EXISTS".to_string(),
                        value: 3,
                    },
                    ConfigurableOptionErrorCode {
                        name: "REGISTRATION_ERROR_WIREGUARD_KEY_INVALID".to_string(),
                        value: 4,
                    },
                    ConfigurableOptionErrorCode {
                        name: "REGISTRATION_ERROR_NETWORK_UNAVAILABLE".to_string(),
                        value: 5,
                    },
                    ConfigurableOptionErrorCode {
                        name: "REGISTRATION_ERROR_ADMIN_REQUIRED".to_string(),
                        value: 6,
                    },
                ],
            },
            user_container: UserContainerOptions {
                source_script: "deploy/scripts/provision-workspace-subscriber.sh".to_string(),
                arguments: vec![
                    Self::option_field("username", "positional string", "string", false, true),
                    Self::option_field("--mac", "option string", "string", true, false),
                    Self::option_field("--psk", "option string", "string", true, false),
                    Self::option_field("--email", "option string", "string", false, false),
                    Self::option_field("--ghostbridge", "flag bool", "boolean", false, false),
                    Self::option_field("--semantic", "flag bool", "boolean", false, false),
                ],
                container_id_template: "ws-{username}".to_string(),
                image: "images:debian/12".to_string(),
                cognitive_mcp_endpoint: "http://100.90.37.254:3003/mcp".to_string(),
                feature_flags: vec!["ghostbridge".to_string(), "semantic_search".to_string()],
            },
            identity_chain: IdentityOptions {
                mac_address_key: "--mac".to_string(),
                pre_shared_key: "--psk".to_string(),
                wireguard_pubkey: "container:{container_id}:identity/wireguard_pubkey".to_string(),
                mcp_token_derivation: "uuid_v5(wireguard_pubkey)".to_string(),
            },
            memory_namespaces: vec![
                Self::memory_namespace(
                    "container:{container_id}:identity",
                    &["wireguard_pubkey", "mcp_token", "mac_address", "psk", "email"],
                    "Container identity namespace. Email is present only when GhostBridge is disabled.",
                ),
                Self::memory_namespace(
                    "container:{container_id}:soul",
                    &["profile"],
                    "Container soul profile bound to the same container identity.",
                ),
                Self::memory_namespace(
                    "container:{container_id}:domain:work",
                    &["MEMORY_INDEX"],
                    "Work-domain namespace bound to the container identity.",
                ),
                Self::memory_namespace(
                    "container:{container_id}:domain:personal",
                    &["MEMORY_INDEX"],
                    "Personal-domain namespace bound to the container identity.",
                ),
                Self::memory_namespace(
                    "container:{container_id}:domain:home",
                    &["MEMORY_INDEX"],
                    "Home-domain namespace bound to the container identity.",
                ),
                Self::memory_namespace(
                    "container:{container_id}:index",
                    &["MEMORY_INDEX"],
                    "Namespace index for identity, soul, and domain namespaces.",
                ),
                Self::memory_namespace(
                    "container:{container_id}:features",
                    &["ghostbridge", "semantic_search"],
                    "Feature flags projected for the container identity.",
                ),
            ],
            privacy_policy: PrivacyOptions {
                email_storage_rule:
                    "Store email only when GhostBridge is disabled; GhostBridge users withhold email from CozoDB."
                        .to_string(),
                sensitive_fields: vec![
                    "token".to_string(),
                    "wireguard_public_key".to_string(),
                    "wireguard_config".to_string(),
                    "public_key".to_string(),
                    "mcp_token".to_string(),
                    "mac_address".to_string(),
                    "psk".to_string(),
                ],
            },
        }
    }
}

#[async_trait]
impl StatePlugin for ZeroclawPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = zeroclaw_schema();
        super::common::llm_projection::rewrite_projection_subids_for_plugin(
            &mut schema,
            PLUGIN_NAME,
        );
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: PLUGIN_NAME.to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-declared".to_string(),
                desired_hash: "schema-declared".to_string(),
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
            atomic_operations: false,
        }
    }
}

// =============================================================================
// PLUGIN EXIT: publish the single PluginSchema contract
// =============================================================================

/// Canonical `zeroclaw` schema derived from [`ZeroclawState`] via schemars.
pub(crate) fn zeroclaw_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(ZeroclawState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());

    if let Ok(state) = simd_json::serde::to_owned_value(ZeroclawPlugin::current_state()) {
        super::schemars_adapter::apply_state_defaults(&mut schema, &state);
        schema.example = Some(state);
    }

    schema.methods.insert(
        "GetState".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetStateOutput>(
            "GetState",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.state.read@v1",
            "obs.service.zeroclaw.state.get@v1",
        ),
    );
    schema.methods.insert(
        "GetModelAssignments".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetModelAssignmentsOutput>(
            "GetModelAssignments",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.model-assignments.read@v1",
            "obs.service.zeroclaw.model-assignments.get@v1",
        ),
    );
    schema.methods.insert(
        "GetConfigurableOptions".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetConfigurableOptionsOutput>(
            "GetConfigurableOptions",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.options.read@v1",
            "obs.service.zeroclaw.options.get@v1",
        ),
    );
    schema.methods.insert(
        "ListUserContainerMemoryNamespaceOptions".to_string(),
        method_decl_from_schemars_with_output::<
            EmptyZeroclawInput,
            ListUserContainerMemoryNamespaceOptionsOutput,
        >(
            "ListUserContainerMemoryNamespaceOptions",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.options.memory-namespaces.read@v1",
            "obs.service.zeroclaw.options.memory-namespaces.list@v1",
        ),
    );
    schema.methods.insert(
        "GetModelRoutes".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetModelRoutesOutput>(
            "GetModelRoutes",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.routes.read@v1",
            "obs.service.zeroclaw.model-routes.list@v1",
        ),
    );
    schema.methods.insert(
        "ListProviders".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, ListProvidersOutput>(
            "ListProviders",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.providers.read@v1",
            "obs.service.zeroclaw.providers.list@v1",
        ),
    );
    schema.methods.insert(
        "GetProviderCatalog".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetProviderCatalogOutput>(
            "GetProviderCatalog",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.providers.read@v1",
            "obs.service.zeroclaw.provider-catalog.list@v1",
        ),
    );
    schema.methods.insert(
        "GetTools".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetToolsOutput>(
            "GetTools",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.tools.read@v1",
            "obs.service.zeroclaw.tools.list@v1",
        ),
    );
    schema.methods.insert(
        "GetRouter".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetRouterOutput>(
            "GetRouter",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.router.read@v1",
            "obs.service.zeroclaw.router.get@v1",
        ),
    );
    schema.methods.insert(
        "GetConfigSchema".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetConfigSchemaOutput>(
            "GetConfigSchema",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.config-schema.read@v1",
            "obs.service.zeroclaw.config-schema.get@v1",
        ),
    );
    schema.methods.insert(
        "ListUiSurfaces".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, ListUiSurfacesOutput>(
            "ListUiSurfaces",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.ui-surfaces.read@v1",
            "obs.service.zeroclaw.ui-surfaces.list@v1",
        ),
    );
    schema.methods.insert(
        "GetStructuredOutput".to_string(),
        method_decl_from_schemars_with_output::<EmptyZeroclawInput, GetStructuredOutputOutput>(
            "GetStructuredOutput",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.structured-output.read@v1",
            "obs.service.zeroclaw.structured-output.get@v1",
        ),
    );
    schema.methods.insert(
        "ResolveRoute".to_string(),
        method_decl_from_schemars_with_output::<ResolveRouteInput, ResolveRouteOutput>(
            "ResolveRoute",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.zeroclaw.route.resolve@v1",
            "obs.service.zeroclaw.route.resolve@v1",
        ),
    );
    schema.methods.insert(
        "SetProvider".to_string(),
        method_decl_from_schemars_with_output::<SetProviderInput, SetProviderOutput>(
            "SetProvider",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.provider.set@v1",
            "mut.service.zeroclaw.provider.set@v1",
        ),
    );
    schema.methods.insert(
        "SetModel".to_string(),
        method_decl_from_schemars_with_output::<SetModelInput, SetModelOutput>(
            "SetModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model.set@v1",
            "mut.service.zeroclaw.model.set@v1",
        ),
    );
    schema.methods.insert(
        "SetOvsRoutingModel".to_string(),
        method_decl_from_schemars_with_output::<SetOvsRoutingModelInput, SetOvsRoutingModelOutput>(
            "SetOvsRoutingModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.ovs-routing.set@v1",
            "mut.service.zeroclaw.model-assignments.ovs-routing.set@v1",
        ),
    );
    schema.methods.insert(
        "SetObfuscationModel".to_string(),
        method_decl_from_schemars_with_output::<SetObfuscationModelInput, SetObfuscationModelOutput>(
            "SetObfuscationModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.obfuscation.set@v1",
            "mut.service.zeroclaw.model-assignments.obfuscation.set@v1",
        ),
    );
    schema.methods.insert(
        "SetVectorizationModel".to_string(),
        method_decl_from_schemars_with_output::<
            SetVectorizationModelInput,
            SetVectorizationModelOutput,
        >(
            "SetVectorizationModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.vectorization.set@v1",
            "mut.service.zeroclaw.model-assignments.vectorization.set@v1",
        ),
    );
    schema.methods.insert(
        "SetQdrantRetrievalModel".to_string(),
        method_decl_from_schemars_with_output::<
            SetQdrantRetrievalModelInput,
            SetQdrantRetrievalModelOutput,
        >(
            "SetQdrantRetrievalModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.qdrant-retrieval.set@v1",
            "mut.service.zeroclaw.model-assignments.qdrant-retrieval.set@v1",
        ),
    );
    schema.methods.insert(
        "SetCozoRetrievalModel".to_string(),
        method_decl_from_schemars_with_output::<
            SetCozoRetrievalModelInput,
            SetCozoRetrievalModelOutput,
        >(
            "SetCozoRetrievalModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.zeroclaw.model-assignments.cozo-retrieval.set@v1",
            "mut.service.zeroclaw.model-assignments.cozo-retrieval.set@v1",
        ),
    );

    schema
}

/// Public accessor for crates that embed the Zeroclaw plugin contract.
pub fn zeroclaw_plugin_schema() -> PluginSchema {
    zeroclaw_schema()
}

/// A signal the bridge can emit after a successful plugin-owned dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchSignal {
    pub name: String,
    pub payload: JsonValue,
}

/// Outcome of a plugin-owned method dispatch.
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    pub result: JsonValue,
    pub signal: Option<DispatchSignal>,
}

impl DispatchOutcome {
    fn plain(result: JsonValue) -> Self {
        Self {
            result,
            signal: None,
        }
    }
}

/// Plugin-owned method dispatch for the Zeroclaw D-Bus/gRPC surface.
pub fn dispatch_zeroclaw_method(
    method: &str,
    json_args: &str,
    state: &ZeroclawState,
) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    match method {
        "GetState" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "state": to_json(state) }),
        )),
        "GetModelRoutes" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "model_routes": to_json(&state.projection.model_routes) }),
        )),
        "GetProviderCatalog" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "providers": to_json(&state.projection.providers) }),
        )),
        "GetTools" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "tools": to_json(&state.projection.tools) }),
        )),
        "ListProviders" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "providers": to_json(&state.projection.providers) }),
        )),
        "GetModelAssignments" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "model_assignments": to_json(&state.model_assignments) }),
        )),
        "GetConfigurableOptions" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "configurable_options": to_json(&state.configurable_options) }),
        )),
        "ListUserContainerMemoryNamespaceOptions" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "memory_namespaces": to_json(&state.configurable_options.memory_namespaces) }),
        )),
        "GetRouter" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "router": to_json(&state.projection.router) }),
        )),
        "GetConfigSchema" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "config_schema": to_json(&state.projection.config_schema) }),
        )),
        "ListUiSurfaces" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "ui_surfaces": to_json(&state.projection.ui_surfaces) }),
        )),
        "GetStructuredOutput" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "structured_output": to_json(&state.projection.structured_output) }),
        )),
        "ResolveRoute" => resolve_route(json_args, state).map(DispatchOutcome::plain),
        "SetProvider" => set_provider_handler(json_args, state),
        "SetModel" => set_model_handler(json_args, state),
        "SetOvsRoutingModel" => set_role_model_handler(json_args, state, "ovs_routing"),
        "SetObfuscationModel" => set_role_model_handler(json_args, state, "obfuscation"),
        "SetVectorizationModel" => set_role_model_handler(json_args, state, "vectorization"),
        "SetQdrantRetrievalModel" => set_role_model_handler(json_args, state, "qdrant_retrieval"),
        "SetCozoRetrievalModel" => set_role_model_handler(json_args, state, "cozo_retrieval"),
        other => Err(ZeroclawError::ExecutionDenied {
            reason: format!("undeclared method: {other}"),
        }),
    }
}

fn to_json<T: Serialize>(value: &T) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

fn parse_args(method: &str, json_args: &str) -> std::result::Result<JsonValue, ZeroclawError> {
    if json_args.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(json_args).map_err(|error| ZeroclawError::ExecutionDenied {
        reason: format!("{method} arguments are not valid JSON: {error}"),
    })
}

fn require_str(
    args: &JsonValue,
    field: &str,
    method: &str,
) -> std::result::Result<String, ZeroclawError> {
    args.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| ZeroclawError::ExecutionDenied {
            reason: format!("{method} requires string field '{field}'"),
        })
}

fn resolve_route(
    json_args: &str,
    state: &ZeroclawState,
) -> std::result::Result<JsonValue, ZeroclawError> {
    let args = parse_args("ResolveRoute", json_args)?;
    let hint = require_str(&args, "hint", "ResolveRoute")?;
    state
        .projection
        .model_routes
        .iter()
        .find(|route| route.hint == hint || route.model == hint)
        .map(|route| serde_json::json!({ "route": to_json(route) }))
        .ok_or(ZeroclawError::RouteNotDeclared { hint })
}

fn set_provider_handler(
    json_args: &str,
    state: &ZeroclawState,
) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetProvider", json_args)?;
    let provider_id = require_str(&args, "provider_id", "SetProvider")?;
    if !state
        .projection
        .providers
        .iter()
        .any(|p| p.id == provider_id)
    {
        return Err(ZeroclawError::ProviderNotDeclared {
            provider: provider_id,
        });
    }
    let old = state.selected_provider.clone();
    Ok(DispatchOutcome {
        result: serde_json::json!({ "selected_provider": provider_id }),
        signal: Some(DispatchSignal {
            name: "ProviderChanged".to_string(),
            payload: serde_json::json!({ "old": old, "new": provider_id, "reason": "explicit set" }),
        }),
    })
}

fn set_model_handler(
    json_args: &str,
    state: &ZeroclawState,
) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetModel", json_args)?;
    let model_id = require_str(&args, "model_id", "SetModel")?;
    if !state
        .projection
        .model_routes
        .iter()
        .any(|r| r.model == model_id)
    {
        return Err(ZeroclawError::ModelNotDeclared { model: model_id });
    }
    let old = state.selected_model.clone();
    Ok(DispatchOutcome {
        result: serde_json::json!({ "selected_model": model_id }),
        signal: Some(DispatchSignal {
            name: "ModelChanged".to_string(),
            payload: serde_json::json!({ "old": old, "new": model_id, "reason": "explicit set" }),
        }),
    })
}

fn set_role_model_handler(
    json_args: &str,
    state: &ZeroclawState,
    role: &str,
) -> std::result::Result<DispatchOutcome, ZeroclawError> {
    let args = parse_args("SetRoleModel", json_args)?;
    let model_id = require_str(&args, "model_id", "SetRoleModel")?;
    let result = match role {
        "ovs_routing" => serde_json::json!({ "ovs_routing": model_id }),
        "obfuscation" => serde_json::json!({ "obfuscation": model_id }),
        "vectorization" => serde_json::json!({ "vectorization": model_id }),
        "qdrant_retrieval" => serde_json::json!({ "qdrant_retrieval": model_id }),
        "cozo_retrieval" => serde_json::json!({ "cozo_retrieval": model_id }),
        _ => {
            return Err(ZeroclawError::ExecutionDenied {
                reason: format!("unknown model role: {role}"),
            })
        }
    };
    let signal = serde_json::json!({
        "role": role,
        "model_id": model_id,
        "selected_provider": state.selected_provider,
    });
    Ok(DispatchOutcome {
        result,
        signal: Some(DispatchSignal {
            name: "ModelRoleChanged".to_string(),
            payload: signal,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use serde_json::Value as JVal;

    fn collect_subids(node: &JVal, out: &mut Vec<String>) {
        if let Some(subid) = node.get("x-oscal-subid").and_then(JVal::as_str) {
            out.push(subid.to_string());
        }
        if let Some(props) = node.get("properties").and_then(JVal::as_object) {
            for v in props.values() {
                collect_subids(v, out);
            }
        }
        if let Some(defs) = node
            .get("$defs")
            .or_else(|| node.get("definitions"))
            .and_then(JVal::as_object)
        {
            for v in defs.values() {
                collect_subids(v, out);
            }
        }
        if let Some(items) = node.get("items") {
            collect_subids(items, out);
        }
        if let Some(alts) = node
            .get("anyOf")
            .or_else(|| node.get("oneOf"))
            .and_then(JVal::as_array)
        {
            for v in alts {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(ZeroclawState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(!subids.is_empty(), "expected at least one x-oscal-subid");
        for subid in subids {
            validate_subid(&subid).expect("invalid subid: {subid}");
        }
    }

    #[test]
    fn public_schema_accessor_returns_zeroclaw_schema() {
        let schema = zeroclaw_plugin_schema();
        assert_eq!(schema.name, PLUGIN_NAME);
        assert_eq!(schema.version, PLUGIN_VERSION);
        assert_eq!(schema.display_name, Some(PLUGIN_DISPLAY_NAME.to_string()));
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(ZeroclawPlugin::new()))
}
