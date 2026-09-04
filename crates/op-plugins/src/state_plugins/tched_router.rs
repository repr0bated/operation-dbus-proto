//! 3tched Router route surface plugin — GB.3tched Router.
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
use op_state_store::CapabilityDecl;
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
const PLUGIN_DESCRIPTION: &str = "3tched Router — schema/RPC-native model router for Antigravity UI, CLI providers, and structured JSON output";
const PLUGIN_DISPLAY_NAME: &str = "GB.3tchedRouter";

/// Transport layer metadata for the 3tched Router plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-transport.schema@v1"))]
pub struct LlmTransport {
    /// D-Bus object path served by this plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router-transport.dbus-object@v1"))]
    pub dbus_object: String,
    /// gRPC upstream target.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router-transport.grpc-target@v1"))]
    pub grpc_target: String,
    /// Incus / WireGuard container target for xray routing. Kept as a
    /// published-schema field for backward compatibility, but zeroclaw's LLM
    /// transport now runs on the host (xray through its runit-managed service and
    /// the gRPC-bridge via `op-grpc-bridge-zeroclaw`); there is no per-service
    /// incus container. Defaults to the `"host"` sentinel.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router-transport.incus-container@v1"))]
    pub incus_container: String,
    /// Browser-facing surface description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.3tched-router-transport.browser-surface@v1"))]
    pub browser_surface: String,
    /// REST aliases exposed by the bridge.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.3tched-router-transport.rest-aliases@v1"))]
    pub rest_aliases: Vec<String>,
    /// Canonical OSCAL/subid mapping authority.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.3tched-router-transport.policy-source@v1"))]
    pub policy_source: String,
}

/// Nested per-capability model assignments.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.3tched-router.model-assignments.schema@v1"))]
pub struct ModelAssignments {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.model-assignments.ovs-routing@v1"))]
    pub ovs_routing: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.model-assignments.obfuscation@v1"))]
    pub obfuscation: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.model-assignments.vectorization@v1"))]
    pub vectorization: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.model-assignments.qdrant-retrieval@v1"))]
    pub qdrant_retrieval: String,
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.model-assignments.cozo-retrieval@v1"))]
    pub cozo_retrieval: String,
}

/// Configurable options RPC contract extracted from `operation.registration.v1`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.rpc.schema@v1"))]
pub struct ConfigurableOptionRpc {
    /// RPC name from RegistrationService.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.rpc.name@v1"))]
    pub name: String,
    /// Request message type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.rpc.request@v1"))]
    pub request_type: String,
    /// Response message type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.rpc.response@v1"))]
    pub response_type: String,
    /// Read or mutation side-effect classification.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.rpc.side-effect@v1"))]
    pub side_effect: String,
    /// Required capability for this RPC.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.rpc.capability@v1"))]
    pub capability_id: String,
    /// OSCAL operation taxonomy key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.rpc.subid@v1"))]
    pub subid: String,
}

/// Configurable options message contract extracted from registration.proto.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.message.schema@v1"))]
pub struct ConfigurableOptionMessage {
    /// Message type name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.message.name@v1"))]
    pub name: String,
    /// Message fields keyed by proto field name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.message.fields@v1"))]
    pub fields: Vec<ConfigurableOptionField>,
}

/// Field contract extracted from configurable options proto messages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.field.schema@v1"))]
pub struct ConfigurableOptionField {
    /// Field name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.field.name@v1"))]
    pub name: String,
    /// Proto type, including optional/repeated marker when applicable.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.field.proto-type@v1"))]
    pub proto_type: String,
    /// JSON/schema rendering type.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.field.json-type@v1"))]
    pub json_type: String,
    /// True when the field contains sensitive material.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.field.sensitive@v1"))]
    pub sensitive: bool,
    /// True when this option is required by the source contract.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.field.required@v1"))]
    pub required: bool,
}

/// Registration error code contract.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.error-code.schema@v1"))]
pub struct ConfigurableOptionErrorCode {
    /// Symbolic proto enum name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.error-code.name@v1"))]
    pub name: String,
    /// Numeric proto value.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.error-code.value@v1"))]
    pub value: i32,
}

/// Magic-link and WireGuard registration service schema.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.registration-service.schema@v1"))]
pub struct RegistrationServiceSchema {
    /// Proto package name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.registration-service.package@v1"))]
    pub package: String,
    /// Owning proto source path.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.3tched-router-options.registration-service.proto@v1"))]
    pub source_proto: String,
    /// Service name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.registration-service.name@v1"))]
    pub service: String,
    /// RPC methods exposed by the registration service.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.registration-service.rpcs@v1"))]
    pub rpcs: Vec<ConfigurableOptionRpc>,
    /// Message schemas used by those RPCs.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.registration-service.messages@v1"))]
    pub messages: Vec<ConfigurableOptionMessage>,
    /// Error enum values.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.registration-service.errors@v1"))]
    pub error_codes: Vec<ConfigurableOptionErrorCode>,
}

/// Identity chain declared for user container options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.identity-chain.schema@v1"))]
pub struct IdentityOptions {
    /// Hardware-bound seed accepted by the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.hardware.3tched-router-options.identity.mac@v1"))]
    pub mac_address_key: String,
    /// Optional shared key accepted by the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.policy.3tched-router-options.identity.psk@v1"))]
    pub pre_shared_key: String,
    /// WireGuard public key identity.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.network.3tched-router-options.identity.wireguard-pubkey@v1"))]
    pub wireguard_pubkey: String,
    /// MCP bearer token derivation rule.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.identity.mcp-token@v1"))]
    pub mcp_token_derivation: String,
}

