//! 3tched Router — GB.TchedRouter.
//!
//! Publishes the Antigravity-facing model and CLI routing contract through
//! `PluginSchema` so the UI can render provider/model controls from D-Bus.

use super::common::errors::TchedRouterError;
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

const PLUGIN_NAME: &str = "tched_router";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "llm";
const PLUGIN_DESCRIPTION: &str = "3tched Router schema/RPC-native model router for Antigravity UI, CLI providers, and structured JSON output";
const PLUGIN_DISPLAY_NAME: &str = "GB.TchedRouter";

/// Transport layer metadata for the TchedRouter plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.tched-router-transport.schema@v1"))]
pub struct LlmTransport {
    /// D-Bus object path served by this plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router-transport.dbus-object@v1"))]
    pub dbus_object: String,
    /// gRPC upstream target.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router-transport.grpc-target@v1"))]
    pub grpc_target: String,
    /// Incus / WireGuard container target for xray routing. Kept as a
    /// published-schema field for backward compatibility, but tched-router's LLM
    /// transport now runs on the host (xray through its runit-managed service and
    /// the gRPC-bridge via `op-grpc-bridge-tched-router`); there is no per-service
    /// incus container. Defaults to the `"host"` sentinel.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router-transport.incus-container@v1"))]
    pub incus_container: String,
    /// Browser-facing surface description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.tched-router-transport.browser-surface@v1"))]
    pub browser_surface: String,
    /// REST aliases exposed by the bridge.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.tched-router-transport.rest-aliases@v1"))]
    pub rest_aliases: Vec<String>,
    /// Canonical OSCAL/subid mapping authority.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.tched-router-transport.policy-source@v1"))]
    pub policy_source: String,
}

/// Nested per-capability model assignments.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.tched-router.model-assignments.schema@v1"))]
pub struct ModelAssignments {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.model-assignments.ovs-routing@v1"))]
    pub ovs_routing: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.model-assignments.obfuscation@v1"))]
    pub obfuscation: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.model-assignments.vectorization@v1"))]
    pub vectorization: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.model-assignments.qdrant-retrieval@v1"))]
    pub qdrant_retrieval: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.model-assignments.cozo-retrieval@v1"))]
    pub cozo_retrieval: String,
}

/// Configurable options RPC contract extracted from `operation.registration.v1`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.rpc.schema@v1"))]
pub struct ConfigurableOptionRpc {
    /// RPC name from RegistrationService.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.rpc.name@v1"))]
    pub name: String,
    /// Request message type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.rpc.request@v1"))]
    pub request_type: String,
    /// Response message type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.rpc.response@v1"))]
    pub response_type: String,
    /// Read or mutation side-effect classification.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.rpc.side-effect@v1"))]
    pub side_effect: String,
    /// Required capability for this RPC.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.rpc.capability@v1"))]
    pub capability_id: String,
    /// OSCAL operation taxonomy key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.rpc.subid@v1"))]
    pub subid: String,
}

/// Configurable options message contract extracted from registration.proto.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.message.schema@v1"))]
pub struct ConfigurableOptionMessage {
    /// Message type name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.message.name@v1"))]
    pub name: String,
    /// Message fields keyed by proto field name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.message.fields@v1"))]
    pub fields: Vec<ConfigurableOptionField>,
}

/// Field contract extracted from configurable options proto messages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.field.schema@v1"))]
pub struct ConfigurableOptionField {
    /// Field name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.field.name@v1"))]
    pub name: String,
    /// Proto type, including optional/repeated marker when applicable.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.field.proto-type@v1"))]
    pub proto_type: String,
    /// JSON/schema rendering type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.field.json-type@v1"))]
    pub json_type: String,
    /// True when the field contains sensitive material.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.field.sensitive@v1"))]
    pub sensitive: bool,
    /// True when this option is required by the source contract.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.field.required@v1"))]
    pub required: bool,
}

/// Registration error code contract.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.error-code.schema@v1"))]
pub struct ConfigurableOptionErrorCode {
    /// Symbolic proto enum name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.error-code.name@v1"))]
    pub name: String,
    /// Numeric proto value.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.error-code.value@v1"))]
    pub value: i32,
}

/// Magic-link and WireGuard registration service schema.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.registration-service.schema@v1"))]
pub struct RegistrationServiceSchema {
    /// Proto package name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.registration-service.package@v1"))]
    pub package: String,
    /// Owning proto source path.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.tched-router-options.registration-service.proto@v1"))]
    pub source_proto: String,
    /// Service name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.registration-service.name@v1"))]
    pub service: String,
    /// RPC methods exposed by the registration service.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.registration-service.rpcs@v1"))]
    pub rpcs: Vec<ConfigurableOptionRpc>,
    /// Message schemas used by those RPCs.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.registration-service.messages@v1"))]
    pub messages: Vec<ConfigurableOptionMessage>,
    /// Error enum values.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.registration-service.errors@v1"))]
    pub error_codes: Vec<ConfigurableOptionErrorCode>,
}

/// Identity chain declared for user container options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.identity-chain.schema@v1"))]
pub struct IdentityOptions {
    /// Hardware-bound seed accepted by the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.hardware.tched-router-options.identity.mac@v1"))]
    pub mac_address_key: String,
    /// Optional shared key accepted by the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.policy.tched-router-options.identity.psk@v1"))]
    pub pre_shared_key: String,
    /// WireGuard public key identity.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.tched-router-options.identity.wireguard-pubkey@v1"))]
    pub wireguard_pubkey: String,
    /// MCP bearer token derivation rule.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.identity.mcp-token@v1"))]
    pub mcp_token_derivation: String,
}

/// Namespace template declared for each user container.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.memory-namespace.schema@v1"))]
pub struct MemoryNamespaceOption {
    /// Namespace template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.memory-namespace.template@v1"))]
    pub namespace_template: String,
    /// Keys owned by this namespace.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.memory-namespace.keys@v1"))]
    pub keys: Vec<String>,
    /// Human-readable purpose.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.memory-namespace.purpose@v1"))]
    pub purpose: String,
    /// Identity key used to bind this namespace to the container/user identity.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.memory-namespace.identity-link@v1"))]
    pub identity_link_key: String,
}

/// User container option contract exposed through TchedRouter.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.process-procedure.tched-router-options.user-container.schema@v1"))]
pub struct UserContainerOptions {
    /// Source script that currently materializes the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.process-procedure.tched-router-options.user-container.script@v1"))]
    pub source_script: String,
    /// User-visible container arguments.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.process-procedure.tched-router-options.user-container.arguments@v1"))]
    pub arguments: Vec<ConfigurableOptionField>,
    /// Incus container name template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.user-container.container-template@v1"))]
    pub container_id_template: String,
    /// Default Incus image.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.user-container.image@v1"))]
    pub image: String,
    /// Cognitive MCP endpoint used by the user container memory flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.tched-router-options.user-container.mcp-endpoint@v1"))]
    pub cognitive_mcp_endpoint: String,
    /// Feature flags declared for the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.tched-router-options.user-container.features@v1"))]
    pub feature_flags: Vec<String>,
}

/// Privacy rules for configurable options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.policy.tched-router-options.privacy.schema@v1"))]
pub struct PrivacyOptions {
    /// Whether email may be written to CozoDB.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.tched-router-options.privacy.email-storage@v1"))]
    pub email_storage_rule: String,
    /// Sensitive fields that must not be rendered casually.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.tched-router-options.privacy.sensitive-fields@v1"))]
    pub sensitive_fields: Vec<String>,
}

/// Complete configurable options schema surface owned by TchedRouter.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.schema@v1"))]
pub struct ConfigurableOptions {
    /// Magic-link and WireGuard registration RPC schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router-options.registration-service@v1"))]
    pub registration_service: RegistrationServiceSchema,
    /// User container configurable option schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.process-procedure.tched-router-options.user-container@v1"))]
    pub user_container: UserContainerOptions,
    /// Identity chain schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.identity-chain@v1"))]
    pub identity_chain: IdentityOptions,
    /// Memory namespace templates.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router-options.memory-namespaces@v1"))]
    pub memory_namespaces: Vec<MemoryNamespaceOption>,
    /// Privacy handling rules.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.tched-router-options.privacy@v1"))]
    pub privacy_policy: PrivacyOptions,
}

/// Top-level TchedRouter state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.tched-router.schema@v1"))]
#[schemars(extend("x-oscal-category" = "llm"))]
pub struct TchedRouterState {
    /// Operational status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.status@v1"))]
    pub status: String,
    /// Selected provider identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.selected-provider@v1"))]
    pub selected_provider: String,
    /// Selected model identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.tched-router.selected-model@v1"))]
    pub selected_model: String,
    /// Per-capability model assignments.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.model-assignments@v1"))]
    pub model_assignments: ModelAssignments,
    /// Transport layer metadata.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.tched-router.transport@v1"))]
    pub transport: LlmTransport,
    /// Configurable options schema: registration, user container options,
    /// identity chain, memory namespaces, and privacy rules.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.tched-router.options@v1"))]
    pub configurable_options: ConfigurableOptions,
    /// Inspector Gadget fields discovered from the upstream ZeroClaw Repomix.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router.inspector-fields@v1"))]
    pub inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
    /// Shared LLM projection fields (flattened to the top level).
    #[serde(flatten)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router.llm-projection@v1"))]
    pub projection: LlmProjection,
}

/// Empty input for read-only ZeroClaw methods.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EmptyTchedRouterInput {}

/// Input for resolving a model route by hint or model identifier.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveRouteInput {
    /// Route hint or model identifier.
    pub hint: String,
}

/// A chat message carried by the schema-declared ZeroClaw method.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TchedRouterChatMessage {
    /// OpenAI-compatible message role such as `system`, `user`, or `assistant`.
    pub role: String,
    /// Text content carried by this conversation turn.
    pub content: String,
}

/// Input for the bridge-owned ZeroClaw chat dispatcher.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChatInput {
    /// Compatibility form for callers sending a single user turn.
    #[serde(default)]
    pub message: String,
    /// Full ordered conversation. When non-empty this takes precedence over
    /// `message`.
    #[serde(default)]
    pub messages: Vec<TchedRouterChatMessage>,
    /// Provider id, route, or alias. Empty uses `selected_provider`.
    #[serde(default)]
    pub provider: String,
    /// Model id or route hint. Empty uses `selected_model`.
    #[serde(default)]
    pub model: String,
}

/// Input for listing model routes, optionally filtered by provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListModelsInput {
    /// Optional provider id, route, or alias used to filter the model catalog.
    #[serde(default)]
    pub provider: String,
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
#[schemars(extend("x-oscal-subid" = "exp.service.tched-router.state.result@v1"))]
pub struct GetStateOutput {
    /// Complete projected ZeroClaw state.
    pub state: TchedRouterState,
}

/// Output for the model route catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.tched-router.model-routes.result@v1"))]
pub struct GetModelRoutesOutput {
    /// Declared model routes.
    pub model_routes: Vec<ModelRoute>,
}

/// Output for the provider catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.tched-router.provider-catalog.result@v1"))]
pub struct GetProviderCatalogOutput {
    /// Declared providers.
    pub providers: Vec<Provider>,
}

/// Output for the declared tool catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.tched-router.tools.result@v1"))]
pub struct GetToolsOutput {
    /// Declared LLM tools.
    pub tools: Vec<LlmTool>,
}

/// Output for the provider list accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.providers.result@v1"))]
pub struct ListProvidersOutput {
    pub providers: Vec<Provider>,
}

/// Output for the router accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.router.result@v1"))]
pub struct GetRouterOutput {
    pub router: Router,
}

/// Output for the config schema accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.config-schema.result@v1"))]
pub struct GetConfigSchemaOutput {
    pub config_schema: ConfigSchema,
}

/// Output for the UI surfaces accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.ui-surfaces.result@v1"))]
pub struct ListUiSurfacesOutput {
    pub ui_surfaces: Vec<UiSurface>,
}

/// Output for the structured output accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.structured-output.result@v1"))]
pub struct GetStructuredOutputOutput {
    pub structured_output: StructuredOutput,
}

/// Output for a resolved model route.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.tched-router.route.result@v1"))]
pub struct ResolveRouteOutput {
    /// Resolved route.
    pub route: ModelRoute,
}

/// Output from the bridge-owned ZeroClaw chat dispatcher.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.tched-router.chat.result@v1"))]
pub struct ChatOutput {
    /// Assistant response text.
    pub content: String,
    /// Resolved upstream provider identifier.
    pub provider: String,
    /// Resolved model identifier.
    pub model: String,
    /// Provider finish reason, or an empty string when none was supplied.
    pub finish_reason: String,
    /// Provider-specific token usage object.
    pub usage: JsonValue,
}

/// Schema-declared model listing output.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.models.result@v1"))]
pub struct ListModelsOutput {
    /// Schema-declared model routes, optionally filtered by provider.
    pub model_routes: Vec<ModelRoute>,
}

/// Output for provider selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.tched-router.selected-provider.result@v1"))]
pub struct SetProviderOutput {
    /// Selected provider identifier.
    pub selected_provider: String,
}

/// Output for model selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.tched-router.selected-model.result@v1"))]
pub struct SetModelOutput {
    /// Selected model identifier.
    pub selected_model: String,
}

/// Output for the model assignments accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.model-assignments.result@v1"))]
pub struct GetModelAssignmentsOutput {
    pub model_assignments: ModelAssignments,
}

/// Output for the visible TchedRouter configurable options.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.options.result@v1"))]
pub struct GetConfigurableOptionsOutput {
    pub configurable_options: ConfigurableOptions,
}

/// Output for the workspace memory namespace option templates.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.tched-router.options.memory-namespaces.result@v1"))]
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

pub struct TchedRouterPlugin;

impl Default for TchedRouterPlugin {
    fn default() -> Self {
        Self
    }
}