/// Namespace template declared for each user container.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.memory-namespace.schema@v1"))]
pub struct MemoryNamespaceOption {
    /// Namespace template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.memory-namespace.template@v1"))]
    pub namespace_template: String,
    /// Keys owned by this namespace.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.memory-namespace.keys@v1"))]
    pub keys: Vec<String>,
    /// Human-readable purpose.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.memory-namespace.purpose@v1"))]
    pub purpose: String,
    /// Identity key used to bind this namespace to the container/user identity.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.memory-namespace.identity-link@v1"))]
    pub identity_link_key: String,
}

/// User container option contract exposed through 3tched Router.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.process-procedure.3tched-router-options.user-container.schema@v1"))]
pub struct UserContainerOptions {
    /// Source script that currently materializes the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.process-procedure.3tched-router-options.user-container.script@v1"))]
    pub source_script: String,
    /// User-visible container arguments.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.process-procedure.3tched-router-options.user-container.arguments@v1"))]
    pub arguments: Vec<ConfigurableOptionField>,
    /// Incus container name template.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.user-container.container-template@v1"))]
    pub container_id_template: String,
    /// Default Incus image.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.user-container.image@v1"))]
    pub image: String,
    /// Cognitive MCP endpoint used by the user container memory flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.3tched-router-options.user-container.mcp-endpoint@v1"))]
    pub cognitive_mcp_endpoint: String,
    /// Feature flags declared for the user container flow.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.3tched-router-options.user-container.features@v1"))]
    pub feature_flags: Vec<String>,
}

/// Privacy rules for configurable options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.policy.3tched-router-options.privacy.schema@v1"))]
pub struct PrivacyOptions {
    /// Whether email may be written to CozoDB.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.3tched-router-options.privacy.email-storage@v1"))]
    pub email_storage_rule: String,
    /// Sensitive fields that must not be rendered casually.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.3tched-router-options.privacy.sensitive-fields@v1"))]
    pub sensitive_fields: Vec<String>,
}

/// Complete configurable options schema surface owned by 3tched Router.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.schema@v1"))]
pub struct ConfigurableOptions {
    /// Magic-link and WireGuard registration RPC schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router-options.registration-service@v1"))]
    pub registration_service: RegistrationServiceSchema,
    /// User container configurable option schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.process-procedure.3tched-router-options.user-container@v1"))]
    pub user_container: UserContainerOptions,
    /// Identity chain schema.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.identity-chain@v1"))]
    pub identity_chain: IdentityOptions,
    /// Memory namespace templates.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router-options.memory-namespaces@v1"))]
    pub memory_namespaces: Vec<MemoryNamespaceOption>,
    /// Privacy handling rules.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.3tched-router-options.privacy@v1"))]
    pub privacy_policy: PrivacyOptions,
}

/// Top-level 3tched Router state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.3tched-router.schema@v1"))]
#[schemars(extend("x-oscal-category" = "llm"))]
pub struct TchedRouterState {
    /// Operational status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.3tched-router.status@v1"))]
    pub status: String,
    /// Selected provider identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.selected-provider@v1"))]
    pub selected_provider: String,
    /// Selected model identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.3tched-router.selected-model@v1"))]
    pub selected_model: String,
    /// Per-capability model assignments.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.model-assignments@v1"))]
    pub model_assignments: ModelAssignments,
    /// Transport layer metadata.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.transport@v1"))]
    pub transport: LlmTransport,
    /// Magic-link and WireGuard registration RPC schema (a plain configuration).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.3tched-router.registration-service@v1"))]
    pub registration_service: RegistrationServiceSchema,
    /// User container configurable option schema (a plain configuration).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.process-procedure.3tched-router.user-container@v1"))]
    pub user_container: UserContainerOptions,
    /// Identity chain schema (a plain configuration).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router.identity-chain@v1"))]
    pub identity_chain: IdentityOptions,
    /// Memory namespace templates (a plain configuration).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router.memory-namespaces@v1"))]
    pub memory_namespaces: Vec<MemoryNamespaceOption>,
    /// Privacy handling rules (a plain configuration).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.policy.3tched-router.privacy@v1"))]
    pub privacy_policy: PrivacyOptions,
    /// Declared LLM catalog fields — flattened to the top level. This is
    /// DECLARED state, not a runtime projection: there is no projection.
    #[serde(flatten)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router.llm-catalog@v1"))]
    pub catalog: LlmProjection,
    /// Pure selector weights and default effort class.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.3tched-router.selector-policy@v1"))]
    pub selector_policy: super::common::llm_projection::SelectorPolicy,
}

/// Empty input for read-only 3tched Router methods.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EmptyTchedInput {}

/// Input for resolving a model route by hint or model identifier.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResolveRouteInput {
    /// Route hint or model identifier.
    pub hint: String,
}

/// A chat message carried by the schema-declared 3tched Router method.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TchedChatMessage {
    /// OpenAI-compatible message role such as `system`, `user`, or `assistant`.
    pub role: String,
    /// Text content carried by this conversation turn.
    pub content: String,
}

/// Input for the bridge-owned 3tched Router chat dispatcher.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChatInput {
    /// Compatibility form for callers sending a single user turn.
    #[serde(default)]
    pub message: String,
    /// Full ordered conversation. When non-empty this takes precedence over
    /// `message`.
    #[serde(default)]
    pub messages: Vec<TchedChatMessage>,
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

/// Output for the complete 3tched Router state surface.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.3tched-router.state.result@v1"))]
pub struct GetStateOutput {
    /// Complete projected 3tched Router state.
    pub state: TchedRouterState,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.3tched-router.model-routes.result@v1"))]
pub struct GetModelRoutesOutput {
    /// Declared model routes.
    pub model_routes: Vec<ModelRoute>,
}

/// Output for the provider catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.3tched-router.provider-catalog.result@v1"))]
pub struct GetProviderCatalogOutput {
    /// Declared providers.
    pub providers: Vec<Provider>,
}

/// Output for the declared tool catalog.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.3tched-router.tools.result@v1"))]
pub struct GetToolsOutput {
    /// Declared LLM tools.
    pub tools: Vec<LlmTool>,
}

/// Output for the provider list accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.3tched-router.providers.result@v1"))]
pub struct ListProvidersOutput {
    pub providers: Vec<Provider>,
}

/// Output for the router accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.3tched-router.router.result@v1"))]
pub struct GetRouterOutput {
    pub router: Router,
}

/// Output for the config schema accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.3tched-router.config-schema.result@v1"))]
pub struct GetConfigSchemaOutput {
    pub config_schema: ConfigSchema,
}

/// Input for sealed CLI config methods (`config_list` / `config_set` / …).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigCliInput {
    /// String-valued CLI options (`path`, `value`, `filter`, `no_interactive`).
    #[serde(default)]
    pub options: std::collections::BTreeMap<String, String>,
}

/// Output for sealed CLI config methods.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigCliOutput {
    /// Human-readable CLI stdout (or structured payload).
    pub message: String,
    /// Whether the command mutated config.
    pub changed: bool,
}

/// Output for the UI surfaces accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.3tched-router.ui-surfaces.result@v1"))]
pub struct ListUiSurfacesOutput {
    pub ui_surfaces: Vec<UiSurface>,
}

/// Output for the structured output accessor.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.service.3tched-router.structured-output.result@v1"))]
pub struct GetStructuredOutputOutput {
    pub structured_output: StructuredOutput,
}

/// Output for a resolved model route.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.3tched-router.route.result@v1"))]
pub struct ResolveRouteOutput {
    /// Resolved route.
    pub route: ModelRoute,
}

/// Output from the bridge-owned 3tched Router chat dispatcher.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.service.3tched-router.chat.result@v1"))]
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
#[schemars(extend("x-oscal-subid" = "obs.service.3tched-router.models.result@v1"))]
pub struct ListModelsOutput {
    /// Schema-declared model routes, optionally filtered by provider.
    pub model_routes: Vec<ModelRoute>,
}

/// Output for provider selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.selected-provider.result@v1"))]
pub struct SetProviderOutput {
    /// Selected provider identifier.
    pub selected_provider: String,
}

/// Output for model selection.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.3tched-router.selected-model.result@v1"))]
pub struct SetModelOutput {
    /// Selected model identifier.
    pub selected_model: String,
}

/// Input for selecting provider and model together (dashboard picker).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetSelectionInput {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "mut.service.tched-router.selection.set.result@v1"))]
pub struct SetSelectionOutput {
    pub selected_provider: String,
    pub selected_model: String,
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
    const DBUS_OBJECT: &'static str = "/org/opdbus/v1/plugins/tched_router";
    const OSCAL_SUBID_REGISTRY_OBJECT: &'static str = "/org/opdbus/v1/plugins/oscal_subid_registry";
    const DEFAULT_CHAT_MODEL: &'static str = "x-preview-f-free";

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
        let router_endpoint = Self::env_or("ZEROCLAW_ROUTER_ENDPOINT", "http://localhost:11434");
        let grpc_target = Self::env_or("ZEROCLAW_GRPC_TARGET", "http://10.200.0.2:50051");
        let grpc_target_for_provider = grpc_target.clone();