impl TchedRouterPlugin {
    const DBUS_OBJECT: &'static str = "/org/opdbus/v1/plugins/tched-router";
    const OSCAL_SUBID_REGISTRY_OBJECT: &'static str = "/org/opdbus/v1/plugins/oscal_subid_registry";
    const DEFAULT_CHAT_MODEL: &'static str = "deepseek-v4-flash-free";

    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    pub fn current_state() -> TchedRouterState {
        let selected_provider = Self::env_or("LLM_PROVIDER", "opencode");
        let selected_model = Self::env_or("LLM_MODEL", Self::DEFAULT_CHAT_MODEL);
        let chat_model = selected_model.clone();
        let router_endpoint = Self::env_or("TCHED_ROUTER_ROUTER_ENDPOINT", "http://localhost:11434");
        let grpc_target = Self::env_or("TCHED_ROUTER_GRPC_TARGET", "http://10.200.0.2:50051");
        let grpc_target_for_provider = grpc_target.clone();

        TchedRouterState {
            status: "declared".to_string(),
            selected_provider: selected_provider.clone(),
            selected_model,
            model_assignments: ModelAssignments {
                ovs_routing: Self::DEFAULT_CHAT_MODEL.to_string(),
                obfuscation: Self::DEFAULT_CHAT_MODEL.to_string(),
                vectorization: "gemini-embedding-001".to_string(),
                qdrant_retrieval: "gemini-embedding-001".to_string(),
                cozo_retrieval: Self::DEFAULT_CHAT_MODEL.to_string(),
            },
            transport: LlmTransport {
                dbus_object: Self::DBUS_OBJECT.to_string(),
                grpc_target: grpc_target_for_provider,
                incus_container: "host".to_string(),
                browser_surface: "gRPC-Web through op-web".to_string(),
                rest_aliases: vec![
                    "/api/tched-router/chat".to_string(),
                    "/api/llm/chat".to_string(),
                ],
                policy_source: Self::OSCAL_SUBID_REGISTRY_OBJECT.to_string(),
            },
            configurable_options: Self::configurable_options(),
            inspector_fields: Default::default(),
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
                        aliases: vec![
                            "opencode.go".to_string(),
                            "deepseek-v4-flash-free".to_string(),
                        ],
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
                    provider: selected_provider.clone(),
                    model: chat_model.clone(),
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
                        provider: "opencode".to_string(),
                        upstream_provider: "opencode".to_string(),
                        transport: "tched-router-loopback".to_string(),
                        model: chat_model.clone(),
                        kind: "router".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: format!(
                            "{chat_model} is the declared OpenCode chat model; availability is projected by the ZeroClaw runtime."
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
                        upstream_provider: "opencode".to_string(),
                        transport: "tched-router-loopback".to_string(),
                        model: chat_model.clone(),
                        kind: "router".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: format!(
                            "Factory local route -> opencode/{chat_model}; requires backend projection."
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
                        name: "tched-router.chat".to_string(),
                        description: "Send an Antigravity/TchedRouter chat turn and return structured JSON when response_schema is present.".to_string(),
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
                        name: "tched-router.models.list".to_string(),
                        description: "List cached or live models for a TchedRouter provider.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {"provider": {"type": "string"}},
                            "required": []
                        }),
                        ..Default::default()
                    },
                ],
                config_schema: ConfigSchema {
                    source: "tched-router config schema".to_string(),
                    schema_crate: "schemars".to_string(),
                    native_type: "tched-router::config::schema::Config".to_string(),
                    status: "available_via_cli_or_gateway".to_string(),
                    ..Default::default()
                },
                ui_surfaces: vec![
                    UiSurface {
                        path: "/grpc".to_string(),
                        name: "gRPC Diagnostics".to_string(),
                        schema: "plugin-service".to_string(),
                    },
                    UiSurface {
                        path: "/models".to_string(),
                        name: "Routable Models".to_string(),
                        schema: "tched-router.providers".to_string(),
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

    /// Query the Salad AI Gateway's live model list (`GET /v1/models`), Bearer-
    /// authenticated with `SALAD_API_KEY`. Returns the reported model IDs, or an
    /// empty list if the key is absent, the request fails, or the response is
    /// unparsable — this is a best-effort present-state probe, never a hard
    /// dependency for schema construction.
    ///
    /// Implemented independently of `op_llm::salad::SaladProvider`: `op-llm`
    /// already depends on `op-plugins`, so the reverse dependency isn't
    /// available without introducing a cycle. The request shape mirrors
    /// `SaladProvider::list_models`.
    async fn probe_salad_models() -> Vec<String> {
        let Ok(api_key) = std::env::var("SALAD_API_KEY") else {
            return Vec::new();
        };
        let base_url = Self::env_or("SALAD_BASE_URL", "https://ai.salad.cloud/v1");
        let url = format!("{}/models", base_url.trim_end_matches('/'));

        let Ok(client) = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        else {
            return Vec::new();
        };

        let response = match client.get(&url).bearer_auth(&api_key).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::warn!("Salad models probe failed ({}): {}", r.status(), url);
                return Vec::new();
            }
            Err(e) => {
                tracing::warn!("Salad models probe unreachable: {}", e);
                return Vec::new();
            }
        };

        let Ok(body) = response.text().await else {
            return Vec::new();
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) else {
            return Vec::new();
        };

        parsed
            .get("data")
            .and_then(|d| d.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|e| e.get("id").and_then(|v| v.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Send a minimal real chat-completion request to confirm a specific
    /// model actually answers within a bounded time — not just that it's
    /// *listed*. `/v1/models` reports every model Salad has ever declared;
    /// it does not guarantee a warm replica is currently serving it. Larger,
    /// less-popular models on Salad's distributed marketplace have been
    /// observed to 503 for minutes with no warm replica while smaller models
    /// answer in seconds, so presence in `/v1/models` alone overstates
    /// availability.
    async fn probe_salad_model_reachable(model: &str, api_key: &str, base_url: &str) -> bool {
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let Ok(client) = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
        else {
            return false;
        };
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 4,
            "stream": false
        });
        match client
            .post(&url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => true,
            Ok(r) => {
                tracing::warn!(
                    "Salad reachability probe for {} failed ({})",
                    model,
                    r.status()
                );
                false
            }
            Err(e) => {
                tracing::warn!(
                    "Salad reachability probe for {} unreachable/timed out: {}",
                    model,
                    e
                );
                false
            }
        }
    }

    /// Live variant of [`current_state`](Self::current_state): folds real
    /// Salad Gateway checks into the declared model routes — both
    /// [`probe_salad_models`](Self::probe_salad_models) (is the model listed
    /// at all) and [`probe_salad_model_reachable`](Self::probe_salad_model_reachable)
    /// (does it actually answer a real request right now). A model must pass
    /// both to be marked `available: true`.
    ///
    /// - Declared routes get `available` = listed AND reachable, with a
    ///   status reason distinguishing "not listed", "listed but didn't
    ///   answer (no warm replica / timeout)", and "confirmed live."
    /// - Any listed-and-reachable model that ISN'T one of the statically
    ///   declared routes is appended as a `"discovered"` route, so new Salad
    ///   models surface without a code change — this is the same live list
    ///   that feeds `tched-router.models.list` and the `/models` UI surface, so
    ///   they inherit it for free once schema construction uses this path.
    /// - If `SALAD_API_KEY` is absent or `/v1/models` returns nothing, the
    ///   declared defaults from `current_state()` are left untouched.
    pub async fn current_state_live() -> TchedRouterState {
        let mut state = Self::current_state();
        let Ok(api_key) = std::env::var("SALAD_API_KEY") else {
            return state;
        };
        let live_models = Self::probe_salad_models().await;
        if live_models.is_empty() {
            return state;
        }
        let base_url = Self::env_or("SALAD_BASE_URL", "https://ai.salad.cloud/v1");

        let live_set: std::collections::HashSet<&str> =
            live_models.iter().map(String::as_str).collect();

        // Reachability is real per-request I/O (up to 20s each); only worth
        // paying for models the catalog actually lists. Sequential is fine
        // here — this runs during an explicit `opblob seal-shm`, not a
        // per-request hot path, and Salad's declared catalog is tiny.
        let mut reachable: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        for model in &live_models {
            let ok = Self::probe_salad_model_reachable(model, &api_key, &base_url).await;
            reachable.insert(model.clone(), ok);
        }

        for route in state.projection.model_routes.iter_mut() {
            if route.provider != "salad" {
                continue;
            }
            let listed = live_set.contains(route.model.as_str());
            let is_reachable = reachable.get(&route.model).copied().unwrap_or(false);
            route.available = listed && is_reachable;
            route.status_reason = match (listed, is_reachable) {
                (true, true) => format!(
                    "{}; confirmed live — listed in /v1/models and answered a real request.",
                    route.model
                ),
                (true, false) => format!(
                    "{}; listed in Salad /v1/models but did not answer a live test request (no warm replica or timeout).",
                    route.model
                ),
                (false, _) => format!(
                    "{}; not present in the live /v1/models response.",
                    route.model
                ),
            };
        }

        let declared_models: std::collections::HashSet<String> = state
            .projection
            .model_routes
            .iter()
            .filter(|r| r.provider == "salad")
            .map(|r| r.model.clone())
            .collect();
        for model in &live_models {
            if declared_models.contains(model.as_str()) {
                continue;
            }
            let is_reachable = reachable.get(model).copied().unwrap_or(false);
            state.projection.model_routes.push(ModelRoute {
                hint: "discovered".to_string(),
                provider: "salad".to_string(),
                upstream_provider: "salad".to_string(),
                transport: "direct".to_string(),
                model: model.clone(),
                kind: "chat".to_string(),
                status: "discovered".to_string(),
                available: is_reachable,
                status_reason: if is_reachable {
                    format!(
                        "{model}; discovered live via Salad GET /v1/models and confirmed reachable (not statically declared)."
                    )
                } else {
                    format!(
                        "{model}; discovered via Salad GET /v1/models but did not answer a live test request (not statically declared)."
                    )
                },
                api_key: Some(JsonValue::Null),
                ..Default::default()
            });
        }

        state
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
                        "cap.software.tched-router.registration.magic-link.send@v1",
                        "mut.service.tched-router.registration.magic-link.send@v1",
                    ),
                    Self::option_rpc(
                        "VerifyMagicLink",
                        "VerifyMagicLinkRequest",
                        "VerifyMagicLinkResponse",
                        "Mutation",
                        "cap.software.tched-router.registration.magic-link.verify@v1",
                        "mut.service.tched-router.registration.magic-link.verify@v1",
                    ),
                    Self::option_rpc(
                        "RegisterUser",
                        "RegisterUserRequest",
                        "RegisterUserResponse",
                        "Mutation",
                        "cap.software.tched-router.registration.user.register@v1",
                        "mut.service.tched-router.registration.user.register@v1",
                    ),
                    Self::option_rpc(
                        "GetUserStatus",
                        "GetUserStatusRequest",
                        "GetUserStatusResponse",
                        "Read",
                        "cap.software.tched-router.registration.user-status.read@v1",
                        "obs.service.tched-router.registration.user-status.get@v1",
                    ),
                    Self::option_rpc(
                        "ListUsers",
                        "ListUsersRequest",
                        "ListUsersResponse",
                        "Read",
                        "cap.software.tched-router.registration.users.read@v1",
                        "obs.service.tched-router.registration.users.list@v1",
                    ),
                    Self::option_rpc(
                        "GetWireGuardConfig",
                        "GetWireGuardConfigRequest",
                        "GetWireGuardConfigResponse",
                        "Read",
                        "cap.software.tched-router.registration.wireguard-config.read@v1",
                        "obs.service.tched-router.registration.wireguard-config.get@v1",
                    ),
                    Self::option_rpc(
                        "AdminUserAction",
                        "AdminUserActionRequest",
                        "AdminUserActionResponse",
                        "Mutation",
                        "cap.software.tched-router.registration.admin-user-action.apply@v1",
                        "mut.service.tched-router.registration.admin-user-action.apply@v1",
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
impl StatePlugin for TchedRouterPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = tched_router_schema();
        super::common::llm_projection::rewrite_projection_subids_for_plugin(
            &mut schema,
            PLUGIN_NAME,
        );
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn schema_live(&self) -> Option<PluginSchema> {
        let mut schema = tched_router_schema_live().await;
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

/// Canonical `tched-router` schema derived from [`TchedRouterState`] via schemars.
pub(crate) fn tched_router_schema() -> PluginSchema {
    tched_router_schema_from_state(TchedRouterPlugin::current_state())
}

/// Live variant of [`tched_router_schema`]: folds in a real reachability probe
/// against the Salad AI Gateway (`SALAD_API_KEY` + `GET /v1/models`) so
/// `available`/`status_reason` on Salad routes reflect the backend's actual
/// current answer instead of a static declaration, and any model the gateway
/// reports that isn't already a declared route is surfaced too. See
/// [`TchedRouterPlugin::current_state_live`].
pub(crate) async fn tched_router_schema_live() -> PluginSchema {
    tched_router_schema_from_state(TchedRouterPlugin::current_state_live().await)
}

fn tched_router_schema_from_state(state: TchedRouterState) -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(TchedRouterState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());

    if let Ok(state) = simd_json::serde::to_owned_value(state) {
        super::schemars_adapter::apply_state_defaults(&mut schema, &state);
        schema.example = Some(state);
    }

    schema.methods.insert(
        "GetState".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetStateOutput>(
            "GetState",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.state.read@v1",
            "obs.service.tched-router.state.get@v1",
        ),
    );
    schema.methods.insert(
        "GetModelAssignments".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetModelAssignmentsOutput>(
            "GetModelAssignments",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.model-assignments.read@v1",
            "obs.service.tched-router.model-assignments.get@v1",
        ),
    );
    schema.methods.insert(
        "GetConfigurableOptions".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetConfigurableOptionsOutput>(
            "GetConfigurableOptions",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.options.read@v1",
            "obs.service.tched-router.options.get@v1",
        ),
    );
    schema.methods.insert(
        "ListUserContainerMemoryNamespaceOptions".to_string(),
        method_decl_from_schemars_with_output::<
            EmptyTchedRouterInput,
            ListUserContainerMemoryNamespaceOptionsOutput,
        >(
            "ListUserContainerMemoryNamespaceOptions",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.options.memory-namespaces.read@v1",
            "obs.service.tched-router.options.memory-namespaces.list@v1",
        ),
    );
    schema.methods.insert(
        "GetModelRoutes".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetModelRoutesOutput>(
            "GetModelRoutes",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.routes.read@v1",
            "obs.service.tched-router.model-routes.list@v1",
        ),
    );
    schema.methods.insert(
        "ListProviders".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, ListProvidersOutput>(
            "ListProviders",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.providers.read@v1",
            "obs.service.tched-router.providers.list@v1",
        ),
    );
    schema.methods.insert(
        "GetProviderCatalog".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetProviderCatalogOutput>(
            "GetProviderCatalog",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.providers.read@v1",
            "obs.service.tched-router.provider-catalog.list@v1",
        ),
    );
    schema.methods.insert(
        "GetTools".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetToolsOutput>(
            "GetTools",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.tools.read@v1",
            "obs.service.tched-router.tools.list@v1",
        ),
    );
    schema.methods.insert(
        "GetRouter".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetRouterOutput>(
            "GetRouter",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.router.read@v1",
            "obs.service.tched-router.router.get@v1",
        ),
    );
    schema.methods.insert(
        "GetConfigSchema".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetConfigSchemaOutput>(
            "GetConfigSchema",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.config-schema.read@v1",
            "obs.service.tched-router.config-schema.get@v1",
        ),
    );
    schema.methods.insert(
        "ListUiSurfaces".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, ListUiSurfacesOutput>(
            "ListUiSurfaces",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.ui-surfaces.read@v1",
            "obs.service.tched-router.ui-surfaces.list@v1",
        ),
    );
    schema.methods.insert(
        "GetStructuredOutput".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedRouterInput, GetStructuredOutputOutput>(
            "GetStructuredOutput",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.structured-output.read@v1",
            "obs.service.tched-router.structured-output.get@v1",
        ),
    );
    schema.methods.insert(
        "ResolveRoute".to_string(),
        method_decl_from_schemars_with_output::<ResolveRouteInput, ResolveRouteOutput>(
            "ResolveRoute",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.route.resolve@v1",
            "obs.service.tched-router.route.resolve@v1",
        ),
    );
    schema.methods.insert(
        "Chat".to_string(),
        method_decl_from_schemars_with_output::<ChatInput, ChatOutput>(
            "Chat",
            op_state_store::SideEffect::Read,
            false,
            "cap.software.tched-router.chat@v1",
            "exp.service.tched-router.chat@v1",
        ),
    );
    schema.methods.insert(
        "ListModels".to_string(),
        method_decl_from_schemars_with_output::<ListModelsInput, ListModelsOutput>(
            "ListModels",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.tched-router.models.read@v1",
            "obs.service.tched-router.models.list@v1",
        ),
    );
    schema.methods.insert(
        "SetProvider".to_string(),
        method_decl_from_schemars_with_output::<SetProviderInput, SetProviderOutput>(
            "SetProvider",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.tched-router.provider.set@v1",
            "mut.service.tched-router.provider.set@v1",
        ),
    );
    schema.methods.insert(
        "SetModel".to_string(),
        method_decl_from_schemars_with_output::<SetModelInput, SetModelOutput>(
            "SetModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.tched-router.model.set@v1",
            "mut.service.tched-router.model.set@v1",
        ),
    );
    schema.methods.insert(
        "SetOvsRoutingModel".to_string(),
        method_decl_from_schemars_with_output::<SetOvsRoutingModelInput, SetOvsRoutingModelOutput>(
            "SetOvsRoutingModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.tched-router.model-assignments.ovs-routing.set@v1",
            "mut.service.tched-router.model-assignments.ovs-routing.set@v1",
        ),
    );
    schema.methods.insert(
        "SetObfuscationModel".to_string(),
        method_decl_from_schemars_with_output::<SetObfuscationModelInput, SetObfuscationModelOutput>(
            "SetObfuscationModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.tched-router.model-assignments.obfuscation.set@v1",
            "mut.service.tched-router.model-assignments.obfuscation.set@v1",
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
            "cap.software.tched-router.model-assignments.vectorization.set@v1",
            "mut.service.tched-router.model-assignments.vectorization.set@v1",
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
            "cap.software.tched-router.model-assignments.qdrant-retrieval.set@v1",
            "mut.service.tched-router.model-assignments.qdrant-retrieval.set@v1",
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
            "cap.software.tched-router.model-assignments.cozo-retrieval.set@v1",
            "mut.service.tched-router.model-assignments.cozo-retrieval.set@v1",
        ),
    );

    inspector_gadget_generated::register_methods(&mut schema);
    schema
}

/// Public accessor for crates that embed the TchedRouter plugin contract.
pub fn tched_router_plugin_schema() -> PluginSchema {
    tched_router_schema()
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

/// Plugin-owned method dispatch for the TchedRouter D-Bus/gRPC surface.
pub fn dispatch_tched_router_method(
    method: &str,
    json_args: &str,
    state: &TchedRouterState,
) -> std::result::Result<DispatchOutcome, TchedRouterError> {
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
        "ListModels" => list_models(json_args, state).map(DispatchOutcome::plain),
        "SetProvider" => set_provider_handler(json_args, state),
        "SetModel" => set_model_handler(json_args, state),
        "SetOvsRoutingModel" => set_role_model_handler(json_args, state, "ovs_routing"),
        "SetObfuscationModel" => set_role_model_handler(json_args, state, "obfuscation"),
        "SetVectorizationModel" => set_role_model_handler(json_args, state, "vectorization"),
        "SetQdrantRetrievalModel" => set_role_model_handler(json_args, state, "qdrant_retrieval"),
        "SetCozoRetrievalModel" => set_role_model_handler(json_args, state, "cozo_retrieval"),
        other
            if inspector_gadget_generated::METHOD_CANDIDATES
                .iter()
                .any(|candidate| candidate.name == other) =>
        {
            dispatch_generated_cli_method(other, json_args).map(DispatchOutcome::plain)
        }
        other => Err(TchedRouterError::ExecutionDenied {
            reason: format!("undeclared method: {other}"),
        }),
    }
}

fn dispatch_generated_cli_method(
    method: &str,
    json_args: &str,
) -> std::result::Result<JsonValue, TchedRouterError> {
    let candidate = inspector_gadget_generated::METHOD_CANDIDATES
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: format!("undeclared method: {method}"),
        })?;
    let command = generated_cli_tokens(candidate.repomix_path);
    if command.is_empty() {
        return Err(TchedRouterError::ExecutionDenied {
            reason: format!("generated method {method} has no CLI mapping"),
        });
    }
    let parsed: JsonValue =
        serde_json::from_str(json_args).map_err(|error| TchedRouterError::ExecutionDenied {
            reason: format!("invalid {method} arguments: {error}"),
        })?;
    let options = parsed
        .get("options")
        .and_then(JsonValue::as_object)
        .cloned()
        .unwrap_or_default();
    let binary = std::env::var("TCHED_ROUTER_CLI")
        .unwrap_or_else(|_| "/home/admin/.cargo/bin/tched-router".to_string());
    let mut process = std::process::Command::new(&binary);
    process.args(&command);
    append_cli_options(&mut process, options)
        .map_err(|reason| TchedRouterError::ExecutionDenied { reason })?;
    let output = process
        .output()
        .map_err(|error| TchedRouterError::ExecutionDenied {
            reason: format!("failed to execute {binary}: {error}"),
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(TchedRouterError::ExecutionDenied {
            reason: format!("{method} exited with {}: {}", output.status, stderr),
        });
    }
    Ok(serde_json::json!({
        "message": if stdout.is_empty() { stderr } else { stdout },
        "changed": candidate.side_effect == "mutation"
    }))
}

fn generated_cli_tokens(path: &str) -> Vec<String> {
    if let Some(command) = path.strip_prefix("cmd.") {
        return command.split('.').map(cli_token).collect();
    }
    if let Some(flag) = path
        .strip_prefix("flag.")
        .and_then(|value| value.rsplit('.').next())
    {
        return vec![format!("--{}", flag.replace('_', "-"))];
    }
    let Some(path) = path.strip_prefix("enum.") else {
        return Vec::new();
    };
    let Some((owner, variant)) = path.rsplit_once('.') else {
        return Vec::new();
    };
    let owner = owner
        .rsplit('.')
        .next()
        .unwrap_or(owner)
        .trim_end_matches("Commands")
        .trim_end_matches("commands");
    vec![cli_token(owner), cli_token(variant)]
}

fn cli_token(raw: &str) -> String {
    let mut token = String::new();
    for (index, ch) in raw.chars().enumerate() {
        if ch.is_ascii_uppercase() && index > 0 {
            token.push('-');
        }
        token.push(ch.to_ascii_lowercase());
    }
    token.replace('_', "-")
}

fn append_cli_options(
    process: &mut std::process::Command,
    options: serde_json::Map<String, JsonValue>,
) -> std::result::Result<(), String> {
    for (key, value) in options {
        if key == "value" || key == "argument" {
            let value = value
                .as_str()
                .ok_or_else(|| format!("{key} must be a string"))?;
            process.arg(value);
            continue;
        }
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(format!("invalid CLI option name: {key}"));
        }
        let value = value
            .as_str()
            .ok_or_else(|| format!("{key} must be a string"))?;
        process.arg(format!("--{}", key.replace('_', "-")));
        if !value.is_empty() {
            process.arg(value);
        }
    }
    Ok(())
}

fn list_models(
    json_args: &str,
    state: &TchedRouterState,
) -> std::result::Result<JsonValue, TchedRouterError> {
    let args = parse_args("ListModels", json_args)?;
    let provider = args
        .get("provider")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let routes = state
        .projection
        .model_routes
        .iter()
        .filter(|route| {
            provider.is_empty()
                || route.provider.eq_ignore_ascii_case(provider)
                || route.upstream_provider.eq_ignore_ascii_case(provider)
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "model_routes": to_json(&routes) }))
}