        let co = Self::configurable_options();
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
                    "/api/zeroclaw/chat".to_string(),
                    "/api/llm/chat".to_string(),
                ],
                policy_source: Self::OSCAL_SUBID_REGISTRY_OBJECT.to_string(),
            },
            registration_service: co.registration_service,
            user_container: co.user_container,
            identity_chain: co.identity_chain,
            memory_namespaces: co.memory_namespaces,
            privacy_policy: co.privacy_policy,
            catalog: LlmProjection {
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
                        transport: "zeroclaw-loopback".to_string(),
                        model: chat_model.clone(),
                        kind: "router".to_string(),
                        status: "declared".to_string(),
                        available: false,
                        status_reason: format!(
                            "{chat_model} is the declared OpenCode chat model; availability is projected by the 3tched Router runtime."
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
                        transport: "zeroclaw-loopback".to_string(),
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
                        name: "3tched-router.chat".to_string(),
                        description: "Send an Antigravity/3tched Router chat turn and return structured JSON when response_schema is present.".to_string(),
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
                        name: "3tched-router.models.list".to_string(),
                        description: "List cached or live models for a 3tched Router provider.".to_string(),
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
                    native_type: format!("zeroclaw config schema v{}", zeroclaw_binary_version()),
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
            selector_policy: Default::default(),
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
                        "cap.software.3tched-router.registration.magic-link.send@v1",
                        "mut.service.3tched-router.registration.magic-link.send@v1",
                    ),
                    Self::option_rpc(
                        "VerifyMagicLink",
                        "VerifyMagicLinkRequest",
                        "VerifyMagicLinkResponse",
                        "Mutation",
                        "cap.software.3tched-router.registration.magic-link.verify@v1",
                        "mut.service.3tched-router.registration.magic-link.verify@v1",
                    ),
                    Self::option_rpc(
                        "RegisterUser",
                        "RegisterUserRequest",
                        "RegisterUserResponse",
                        "Mutation",
                        "cap.software.3tched-router.registration.user.register@v1",
                        "mut.service.3tched-router.registration.user.register@v1",
                    ),
                    Self::option_rpc(
                        "GetUserStatus",
                        "GetUserStatusRequest",
                        "GetUserStatusResponse",
                        "Read",
                        "cap.software.3tched-router.registration.user-status.read@v1",
                        "obs.service.3tched-router.registration.user-status.get@v1",
                    ),
                    Self::option_rpc(
                        "ListUsers",
                        "ListUsersRequest",
                        "ListUsersResponse",
                        "Read",
                        "cap.software.3tched-router.registration.users.read@v1",
                        "obs.service.3tched-router.registration.users.list@v1",
                    ),
                    Self::option_rpc(
                        "GetWireGuardConfig",
                        "GetWireGuardConfigRequest",
                        "GetWireGuardConfigResponse",
                        "Read",
                        "cap.software.3tched-router.registration.wireguard-config.read@v1",
                        "obs.service.3tched-router.registration.wireguard-config.get@v1",
                    ),
                    Self::option_rpc(
                        "AdminUserAction",
                        "AdminUserActionRequest",
                        "AdminUserActionResponse",
                        "Mutation",
                        "cap.software.3tched-router.registration.admin-user-action.apply@v1",
                        "mut.service.3tched-router.registration.admin-user-action.apply@v1",
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
                cognitive_mcp_endpoint: "unix:path=/run/opdbus/session-bus.sock".to_string(),
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
            "3tched-router",
        );
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn schema_live(&self) -> Option<PluginSchema> {
        // No live probes — there is no projection, only declared state.
        self.schema()
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

/// Canonical `zeroclaw` schema derived from [`TchedRouterState`] via schemars.
pub(crate) fn tched_router_schema() -> PluginSchema {
    tched_router_schema_from_state(TchedRouterPlugin::current_state())
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
        method_decl_from_schemars_with_output::<EmptyTchedInput, GetStateOutput>(
            "GetState",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.state.read@v1",
            "obs.service.3tched-router.state.get@v1",
        ),
    );
    schema.methods.insert(
        "GetModelRoutes".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, GetModelRoutesOutput>(
            "GetModelRoutes",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.routes.read@v1",
            "obs.service.3tched-router.model-routes.list@v1",
        ),
    );
    schema.methods.insert(
        "ListProviders".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, ListProvidersOutput>(
            "ListProviders",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.providers.read@v1",
            "obs.service.3tched-router.providers.list@v1",
        ),
    );
    schema.methods.insert(
        "GetProviderCatalog".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, GetProviderCatalogOutput>(
            "GetProviderCatalog",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.providers.read@v1",
            "obs.service.3tched-router.provider-catalog.list@v1",
        ),
    );
    schema.methods.insert(
        "GetTools".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, GetToolsOutput>(
            "GetTools",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.tools.read@v1",
            "obs.service.3tched-router.tools.list@v1",
        ),
    );
    schema.methods.insert(
        "GetRouter".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, GetRouterOutput>(
            "GetRouter",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.router.read@v1",
            "obs.service.3tched-router.router.get@v1",
        ),
    );
    schema.methods.insert(
        "GetConfigSchema".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, GetConfigSchemaOutput>(
            "GetConfigSchema",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.config-schema.read@v1",
            "obs.service.3tched-router.config-schema.get@v1",
        ),
    );
    schema.methods.insert(
        "ListUiSurfaces".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, ListUiSurfacesOutput>(
            "ListUiSurfaces",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.ui-surfaces.read@v1",
            "obs.service.3tched-router.ui-surfaces.list@v1",
        ),
    );
    schema.methods.insert(
        "GetStructuredOutput".to_string(),
        method_decl_from_schemars_with_output::<EmptyTchedInput, GetStructuredOutputOutput>(
            "GetStructuredOutput",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.structured-output.read@v1",
            "obs.service.3tched-router.structured-output.get@v1",
        ),
    );
    schema.methods.insert(
        "ResolveRoute".to_string(),
        method_decl_from_schemars_with_output::<ResolveRouteInput, ResolveRouteOutput>(
            "ResolveRoute",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.route.resolve@v1",
            "obs.service.3tched-router.route.resolve@v1",
        ),
    );
    schema.methods.insert(
        "Chat".to_string(),
        method_decl_from_schemars_with_output::<ChatInput, ChatOutput>(
            "Chat",
            op_state_store::SideEffect::Read,
            false,
            "cap.software.3tched-router.chat@v1",
            "exp.service.3tched-router.chat@v1",
        ),
    );
    schema.methods.insert(
        "ListModels".to_string(),
        method_decl_from_schemars_with_output::<ListModelsInput, ListModelsOutput>(
            "ListModels",
            op_state_store::SideEffect::Read,
            true,
            "cap.software.3tched-router.models.read@v1",
            "obs.service.3tched-router.models.list@v1",
        ),
    );
    schema.methods.insert(
        "SetProvider".to_string(),
        method_decl_from_schemars_with_output::<SetProviderInput, SetProviderOutput>(
            "SetProvider",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.3tched-router.provider.set@v1",
            "mut.service.3tched-router.provider.set@v1",
        ),
    );
    schema.methods.insert(
        "SetModel".to_string(),
        method_decl_from_schemars_with_output::<SetModelInput, SetModelOutput>(
            "SetModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.3tched-router.model.set@v1",
            "mut.service.3tched-router.model.set@v1",
        ),
    );
    schema.methods.insert(
        "SetSelection".to_string(),
        method_decl_from_schemars_with_output::<SetSelectionInput, SetSelectionOutput>(
            "SetSelection",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.tched-router.selection.set@v1",
            "mut.service.tched-router.selection.set@v1",
        ),
    );
    schema.methods.insert(
        "SetOvsRoutingModel".to_string(),
        method_decl_from_schemars_with_output::<SetOvsRoutingModelInput, SetOvsRoutingModelOutput>(
            "SetOvsRoutingModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.3tched-router.model-assignments.ovs-routing.set@v1",
            "mut.service.3tched-router.model-assignments.ovs-routing.set@v1",
        ),
    );
    schema.methods.insert(
        "SetObfuscationModel".to_string(),
        method_decl_from_schemars_with_output::<SetObfuscationModelInput, SetObfuscationModelOutput>(
            "SetObfuscationModel",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.software.3tched-router.model-assignments.obfuscation.set@v1",
            "mut.service.3tched-router.model-assignments.obfuscation.set@v1",
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
            "cap.software.3tched-router.model-assignments.vectorization.set@v1",
            "mut.service.3tched-router.model-assignments.vectorization.set@v1",
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
            "cap.software.3tched-router.model-assignments.qdrant-retrieval.set@v1",
            "mut.service.3tched-router.model-assignments.qdrant-retrieval.set@v1",
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
            "cap.software.3tched-router.model-assignments.cozo-retrieval.set@v1",
            "mut.service.3tched-router.model-assignments.cozo-retrieval.set@v1",
        ),
    );

    // Declare every capability this plugin's methods gate on — the plugin is
    // the single declaration point (closure-clean under
    // `validate_capability_closure`).
    for (id, description) in CARRIED_CAPABILITIES {
        schema.capabilities.insert(
            id.to_string(),
            CapabilityDecl {
                id: id.to_string(),
                description: description.to_string(),
            },
        );
    }

    // Complete config surface — generated from the upstream `zeroclaw` crate's
    // real types, the single source of truth for configuration.
    super::tched_router_config_surface::register_config_methods(&mut schema);

    register_cli_config_methods(&mut schema);

    schema
}

fn register_cli_config_methods(schema: &mut PluginSchema) {
    let reads = [
        (
            "config_list",
            "obs.software.tched-router.config-list@v1",
            "List every zeroclaw config property.",
        ),
        (
            "config_get",
            "obs.software.tched-router.config-get@v1",
            "Get one zeroclaw config property.",
        ),
    ];
    let writes = [
        (
            "config_set",
            "mut.software.tched-router.config-set@v1",
            "Set one zeroclaw config property.",
        ),
        (
            "config_patch",
            "mut.software.tched-router.config-patch@v1",
            "Apply a JSON Patch to zeroclaw config.",
        ),
        (
            "config_init",
            "mut.software.tched-router.config-init@v1",
            "Initialize a zeroclaw config section.",
        ),
        (
            "config_migrate",
            "mut.software.tched-router.config-migrate@v1",
            "Migrate zeroclaw config.toml to the current schema.",
        ),
    ];
    for (name, subid, description) in reads {
        schema.methods.insert(
            name.to_string(),
            method_decl_from_schemars_with_output::<ConfigCliInput, ConfigCliOutput>(
                name,
                op_state_store::SideEffect::Read,
                true,
                "cap.software.tched-router.router.read@v1",
                subid,
            ),
        );
        let _ = description;
    }
    for (name, subid, description) in writes {
        schema.methods.insert(
            name.to_string(),
            method_decl_from_schemars_with_output::<ConfigCliInput, ConfigCliOutput>(
                name,
                op_state_store::SideEffect::Mutation,
                false,
                "cap.software.tched-router.router.write@v1",
                subid,
            ),
        );
        let _ = description;
    }
    schema.capabilities.insert(
        "tched-router.read".to_string(),
        CapabilityDecl {
            id: "tched-router.read".to_string(),
            description: "Read ZeroClaw / 3tched Router configuration via config_list/get."
                .to_string(),
        },
    );
    schema.capabilities.insert(
        "tched-router.write".to_string(),
        CapabilityDecl {
            id: "tched-router.write".to_string(),
            description: "Mutate ZeroClaw / 3tched Router configuration via config_set/patch."
                .to_string(),
        },
    );
}

/// Capabilities gated by the hand-carried (non-generated) methods.
const CARRIED_CAPABILITIES: &[(&str, &str)] = &[
    (
        "cap.software.3tched-router.state.read@v1",
        "Read the full 3tched Router declared state.",
    ),
    (
        "cap.software.3tched-router.providers.read@v1",
        "Read the declared provider catalog.",
    ),
    (
        "cap.software.3tched-router.provider.set@v1",
        "Select the active provider.",
    ),
    (
        "cap.software.3tched-router.routes.read@v1",
        "Read declared model routes.",
    ),
    (
        "cap.software.3tched-router.models.read@v1",
        "List models, optionally filtered by provider.",
    ),
    (
        "cap.software.3tched-router.model.set@v1",
        "Select the active model.",
    ),
    (
        "cap.software.3tched-router.route.resolve@v1",
        "Resolve a route hint to a declared model route.",
    ),
    (
        "cap.software.3tched-router.router.read@v1",
        "Read the router configuration.",
    ),
    (
        "cap.software.3tched-router.tools.read@v1",
        "Read the declared LLM tool catalog.",
    ),
    (
        "cap.software.3tched-router.config-schema.read@v1",
        "Read the config schema descriptor.",
    ),
    (
        "cap.software.3tched-router.ui-surfaces.read@v1",
        "List registered UI surfaces.",
    ),
    (
        "cap.software.3tched-router.structured-output.read@v1",
        "Read the structured-output contract.",
    ),
    (
        "cap.software.3tched-router.chat@v1",
        "Send a chat turn through the bridge-owned dispatcher.",
    ),
    (
        "cap.software.tched-router.router.read@v1",
        "Read ZeroClaw / tched_router configuration via config_list/get.",
    ),
    (
        "cap.software.tched-router.selection.set@v1",
        "Select the active provider and model together.",
    ),
    (
        "cap.software.tched-router.router.write@v1",
        "Mutate ZeroClaw / tched_router configuration via config_set/patch.",
    ),
    (
        "cap.software.3tched-router.model-assignments.ovs-routing.set@v1",
        "Set the OVS-routing model assignment.",
    ),
    (
        "cap.software.3tched-router.model-assignments.obfuscation.set@v1",
        "Set the obfuscation model assignment.",
    ),
    (
        "cap.software.3tched-router.model-assignments.vectorization.set@v1",
        "Set the vectorization model assignment.",
    ),
    (
        "cap.software.3tched-router.model-assignments.qdrant-retrieval.set@v1",
        "Set the Qdrant-retrieval model assignment.",
    ),
    (
        "cap.software.3tched-router.model-assignments.cozo-retrieval.set@v1",
        "Set the Cozo-retrieval model assignment.",
    ),
];

/// Public accessor for crates that embed the 3tched Router plugin contract.
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
    pub(crate) fn plain(result: JsonValue) -> Self {
        Self {
            result,
            signal: None,
        }
    }
}

/// Plugin-owned method dispatch for the 3tched Router D-Bus/gRPC surface.
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
            serde_json::json!({ "model_routes": to_json(&state.catalog.model_routes) }),
        )),
        "GetProviderCatalog" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "providers": to_json(&state.catalog.providers) }),
        )),
        "GetTools" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "tools": to_json(&state.catalog.tools) }),
        )),
        "ListProviders" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "providers": to_json(&state.catalog.providers) }),
        )),
        "GetRouter" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "router": to_json(&state.catalog.router) }),
        )),
        "GetConfigSchema" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "config_schema": to_json(&state.catalog.config_schema) }),
        )),
        "ListUiSurfaces" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "ui_surfaces": to_json(&state.catalog.ui_surfaces) }),
        )),
        "GetStructuredOutput" => Ok(DispatchOutcome::plain(
            serde_json::json!({ "structured_output": to_json(&state.catalog.structured_output) }),
        )),
        "ResolveRoute" => resolve_route(json_args, state).map(DispatchOutcome::plain),
        "ListModels" => list_models(json_args, state).map(DispatchOutcome::plain),
        "SetProvider" => set_provider_handler(json_args, state),
        "SetModel" => set_model_handler(json_args, state),
        "SetSelection" => set_selection_handler(json_args, state),
        "SetOvsRoutingModel" => set_role_model_handler(json_args, state, "ovs_routing"),
        "SetObfuscationModel" => set_role_model_handler(json_args, state, "obfuscation"),
        "SetVectorizationModel" => set_role_model_handler(json_args, state, "vectorization"),
        "SetQdrantRetrievalModel" => set_role_model_handler(json_args, state, "qdrant_retrieval"),
        "SetCozoRetrievalModel" => set_role_model_handler(json_args, state, "cozo_retrieval"),
        config_method if zeroclaw_config_subcommand(config_method).is_some() => {
            run_zeroclaw_config(config_method, json_args)
        }
        other => {
            super::tched_router_config_surface::dispatch_config_method(other, json_args, state)
                .unwrap_or_else(|| {
                    Err(TchedRouterError::ExecutionDenied {
                        reason: format!("undeclared method: {other}"),
                    })
                })
        }
    }
}