fn to_json<T: Serialize>(value: &T) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

fn parse_args(method: &str, json_args: &str) -> std::result::Result<JsonValue, TchedRouterError> {
    if json_args.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(json_args).map_err(|error| TchedRouterError::ExecutionDenied {
        reason: format!("{method} arguments are not valid JSON: {error}"),
    })
}

fn require_str(
    args: &JsonValue,
    field: &str,
    method: &str,
) -> std::result::Result<String, TchedRouterError> {
    args.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: format!("{method} requires string field '{field}'"),
        })
}

fn resolve_route(
    json_args: &str,
    state: &TchedRouterState,
) -> std::result::Result<JsonValue, TchedRouterError> {
    let args = parse_args("ResolveRoute", json_args)?;
    let hint = require_str(&args, "hint", "ResolveRoute")?;
    state
        .projection
        .model_routes
        .iter()
        .find(|route| route.hint == hint || route.model == hint)
        .map(|route| serde_json::json!({ "route": to_json(route) }))
        .ok_or(TchedRouterError::RouteNotDeclared { hint })
}

fn set_provider_handler(
    json_args: &str,
    state: &TchedRouterState,
) -> std::result::Result<DispatchOutcome, TchedRouterError> {
    let args = parse_args("SetProvider", json_args)?;
    let provider_id = require_str(&args, "provider_id", "SetProvider")?;
    if !state
        .projection
        .providers
        .iter()
        .any(|p| p.id == provider_id)
    {
        return Err(TchedRouterError::ProviderNotDeclared {
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
    state: &TchedRouterState,
) -> std::result::Result<DispatchOutcome, TchedRouterError> {
    let args = parse_args("SetModel", json_args)?;
    let model_id = require_str(&args, "model_id", "SetModel")?;
    if !state
        .projection
        .model_routes
        .iter()
        .any(|r| r.model == model_id)
    {
        return Err(TchedRouterError::ModelNotDeclared { model: model_id });
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
    state: &TchedRouterState,
    role: &str,
) -> std::result::Result<DispatchOutcome, TchedRouterError> {
    let args = parse_args("SetRoleModel", json_args)?;
    let model_id = require_str(&args, "model_id", "SetRoleModel")?;
    let result = match role {
        "ovs_routing" => serde_json::json!({ "ovs_routing": model_id }),
        "obfuscation" => serde_json::json!({ "obfuscation": model_id }),
        "vectorization" => serde_json::json!({ "vectorization": model_id }),
        "qdrant_retrieval" => serde_json::json!({ "qdrant_retrieval": model_id }),
        "cozo_retrieval" => serde_json::json!({ "cozo_retrieval": model_id }),
        _ => {
            return Err(TchedRouterError::ExecutionDenied {
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
        let raw = serde_json::to_value(schemars::schema_for!(TchedRouterState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(!subids.is_empty(), "expected at least one x-oscal-subid");
        for subid in subids {
            validate_subid(&subid).expect("invalid subid: {subid}");
        }
    }

    #[test]
    fn public_schema_accessor_returns_tched_router_schema() {
        let schema = tched_router_plugin_schema();
        assert_eq!(schema.name, PLUGIN_NAME);
        assert_eq!(schema.version, PLUGIN_VERSION);
        assert_eq!(schema.display_name, Some(PLUGIN_DISPLAY_NAME.to_string()));
    }

    #[test]
    fn generated_method_docs_include_input_and_output_field_descriptions() {
        let schema = tched_router_plugin_schema();
        let chat = schema.methods.get("Chat").unwrap();
        let chat_returns = serde_json::to_value(chat.returns.as_ref().unwrap()).unwrap();
        assert_eq!(
            chat_returns
                .pointer("/properties/content/description")
                .and_then(JVal::as_str),
            Some("Assistant response text.")
        );

        let list_models = schema.methods.get("ListModels").unwrap();
        let list_args = serde_json::to_value(&list_models.args).unwrap();
        assert_eq!(
            list_args
                .pointer("/properties/provider/description")
                .and_then(JVal::as_str),
            Some("Optional provider id, route, or alias used to filter the model catalog.")
        );
    }

    #[test]
    fn every_declared_method_has_a_domain_dispatcher() {
        let schema = tched_router_plugin_schema();
        let state = TchedRouterPlugin::current_state();

        for method in schema.methods.keys() {
            if method == "Chat" {
                // Chat is declared here but executed by the bridge runtime,
                // which owns provider credentials and the event chain.
                continue;
            }
            if inspector_gadget_generated::METHOD_CANDIDATES
                .iter()
                .any(|candidate| candidate.name == method)
            {
                assert!(
                    !generated_cli_tokens(
                        inspector_gadget_generated::METHOD_CANDIDATES
                            .iter()
                            .find(|candidate| candidate.name == method)
                            .unwrap()
                            .repomix_path
                    )
                    .is_empty(),
                    "{method} has no CLI mapping"
                );
                continue;
            }
            let args = match method.as_str() {
                "ResolveRoute" => serde_json::json!({ "hint": "balanced" }),
                "ListModels" => serde_json::json!({ "provider": "salad" }),
                "SetProvider" => serde_json::json!({ "provider_id": "salad" }),
                "SetModel" => serde_json::json!({ "model_id": "qwen3.6-27b" }),
                name if name.starts_with("Set") => {
                    serde_json::json!({ "model_id": "qwen3.6-27b" })
                }
                _ => serde_json::json!({}),
            };
            dispatch_tched_router_method(method, &args.to_string(), &state)
                .unwrap_or_else(|error| panic!("{method} is not executable: {error}"));
        }
    }

    #[test]
    fn undeclared_method_is_rejected() {
        let error = dispatch_tched_router_method("NotDeclared", "{}", &TchedRouterPlugin::current_state())
            .unwrap_err();
        assert!(error.to_string().contains("undeclared method"));
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(TchedRouterPlugin::new()))
}

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
pub mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.tched-router.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `enum.tched_router_config.AccessMode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.accessmode@v1"))]
        pub accessmode: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AccessMode.Read`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.read@v1"))]
        pub read: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AccessMode.ReadWrite`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.readwrite@v1"))]
        pub readwrite: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AccessMode.Write`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.write@v1"))]
        pub write: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasKind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.aliaskind@v1"))]
        pub aliaskind: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasKind.Agent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agent@v1"))]
        pub agent: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasKind.Channel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.channel@v1"))]
        pub channel: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.aliassource@v1"))]
        pub aliassource: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.Agents`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agents@v1"))]
        pub agents: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.Channels`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.channels@v1"))]
        pub channels: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.KnowledgeBundles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.knowledgebundles@v1"))]
        pub knowledgebundles: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.McpBundles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.mcpbundles@v1"))]
        pub mcpbundles: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.ModelProviders`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.modelproviders@v1"))]
        pub modelproviders: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.RiskProfiles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.riskprofiles@v1"))]
        pub riskprofiles: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.RuntimeProfiles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.runtimeprofiles@v1"))]
        pub runtimeprofiles: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.SkillBundles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.skillbundles@v1"))]
        pub skillbundles: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.TranscriptionProviders`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.transcriptionproviders@v1"))]
        pub transcriptionproviders: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AliasSource.TtsProviders`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.ttsproviders@v1"))]
        pub ttsproviders: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AutonomyLevel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.autonomylevel@v1"))]
        pub autonomylevel: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AutonomyLevel.Full`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.full@v1"))]
        pub full: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AutonomyLevel.ReadOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.readonly@v1"))]
        pub readonly: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.AutonomyLevel.Supervised`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.supervised@v1"))]
        pub supervised: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BudgetCheck`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.budgetcheck@v1"))]
        pub budgetcheck: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BudgetCheck.Allowed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed@v1"))]
        pub allowed: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BudgetCheck.Exceeded`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.exceeded@v1"))]
        pub exceeded: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BudgetCheck.Warning`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.warning@v1"))]
        pub warning: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BundleDirectoryError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.bundledirectoryerror@v1"))]
        pub bundledirectoryerror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BundleDirectoryError.DirectoryCollision`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.directorycollision@v1"))]
        pub directorycollision: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BundleDirectoryError.EscapesShared`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.escapesshared@v1"))]
        pub escapesshared: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.BundleDirectoryError.UnknownBundle`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.unknownbundle@v1"))]
        pub unknownbundle: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadeError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cascadeerror@v1"))]
        pub cascadeerror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadeError.NotFound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.notfound@v1"))]
        pub notfound: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadeError.NotImplemented`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.notimplemented@v1"))]
        pub notimplemented: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadeError.PostCondition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.postcondition@v1"))]
        pub postcondition: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadeError.Refused`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.refused@v1"))]
        pub refused: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadePolicy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cascadepolicy@v1"))]
        pub cascadepolicy: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadePolicy.DryRun`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.dryrun@v1"))]
        pub dryrun: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CascadePolicy.RefuseOnHard`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.refuseonhard@v1"))]
        pub refuseonhard: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CommandRiskLevel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.commandrisklevel@v1"))]
        pub commandrisklevel: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CommandRiskLevel.High`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.high@v1"))]
        pub high: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CommandRiskLevel.Low`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.low@v1"))]
        pub low: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CommandRiskLevel.Medium`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.medium@v1"))]
        pub medium: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.configapicode@v1"))]
        pub configapicode: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.ConfigChangedExternally`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.configchangedexternally@v1"))]
        pub configchangedexternally: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.DanglingReference`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.danglingreference@v1"))]
        pub danglingreference: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.InternalError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.internalerror@v1"))]
        pub internalerror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.InvalidEnumVariant`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.invalidenumvariant@v1"))]
        pub invalidenumvariant: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.InvalidFormat`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.invalidformat@v1"))]
        pub invalidformat: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.InvalidNumericRange`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.invalidnumericrange@v1"))]
        pub invalidnumericrange: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.OpNotSupported`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.opnotsupported@v1"))]
        pub opnotsupported: Option<u64>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.PathNotFound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.pathnotfound@v1"))]
        pub pathnotfound: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.ReloadFailed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.reloadfailed@v1"))]
        pub reloadfailed: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.RequiredFieldEmpty`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.requiredfieldempty@v1"))]
        pub requiredfieldempty: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.SecretTestForbidden`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.secrettestforbidden@v1"))]
        pub secrettestforbidden: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.ValidationFailed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.validationfailed@v1"))]
        pub validationfailed: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigApiCode.ValueTypeMismatch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.valuetypemismatch@v1"))]
        pub valuetypemismatch: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.configtab@v1"))]
        pub configtab: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Advanced`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.advanced@v1"))]
        pub advanced: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Behavior`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.behavior@v1"))]
        pub behavior: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Bundles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.bundles@v1"))]
        pub bundles: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Connection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.connection@v1"))]
        pub connection: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Costs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.costs@v1"))]
        pub costs: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Cron`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cron@v1"))]
        pub cron: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.General`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.general@v1"))]
        pub general: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Limits`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.limits@v1"))]
        pub limits: Option<u64>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Memory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.memory@v1"))]
        pub memory: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.None`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.none@v1"))]
        pub none: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.PeerGroups`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.peergroups@v1"))]
        pub peergroups: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Personality`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.personality@v1"))]
        pub personality: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Servers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.servers@v1"))]
        pub servers: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Settings`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.settings@v1"))]
        pub settings: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Skills`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.skills@v1"))]
        pub skills: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Tuning`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tuning@v1"))]
        pub tuning: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ConfigTab.Workspace`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.workspace@v1"))]
        pub workspace: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CreateError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.createerror@v1"))]
        pub createerror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CreateError.Invalid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.invalid@v1"))]
        pub invalid: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CreateError.Reserved`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.reserved@v1"))]
        pub reserved: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CredentialSurfaceClass`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.credentialsurfaceclass@v1"))]
        pub credentialsurfaceclass: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CredentialSurfaceClass.EncryptedSecret`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.encryptedsecret@v1"))]
        pub encryptedsecret: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CredentialSurfaceClass.ExternalAuthStore`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.externalauthstore@v1"))]
        pub externalauthstore: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CredentialSurfaceClass.LegacyEnvPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.legacyenvpath@v1"))]
        pub legacyenvpath: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CredentialSurfaceClass.PathOnlyReference`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.pathonlyreference@v1"))]
        pub pathonlyreference: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CredentialSurfaceClass.PublicValue`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.publicvalue@v1"))]
        pub publicvalue: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.CredentialSurfaceClass.RequiresFollowUp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.requiresfollowup@v1"))]
        pub requiresfollowup: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.DelegationMode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.delegationmode@v1"))]
        pub delegationmode: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.DelegationMode.Allow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allow@v1"))]
        pub allow: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.DelegationMode.Forbidden`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.forbidden@v1"))]
        pub forbidden: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.DockerWorkspaceMountError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.dockerworkspacemounterror@v1"))]
        pub dockerworkspacemounterror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.DockerWorkspaceMountError.AllowedRoot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowedroot@v1"))]
        pub allowedroot: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.DockerWorkspaceMountError.WorkspacePath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.workspacepath@v1"))]
        pub workspacepath: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.escalationviolation@v1"))]
        pub escalationviolation: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.AutonomyAboveParent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.autonomyaboveparent@v1"))]
        pub autonomyaboveparent: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.BlockHighRiskCommandsDisabledByChild`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.blockhighriskcommandsdisabledbychild@v1"))]
        pub blockhighriskcommandsdisabledbychild: Option<bool>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.CommandNotInParent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.commandnotinparent@v1"))]
        pub commandnotinparent: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.ForbiddenPathDroppedByChild`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.forbiddenpathdroppedbychild@v1"))]
        pub forbiddenpathdroppedbychild: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.MaxActionsExceeded`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.maxactionsexceeded@v1"))]
        pub maxactionsexceeded: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.MaxCostExceeded`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.maxcostexceeded@v1"))]
        pub maxcostexceeded: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.ReadOnlyRootNotInParent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.readonlyrootnotinparent@v1"))]
        pub readonlyrootnotinparent: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.ReadWriteRootNotInParent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.readwriterootnotinparent@v1"))]
        pub readwriterootnotinparent: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.RequireApprovalDisabledByChild`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.requireapprovaldisabledbychild@v1"))]
        pub requireapprovaldisabledbychild: Option<bool>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.ShellEnvPassthroughExpanded`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.shellenvpassthroughexpanded@v1"))]
        pub shellenvpassthroughexpanded: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.ShellTimeoutExceeded`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.shelltimeoutexceeded@v1"))]
        pub shelltimeoutexceeded: Option<u64>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.WorkspaceOnlyDisabledByChild`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.workspaceonlydisabledbychild@v1"))]
        pub workspaceonlydisabledbychild: Option<bool>,

        /// Discovered from Repomix path `enum.tched_router_config.EscalationViolation.WriteOnlyRootNotInParent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.writeonlyrootnotinparent@v1"))]
        pub writeonlyrootnotinparent: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.GeneratePairingCodeError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.generatepairingcodeerror@v1"))]
        pub generatepairingcodeerror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.GeneratePairingCodeError.PairingDisabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.pairingdisabled@v1"))]
        pub pairingdisabled: Option<bool>,

        /// Discovered from Repomix path `enum.tched_router_config.GeneratePairingCodeError.Pending`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.pending@v1"))]
        pub pending: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MapKeyKind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.mapkeykind@v1"))]
        pub mapkeykind: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MapKeyKind.List`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.list@v1"))]
        pub list: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MapKeyKind.Map`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.map@v1"))]
        pub map: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MemoryBackendKind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.memorybackendkind@v1"))]
        pub memorybackendkind: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MemoryBackendKind.Lucid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.lucid@v1"))]
        pub lucid: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MemoryBackendKind.Markdown`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.markdown@v1"))]
        pub markdown: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MemoryBackendKind.Postgres`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.postgres@v1"))]
        pub postgres: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MemoryBackendKind.Qdrant`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.qdrant@v1"))]
        pub qdrant: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.MemoryBackendKind.Sqlite`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.sqlite@v1"))]
        pub sqlite: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.OnNoApprover`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.onnoapprover@v1"))]
        pub onnoapprover: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.OnNoApprover.Deny`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.deny@v1"))]
        pub deny: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.OnNoApprover.InheritOriginator`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.inheritoriginator@v1"))]
        pub inheritoriginator: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.OutputModality`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.outputmodality@v1"))]
        pub outputmodality: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.OutputModality.Mirror`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.mirror@v1"))]
        pub mirror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.OutputModality.Text`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.text@v1"))]
        pub text: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.OutputModality.Voice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.voice@v1"))]
        pub voice: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.propkind@v1"))]
        pub propkind: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.AliasRef`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.aliasref@v1"))]
        pub aliasref: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.Bool`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.bool@v1"))]
        pub bool: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.Enum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.enum-field@v1"))]
        pub enum_field: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.Float`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.float@v1"))]
        pub float: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.Integer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.integer@v1"))]
        pub integer: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.Object`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.object@v1"))]
        pub object: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.ObjectArray`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.objectarray@v1"))]
        pub objectarray: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.String`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.string@v1"))]
        pub string: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.PropKind.StringArray`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.stringarray@v1"))]
        pub stringarray: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ProviderCategory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.providercategory@v1"))]
        pub providercategory: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ProviderCategory.Models`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.models@v1"))]
        pub models: Option<Vec<String>>,

        /// Discovered from Repomix path `enum.tched_router_config.ProviderCategory.Transcription`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.transcription@v1"))]
        pub transcription: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ProviderCategory.Tts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tts@v1"))]
        pub tts: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.QuoteState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.quotestate@v1"))]
        pub quotestate: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.QuoteState.Double`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.double@v1"))]
        pub double: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.QuoteState.Single`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.single@v1"))]
        pub single: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RedirectionArgument`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.redirectionargument@v1"))]
        pub redirectionargument: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RedirectionArgument.FdOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.fdonly@v1"))]
        pub fdonly: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RedirectionArgument.NeedsNextToken`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.needsnexttoken@v1"))]
        pub needsnexttoken: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RedirectionArgument.Target`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.target@v1"))]
        pub target: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RefStrength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.refstrength@v1"))]
        pub refstrength: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RefStrength.Hard`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.hard@v1"))]
        pub hard: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RefStrength.Soft`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.soft@v1"))]
        pub soft: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RenameError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.renameerror@v1"))]
        pub renameerror: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.RenameError.InvalidName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.invalidname@v1"))]
        pub invalidname: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ScrubAction`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.scrubaction@v1"))]
        pub scrubaction: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ScrubAction.ClearOptional`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.clearoptional@v1"))]
        pub clearoptional: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ScrubAction.DropFromVec`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.dropfromvec@v1"))]
        pub dropfromvec: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ScrubAction.Refuse`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.refuse@v1"))]
        pub refuse: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ScrubAction.RemoveMapKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.removemapkey@v1"))]
        pub removemapkey: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.sectiongroup@v1"))]
        pub sectiongroup: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup.Foundation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.foundation@v1"))]
        pub foundation: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup.Integrations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.integrations@v1"))]
        pub integrations: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup.MultiAgent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.multiagent@v1"))]
        pub multiagent: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup.Network`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.network@v1"))]
        pub network: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup.Operations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.operations@v1"))]
        pub operations: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup.Other`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.other@v1"))]
        pub other: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionGroup.Storage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.storage@v1"))]
        pub storage: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionShape`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.sectionshape@v1"))]
        pub sectionshape: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionShape.BackendPicker`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.backendpicker@v1"))]
        pub backendpicker: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionShape.DirectForm`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.directform@v1"))]
        pub directform: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionShape.OneTierAliasMap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.onetieraliasmap@v1"))]
        pub onetieraliasmap: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SectionShape.TypedFamilyMap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.typedfamilymap@v1"))]
        pub typedfamilymap: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SelectorChoice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.selectorchoice@v1"))]
        pub selectorchoice: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SelectorChoice.Existing`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.existing@v1"))]
        pub existing: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.SelectorChoice.Fresh`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.fresh@v1"))]
        pub fresh: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ThinkingLevel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.thinkinglevel@v1"))]
        pub thinkinglevel: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ThinkingLevel.Max`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max@v1"))]
        pub max: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ThinkingLevel.Minimal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.minimal@v1"))]
        pub minimal: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ThinkingLevel.Off`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.off@v1"))]
        pub off: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ToolOperation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tooloperation@v1"))]
        pub tooloperation: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.ToolOperation.Act`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.act@v1"))]
        pub act: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.TranscriptionProviderEntry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.transcriptionproviderentry@v1"))]
        pub transcriptionproviderentry: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.TranscriptionProviderEntry.AssemblyAi`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.assemblyai@v1"))]
        pub assemblyai: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.TranscriptionProviderEntry.Deepgram`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.deepgram@v1"))]
        pub deepgram: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.TranscriptionProviderEntry.Google`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.google@v1"))]
        pub google: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.TranscriptionProviderEntry.Groq`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.groq@v1"))]
        pub groq: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.TranscriptionProviderEntry.LocalWhisper`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.localwhisper@v1"))]
        pub localwhisper: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.TranscriptionProviderEntry.OpenAi`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.openai@v1"))]
        pub openai: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.UsagePeriod`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.usageperiod@v1"))]
        pub usageperiod: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.UsagePeriod.Day`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.day@v1"))]
        pub day: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.UsagePeriod.Month`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.month@v1"))]
        pub month: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.UsagePeriod.Session`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.session@v1"))]
        pub session: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.V2WorkspaceDest`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.v2workspacedest@v1"))]
        pub v2workspacedest: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.V2WorkspaceDest.AgentDefault`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agentdefault@v1"))]
        pub agentdefault: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.V2WorkspaceDest.DataDir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.datadir@v1"))]
        pub datadir: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.V2WorkspaceDest.MemorySubentryDispatch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.memorysubentrydispatch@v1"))]
        pub memorysubentrydispatch: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.V2WorkspaceDest.SharedDir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.shareddir@v1"))]
        pub shareddir: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VecRoute`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.vecroute@v1"))]
        pub vecroute: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VecRoute.Ambiguous`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.ambiguous@v1"))]
        pub ambiguous: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VecRoute.Hit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.hit@v1"))]
        pub hit: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VecRoute.Miss`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.miss@v1"))]
        pub miss: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VoiceProvider`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.voiceprovider@v1"))]
        pub voiceprovider: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VoiceProvider.Plivo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.plivo@v1"))]
        pub plivo: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VoiceProvider.Telnyx`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.telnyx@v1"))]
        pub telnyx: Option<String>,

        /// Discovered from Repomix path `enum.tched_router_config.VoiceProvider.Twilio`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.twilio@v1"))]
        pub twilio: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.A2aServerConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.a2aserverconfig@v1"))]
        pub a2aserverconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.A2aServerConfig.bind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.bind@v1"))]
        pub bind: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.A2aServerConfig.enabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.enabled@v1"))]
        pub enabled: Option<bool>,

        /// Discovered from Repomix path `struct.tched_router_config.A2aServerConfig.port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.port@v1"))]
        pub port: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.A2aServerConfig.public_base_url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.public-base-url@v1"))]
        pub public_base_url: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.A2aServerSection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.a2aserversection@v1"))]
        pub a2aserversection: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.A2aServerSection.server`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.server@v1"))]
        pub server: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ActionTracker`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.actiontracker@v1"))]
        pub actiontracker: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ActionTracker.actions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.actions@v1"))]
        pub actions: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentA2aConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agenta2aconfig@v1"))]
        pub agenta2aconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentA2aConfig.exposed_skills`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.exposed-skills@v1"))]
        pub exposed_skills: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentA2aConfig.published`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.published@v1"))]
        pub published: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agentcoststats@v1"))]
        pub agentcoststats: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats.agent_alias`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agent-alias@v1"))]
        pub agent_alias: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats.cached_input_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cached-input-tokens@v1"))]
        pub cached_input_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats.cost_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cost-usd@v1"))]
        pub cost_usd: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats.input_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.input-tokens@v1"))]
        pub input_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats.output_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.output-tokens@v1"))]
        pub output_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats.request_count`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.request-count@v1"))]
        pub request_count: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentCostStats.total_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.total-tokens@v1"))]
        pub total_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentIdentity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agentidentity@v1"))]
        pub agentidentity: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentIdentity.personality_file`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.personality-file@v1"))]
        pub personality_file: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentIdentity.personality_files`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.personality-files@v1"))]
        pub personality_files: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentIdentity.system_prompt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.system-prompt@v1"))]
        pub system_prompt: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentMemoryConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agentmemoryconfig@v1"))]
        pub agentmemoryconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentMemoryConfig.backend`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.backend@v1"))]
        pub backend: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentTotals`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agenttotals@v1"))]
        pub agenttotals: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentWorkspaceConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.agentworkspaceconfig@v1"))]
        pub agentworkspaceconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentWorkspaceConfig.access`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.access@v1"))]
        pub access: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentWorkspaceConfig.read_memory_from`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.read-memory-from@v1"))]
        pub read_memory_from: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AgentWorkspaceConfig.unrestricted_filesystem`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.unrestricted-filesystem@v1"))]
        pub unrestricted_filesystem: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AliasKind.Channel.channel_type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.channel-type@v1"))]
        pub channel_type: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AliasKind.Provider.category`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.category@v1"))]
        pub category: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AliasKind.Provider.family`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.family@v1"))]
        pub family: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AppliedOverrides`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.appliedoverrides@v1"))]
        pub appliedoverrides: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AppliedOverrides.paths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.paths@v1"))]
        pub paths: Option<Vec<String>>,

        /// Discovered from Repomix path `struct.tched_router_config.AppliedOverrides.snapshots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.snapshots@v1"))]
        pub snapshots: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ApprovalRoute`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.approvalroute@v1"))]
        pub approvalroute: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ApprovalRoute.approver_channel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.approver-channel@v1"))]
        pub approver_channel: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ApprovalRoute.timeout_secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.timeout-secs@v1"))]
        pub timeout_secs: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.AutoClassifyConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.autoclassifyconfig@v1"))]
        pub autoclassifyconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AutoClassifyConfig.complex_hint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.complex-hint@v1"))]
        pub complex_hint: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AutoClassifyConfig.cost_optimized_hint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cost-optimized-hint@v1"))]
        pub cost_optimized_hint: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AutoClassifyConfig.simple_hint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.simple-hint@v1"))]
        pub simple_hint: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.AutoClassifyConfig.standard_hint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.standard-hint@v1"))]
        pub standard_hint: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BrowserDelegateConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.browserdelegateconfig@v1"))]
        pub browserdelegateconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BrowserDelegateConfig.allowed_domains`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed-domains@v1"))]
        pub allowed_domains: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BrowserDelegateConfig.blocked_domains`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.blocked-domains@v1"))]
        pub blocked_domains: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BrowserDelegateConfig.chrome_profile_dir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.chrome-profile-dir@v1"))]
        pub chrome_profile_dir: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BrowserDelegateConfig.cli_binary`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cli-binary@v1"))]
        pub cli_binary: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BrowserDelegateConfig.task_timeout_secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.task-timeout-secs@v1"))]
        pub task_timeout_secs: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.BudgetCheck.Exceeded.current_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.current-usd@v1"))]
        pub current_usd: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BudgetCheck.Exceeded.limit_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.limit-usd@v1"))]
        pub limit_usd: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.BudgetCheck.Exceeded.period`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.period@v1"))]
        pub period: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BuilderSubmission`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.buildersubmission@v1"))]
        pub buildersubmission: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BuilderSubmission.model_provider`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.model-provider@v1"))]
        pub model_provider: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BuilderSubmission.risk_profile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.risk-profile@v1"))]
        pub risk_profile: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BuilderSubmission.runtime_profile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.runtime-profile@v1"))]
        pub runtime_profile: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BundleDirectoryError.DirectoryCollision.first`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.first@v1"))]
        pub first: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BundleDirectoryError.DirectoryCollision.second`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.second@v1"))]
        pub second: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.BundleDirectoryError.EscapesShared.shared`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.shared@v1"))]
        pub shared: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CascadeReport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cascadereport@v1"))]
        pub cascadereport: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.CascadeReport.applied`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.applied@v1"))]
        pub applied: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.CascadeReport.deleted_entry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.deleted-entry@v1"))]
        pub deleted_entry: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.CascadeReport.plan`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.plan@v1"))]
        pub plan: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ChannelInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.channelinfo@v1"))]
        pub channelinfo: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ChannelInfo.configured`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.configured@v1"))]
        pub configured: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ChannelInfo.desc`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.desc@v1"))]
        pub desc: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ChannelPrecheckConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.channelprecheckconfig@v1"))]
        pub channelprecheckconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ChannelQuickStart`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.channelquickstart@v1"))]
        pub channelquickstart: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ChannelQuickStart.alias`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.alias@v1"))]
        pub alias: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ClawdTalkConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.clawdtalkconfig@v1"))]
        pub clawdtalkconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ClawdTalkConfig.allowed_destinations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed-destinations@v1"))]
        pub allowed_destinations: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ClawdTalkConfig.connection_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.connection-id@v1"))]
        pub connection_id: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ClawdTalkConfig.excluded_tools`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.excluded-tools@v1"))]
        pub excluded_tools: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ClawdTalkConfig.from_number`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.from-number@v1"))]
        pub from_number: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ClawdTalkConfig.webhook_secret`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.webhook-secret@v1"))]
        pub webhook_secret: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigApiError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.configapierror@v1"))]
        pub configapierror: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigApiError.code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.code@v1"))]
        pub code: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigApiError.message`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.message@v1"))]
        pub message: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigApiError.op_index`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.op-index@v1"))]
        pub op_index: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.configfieldentry@v1"))]
        pub configfieldentry: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.enum_variants`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.enum-variants@v1"))]
        pub enum_variants: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.is_env_overridden`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.is-env-overridden@v1"))]
        pub is_env_overridden: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.is_secret`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.is-secret@v1"))]
        pub is_secret: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.multiline`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.multiline@v1"))]
        pub multiline: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.populated`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.populated@v1"))]
        pub populated: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.section`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.section@v1"))]
        pub section: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.tab`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tab@v1"))]
        pub tab: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigFieldEntry.type_hint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.type-hint@v1"))]
        pub type_hint: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ConfigLoadAttribution`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.configloadattribution@v1"))]
        pub configloadattribution: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.contextcompressionconfig@v1"))]
        pub contextcompressionconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.identifier_policy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.identifier-policy@v1"))]
        pub identifier_policy: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.max_passes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-passes@v1"))]
        pub max_passes: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.protect_first_n`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.protect-first-n@v1"))]
        pub protect_first_n: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.protect_last_n`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.protect-last-n@v1"))]
        pub protect_last_n: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.source_max_chars`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.source-max-chars@v1"))]
        pub source_max_chars: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.summary_max_chars`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.summary-max-chars@v1"))]
        pub summary_max_chars: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.summary_model`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.summary-model@v1"))]
        pub summary_model: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.summary_provider`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.summary-provider@v1"))]
        pub summary_provider: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.threshold_ratio`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.threshold-ratio@v1"))]
        pub threshold_ratio: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.tool_result_retrim_chars`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tool-result-retrim-chars@v1"))]
        pub tool_result_retrim_chars: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ContextCompressionConfig.tool_result_trim_exempt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tool-result-trim-exempt@v1"))]
        pub tool_result_trim_exempt: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostRecord`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.costrecord@v1"))]
        pub costrecord: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostRecord.session_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.session-id@v1"))]
        pub session_id: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostRecord.task_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.task-id@v1"))]
        pub task_id: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostRecord.usage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.usage@v1"))]
        pub usage: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostStorage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.coststorage@v1"))]
        pub coststorage: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostStorage.aggregates_current`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.aggregates-current@v1"))]
        pub aggregates_current: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostStorage.cached_day`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cached-day@v1"))]
        pub cached_day: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostStorage.cached_month`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cached-month@v1"))]
        pub cached_month: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostStorage.cached_year`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cached-year@v1"))]
        pub cached_year: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostStorage.daily_cost_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.daily-cost-usd@v1"))]
        pub daily_cost_usd: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostStorage.monthly_cost_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.monthly-cost-usd@v1"))]
        pub monthly_cost_usd: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostSummary`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.costsummary@v1"))]
        pub costsummary: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostSummary.by_agent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.by-agent@v1"))]
        pub by_agent: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostSummary.by_model`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.by-model@v1"))]
        pub by_model: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostSummary.session_cost_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.session-cost-usd@v1"))]
        pub session_cost_usd: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostSummaryAccumulator`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.costsummaryaccumulator@v1"))]
        pub costsummaryaccumulator: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostSummaryAccumulator.total_cost`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.total-cost@v1"))]
        pub total_cost: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostTracker`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.costtracker@v1"))]
        pub costtracker: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostTracker.config`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.config@v1"))]
        pub config: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.CostTracker.session_totals`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.session-totals@v1"))]
        pub session_totals: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.DelegationPolicy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.delegationpolicy@v1"))]
        pub delegationpolicy: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.DelegationPolicy.mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.mode@v1"))]
        pub mode: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.DockerRuntime`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.dockerruntime@v1"))]
        pub dockerruntime: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.emailconfig@v1"))]
        pub emailconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.default_subject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.default-subject@v1"))]
        pub default_subject: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.from_address`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.from-address@v1"))]
        pub from_address: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.html_body`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.html-body@v1"))]
        pub html_body: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.idle_timeout_secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.idle-timeout-secs@v1"))]
        pub idle_timeout_secs: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.imap_folder`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.imap-folder@v1"))]
        pub imap_folder: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.imap_host`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.imap-host@v1"))]
        pub imap_host: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.imap_port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.imap-port@v1"))]
        pub imap_port: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.max_attachment_bytes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-attachment-bytes@v1"))]
        pub max_attachment_bytes: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.oauth2`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.oauth2@v1"))]
        pub oauth2: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.observer_mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.observer-mode@v1"))]
        pub observer_mode: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.password`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.password@v1"))]
        pub password: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.poll_interval_secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.poll-interval-secs@v1"))]
        pub poll_interval_secs: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.smtp_host`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.smtp-host@v1"))]
        pub smtp_host: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.smtp_password`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.smtp-password@v1"))]
        pub smtp_password: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.smtp_port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.smtp-port@v1"))]
        pub smtp_port: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.smtp_tls`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.smtp-tls@v1"))]
        pub smtp_tls: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.smtp_username`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.smtp-username@v1"))]
        pub smtp_username: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailConfig.username`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.username@v1"))]
        pub username: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailOAuth2Config`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.emailoauth2config@v1"))]
        pub emailoauth2config: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailOAuth2Config.client_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.client-id@v1"))]
        pub client_id: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailOAuth2Config.device_code_url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.device-code-url@v1"))]
        pub device_code_url: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailOAuth2Config.scopes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.scopes@v1"))]
        pub scopes: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EmailOAuth2Config.token_url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.token-url@v1"))]
        pub token_url: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EscalationViolation.AutonomyAboveParent.child`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.child@v1"))]
        pub child: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EscalationViolation.AutonomyAboveParent.parent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.parent@v1"))]
        pub parent: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EscalationViolation.CommandNotInParent.command`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.command@v1"))]
        pub command: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EscalationViolation.ShellEnvPassthroughExpanded.variable`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.variable@v1"))]
        pub variable: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EvalConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.evalconfig@v1"))]
        pub evalconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EvalConfig.max_retries`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-retries@v1"))]
        pub max_retries: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EvalConfig.min_quality_score`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.min-quality-score@v1"))]
        pub min_quality_score: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EvalHarnessConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.evalharnessconfig@v1"))]
        pub evalharnessconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.EvalHarnessConfig.suite_dir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.suite-dir@v1"))]
        pub suite_dir: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.FailedAttemptState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.failedattemptstate@v1"))]
        pub failedattemptstate: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.FailedAttemptState.count`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.count@v1"))]
        pub count: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.FailedAttemptState.last_attempt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.last-attempt@v1"))]
        pub last_attempt: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.FailedAttemptState.lockout_until`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.lockout-until@v1"))]
        pub lockout_until: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.FilesystemMigrationReport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.filesystemmigrationreport@v1"))]
        pub filesystemmigrationreport: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.FilesystemMigrationReport.backup_dir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.backup-dir@v1"))]
        pub backup_dir: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.FilesystemMigrationReport.entries_relocated`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.entries-relocated@v1"))]
        pub entries_relocated: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.GenerateOptions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.generateoptions@v1"))]
        pub generateoptions: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.GenerateOptions.encrypt_secrets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.encrypt-secrets@v1"))]
        pub encrypt_secrets: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.GenerateOptions.secret_store_dir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.secret-store-dir@v1"))]
        pub secret_store_dir: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.GmailPushConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.gmailpushconfig@v1"))]
        pub gmailpushconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.GmailPushConfig.label_filter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.label-filter@v1"))]
        pub label_filter: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.GmailPushConfig.oauth_token`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.oauth-token@v1"))]
        pub oauth_token: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.GmailPushConfig.topic`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.topic@v1"))]
        pub topic: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.GmailPushConfig.webhook_url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.webhook-url@v1"))]
        pub webhook_url: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.HistoryPrunerConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.historyprunerconfig@v1"))]
        pub historyprunerconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.HistoryPrunerConfig.collapse_tool_results`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.collapse-tool-results@v1"))]
        pub collapse_tool_results: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.HistoryPrunerConfig.keep_recent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.keep-recent@v1"))]
        pub keep_recent: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.HistoryPrunerConfig.max_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-tokens@v1"))]
        pub max_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ImpactReport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.impactreport@v1"))]
        pub impactreport: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ImpactReport.blockers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.blockers@v1"))]
        pub blockers: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ImpactReport.owned_state`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.owned-state@v1"))]
        pub owned_state: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ImpactReport.scrubs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.scrubs@v1"))]
        pub scrubs: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ImpactReport.target_alias`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.target-alias@v1"))]
        pub target_alias: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ImpactReport.target_kind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.target-kind@v1"))]
        pub target_kind: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.IntegrationDescriptor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.integrationdescriptor@v1"))]
        pub integrationdescriptor: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.IntegrationDescriptor.active`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.active@v1"))]
        pub active: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.IntegrationDescriptor.display_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.display-name@v1"))]
        pub display_name: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.MapKeySection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.mapkeysection@v1"))]
        pub mapkeysection: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.MapKeySection.natural_key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.natural-key@v1"))]
        pub natural_key: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.MapKeySection.resource_key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.resource-key@v1"))]
        pub resource_key: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.MapKeySection.value_type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.value-type@v1"))]
        pub value_type: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.MigrateReport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.migratereport@v1"))]
        pub migratereport: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.MigrateReport.backup_path`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.backup-path@v1"))]
        pub backup_path: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.MigrateReport.to_version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.to-version@v1"))]
        pub to_version: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ModelProviderChoice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.modelproviderchoice@v1"))]
        pub modelproviderchoice: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ModelProviderChoice.provider_type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.provider-type@v1"))]
        pub provider_type: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ModelStats`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.modelstats@v1"))]
        pub modelstats: Option<Vec<String>>,

        /// Discovered from Repomix path `struct.tched_router_config.NativeRuntime`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.nativeruntime@v1"))]
        pub nativeruntime: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.NativeRuntime.shell`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.shell@v1"))]
        pub shell: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.NestedOptionEntry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.nestedoptionentry@v1"))]
        pub nestedoptionentry: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.NestedOptionEntry.field`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.field@v1"))]
        pub field: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.NestedOptionEntry.present`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.present@v1"))]
        pub present: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.NormalizedRootlessPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.normalizedrootlesspath@v1"))]
        pub normalizedrootlesspath: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.NormalizedRootlessPath.drive`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.drive@v1"))]
        pub drive: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.OwnedArtifact`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.ownedartifact@v1"))]
        pub ownedartifact: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.OwnedArtifact.action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.OwnedArtifact.locator`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.locator@v1"))]
        pub locator: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.OwnedArtifact.store`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.store@v1"))]
        pub store: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.OwnedArtifact.strength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.strength@v1"))]
        pub strength: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PairingGuard`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.pairingguard@v1"))]
        pub pairingguard: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PairingGuard.failed_attempts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.failed-attempts@v1"))]
        pub failed_attempts: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PairingGuard.paired_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.paired-tokens@v1"))]
        pub paired_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PairingGuard.pairing_code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.pairing-code@v1"))]
        pub pairing_code: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PairingGuard.require_pairing`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.require-pairing@v1"))]
        pub require_pairing: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PeerGroupConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.peergroupconfig@v1"))]
        pub peergroupconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PeerGroupConfig.admin_for_agent_scope`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.admin-for-agent-scope@v1"))]
        pub admin_for_agent_scope: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PeerGroupConfig.external_peers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.external-peers@v1"))]
        pub external_peers: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PeerGroupConfig.ignore`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.ignore@v1"))]
        pub ignore: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PerSenderTracker`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.persendertracker@v1"))]
        pub persendertracker: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PerSenderTracker.buckets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.buckets@v1"))]
        pub buckets: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PropFieldInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.propfieldinfo@v1"))]
        pub propfieldinfo: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PropFieldInfo.credential_class`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.credential-class@v1"))]
        pub credential_class: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PropFieldInfo.derived_from_secret`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.derived-from-secret@v1"))]
        pub derived_from_secret: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.PropFieldInfo.display_value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.display-value@v1"))]
        pub display_value: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.QuickstartPeerGroup`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.quickstartpeergroup@v1"))]
        pub quickstartpeergroup: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.QuickstartPersonalityFile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.quickstartpersonalityfile@v1"))]
        pub quickstartpersonalityfile: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.QuickstartPersonalityFile.content`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.content@v1"))]
        pub content: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.QuickstartPersonalityFile.filename`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.filename@v1"))]
        pub filename: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RedirectionArgument.FdOnly.prefix`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.prefix@v1"))]
        pub prefix: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RefSite`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.refsite@v1"))]
        pub refsite: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RefSite.raw_value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.raw-value@v1"))]
        pub raw_value: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RenameReport`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.renamereport@v1"))]
        pub renamereport: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.RenameReport.dirty_paths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.dirty-paths@v1"))]
        pub dirty_paths: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.RenameReport.new_alias`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.new-alias@v1"))]
        pub new_alias: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.RenameReport.old_alias`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.old-alias@v1"))]
        pub old_alias: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.ResilientLoad`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.resilientload@v1"))]
        pub resilientload: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ResilientLoad.dropped`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.dropped@v1"))]
        pub dropped: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ResilientLoad.dropped_security`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.dropped-security@v1"))]
        pub dropped_security: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RiskPreset`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.riskpreset@v1"))]
        pub riskpreset: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RiskPreset.help`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.help@v1"))]
        pub help: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RiskPreset.label`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.label@v1"))]
        pub label: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RiskPreset.preset_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.preset-name@v1"))]
        pub preset_name: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RiskPreset.values`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.values@v1"))]
        pub values: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RootEscapeError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.rootescapeerror@v1"))]
        pub rootescapeerror: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RootEscapeError.input`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.input@v1"))]
        pub input: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RootEscapeError.root`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.root@v1"))]
        pub root: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.RuntimePreset`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.runtimepreset@v1"))]
        pub runtimepreset: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ScrubAction.DropFromVec.index`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.index@v1"))]
        pub index: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ScrubAction.RemoveMapKey.key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.key@v1"))]
        pub key: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecretFieldInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.secretfieldinfo@v1"))]
        pub secretfieldinfo: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecretFieldInfo.is_set`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.is-set@v1"))]
        pub is_set: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecretStore`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.secretstore@v1"))]
        pub secretstore: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecretStore.key_path`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.key-path@v1"))]
        pub key_path: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.securitypolicy@v1"))]
        pub securitypolicy: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.allowed_commands`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed-commands@v1"))]
        pub allowed_commands: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.allowed_roots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed-roots@v1"))]
        pub allowed_roots: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.allowed_roots_read_only`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed-roots-read-only@v1"))]
        pub allowed_roots_read_only: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.allowed_roots_write_only`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed-roots-write-only@v1"))]
        pub allowed_roots_write_only: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.allowed_tools`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allowed-tools@v1"))]
        pub allowed_tools: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.always_ask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.always-ask@v1"))]
        pub always_ask: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.auto_approve`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.auto-approve@v1"))]
        pub auto_approve: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.autonomy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.autonomy@v1"))]
        pub autonomy: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.block_high_risk_commands`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.block-high-risk-commands@v1"))]
        pub block_high_risk_commands: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.config_path`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.config-path@v1"))]
        pub config_path: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.firejail_args`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.firejail-args@v1"))]
        pub firejail_args: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.forbidden_paths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.forbidden-paths@v1"))]
        pub forbidden_paths: Option<Vec<String>>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.max_actions_per_hour`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-actions-per-hour@v1"))]
        pub max_actions_per_hour: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.max_cost_per_day_cents`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-cost-per-day-cents@v1"))]
        pub max_cost_per_day_cents: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.require_approval_for_medium_risk`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.require-approval-for-medium-risk@v1"))]
        pub require_approval_for_medium_risk: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.risk_profile_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.risk-profile-name@v1"))]
        pub risk_profile_name: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.sandbox_backend`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.sandbox-backend@v1"))]
        pub sandbox_backend: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.sandbox_enabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.sandbox-enabled@v1"))]
        pub sandbox_enabled: Option<bool>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.shell_env_passthrough`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.shell-env-passthrough@v1"))]
        pub shell_env_passthrough: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.shell_timeout_secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.shell-timeout-secs@v1"))]
        pub shell_timeout_secs: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.tracker`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tracker@v1"))]
        pub tracker: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.workspace_dir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.workspace-dir@v1"))]
        pub workspace_dir: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.SecurityPolicy.workspace_only`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.workspace-only@v1"))]
        pub workspace_only: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ThinkingConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.thinkingconfig@v1"))]
        pub thinkingconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ThinkingConfig.budget_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.budget-tokens@v1"))]
        pub budget_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ThinkingConfig.default_level`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.default-level@v1"))]
        pub default_level: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ThinkingConfig.native_thinking`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.native-thinking@v1"))]
        pub native_thinking: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TokenUsage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tokenusage@v1"))]
        pub tokenusage: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TokenUsage.pricing_available`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.pricing-available@v1"))]
        pub pricing_available: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TokenUsage.timestamp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.timestamp@v1"))]
        pub timestamp: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TrustConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.trustconfig@v1"))]
        pub trustconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TrustConfig.correction_penalty`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.correction-penalty@v1"))]
        pub correction_penalty: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TrustConfig.decay_half_life_days`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.decay-half-life-days@v1"))]
        pub decay_half_life_days: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TrustConfig.initial_score`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.initial-score@v1"))]
        pub initial_score: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TrustConfig.regression_threshold`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.regression-threshold@v1"))]
        pub regression_threshold: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TrustConfig.success_boost`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.success-boost@v1"))]
        pub success_boost: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TtsProviders.edge`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.edge@v1"))]
        pub edge: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TtsProviders.elevenlabs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.elevenlabs@v1"))]
        pub elevenlabs: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.TtsProviders.piper`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.piper@v1"))]
        pub piper: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.v1config@v1"))]
        pub v1config: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.api_path`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.api-path@v1"))]
        pub api_path: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.api_url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.api-url@v1"))]
        pub api_url: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.channels_config`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.channels-config@v1"))]
        pub channels_config: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.default_model`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.default-model@v1"))]
        pub default_model: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.default_provider`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.default-provider@v1"))]
        pub default_provider: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.default_temperature`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.default-temperature@v1"))]
        pub default_temperature: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.embedding_routes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.embedding-routes@v1"))]
        pub embedding_routes: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.extra_headers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.extra-headers@v1"))]
        pub extra_headers: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.passthrough`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.passthrough@v1"))]
        pub passthrough: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.provider_max_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.provider-max-tokens@v1"))]
        pub provider_max_tokens: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V1Config.provider_timeout_secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.provider-timeout-secs@v1"))]
        pub provider_timeout_secs: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.V2Config`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.v2config@v1"))]
        pub v2config: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V2Config.cost`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.cost@v1"))]
        pub cost: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V2Config.schema_version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.schema-version@v1"))]
        pub schema_version: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.V2Config.swarms`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.swarms@v1"))]
        pub swarms: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.ValidationWarning`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.validationwarning@v1"))]
        pub validationwarning: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VecRoute.Hit.inner_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.inner-name@v1"))]
        pub inner_name: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.voicecallconfig@v1"))]
        pub voicecallconfig: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.account_id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.account-id@v1"))]
        pub account_id: Option<u64>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.auth_token`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.auth-token@v1"))]
        pub auth_token: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.max_call_duration_secs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-call-duration-secs@v1"))]
        pub max_call_duration_secs: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.require_outbound_approval`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.require-outbound-approval@v1"))]
        pub require_outbound_approval: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.transcription_logging`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.transcription-logging@v1"))]
        pub transcription_logging: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.tts_voice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tts-voice@v1"))]
        pub tts_voice: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.webhook_base_url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.webhook-base-url@v1"))]
        pub webhook_base_url: Option<String>,

        /// Discovered from Repomix path `struct.tched_router_config.VoiceCallConfig.webhook_port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.webhook-port@v1"))]
        pub webhook_port: Option<u64>,

        /// Discovered from Repomix path `toml.dev.config.harness-test`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.harness-test@v1"))]
        pub harness_test: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.agents.default`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.default@v1"))]
        pub default: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.gateway`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.gateway@v1"))]
        pub gateway: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.gateway.allow_public_bind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allow-public-bind@v1"))]
        pub allow_public_bind: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.gateway.host`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.host@v1"))]
        pub host: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.memory.archive_after_days`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.archive-after-days@v1"))]
        pub archive_after_days: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.memory.auto_save`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.auto-save@v1"))]
        pub auto_save: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.memory.embedding_provider`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.embedding-provider@v1"))]
        pub embedding_provider: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.memory.hygiene_enabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.hygiene-enabled@v1"))]
        pub hygiene_enabled: Option<bool>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.memory.purge_after_days`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.purge-after-days@v1"))]
        pub purge_after_days: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.providers.models.ollama`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.ollama@v1"))]
        pub ollama: Option<Vec<String>>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.providers.models.ollama.default.temperature`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.temperature@v1"))]
        pub temperature: Option<Vec<String>>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.providers.models.ollama.default.uri`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.uri@v1"))]
        pub uri: Option<Vec<String>>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.risk_profiles.default.level`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.level@v1"))]
        pub level: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.runtime_profiles.default.context_compression`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.context-compression@v1"))]
        pub context_compression: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.runtime_profiles.default.max_context_tokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-context-tokens@v1"))]
        pub max_context_tokens: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.runtime_profiles.default.max_tool_iterations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-tool-iterations@v1"))]
        pub max_tool_iterations: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.harness-test.runtime_profiles.default.max_tool_result_chars`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.max-tool-result-chars@v1"))]
        pub max_tool_result_chars: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.template`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.template@v1"))]
        pub template: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.template.cost.allow_override`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.allow-override@v1"))]
        pub allow_override: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.template.cost.daily_limit_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.daily-limit-usd@v1"))]
        pub daily_limit_usd: Option<u64>,

        /// Discovered from Repomix path `toml.dev.config.template.cost.monthly_limit_usd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.monthly-limit-usd@v1"))]
        pub monthly_limit_usd: Option<u64>,

        /// Discovered from Repomix path `toml.dev.config.template.cost.warn_at_percent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.warn-at-percent@v1"))]
        pub warn_at_percent: Option<String>,

        /// Discovered from Repomix path `toml.dev.config.template.gateway.web_dist_dir`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.web-dist-dir@v1"))]
        pub web_dist_dir: Option<String>,

        /// Discovered from Repomix path `toml.scripts.rpi-config.storage.provider.config.table`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.table@v1"))]
        pub table: Option<String>,

        /// Discovered from Repomix path `toml.scripts.rpi-config.storage.provider.config.tls`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.tched-router.tls@v1"))]
        pub tls: Option<String>,
    }

    /// Metadata needed when promoting a generated typed method into `schema.methods`.
    pub struct MethodCandidate {
        pub name: &'static str,
        pub side_effect: &'static str,
        pub idempotent: bool,
        pub required_capability: &'static str,
        pub subid: &'static str,
        pub repomix_path: &'static str,
    }

    /// Typed input candidate for `agents_create` discovered at `enum.tched-router.AgentsCommands.Create`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AgentsCreateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AgentsCreateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `agents_delete` discovered at `enum.tched-router.AgentsCommands.Delete`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AgentsDeleteInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AgentsDeleteOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `agents_list` discovered at `enum.tched-router.AgentsCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AgentsListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AgentsListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `agents_rename` discovered at `enum.tched-router.AgentsCommands.Rename`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AgentsRenameInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AgentsRenameOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_emaillogin` discovered at `enum.tched-router.AuthCommands.EmailLogin`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthEmailloginInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthEmailloginOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_list` discovered at `enum.tched-router.AuthCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_login` discovered at `enum.tched-router.AuthCommands.Login`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthLoginInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthLoginOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_logout` discovered at `enum.tched-router.AuthCommands.Logout`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthLogoutInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthLogoutOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_pasteredirect` discovered at `enum.tched-router.AuthCommands.PasteRedirect`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthPasteredirectInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthPasteredirectOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_pastetoken` discovered at `enum.tched-router.AuthCommands.PasteToken`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthPastetokenInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthPastetokenOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_refresh` discovered at `enum.tched-router.AuthCommands.Refresh`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthRefreshInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthRefreshOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_setuptoken` discovered at `enum.tched-router.AuthCommands.SetupToken`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthSetuptokenInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthSetuptokenOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `auth_use` discovered at `enum.tched-router.AuthCommands.Use`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AuthUseInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AuthUseOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channel_add` discovered at `enum.tched-router.ChannelCommands.Add`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelAddInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelAddOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channel_bindtelegram` discovered at `enum.tched-router.ChannelCommands.BindTelegram`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelBindtelegramInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelBindtelegramOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channel_doctor` discovered at `enum.tched-router.ChannelCommands.Doctor`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelDoctorInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelDoctorOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channel_list` discovered at `enum.tched-router.ChannelCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channel_remove` discovered at `enum.tched-router.ChannelCommands.Remove`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelRemoveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelRemoveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channel_send` discovered at `enum.tched-router.ChannelCommands.Send`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelSendInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelSendOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channel_start` discovered at `enum.tched-router.ChannelCommands.Start`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelStartInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelStartOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channels_create` discovered at `enum.tched-router.ChannelsCommands.Create`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelsCreateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelsCreateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channels_delete` discovered at `enum.tched-router.ChannelsCommands.Delete`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelsDeleteInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelsDeleteOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channels_list` discovered at `enum.tched-router.ChannelsCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelsListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelsListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `channels_rename` discovered at `enum.tched-router.ChannelsCommands.Rename`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ChannelsRenameInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ChannelsRenameOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_complete` discovered at `enum.tched-router.ConfigCommands.Complete`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigCompleteInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigCompleteOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_docs` discovered at `enum.tched-router.ConfigCommands.Docs`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigDocsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigDocsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_generate` discovered at `enum.tched-router.ConfigCommands.Generate`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigGenerateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigGenerateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_get` discovered at `enum.tched-router.ConfigCommands.Get`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigGetInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigGetOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_init` discovered at `enum.tched-router.ConfigCommands.Init`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigInitInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigInitOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_list` discovered at `enum.tched-router.ConfigCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_migrate` discovered at `enum.tched-router.ConfigCommands.Migrate`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigMigrateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigMigrateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_patch` discovered at `enum.tched-router.ConfigCommands.Patch`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigPatchInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigPatchOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `config_set` discovered at `enum.tched-router.ConfigCommands.Set`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConfigSetInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigSetOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_add` discovered at `enum.tched-router.CronCommands.Add`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronAddInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronAddOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_addat` discovered at `enum.tched-router.CronCommands.AddAt`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronAddatInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronAddatOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_addevery` discovered at `enum.tched-router.CronCommands.AddEvery`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronAddeveryInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronAddeveryOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_list` discovered at `enum.tched-router.CronCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_once` discovered at `enum.tched-router.CronCommands.Once`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronOnceInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronOnceOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_pause` discovered at `enum.tched-router.CronCommands.Pause`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronPauseInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronPauseOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_remove` discovered at `enum.tched-router.CronCommands.Remove`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronRemoveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronRemoveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_resume` discovered at `enum.tched-router.CronCommands.Resume`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronResumeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronResumeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `cron_update` discovered at `enum.tched-router.CronCommands.Update`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CronUpdateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CronUpdateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `deprecatedprops_any` discovered at `enum.tched-router.DeprecatedPropsCommands.Any`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DeprecatedpropsAnyInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DeprecatedpropsAnyOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `doctor_models` discovered at `enum.tched-router.DoctorCommands.Models`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DoctorModelsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DoctorModelsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `doctor_traces` discovered at `enum.tched-router.DoctorCommands.Traces`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DoctorTracesInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DoctorTracesOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `doctor_updatecontextwindows` discovered at `enum.tched-router.DoctorCommands.UpdateContextWindows`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DoctorUpdatecontextwindowsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DoctorUpdatecontextwindowsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `eval_run` discovered at `enum.tched-router.EvalCommands.Run`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EvalRunInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EvalRunOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `gateway_getpaircode` discovered at `enum.tched-router.GatewayCommands.GetPaircode`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GatewayGetpaircodeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GatewayGetpaircodeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `gateway_restart` discovered at `enum.tched-router.GatewayCommands.Restart`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GatewayRestartInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GatewayRestartOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `gateway_start` discovered at `enum.tched-router.GatewayCommands.Start`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GatewayStartInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GatewayStartOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `hardware_discover` discovered at `enum.tched-router.HardwareCommands.Discover`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct HardwareDiscoverInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct HardwareDiscoverOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `hardware_info` discovered at `enum.tched-router.HardwareCommands.Info`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct HardwareInfoInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct HardwareInfoOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `hardware_introspect` discovered at `enum.tched-router.HardwareCommands.Introspect`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct HardwareIntrospectInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct HardwareIntrospectOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `integration_info` discovered at `enum.tched-router.IntegrationCommands.Info`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IntegrationInfoInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IntegrationInfoOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `memory_clear` discovered at `enum.tched-router.MemoryCommands.Clear`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MemoryClearInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MemoryClearOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `memory_get` discovered at `enum.tched-router.MemoryCommands.Get`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MemoryGetInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MemoryGetOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `memory_list` discovered at `enum.tched-router.MemoryCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MemoryListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MemoryListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `memory_reindex` discovered at `enum.tched-router.MemoryCommands.Reindex`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MemoryReindexInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MemoryReindexOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `memory_stats` discovered at `enum.tched-router.MemoryCommands.Stats`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MemoryStatsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MemoryStatsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `migrate_openclaw` discovered at `enum.tched-router.MigrateCommands.Openclaw`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MigrateOpenclawInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MigrateOpenclawOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `model_list` discovered at `enum.tched-router.ModelCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ModelListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ModelListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `model_refresh` discovered at `enum.tched-router.ModelCommands.Refresh`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ModelRefreshInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ModelRefreshOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `peripheral_add` discovered at `enum.tched-router.PeripheralCommands.Add`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PeripheralAddInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PeripheralAddOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `peripheral_flash` discovered at `enum.tched-router.PeripheralCommands.Flash`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PeripheralFlashInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PeripheralFlashOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `peripheral_flashnucleo` discovered at `enum.tched-router.PeripheralCommands.FlashNucleo`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PeripheralFlashnucleoInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PeripheralFlashnucleoOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `peripheral_list` discovered at `enum.tched-router.PeripheralCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PeripheralListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PeripheralListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `peripheral_setupunoq` discovered at `enum.tched-router.PeripheralCommands.SetupUnoQ`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PeripheralSetupunoqInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PeripheralSetupunoqOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `plugin_info` discovered at `enum.tched-router.PluginCommands.Info`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PluginInfoInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PluginInfoOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `plugin_install` discovered at `enum.tched-router.PluginCommands.Install`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PluginInstallInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PluginInstallOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `plugin_list` discovered at `enum.tched-router.PluginCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PluginListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PluginListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `plugin_migrate` discovered at `enum.tched-router.PluginCommands.Migrate`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PluginMigrateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PluginMigrateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `plugin_remove` discovered at `enum.tched-router.PluginCommands.Remove`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PluginRemoveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PluginRemoveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `plugin_search` discovered at `enum.tched-router.PluginCommands.Search`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PluginSearchInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PluginSearchOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `providers_create` discovered at `enum.tched-router.ProvidersCommands.Create`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ProvidersCreateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ProvidersCreateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `providers_delete` discovered at `enum.tched-router.ProvidersCommands.Delete`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ProvidersDeleteInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ProvidersDeleteOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `providers_rename` discovered at `enum.tched-router.ProvidersCommands.Rename`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ProvidersRenameInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ProvidersRenameOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `service_install` discovered at `enum.tched-router.ServiceCommands.Install`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ServiceInstallInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ServiceInstallOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `service_logs` discovered at `enum.tched-router.ServiceCommands.Logs`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ServiceLogsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ServiceLogsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `service_restart` discovered at `enum.tched-router.ServiceCommands.Restart`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ServiceRestartInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ServiceRestartOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `service_start` discovered at `enum.tched-router.ServiceCommands.Start`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ServiceStartInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ServiceStartOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `service_stop` discovered at `enum.tched-router.ServiceCommands.Stop`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ServiceStopInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ServiceStopOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `service_uninstall` discovered at `enum.tched-router.ServiceCommands.Uninstall`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ServiceUninstallInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ServiceUninstallOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skillbundle_add` discovered at `enum.tched-router.SkillBundleCommands.Add`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillbundleAddInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillbundleAddOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skillbundle_list` discovered at `enum.tched-router.SkillBundleCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillbundleListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillbundleListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skillbundle_remove` discovered at `enum.tched-router.SkillBundleCommands.Remove`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillbundleRemoveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillbundleRemoveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skillbundle_rename` discovered at `enum.tched-router.SkillBundleCommands.Rename`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillbundleRenameInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillbundleRenameOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skillbundle_show` discovered at `enum.tched-router.SkillBundleCommands.Show`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillbundleShowInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillbundleShowOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_add` discovered at `enum.tched-router.SkillCommands.Add`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillAddInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillAddOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_audit` discovered at `enum.tched-router.SkillCommands.Audit`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillAuditInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillAuditOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_bundle` discovered at `enum.tched-router.SkillCommands.Bundle`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillBundleInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillBundleOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_edit` discovered at `enum.tched-router.SkillCommands.Edit`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillEditInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillEditOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_install` discovered at `enum.tched-router.SkillCommands.Install`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillInstallInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillInstallOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_list` discovered at `enum.tched-router.SkillCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_remove` discovered at `enum.tched-router.SkillCommands.Remove`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillRemoveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillRemoveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `skill_test` discovered at `enum.tched-router.SkillCommands.Test`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SkillTestInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SkillTestOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_approve` discovered at `enum.tched-router.SopCommands.Approve`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopApproveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopApproveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_delete` discovered at `enum.tched-router.SopCommands.Delete`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopDeleteInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopDeleteOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_deny` discovered at `enum.tched-router.SopCommands.Deny`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopDenyInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopDenyOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_graph` discovered at `enum.tched-router.SopCommands.Graph`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopGraphInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopGraphOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_list` discovered at `enum.tched-router.SopCommands.List`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopListInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopListOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_pending` discovered at `enum.tched-router.SopCommands.Pending`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopPendingInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopPendingOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_show` discovered at `enum.tched-router.SopCommands.Show`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopShowInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopShowOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `sop_validate` discovered at `enum.tched-router.SopCommands.Validate`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SopValidateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SopValidateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[
        MethodCandidate {
            name: "agents_create",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.agents-create@v1",
            repomix_path: "enum.tched-router.AgentsCommands.Create",
        },
        MethodCandidate {
            name: "agents_delete",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.agents-delete@v1",
            repomix_path: "enum.tched-router.AgentsCommands.Delete",
        },
        MethodCandidate {
            name: "agents_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.agents-list@v1",
            repomix_path: "enum.tched-router.AgentsCommands.List",
        },
        MethodCandidate {
            name: "agents_rename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.agents-rename@v1",
            repomix_path: "enum.tched-router.AgentsCommands.Rename",
        },
        MethodCandidate {
            name: "auth_emaillogin",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-emaillogin@v1",
            repomix_path: "enum.tched-router.AuthCommands.EmailLogin",
        },
        MethodCandidate {
            name: "auth_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.auth-list@v1",
            repomix_path: "enum.tched-router.AuthCommands.List",
        },
        MethodCandidate {
            name: "auth_login",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-login@v1",
            repomix_path: "enum.tched-router.AuthCommands.Login",
        },
        MethodCandidate {
            name: "auth_logout",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-logout@v1",
            repomix_path: "enum.tched-router.AuthCommands.Logout",
        },
        MethodCandidate {
            name: "auth_pasteredirect",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-pasteredirect@v1",
            repomix_path: "enum.tched-router.AuthCommands.PasteRedirect",
        },
        MethodCandidate {
            name: "auth_pastetoken",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-pastetoken@v1",
            repomix_path: "enum.tched-router.AuthCommands.PasteToken",
        },
        MethodCandidate {
            name: "auth_refresh",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-refresh@v1",
            repomix_path: "enum.tched-router.AuthCommands.Refresh",
        },
        MethodCandidate {
            name: "auth_setuptoken",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-setuptoken@v1",
            repomix_path: "enum.tched-router.AuthCommands.SetupToken",
        },
        MethodCandidate {
            name: "auth_use",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.auth-use@v1",
            repomix_path: "enum.tched-router.AuthCommands.Use",
        },
        MethodCandidate {
            name: "channel_add",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channel-add@v1",
            repomix_path: "enum.tched-router.ChannelCommands.Add",
        },
        MethodCandidate {
            name: "channel_bindtelegram",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channel-bindtelegram@v1",
            repomix_path: "enum.tched-router.ChannelCommands.BindTelegram",
        },
        MethodCandidate {
            name: "channel_doctor",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channel-doctor@v1",
            repomix_path: "enum.tched-router.ChannelCommands.Doctor",
        },
        MethodCandidate {
            name: "channel_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.channel-list@v1",
            repomix_path: "enum.tched-router.ChannelCommands.List",
        },
        MethodCandidate {
            name: "channel_remove",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channel-remove@v1",
            repomix_path: "enum.tched-router.ChannelCommands.Remove",
        },
        MethodCandidate {
            name: "channel_send",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channel-send@v1",
            repomix_path: "enum.tched-router.ChannelCommands.Send",
        },
        MethodCandidate {
            name: "channel_start",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channel-start@v1",
            repomix_path: "enum.tched-router.ChannelCommands.Start",
        },
        MethodCandidate {
            name: "channels_create",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channels-create@v1",
            repomix_path: "enum.tched-router.ChannelsCommands.Create",
        },
        MethodCandidate {
            name: "channels_delete",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channels-delete@v1",
            repomix_path: "enum.tched-router.ChannelsCommands.Delete",
        },
        MethodCandidate {
            name: "channels_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.channels-list@v1",
            repomix_path: "enum.tched-router.ChannelsCommands.List",
        },
        MethodCandidate {
            name: "channels_rename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.channels-rename@v1",
            repomix_path: "enum.tched-router.ChannelsCommands.Rename",
        },
        MethodCandidate {
            name: "config_complete",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.config-complete@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Complete",
        },
        MethodCandidate {
            name: "config_docs",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.config-docs@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Docs",
        },
        MethodCandidate {
            name: "config_generate",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.config-generate@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Generate",
        },
        MethodCandidate {
            name: "config_get",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.config-get@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Get",
        },
        MethodCandidate {
            name: "config_init",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.config-init@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Init",
        },
        MethodCandidate {
            name: "config_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.config-list@v1",
            repomix_path: "enum.tched-router.ConfigCommands.List",
        },
        MethodCandidate {
            name: "config_migrate",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.config-migrate@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Migrate",
        },
        MethodCandidate {
            name: "config_patch",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.config-patch@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Patch",
        },
        MethodCandidate {
            name: "config_set",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.config-set@v1",
            repomix_path: "enum.tched-router.ConfigCommands.Set",
        },
        MethodCandidate {
            name: "cron_add",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-add@v1",
            repomix_path: "enum.tched-router.CronCommands.Add",
        },
        MethodCandidate {
            name: "cron_addat",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-addat@v1",
            repomix_path: "enum.tched-router.CronCommands.AddAt",
        },
        MethodCandidate {
            name: "cron_addevery",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-addevery@v1",
            repomix_path: "enum.tched-router.CronCommands.AddEvery",
        },
        MethodCandidate {
            name: "cron_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.cron-list@v1",
            repomix_path: "enum.tched-router.CronCommands.List",
        },
        MethodCandidate {
            name: "cron_once",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-once@v1",
            repomix_path: "enum.tched-router.CronCommands.Once",
        },
        MethodCandidate {
            name: "cron_pause",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-pause@v1",
            repomix_path: "enum.tched-router.CronCommands.Pause",
        },
        MethodCandidate {
            name: "cron_remove",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-remove@v1",
            repomix_path: "enum.tched-router.CronCommands.Remove",
        },
        MethodCandidate {
            name: "cron_resume",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-resume@v1",
            repomix_path: "enum.tched-router.CronCommands.Resume",
        },
        MethodCandidate {
            name: "cron_update",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.cron-update@v1",
            repomix_path: "enum.tched-router.CronCommands.Update",
        },
        MethodCandidate {
            name: "deprecatedprops_any",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.deprecatedprops-any@v1",
            repomix_path: "enum.tched-router.DeprecatedPropsCommands.Any",
        },
        MethodCandidate {
            name: "doctor_models",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.doctor-models@v1",
            repomix_path: "enum.tched-router.DoctorCommands.Models",
        },
        MethodCandidate {
            name: "doctor_traces",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.doctor-traces@v1",
            repomix_path: "enum.tched-router.DoctorCommands.Traces",
        },
        MethodCandidate {
            name: "doctor_updatecontextwindows",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.doctor-updatecontextwindows@v1",
            repomix_path: "enum.tched-router.DoctorCommands.UpdateContextWindows",
        },
        MethodCandidate {
            name: "eval_run",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.eval-run@v1",
            repomix_path: "enum.tched-router.EvalCommands.Run",
        },
        MethodCandidate {
            name: "gateway_getpaircode",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.gateway-getpaircode@v1",
            repomix_path: "enum.tched-router.GatewayCommands.GetPaircode",
        },
        MethodCandidate {
            name: "gateway_restart",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.gateway-restart@v1",
            repomix_path: "enum.tched-router.GatewayCommands.Restart",
        },
        MethodCandidate {
            name: "gateway_start",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.gateway-start@v1",
            repomix_path: "enum.tched-router.GatewayCommands.Start",
        },
        MethodCandidate {
            name: "hardware_discover",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.hardware-discover@v1",
            repomix_path: "enum.tched-router.HardwareCommands.Discover",
        },
        MethodCandidate {
            name: "hardware_info",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.hardware-info@v1",
            repomix_path: "enum.tched-router.HardwareCommands.Info",
        },
        MethodCandidate {
            name: "hardware_introspect",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.hardware-introspect@v1",
            repomix_path: "enum.tched-router.HardwareCommands.Introspect",
        },
        MethodCandidate {
            name: "integration_info",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.integration-info@v1",
            repomix_path: "enum.tched-router.IntegrationCommands.Info",
        },
        MethodCandidate {
            name: "memory_clear",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.memory-clear@v1",
            repomix_path: "enum.tched-router.MemoryCommands.Clear",
        },
        MethodCandidate {
            name: "memory_get",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.memory-get@v1",
            repomix_path: "enum.tched-router.MemoryCommands.Get",
        },
        MethodCandidate {
            name: "memory_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.memory-list@v1",
            repomix_path: "enum.tched-router.MemoryCommands.List",
        },
        MethodCandidate {
            name: "memory_reindex",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.memory-reindex@v1",
            repomix_path: "enum.tched-router.MemoryCommands.Reindex",
        },
        MethodCandidate {
            name: "memory_stats",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.memory-stats@v1",
            repomix_path: "enum.tched-router.MemoryCommands.Stats",
        },
        MethodCandidate {
            name: "migrate_openclaw",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.migrate-openclaw@v1",
            repomix_path: "enum.tched-router.MigrateCommands.Openclaw",
        },
        MethodCandidate {
            name: "model_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.model-list@v1",
            repomix_path: "enum.tched-router.ModelCommands.List",
        },
        MethodCandidate {
            name: "model_refresh",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.model-refresh@v1",
            repomix_path: "enum.tched-router.ModelCommands.Refresh",
        },
        MethodCandidate {
            name: "peripheral_add",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.peripheral-add@v1",
            repomix_path: "enum.tched-router.PeripheralCommands.Add",
        },
        MethodCandidate {
            name: "peripheral_flash",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.peripheral-flash@v1",
            repomix_path: "enum.tched-router.PeripheralCommands.Flash",
        },
        MethodCandidate {
            name: "peripheral_flashnucleo",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.peripheral-flashnucleo@v1",
            repomix_path: "enum.tched-router.PeripheralCommands.FlashNucleo",
        },
        MethodCandidate {
            name: "peripheral_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.peripheral-list@v1",
            repomix_path: "enum.tched-router.PeripheralCommands.List",
        },
        MethodCandidate {
            name: "peripheral_setupunoq",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.peripheral-setupunoq@v1",
            repomix_path: "enum.tched-router.PeripheralCommands.SetupUnoQ",
        },
        MethodCandidate {
            name: "plugin_info",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.plugin-info@v1",
            repomix_path: "enum.tched-router.PluginCommands.Info",
        },
        MethodCandidate {
            name: "plugin_install",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.plugin-install@v1",
            repomix_path: "enum.tched-router.PluginCommands.Install",
        },
        MethodCandidate {
            name: "plugin_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.plugin-list@v1",
            repomix_path: "enum.tched-router.PluginCommands.List",
        },
        MethodCandidate {
            name: "plugin_migrate",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.plugin-migrate@v1",
            repomix_path: "enum.tched-router.PluginCommands.Migrate",
        },
        MethodCandidate {
            name: "plugin_remove",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.plugin-remove@v1",
            repomix_path: "enum.tched-router.PluginCommands.Remove",
        },
        MethodCandidate {
            name: "plugin_search",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.plugin-search@v1",
            repomix_path: "enum.tched-router.PluginCommands.Search",
        },
        MethodCandidate {
            name: "providers_create",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.providers-create@v1",
            repomix_path: "enum.tched-router.ProvidersCommands.Create",
        },
        MethodCandidate {
            name: "providers_delete",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.providers-delete@v1",
            repomix_path: "enum.tched-router.ProvidersCommands.Delete",
        },
        MethodCandidate {
            name: "providers_rename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.providers-rename@v1",
            repomix_path: "enum.tched-router.ProvidersCommands.Rename",
        },
        MethodCandidate {
            name: "service_install",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.service-install@v1",
            repomix_path: "enum.tched-router.ServiceCommands.Install",
        },
        MethodCandidate {
            name: "service_logs",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.service-logs@v1",
            repomix_path: "enum.tched-router.ServiceCommands.Logs",
        },
        MethodCandidate {
            name: "service_restart",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.service-restart@v1",
            repomix_path: "enum.tched-router.ServiceCommands.Restart",
        },
        MethodCandidate {
            name: "service_start",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.service-start@v1",
            repomix_path: "enum.tched-router.ServiceCommands.Start",
        },
        MethodCandidate {
            name: "service_stop",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.service-stop@v1",
            repomix_path: "enum.tched-router.ServiceCommands.Stop",
        },
        MethodCandidate {
            name: "service_uninstall",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.service-uninstall@v1",
            repomix_path: "enum.tched-router.ServiceCommands.Uninstall",
        },
        MethodCandidate {
            name: "skillbundle_add",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skillbundle-add@v1",
            repomix_path: "enum.tched-router.SkillBundleCommands.Add",
        },
        MethodCandidate {
            name: "skillbundle_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.skillbundle-list@v1",
            repomix_path: "enum.tched-router.SkillBundleCommands.List",
        },
        MethodCandidate {
            name: "skillbundle_remove",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skillbundle-remove@v1",
            repomix_path: "enum.tched-router.SkillBundleCommands.Remove",
        },
        MethodCandidate {
            name: "skillbundle_rename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skillbundle-rename@v1",
            repomix_path: "enum.tched-router.SkillBundleCommands.Rename",
        },
        MethodCandidate {
            name: "skillbundle_show",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.skillbundle-show@v1",
            repomix_path: "enum.tched-router.SkillBundleCommands.Show",
        },
        MethodCandidate {
            name: "skill_add",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skill-add@v1",
            repomix_path: "enum.tched-router.SkillCommands.Add",
        },
        MethodCandidate {
            name: "skill_audit",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skill-audit@v1",
            repomix_path: "enum.tched-router.SkillCommands.Audit",
        },
        MethodCandidate {
            name: "skill_bundle",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skill-bundle@v1",
            repomix_path: "enum.tched-router.SkillCommands.Bundle",
        },
        MethodCandidate {
            name: "skill_edit",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skill-edit@v1",
            repomix_path: "enum.tched-router.SkillCommands.Edit",
        },
        MethodCandidate {
            name: "skill_install",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skill-install@v1",
            repomix_path: "enum.tched-router.SkillCommands.Install",
        },
        MethodCandidate {
            name: "skill_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.skill-list@v1",
            repomix_path: "enum.tched-router.SkillCommands.List",
        },
        MethodCandidate {
            name: "skill_remove",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skill-remove@v1",
            repomix_path: "enum.tched-router.SkillCommands.Remove",
        },
        MethodCandidate {
            name: "skill_test",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.skill-test@v1",
            repomix_path: "enum.tched-router.SkillCommands.Test",
        },
        MethodCandidate {
            name: "sop_approve",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.sop-approve@v1",
            repomix_path: "enum.tched-router.SopCommands.Approve",
        },
        MethodCandidate {
            name: "sop_delete",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.sop-delete@v1",
            repomix_path: "enum.tched-router.SopCommands.Delete",
        },
        MethodCandidate {
            name: "sop_deny",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.sop-deny@v1",
            repomix_path: "enum.tched-router.SopCommands.Deny",
        },
        MethodCandidate {
            name: "sop_graph",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.sop-graph@v1",
            repomix_path: "enum.tched-router.SopCommands.Graph",
        },
        MethodCandidate {
            name: "sop_list",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.sop-list@v1",
            repomix_path: "enum.tched-router.SopCommands.List",
        },
        MethodCandidate {
            name: "sop_pending",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.sop-pending@v1",
            repomix_path: "enum.tched-router.SopCommands.Pending",
        },
        MethodCandidate {
            name: "sop_show",
            side_effect: "read",
            idempotent: true,
            required_capability: "tched-router.read",
            subid: "obs.software.tched-router.sop-show@v1",
            repomix_path: "enum.tched-router.SopCommands.Show",
        },
        MethodCandidate {
            name: "sop_validate",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "tched-router.write",
            subid: "mut.software.tched-router.sop-validate@v1",
            repomix_path: "enum.tched-router.SopCommands.Validate",
        },
    ];

    /// Promote every generated method into the sealed plugin schema.
    pub(super) fn register_methods(schema: &mut op_state_store::PluginSchema) {
        use super::super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
        schema.methods.insert(
            "agents_create".to_string(),
            method_decl_from_schemars_with_output::<AgentsCreateInput, AgentsCreateOutput>(
                "agents_create",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.agents-create@v1",
            ),
        );
        schema.methods.insert(
            "agents_delete".to_string(),
            method_decl_from_schemars_with_output::<AgentsDeleteInput, AgentsDeleteOutput>(
                "agents_delete",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.agents-delete@v1",
            ),
        );
        schema.methods.insert(
            "agents_list".to_string(),
            method_decl_from_schemars_with_output::<AgentsListInput, AgentsListOutput>(
                "agents_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.agents-list@v1",
            ),
        );
        schema.methods.insert(
            "agents_rename".to_string(),
            method_decl_from_schemars_with_output::<AgentsRenameInput, AgentsRenameOutput>(
                "agents_rename",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.agents-rename@v1",
            ),
        );
        schema.methods.insert(
            "auth_emaillogin".to_string(),
            method_decl_from_schemars_with_output::<AuthEmailloginInput, AuthEmailloginOutput>(
                "auth_emaillogin",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.auth-emaillogin@v1",
            ),
        );
        schema.methods.insert(
            "auth_list".to_string(),
            method_decl_from_schemars_with_output::<AuthListInput, AuthListOutput>(
                "auth_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.auth-list@v1",
            ),
        );
        schema.methods.insert(
            "auth_login".to_string(),
            method_decl_from_schemars_with_output::<AuthLoginInput, AuthLoginOutput>(
                "auth_login",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.auth-login@v1",
            ),
        );
        schema.methods.insert(
            "auth_logout".to_string(),
            method_decl_from_schemars_with_output::<AuthLogoutInput, AuthLogoutOutput>(
                "auth_logout",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.auth-logout@v1",
            ),
        );
        schema.methods.insert("auth_pasteredirect".to_string(), method_decl_from_schemars_with_output::<AuthPasteredirectInput, AuthPasteredirectOutput>("auth_pasteredirect", op_state_store::SideEffect::Mutation, false, "tched-router.write", "mut.software.tched-router.auth-pasteredirect@v1"));
        schema.methods.insert(
            "auth_pastetoken".to_string(),
            method_decl_from_schemars_with_output::<AuthPastetokenInput, AuthPastetokenOutput>(
                "auth_pastetoken",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.auth-pastetoken@v1",
            ),
        );
        schema.methods.insert(
            "auth_refresh".to_string(),
            method_decl_from_schemars_with_output::<AuthRefreshInput, AuthRefreshOutput>(
                "auth_refresh",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.auth-refresh@v1",
            ),
        );
        schema.methods.insert(
            "auth_setuptoken".to_string(),
            method_decl_from_schemars_with_output::<AuthSetuptokenInput, AuthSetuptokenOutput>(
                "auth_setuptoken",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.auth-setuptoken@v1",
            ),
        );
        schema.methods.insert(
            "auth_use".to_string(),
            method_decl_from_schemars_with_output::<AuthUseInput, AuthUseOutput>(
                "auth_use",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.auth-use@v1",
            ),
        );
        schema.methods.insert(
            "channel_add".to_string(),
            method_decl_from_schemars_with_output::<ChannelAddInput, ChannelAddOutput>(
                "channel_add",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channel-add@v1",
            ),
        );
        schema.methods.insert(
            "channel_bindtelegram".to_string(),
            method_decl_from_schemars_with_output::<
                ChannelBindtelegramInput,
                ChannelBindtelegramOutput,
            >(
                "channel_bindtelegram",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channel-bindtelegram@v1",
            ),
        );
        schema.methods.insert(
            "channel_doctor".to_string(),
            method_decl_from_schemars_with_output::<ChannelDoctorInput, ChannelDoctorOutput>(
                "channel_doctor",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channel-doctor@v1",
            ),
        );
        schema.methods.insert(
            "channel_list".to_string(),
            method_decl_from_schemars_with_output::<ChannelListInput, ChannelListOutput>(
                "channel_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.channel-list@v1",
            ),
        );
        schema.methods.insert(
            "channel_remove".to_string(),
            method_decl_from_schemars_with_output::<ChannelRemoveInput, ChannelRemoveOutput>(
                "channel_remove",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channel-remove@v1",
            ),
        );
        schema.methods.insert(
            "channel_send".to_string(),
            method_decl_from_schemars_with_output::<ChannelSendInput, ChannelSendOutput>(
                "channel_send",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channel-send@v1",
            ),
        );
        schema.methods.insert(
            "channel_start".to_string(),
            method_decl_from_schemars_with_output::<ChannelStartInput, ChannelStartOutput>(
                "channel_start",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channel-start@v1",
            ),
        );
        schema.methods.insert(
            "channels_create".to_string(),
            method_decl_from_schemars_with_output::<ChannelsCreateInput, ChannelsCreateOutput>(
                "channels_create",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channels-create@v1",
            ),
        );
        schema.methods.insert(
            "channels_delete".to_string(),
            method_decl_from_schemars_with_output::<ChannelsDeleteInput, ChannelsDeleteOutput>(
                "channels_delete",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channels-delete@v1",
            ),
        );
        schema.methods.insert(
            "channels_list".to_string(),
            method_decl_from_schemars_with_output::<ChannelsListInput, ChannelsListOutput>(
                "channels_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.channels-list@v1",
            ),
        );
        schema.methods.insert(
            "channels_rename".to_string(),
            method_decl_from_schemars_with_output::<ChannelsRenameInput, ChannelsRenameOutput>(
                "channels_rename",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.channels-rename@v1",
            ),
        );
        schema.methods.insert(
            "config_complete".to_string(),
            method_decl_from_schemars_with_output::<ConfigCompleteInput, ConfigCompleteOutput>(
                "config_complete",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.config-complete@v1",
            ),
        );
        schema.methods.insert(
            "config_docs".to_string(),
            method_decl_from_schemars_with_output::<ConfigDocsInput, ConfigDocsOutput>(
                "config_docs",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.config-docs@v1",
            ),
        );
        schema.methods.insert(
            "config_generate".to_string(),
            method_decl_from_schemars_with_output::<ConfigGenerateInput, ConfigGenerateOutput>(
                "config_generate",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.config-generate@v1",
            ),
        );
        schema.methods.insert(
            "config_get".to_string(),
            method_decl_from_schemars_with_output::<ConfigGetInput, ConfigGetOutput>(
                "config_get",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.config-get@v1",
            ),
        );
        schema.methods.insert(
            "config_init".to_string(),
            method_decl_from_schemars_with_output::<ConfigInitInput, ConfigInitOutput>(
                "config_init",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.config-init@v1",
            ),
        );
        schema.methods.insert(
            "config_list".to_string(),
            method_decl_from_schemars_with_output::<ConfigListInput, ConfigListOutput>(
                "config_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.config-list@v1",
            ),
        );
        schema.methods.insert(
            "config_migrate".to_string(),
            method_decl_from_schemars_with_output::<ConfigMigrateInput, ConfigMigrateOutput>(
                "config_migrate",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.config-migrate@v1",
            ),
        );
        schema.methods.insert(
            "config_patch".to_string(),
            method_decl_from_schemars_with_output::<ConfigPatchInput, ConfigPatchOutput>(
                "config_patch",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.config-patch@v1",
            ),
        );
        schema.methods.insert(
            "config_set".to_string(),
            method_decl_from_schemars_with_output::<ConfigSetInput, ConfigSetOutput>(
                "config_set",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.config-set@v1",
            ),
        );
        schema.methods.insert(
            "cron_add".to_string(),
            method_decl_from_schemars_with_output::<CronAddInput, CronAddOutput>(
                "cron_add",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-add@v1",
            ),
        );
        schema.methods.insert(
            "cron_addat".to_string(),
            method_decl_from_schemars_with_output::<CronAddatInput, CronAddatOutput>(
                "cron_addat",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-addat@v1",
            ),
        );
        schema.methods.insert(
            "cron_addevery".to_string(),
            method_decl_from_schemars_with_output::<CronAddeveryInput, CronAddeveryOutput>(
                "cron_addevery",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-addevery@v1",
            ),
        );
        schema.methods.insert(
            "cron_list".to_string(),
            method_decl_from_schemars_with_output::<CronListInput, CronListOutput>(
                "cron_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.cron-list@v1",
            ),
        );
        schema.methods.insert(
            "cron_once".to_string(),
            method_decl_from_schemars_with_output::<CronOnceInput, CronOnceOutput>(
                "cron_once",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-once@v1",
            ),
        );
        schema.methods.insert(
            "cron_pause".to_string(),
            method_decl_from_schemars_with_output::<CronPauseInput, CronPauseOutput>(
                "cron_pause",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-pause@v1",
            ),
        );
        schema.methods.insert(
            "cron_remove".to_string(),
            method_decl_from_schemars_with_output::<CronRemoveInput, CronRemoveOutput>(
                "cron_remove",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-remove@v1",
            ),
        );
        schema.methods.insert(
            "cron_resume".to_string(),
            method_decl_from_schemars_with_output::<CronResumeInput, CronResumeOutput>(
                "cron_resume",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-resume@v1",
            ),
        );
        schema.methods.insert(
            "cron_update".to_string(),
            method_decl_from_schemars_with_output::<CronUpdateInput, CronUpdateOutput>(
                "cron_update",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.cron-update@v1",
            ),
        );
        schema.methods.insert(
            "deprecatedprops_any".to_string(),
            method_decl_from_schemars_with_output::<
                DeprecatedpropsAnyInput,
                DeprecatedpropsAnyOutput,
            >(
                "deprecatedprops_any",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.deprecatedprops-any@v1",
            ),
        );
        schema.methods.insert(
            "doctor_models".to_string(),
            method_decl_from_schemars_with_output::<DoctorModelsInput, DoctorModelsOutput>(
                "doctor_models",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.doctor-models@v1",
            ),
        );
        schema.methods.insert(
            "doctor_traces".to_string(),
            method_decl_from_schemars_with_output::<DoctorTracesInput, DoctorTracesOutput>(
                "doctor_traces",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.doctor-traces@v1",
            ),
        );
        schema.methods.insert(
            "doctor_updatecontextwindows".to_string(),
            method_decl_from_schemars_with_output::<
                DoctorUpdatecontextwindowsInput,
                DoctorUpdatecontextwindowsOutput,
            >(
                "doctor_updatecontextwindows",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.doctor-updatecontextwindows@v1",
            ),
        );
        schema.methods.insert(
            "eval_run".to_string(),
            method_decl_from_schemars_with_output::<EvalRunInput, EvalRunOutput>(
                "eval_run",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.eval-run@v1",
            ),
        );
        schema.methods.insert(
            "gateway_getpaircode".to_string(),
            method_decl_from_schemars_with_output::<
                GatewayGetpaircodeInput,
                GatewayGetpaircodeOutput,
            >(
                "gateway_getpaircode",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.gateway-getpaircode@v1",
            ),
        );
        schema.methods.insert(
            "gateway_restart".to_string(),
            method_decl_from_schemars_with_output::<GatewayRestartInput, GatewayRestartOutput>(
                "gateway_restart",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.gateway-restart@v1",
            ),
        );
        schema.methods.insert(
            "gateway_start".to_string(),
            method_decl_from_schemars_with_output::<GatewayStartInput, GatewayStartOutput>(
                "gateway_start",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.gateway-start@v1",
            ),
        );
        schema.methods.insert(
            "hardware_discover".to_string(),
            method_decl_from_schemars_with_output::<HardwareDiscoverInput, HardwareDiscoverOutput>(
                "hardware_discover",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.hardware-discover@v1",
            ),
        );
        schema.methods.insert(
            "hardware_info".to_string(),
            method_decl_from_schemars_with_output::<HardwareInfoInput, HardwareInfoOutput>(
                "hardware_info",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.hardware-info@v1",
            ),
        );
        schema.methods.insert(
            "hardware_introspect".to_string(),
            method_decl_from_schemars_with_output::<
                HardwareIntrospectInput,
                HardwareIntrospectOutput,
            >(
                "hardware_introspect",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.hardware-introspect@v1",
            ),
        );
        schema.methods.insert(
            "integration_info".to_string(),
            method_decl_from_schemars_with_output::<IntegrationInfoInput, IntegrationInfoOutput>(
                "integration_info",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.integration-info@v1",
            ),
        );
        schema.methods.insert(
            "memory_clear".to_string(),
            method_decl_from_schemars_with_output::<MemoryClearInput, MemoryClearOutput>(
                "memory_clear",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.memory-clear@v1",
            ),
        );
        schema.methods.insert(
            "memory_get".to_string(),
            method_decl_from_schemars_with_output::<MemoryGetInput, MemoryGetOutput>(
                "memory_get",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.memory-get@v1",
            ),
        );
        schema.methods.insert(
            "memory_list".to_string(),
            method_decl_from_schemars_with_output::<MemoryListInput, MemoryListOutput>(
                "memory_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.memory-list@v1",
            ),
        );
        schema.methods.insert(
            "memory_reindex".to_string(),
            method_decl_from_schemars_with_output::<MemoryReindexInput, MemoryReindexOutput>(
                "memory_reindex",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.memory-reindex@v1",
            ),
        );
        schema.methods.insert(
            "memory_stats".to_string(),
            method_decl_from_schemars_with_output::<MemoryStatsInput, MemoryStatsOutput>(
                "memory_stats",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.memory-stats@v1",
            ),
        );
        schema.methods.insert(
            "migrate_openclaw".to_string(),
            method_decl_from_schemars_with_output::<MigrateOpenclawInput, MigrateOpenclawOutput>(
                "migrate_openclaw",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.migrate-openclaw@v1",
            ),
        );
        schema.methods.insert(
            "model_list".to_string(),
            method_decl_from_schemars_with_output::<ModelListInput, ModelListOutput>(
                "model_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.model-list@v1",
            ),
        );
        schema.methods.insert(
            "model_refresh".to_string(),
            method_decl_from_schemars_with_output::<ModelRefreshInput, ModelRefreshOutput>(
                "model_refresh",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.model-refresh@v1",
            ),
        );
        schema.methods.insert(
            "peripheral_add".to_string(),
            method_decl_from_schemars_with_output::<PeripheralAddInput, PeripheralAddOutput>(
                "peripheral_add",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.peripheral-add@v1",
            ),
        );
        schema.methods.insert(
            "peripheral_flash".to_string(),
            method_decl_from_schemars_with_output::<PeripheralFlashInput, PeripheralFlashOutput>(
                "peripheral_flash",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.peripheral-flash@v1",
            ),
        );
        schema.methods.insert(
            "peripheral_flashnucleo".to_string(),
            method_decl_from_schemars_with_output::<
                PeripheralFlashnucleoInput,
                PeripheralFlashnucleoOutput,
            >(
                "peripheral_flashnucleo",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.peripheral-flashnucleo@v1",
            ),
        );
        schema.methods.insert(
            "peripheral_list".to_string(),
            method_decl_from_schemars_with_output::<PeripheralListInput, PeripheralListOutput>(
                "peripheral_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.peripheral-list@v1",
            ),
        );
        schema.methods.insert(
            "peripheral_setupunoq".to_string(),
            method_decl_from_schemars_with_output::<
                PeripheralSetupunoqInput,
                PeripheralSetupunoqOutput,
            >(
                "peripheral_setupunoq",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.peripheral-setupunoq@v1",
            ),
        );
        schema.methods.insert(
            "plugin_info".to_string(),
            method_decl_from_schemars_with_output::<PluginInfoInput, PluginInfoOutput>(
                "plugin_info",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.plugin-info@v1",
            ),
        );
        schema.methods.insert(
            "plugin_install".to_string(),
            method_decl_from_schemars_with_output::<PluginInstallInput, PluginInstallOutput>(
                "plugin_install",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.plugin-install@v1",
            ),
        );
        schema.methods.insert(
            "plugin_list".to_string(),
            method_decl_from_schemars_with_output::<PluginListInput, PluginListOutput>(
                "plugin_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.plugin-list@v1",
            ),
        );
        schema.methods.insert(
            "plugin_migrate".to_string(),
            method_decl_from_schemars_with_output::<PluginMigrateInput, PluginMigrateOutput>(
                "plugin_migrate",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.plugin-migrate@v1",
            ),
        );
        schema.methods.insert(
            "plugin_remove".to_string(),
            method_decl_from_schemars_with_output::<PluginRemoveInput, PluginRemoveOutput>(
                "plugin_remove",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.plugin-remove@v1",
            ),
        );
        schema.methods.insert(
            "plugin_search".to_string(),
            method_decl_from_schemars_with_output::<PluginSearchInput, PluginSearchOutput>(
                "plugin_search",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.plugin-search@v1",
            ),
        );
        schema.methods.insert(
            "providers_create".to_string(),
            method_decl_from_schemars_with_output::<ProvidersCreateInput, ProvidersCreateOutput>(
                "providers_create",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.providers-create@v1",
            ),
        );
        schema.methods.insert(
            "providers_delete".to_string(),
            method_decl_from_schemars_with_output::<ProvidersDeleteInput, ProvidersDeleteOutput>(
                "providers_delete",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.providers-delete@v1",
            ),
        );
        schema.methods.insert(
            "providers_rename".to_string(),
            method_decl_from_schemars_with_output::<ProvidersRenameInput, ProvidersRenameOutput>(
                "providers_rename",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.providers-rename@v1",
            ),
        );
        schema.methods.insert(
            "service_install".to_string(),
            method_decl_from_schemars_with_output::<ServiceInstallInput, ServiceInstallOutput>(
                "service_install",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.service-install@v1",
            ),
        );
        schema.methods.insert(
            "service_logs".to_string(),
            method_decl_from_schemars_with_output::<ServiceLogsInput, ServiceLogsOutput>(
                "service_logs",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.service-logs@v1",
            ),
        );
        schema.methods.insert(
            "service_restart".to_string(),
            method_decl_from_schemars_with_output::<ServiceRestartInput, ServiceRestartOutput>(
                "service_restart",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.service-restart@v1",
            ),
        );
        schema.methods.insert(
            "service_start".to_string(),
            method_decl_from_schemars_with_output::<ServiceStartInput, ServiceStartOutput>(
                "service_start",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.service-start@v1",
            ),
        );
        schema.methods.insert(
            "service_stop".to_string(),
            method_decl_from_schemars_with_output::<ServiceStopInput, ServiceStopOutput>(
                "service_stop",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.service-stop@v1",
            ),
        );
        schema.methods.insert(
            "service_uninstall".to_string(),
            method_decl_from_schemars_with_output::<ServiceUninstallInput, ServiceUninstallOutput>(
                "service_uninstall",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.service-uninstall@v1",
            ),
        );
        schema.methods.insert(
            "skillbundle_add".to_string(),
            method_decl_from_schemars_with_output::<SkillbundleAddInput, SkillbundleAddOutput>(
                "skillbundle_add",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skillbundle-add@v1",
            ),
        );
        schema.methods.insert(
            "skillbundle_list".to_string(),
            method_decl_from_schemars_with_output::<SkillbundleListInput, SkillbundleListOutput>(
                "skillbundle_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.skillbundle-list@v1",
            ),
        );
        schema.methods.insert("skillbundle_remove".to_string(), method_decl_from_schemars_with_output::<SkillbundleRemoveInput, SkillbundleRemoveOutput>("skillbundle_remove", op_state_store::SideEffect::Mutation, false, "tched-router.write", "mut.software.tched-router.skillbundle-remove@v1"));
        schema.methods.insert("skillbundle_rename".to_string(), method_decl_from_schemars_with_output::<SkillbundleRenameInput, SkillbundleRenameOutput>("skillbundle_rename", op_state_store::SideEffect::Mutation, false, "tched-router.write", "mut.software.tched-router.skillbundle-rename@v1"));
        schema.methods.insert(
            "skillbundle_show".to_string(),
            method_decl_from_schemars_with_output::<SkillbundleShowInput, SkillbundleShowOutput>(
                "skillbundle_show",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.skillbundle-show@v1",
            ),
        );
        schema.methods.insert(
            "skill_add".to_string(),
            method_decl_from_schemars_with_output::<SkillAddInput, SkillAddOutput>(
                "skill_add",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skill-add@v1",
            ),
        );
        schema.methods.insert(
            "skill_audit".to_string(),
            method_decl_from_schemars_with_output::<SkillAuditInput, SkillAuditOutput>(
                "skill_audit",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skill-audit@v1",
            ),
        );
        schema.methods.insert(
            "skill_bundle".to_string(),
            method_decl_from_schemars_with_output::<SkillBundleInput, SkillBundleOutput>(
                "skill_bundle",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skill-bundle@v1",
            ),
        );
        schema.methods.insert(
            "skill_edit".to_string(),
            method_decl_from_schemars_with_output::<SkillEditInput, SkillEditOutput>(
                "skill_edit",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skill-edit@v1",
            ),
        );
        schema.methods.insert(
            "skill_install".to_string(),
            method_decl_from_schemars_with_output::<SkillInstallInput, SkillInstallOutput>(
                "skill_install",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skill-install@v1",
            ),
        );
        schema.methods.insert(
            "skill_list".to_string(),
            method_decl_from_schemars_with_output::<SkillListInput, SkillListOutput>(
                "skill_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.skill-list@v1",
            ),
        );
        schema.methods.insert(
            "skill_remove".to_string(),
            method_decl_from_schemars_with_output::<SkillRemoveInput, SkillRemoveOutput>(
                "skill_remove",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skill-remove@v1",
            ),
        );
        schema.methods.insert(
            "skill_test".to_string(),
            method_decl_from_schemars_with_output::<SkillTestInput, SkillTestOutput>(
                "skill_test",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.skill-test@v1",
            ),
        );
        schema.methods.insert(
            "sop_approve".to_string(),
            method_decl_from_schemars_with_output::<SopApproveInput, SopApproveOutput>(
                "sop_approve",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.sop-approve@v1",
            ),
        );
        schema.methods.insert(
            "sop_delete".to_string(),
            method_decl_from_schemars_with_output::<SopDeleteInput, SopDeleteOutput>(
                "sop_delete",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.sop-delete@v1",
            ),
        );
        schema.methods.insert(
            "sop_deny".to_string(),
            method_decl_from_schemars_with_output::<SopDenyInput, SopDenyOutput>(
                "sop_deny",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.sop-deny@v1",
            ),
        );
        schema.methods.insert(
            "sop_graph".to_string(),
            method_decl_from_schemars_with_output::<SopGraphInput, SopGraphOutput>(
                "sop_graph",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.sop-graph@v1",
            ),
        );
        schema.methods.insert(
            "sop_list".to_string(),
            method_decl_from_schemars_with_output::<SopListInput, SopListOutput>(
                "sop_list",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.sop-list@v1",
            ),
        );
        schema.methods.insert(
            "sop_pending".to_string(),
            method_decl_from_schemars_with_output::<SopPendingInput, SopPendingOutput>(
                "sop_pending",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.sop-pending@v1",
            ),
        );
        schema.methods.insert(
            "sop_show".to_string(),
            method_decl_from_schemars_with_output::<SopShowInput, SopShowOutput>(
                "sop_show",
                op_state_store::SideEffect::Read,
                true,
                "tched-router.read",
                "obs.software.tched-router.sop-show@v1",
            ),
        );
        schema.methods.insert(
            "sop_validate".to_string(),
            method_decl_from_schemars_with_output::<SopValidateInput, SopValidateOutput>(
                "sop_validate",
                op_state_store::SideEffect::Mutation,
                false,
                "tched-router.write",
                "mut.software.tched-router.sop-validate@v1",
            ),
        );
    }
}

// Promotion checklist (Fable contract):
// 1. Move owned fields into the plugin State struct with concrete Rust types.
// 2. Replace method placeholders with dedicated typed Input/Output fields.
// 3. Register with method_decl_from_schemars_with_output and correct SideEffect.
// 4. Register every subid, implement dispatch, and add schema/subid tests.
// 5. Re-run op-plugin-lint; only then replace the original plugin file.