fn run_zeroclaw_config(
    method: &str,
    json_args: &str,
) -> std::result::Result<DispatchOutcome, TchedRouterError> {
    let args: JsonValue = serde_json::from_str(json_args).unwrap_or_else(|_| serde_json::json!({}));
    let options = args
        .get("options")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let path = args
        .get("path")
        .and_then(JsonValue::as_str)
        .or_else(|| options.get("path").and_then(JsonValue::as_str))
        .unwrap_or("");
    let value_raw = args.get("value").or_else(|| options.get("value"));
    let value_str = match value_raw {
        Some(JsonValue::String(s)) => Some(s.clone()),
        Some(JsonValue::Null) | None => None,
        Some(other) => Some(other.to_string()),
    };
    let sub =
        zeroclaw_config_subcommand(method).ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: format!("undeclared zeroclaw config method: {method}"),
        })?;
    if matches!(sub, "set" | "get") && path.is_empty() {
        return Err(TchedRouterError::ExecutionDenied {
            reason: format!("{method} requires a nonempty string path"),
        });
    }
    let mut command = std::process::Command::new("zeroclaw");
    command.arg("config").arg(sub);
    if let Some(filter) = options.get("filter").and_then(JsonValue::as_str) {
        command.args(["--filter", filter]);
    }
    if options.get("secrets").and_then(JsonValue::as_bool) == Some(true)
        || options.get("secrets").and_then(JsonValue::as_str) == Some("true")
    {
        command.arg("--secrets");
    }
    if !path.is_empty() {
        command.arg(path);
    }
    if let Some(val) = &value_str {
        command.arg(val);
    }
    let is_no_interactive = options.get("no_interactive").and_then(JsonValue::as_bool)
        == Some(true)
        || options.get("no_interactive").and_then(JsonValue::as_str) == Some("true")
        || options.get("no-interactive").and_then(JsonValue::as_bool) == Some(true)
        || options.get("no-interactive").and_then(JsonValue::as_str) == Some("true");
    if is_no_interactive {
        command.arg("--no-interactive");
    }
    let patch_body = if sub == "patch" {
        if let Some(operations) = args.get("operations").and_then(JsonValue::as_array) {
            Some(serde_json::to_string(operations).unwrap_or_else(|_| "[]".to_string()))
        } else {
            Some(
                args.get("input")
                    .and_then(JsonValue::as_str)
                    .or(value_str.as_deref())
                    .unwrap_or("[]")
                    .to_string(),
            )
        }
    } else {
        None
    };
    if patch_body.is_some() && path.is_empty() {
        command.arg("-");
    }
    let output = if let Some(body) = patch_body {
        use std::io::Write;
        use std::process::Stdio;
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| TchedRouterError::ExecutionDenied {
                reason: format!("zeroclaw {method}: {error}"),
            })?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(body.as_bytes()).map_err(|error| {
                TchedRouterError::ExecutionDenied {
                    reason: format!("zeroclaw {method} stdin: {error}"),
                }
            })?;
        }
        child
            .wait_with_output()
            .map_err(|error| TchedRouterError::ExecutionDenied {
                reason: format!("zeroclaw {method}: {error}"),
            })?
    } else {
        command
            .output()
            .map_err(|error| TchedRouterError::ExecutionDenied {
                reason: format!("zeroclaw {method}: {error}"),
            })?
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(TchedRouterError::ExecutionDenied {
            reason: format!(
                "zeroclaw {method} failed: {}",
                stderr.trim().if_empty(stdout.trim())
            ),
        });
    }
    Ok(DispatchOutcome::plain(serde_json::json!({
        "message": stdout,
        "changed": method != "config_list" && method != "config_get",
    })))
}

/// Exact CLI method inventory shared by dispatch and hermetic dispatcher tests.
/// Keeping this mapping pure avoids invoking or mutating a host-installed CLI
/// merely to prove that a schema declaration has an implementation route.
fn zeroclaw_config_subcommand(method: &str) -> Option<&'static str> {
    match method {
        "config_list" => Some("list"),
        "config_get" => Some("get"),
        "config_set" => Some("set"),
        "config_patch" => Some("patch"),
        "config_init" => Some("init"),
        "config_migrate" => Some("migrate"),
        _ => None,
    }
}

trait IfEmpty {
    fn if_empty(self, fallback: Self) -> Self;
}

impl IfEmpty for &str {
    fn if_empty(self, fallback: Self) -> Self {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
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
        .catalog
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
        .catalog
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
    if !state.catalog.providers.iter().any(|p| p.id == provider_id) {
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

fn set_selection_handler(
    json_args: &str,
    state: &TchedRouterState,
) -> std::result::Result<DispatchOutcome, TchedRouterError> {
    let args = parse_args("SetSelection", json_args)?;
    let provider_id = args
        .get("provider_id")
        .or_else(|| args.get("providerId"))
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: "SetSelection requires string field 'provider_id'".to_string(),
        })?;
    let model_id = args
        .get("model_id")
        .or_else(|| args.get("modelId"))
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| TchedRouterError::ExecutionDenied {
            reason: "SetSelection requires string field 'model_id'".to_string(),
        })?;
    if !state.catalog.providers.iter().any(|p| p.id == provider_id) {
        return Err(TchedRouterError::ProviderNotDeclared {
            provider: provider_id,
        });
    }
    if !state
        .catalog
        .model_routes
        .iter()
        .any(|r| r.model == model_id)
    {
        return Err(TchedRouterError::ModelNotDeclared { model: model_id });
    }
    Ok(DispatchOutcome {
        result: serde_json::json!({
            "selected_provider": provider_id,
            "selected_model": model_id,
        }),
        signal: Some(DispatchSignal {
            name: "SelectionChanged".to_string(),
            payload: serde_json::json!({
                "provider": provider_id,
                "model": model_id,
            }),
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
        .catalog
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
        assert!(
            schema.capability_grants.is_empty(),
            "plugin schemas declare capabilities but never grant principal authority"
        );
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
            if method == "PatchConfig" {
                // Mutation that validates + writes the live config file —
                // never exercised by a dispatch test.
                continue;
            }
            if let Some(subcommand) = zeroclaw_config_subcommand(method) {
                assert_eq!(
                    subcommand,
                    method.trim_start_matches("config_"),
                    "{method} has the wrong sealed CLI subcommand"
                );
                continue;
            }
            let args = match method.as_str() {
                "ResolveRoute" => serde_json::json!({ "hint": "balanced" }),
                "ListModels" => serde_json::json!({ "provider": "salad" }),
                "SetProvider" => serde_json::json!({ "provider_id": "salad" }),
                "SetModel" => serde_json::json!({ "model_id": "qwen3.6-27b" }),
                "SetSelection" => serde_json::json!({
                    "provider_id": "salad",
                    "model_id": "qwen3.6-27b"
                }),
                name if name.starts_with("Set") => {
                    serde_json::json!({ "model_id": "qwen3.6-27b" })
                }
                _ => serde_json::json!({}),
            };
            match dispatch_tched_router_method(method, &args.to_string(), &state) {
                Ok(_) => {}
                Err(error) => {
                    let msg = error.to_string();
                    if msg.contains("config section missing")
                        || msg.contains("required field")
                        || msg.contains("unknown field")
                        || msg.contains("execution denied: read")
                    {
                        continue;
                    }
                    panic!("{method} is not executable: {error}");
                }
            }
        }
    }

    #[test]
    fn undeclared_method_is_rejected() {
        let error =
            dispatch_tched_router_method("NotDeclared", "{}", &TchedRouterPlugin::current_state())
                .unwrap_err();
        assert!(error.to_string().contains("undeclared method"));
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(TchedRouterPlugin::new()))
}

/// Version of the official zeroclaw binary whose `config schema` output backs
/// the generated config surface, from `schemas/zeroclaw/VERSION`.
///
/// Read from the captured file rather than the generated surface so this module
/// never depends on generated code in order to compile — that circularity is
/// what makes a stubbed regeneration hard to recover from.
pub fn zeroclaw_binary_version() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/zeroclaw/VERSION"
    ))
    .trim()
}

#[cfg(test)]
mod official_schema_tests {
    //! The OFFICIAL zeroclaw binary is the source of truth for the config
    //! surface: `zeroclaw config schema` is captured at
    //! `schemas/zeroclaw/config.schema.json` and the surface is generated from
    //! that document. There is no linked `zeroclaw` crate to drift against.
    //!
    //! These tests are the alarm for the failure that actually happened: the
    //! dependency resolved to a 193-line stand-in whose every config type was
    //! an empty struct, so the whole surface compiled and sealed while carrying
    //! no fields at all. The previous version of this module early-returned in
    //! exactly that case, which is why nothing caught it.

    use super::zeroclaw_binary_version;
    use crate::state_plugins::tched_router_config_surface::GetGatewayConfigOutput;

    const CAPTURED: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/zeroclaw/config.schema.json"
    ));

    #[test]
    fn captured_schema_is_not_a_stub() {
        let value: serde_json::Value =
            serde_json::from_str(CAPTURED).expect("captured schema is valid JSON");
        let sections = value
            .get("properties")
            .and_then(|p| p.as_object())
            .expect("captured schema has properties");
        let defs = value
            .get("$defs")
            .and_then(|d| d.as_object())
            .expect("captured schema has $defs");
        assert!(
            sections.len() >= 32,
            "captured schema has only {} sections — re-capture with \
             `zeroclaw config schema > schemas/zeroclaw/config.schema.json`",
            sections.len()
        );
        assert!(
            defs.len() >= 64,
            "captured schema has only {} $defs — looks stubbed or truncated",
            defs.len()
        );

        let gateway = defs
            .get("GatewayConfig")
            .and_then(|g| g.get("properties"))
            .and_then(|p| p.as_object())
            .expect("GatewayConfig is defined with properties");
        for field in [
            "port",
            "host",
            "require_pairing",
            "paired_tokens",
            "session_ttl_hours",
        ] {
            assert!(gateway.contains_key(field), "GatewayConfig missing {field}");
        }
        assert_eq!(
            gateway
                .get("require_pairing")
                .and_then(|v| v.get("type"))
                .and_then(|t| t.as_str()),
            Some("boolean")
        );
    }

    #[test]
    fn sealed_method_schema_carries_real_section_fields() {
        // With a stubbed source this passed while the payload was `{}`.
        let schema = schemars::schema_for!(GetGatewayConfigOutput);
        let text = serde_json::to_value(&schema)
            .expect("schema serializes")
            .to_string();
        for field in ["port", "host", "require_pairing", "session_ttl_hours"] {
            assert!(
                text.contains(field),
                "GetGatewayConfigOutput payload is missing `{field}` — generated from a stub?"
            );
        }
    }

    #[test]
    fn captured_schema_records_the_binary_version() {
        let version = zeroclaw_binary_version();
        assert!(!version.is_empty());
        assert_ne!(
            version, "unknown",
            "populate schemas/zeroclaw/VERSION from `zeroclaw --version`"
        );
    }
}
