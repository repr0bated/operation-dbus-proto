//! json_render plugin — GB.JsonRender.
//!
//! Runtime projection for the json-render generative UI framework.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{PluginSchema, SideEffect};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;

// =============================================================================
// PLUGIN ENTRY: identity and typed schema seed
// =============================================================================

const PLUGIN_NAME: &str = "json_render";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "software";
const PLUGIN_DESCRIPTION: &str = "json-render catalog projection — schema, specs, catalogs, renderers, actions, validation, and upstream API inventory";
const PLUGIN_DISPLAY_NAME: &str = "GB.JsonRender";
const JSON_RENDER_SOURCE_COMMIT: &str = "e2d00faeaabe2871ca18a4594a9ec39a245f9b6c";

/// Runtime projection for the json-render generative UI framework.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.schema@v1"))]
#[schemars(extend("x-oscal-category" = "software"))]
pub struct JsonRenderState {
    /// Operational status of the catalog projection.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.status@v1"))]
    pub status: String,
    /// Source authority used to define this projection.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.plugin.json-render.source@v1"))]
    pub source: JsonRenderSource,
    /// Local policy for exposing json-render through D-Bus projections.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.config@v1"))]
    pub config: JsonRenderConfig,
    /// Flat UI tree contract used by json-render specs.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.spec-contract@v1"))]
    pub spec_contract: SpecContract,
    /// Package inventory observed from the upstream workspace.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.packages@v1"))]
    pub packages: Vec<JsonRenderPackage>,
    /// Core exported functions and schemas available to hosts.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.core-exports@v1"))]
    pub core_exports: Vec<ApiExport>,
    /// Renderer packages and their host integration surfaces.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.renderers@v1"))]
    pub renderers: Vec<RendererSurface>,
    /// Catalog component declarations from the shadcn catalog exemplar.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.components@v1"))]
    pub components: Vec<ComponentDecl>,
    /// Built-in runtime actions exposed by the React action provider.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.actions@v1"))]
    pub actions: Vec<ActionDecl>,
    /// Validation checks built into the core validation module.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.validation-checks@v1"))]
    pub validation_checks: Vec<NamedCapability>,
    /// Built-in and custom directive vocabulary.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.directives@v1"))]
    pub directives: Vec<NamedCapability>,
    /// D-Bus method declarations exported by this plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.methods@v1"))]
    pub methods: Vec<MethodSurface>,
    /// Uncapped fields discovered from the authoritative json-render packages.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.inspector-fields@v1"))]
    pub inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
}

/// Source provenance for this plugin's curated json-render model.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "src.software.plugin.json-render.source.schema@v1"))]
pub struct JsonRenderSource {
    /// Documentation home page.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.plugin.json-render.source-docs@v1"))]
    pub docs_url: String,
    /// Upstream repository URL.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.plugin.json-render.source-repository@v1"))]
    pub repository_url: String,
    /// Upstream commit inspected for package and API inventory.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.software.plugin.json-render.source-commit@v1"))]
    pub commit: String,
    /// Short source summary.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.source-summary@v1"))]
    pub summary: String,
}

impl Default for JsonRenderSource {
    fn default() -> Self {
        Self {
            docs_url: "https://json-render.dev/docs".to_string(),
            repository_url: "https://github.com/vercel-labs/json-render".to_string(),
            commit: JSON_RENDER_SOURCE_COMMIT.to_string(),
            summary: "Generative UI framework with schema-defined specs, catalogs, registries, renderers, streaming, validation, actions, directives, and code export.".to_string(),
        }
    }
}

/// Local D-Bus projection policy for json-render.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.config.schema@v1"))]
pub struct JsonRenderConfig {
    /// Allow catalog metadata to be exposed through D-Bus.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.json-render.config-catalog-exposure@v1"))]
    pub expose_catalog: bool,
    /// Allow prompt-building metadata to be exposed through D-Bus.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.json-render.config-prompt-exposure@v1"))]
    pub expose_prompt_surface: bool,
    /// Default schema export mode.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.json-render.config-schema-mode@v1"))]
    pub schema_mode: String,
    /// Default renderer target for UI projection previews.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.json-render.config-renderer@v1"))]
    pub default_renderer: String,
}

impl Default for JsonRenderConfig {
    fn default() -> Self {
        Self {
            expose_catalog: true,
            expose_prompt_surface: true,
            schema_mode: "strict".to_string(),
            default_renderer: "react".to_string(),
        }
    }
}

/// json-render spec grammar at the shape level.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.spec.schema@v1"))]
pub struct SpecContract {
    /// Required root element key.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.spec-root@v1"))]
    pub root_field: String,
    /// Required elements map.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.spec-elements@v1"))]
    pub elements_field: String,
    /// Optional initial state field.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.spec-state@v1"))]
    pub state_field: String,
    /// UI element fields recognized by core.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.element-fields@v1"))]
    pub element_fields: Vec<String>,
    /// Supported JSON Patch operations for streaming edits.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.json-render.patch-ops@v1"))]
    pub patch_ops: Vec<String>,
}

impl Default for SpecContract {
    fn default() -> Self {
        Self {
            root_field: "root".to_string(),
            elements_field: "elements".to_string(),
            state_field: "state".to_string(),
            element_fields: vec![
                "type".to_string(),
                "props".to_string(),
                "children".to_string(),
                "visible".to_string(),
                "on".to_string(),
                "repeat".to_string(),
                "watch".to_string(),
            ],
            patch_ops: vec![
                "add".to_string(),
                "remove".to_string(),
                "replace".to_string(),
                "move".to_string(),
                "copy".to_string(),
                "test".to_string(),
            ],
        }
    }
}

/// Upstream package inventory item.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.package.schema@v1"))]
pub struct JsonRenderPackage {
    /// Workspace folder name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.package-folder@v1"))]
    pub folder: String,
    /// npm package name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.package-name@v1"))]
    pub package_name: String,
    /// Package version observed upstream.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.package-version@v1"))]
    pub version: String,
    /// Package role in the json-render system.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.package-role@v1"))]
    pub role: String,
}

/// Public API export or host method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.api-export.schema@v1"))]
pub struct ApiExport {
    /// Exported symbol.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.api-export-name@v1"))]
    pub name: String,
    /// Package or module that owns this symbol.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.api-export-package@v1"))]
    pub package: String,
    /// Export kind.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.api-export-kind@v1"))]
    pub kind: String,
    /// Operational category.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.api-export-category@v1"))]
    pub category: String,
}

/// Renderer package surface.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.renderer.schema@v1"))]
pub struct RendererSurface {
    /// Renderer identifier.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.renderer-id@v1"))]
    pub id: String,
    /// Package providing this renderer.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.renderer-package@v1"))]
    pub package_name: String,
    /// Render target.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.renderer-target@v1"))]
    pub target: String,
    /// Important exported host methods.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.renderer-exports@v1"))]
    pub exports: Vec<String>,
}

/// Catalog component declaration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.component.schema@v1"))]
pub struct ComponentDecl {
    /// Component name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.component-name@v1"))]
    pub name: String,
    /// Component category.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.component-category@v1"))]
    pub category: String,
    /// Required or commonly used props.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.component-props@v1"))]
    pub props: Vec<String>,
    /// Supported event names.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.component-events@v1"))]
    pub events: Vec<String>,
    /// Human-readable description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.component-description@v1"))]
    pub description: String,
}

/// Runtime action declaration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.action.schema@v1"))]
pub struct ActionDecl {
    /// Action name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.action-name@v1"))]
    pub name: String,
    /// Action parameter names.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.action-params@v1"))]
    pub params: Vec<String>,
    /// Whether the action mutates state.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.action-mutates-state@v1"))]
    pub mutates_state: bool,
    /// Human-readable description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.action-description@v1"))]
    pub description: String,
}

/// Named capability for validation, directives, and feature inventories.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.capability.schema@v1"))]
pub struct NamedCapability {
    /// Capability name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.capability-name@v1"))]
    pub name: String,
    /// Capability category.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.capability-category@v1"))]
    pub category: String,
    /// Short description.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.json-render.capability-description@v1"))]
    pub description: String,
}

/// D-Bus method surface exposed by this state plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.method.schema@v1"))]
pub struct MethodSurface {
    /// D-Bus method name.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.method-name@v1"))]
    pub name: String,
    /// Read or mutation side effect.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.method-side-effect@v1"))]
    pub side_effect: String,
    /// Required capability.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.method-capability@v1"))]
    pub required_capability: String,
    /// Stable OSCAL subid for the method.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.json-render.method-subid@v1"))]
    pub subid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EmptyJsonRenderInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetJsonRenderConfigInput {
    pub config: JsonRenderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComponentLookupInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageLookupInput {
    pub package_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SpecValidationInput {
    pub spec: serde_json::Value,
    #[serde(default)]
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SchemaExportInput {
    #[serde(default)]
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PromptSurfaceInput {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub custom_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealthOutput {
    pub status: String,
    pub package_count: usize,
    pub component_count: usize,
    pub action_count: usize,
    pub source_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigOutput {
    pub config: JsonRenderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetConfigOutput {
    pub accepted: bool,
    pub config: JsonRenderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolsOutput {
    pub tools: Vec<MethodSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackagesOutput {
    pub packages: Vec<JsonRenderPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageOutput {
    pub package: Option<JsonRenderPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComponentsOutput {
    pub components: Vec<ComponentDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComponentOutput {
    pub component: Option<ComponentDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ActionsOutput {
    pub actions: Vec<ActionDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CoreExportsOutput {
    pub exports: Vec<ApiExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RenderersOutput {
    pub renderers: Vec<RendererSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SpecContractOutput {
    pub spec_contract: SpecContract,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ValidationResultOutput {
    pub valid: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SchemaExportOutput {
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PromptSurfaceOutput {
    pub mode: String,
    pub rules: Vec<String>,
    pub components: Vec<String>,
    pub actions: Vec<String>,
}

// =============================================================================
// CHILD COLLECTION TYPES — for json-render `repeat` binding
// =============================================================================
//
// These types support the `repeat` primitive in json-render specs. Each plugin
// that manages child objects (mutations, sessions, events, devices) projects
// them as a `BoundedChildren` collection. The LLM generates specs that bind
// to these collections, and each item carries its own `actions` array so the
// UI adapts per-item without regeneration.
//
// Key insight from Builder.io: "actions are data too" — the available actions
// for each datum are part of the data model, not hardcoded in the UI.

/// Status of a child object for UI rendering.
///
/// Maps to json-render StatusPill component colors and the `repeat` item
/// conditional styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(extend("x-oscal-subid" = "sch.service.child-status.enum@v1"))]
pub enum ChildStatus {
    /// Operation completed successfully.
    Ok,
    /// Operation completed with warnings.
    Warn,
    /// Operation failed.
    Err,
    /// Operation is in progress or queued.
    Pending,
    /// Status cannot be determined.
    Unknown,
}

impl Default for ChildStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl std::fmt::Display for ChildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Warn => write!(f, "warn"),
            Self::Err => write!(f, "err"),
            Self::Pending => write!(f, "pending"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Summary of a child object for json-render `repeat` binding.
///
/// This is the render-oriented index that lands in the collection payload.
/// Each item carries its own `actions` array — the UI adapts per-item without
/// spec regeneration.
///
/// The `data` field holds arbitrary plugin-specific payload for custom rendering.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.child-summary.schema@v1"))]
pub struct ChildSummary {
    /// Unique identifier for this child (UUID or plugin-specific ID).
    #[schemars(extend("x-oscal-subid" = "sch.service.child-summary.id@v1"))]
    pub id: String,

    /// D-Bus object path for direct addressing (e.g., `/org/odbus/mutations/019...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.service.child-summary.dbus-path@v1"))]
    pub dbus_path: Option<String>,

    /// Operation or type label (e.g., "configure", "restart", "session").
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.child-summary.operation@v1"))]
    pub operation: String,

    /// Current status for UI display.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.child-summary.status@v1"))]
    pub status: ChildStatus,

    /// When this item was created or last updated (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.service.child-summary.occurred-at@v1"))]
    pub occurred_at: Option<String>,

    /// Available actions for this item — the UI renders buttons for each.
    /// Example: `["view", "retry", "rollback"]`
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.child-summary.actions@v1"))]
    pub actions: Vec<String>,

    /// Plugin-specific payload for custom rendering.
    /// The LLM binds to fields within this object via `$state` expressions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.service.child-summary.data@v1"))]
    pub data: Option<serde_json::Value>,
}

impl Default for ChildSummary {
    fn default() -> Self {
        Self {
            id: String::new(),
            dbus_path: None,
            operation: String::new(),
            status: ChildStatus::Unknown,
            occurred_at: None,
            actions: Vec::new(),
            data: None,
        }
    }
}

/// Bounded collection of child summaries for json-render `repeat` binding.
///
/// Supports windowed pagination with cursor-based navigation. The `repeat`
/// primitive walks `items`; the UI shows `total` and provides navigation
/// when `next_cursor` is present.
///
/// Items are ordered newest-first by default (LIFO for recent activity).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.bounded-children.schema@v1"))]
pub struct BoundedChildren {
    /// Child summaries in the current window (newest first).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.bounded-children.items@v1"))]
    pub items: Vec<ChildSummary>,

    /// Cursor for fetching the next page, if more items exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-oscal-subid" = "sch.service.bounded-children.next-cursor@v1"))]
    pub next_cursor: Option<String>,

    /// Total number of items across all pages.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.bounded-children.total@v1"))]
    pub total: usize,

    /// Maximum items per window (for UI to show "5 of 42").
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.service.bounded-children.window-size@v1"))]
    pub window_size: usize,
}

impl Default for BoundedChildren {
    fn default() -> Self {
        Self::new(20)
    }
}

impl BoundedChildren {
    /// Create a new bounded collection with the given window size.
    pub fn new(window_size: usize) -> Self {
        Self {
            items: Vec::new(),
            next_cursor: None,
            total: 0,
            window_size,
        }
    }

    /// Push a new child to the front (newest first), maintaining the window bound.
    ///
    /// If the collection exceeds `window_size`, the oldest item is removed and
    /// `next_cursor` is set to enable pagination.
    pub fn push(&mut self, child: ChildSummary) {
        self.items.insert(0, child);
        self.total += 1;

        if self.items.len() > self.window_size {
            if let Some(removed) = self.items.pop() {
                // Set cursor to the removed item's ID for pagination
                self.next_cursor = Some(removed.id);
            }
        }
    }

    /// Upsert a child by ID — update if exists, insert at front if new.
    pub fn upsert(&mut self, child: ChildSummary) {
        if let Some(pos) = self.items.iter().position(|c| c.id == child.id) {
            self.items[pos] = child;
        } else {
            self.push(child);
        }
    }

    /// Remove a child by ID.
    pub fn remove(&mut self, id: &str) -> Option<ChildSummary> {
        if let Some(pos) = self.items.iter().position(|c| c.id == id) {
            self.total = self.total.saturating_sub(1);
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    /// Get a child by ID.
    pub fn get(&self, id: &str) -> Option<&ChildSummary> {
        self.items.iter().find(|c| c.id == id)
    }

    /// Check if the collection has more items beyond the current window.
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }
}

impl Default for JsonRenderState {
    fn default() -> Self {
        Self {
            status: "active".to_string(),
            source: JsonRenderSource::default(),
            config: JsonRenderConfig::default(),
            spec_contract: SpecContract::default(),
            packages: default_packages(),
            core_exports: default_core_exports(),
            renderers: default_renderers(),
            components: default_components(),
            actions: default_actions(),
            validation_checks: default_validation_checks(),
            directives: default_directives(),
            methods: default_methods(),
            inspector_fields: inspector_gadget_generated::InspectorGadgetFields::default(),
        }
    }
}

// =============================================================================
// PLUGIN BODY: D-Bus-backed behavior only
// =============================================================================

pub struct JsonRenderPlugin;

impl Default for JsonRenderPlugin {
    fn default() -> Self {
        Self
    }
}

impl JsonRenderPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn current_state() -> JsonRenderState {
        let mut state = JsonRenderState::default();
        if state.status.is_empty() {
            state.status = "ready".to_string();
        }
        state
    }
}

/// Mutation-path dispatch for json_render UI reads.
pub fn dispatch_json_render_method(
    method: &str,
    state: &JsonRenderState,
) -> Result<serde_json::Value> {
    match method {
        "get_health" => Ok(serde_json::to_value(HealthOutput {
            status: if state.status.is_empty() {
                "ready".to_string()
            } else {
                state.status.clone()
            },
            package_count: state.packages.len(),
            component_count: state.components.len(),
            action_count: state.actions.len(),
            source_commit: if state.source.commit.is_empty() {
                JSON_RENDER_SOURCE_COMMIT.to_string()
            } else {
                state.source.commit.clone()
            },
        })?),
        "get_config" => Ok(serde_json::to_value(ConfigOutput {
            config: state.config.clone(),
        })?),
        "list_tools" => Ok(serde_json::to_value(ToolsOutput {
            tools: state.methods.clone(),
        })?),
        "list_packages" => Ok(serde_json::json!({ "packages": state.packages })),
        "list_components" => Ok(serde_json::json!({ "components": state.components })),
        "list_actions" => Ok(serde_json::json!({ "actions": state.actions })),
        "list_core_exports" => Ok(serde_json::json!({ "exports": state.core_exports })),
        "list_renderers" => Ok(serde_json::json!({ "renderers": state.renderers })),
        "get_spec_schema" => Ok(serde_json::json!({ "spec_contract": state.spec_contract })),
        other => Err(anyhow::anyhow!(
            "json_render method '{other}' has no mutation dispatch arm"
        )),
    }
}

#[async_trait]
impl StatePlugin for JsonRenderPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(json_render_schema())
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

/// Derived `json_render` schema from the typed [`JsonRenderState`] struct via schemars.
pub(crate) fn json_render_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(JsonRenderState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());
    let state = simd_json::serde::to_owned_value(&JsonRenderState::default())
        .expect("JsonRenderState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);

    // Add methods
    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, ConfigOutput>(
            "get_config",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.config.get@v1",
        ),
    );
    schema.methods.insert(
        "set_config".to_string(),
        method_decl_from_schemars_with_output::<SetJsonRenderConfigInput, SetConfigOutput>(
            "set_config",
            SideEffect::Mutation,
            false,
            "json_render.invoke",
            "mut.software.plugin.json-render.config.set@v1",
        ),
    );
    schema.methods.insert(
        "get_health".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, HealthOutput>(
            "get_health",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.health@v1",
        ),
    );
    schema.methods.insert(
        "list_tools".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, ToolsOutput>(
            "list_tools",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.tool.list@v1",
        ),
    );
    schema.methods.insert(
        "list_packages".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, PackagesOutput>(
            "list_packages",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.package.list@v1",
        ),
    );
    schema.methods.insert(
        "get_package".to_string(),
        method_decl_from_schemars_with_output::<PackageLookupInput, PackageOutput>(
            "get_package",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.package.get@v1",
        ),
    );
    schema.methods.insert(
        "list_components".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, ComponentsOutput>(
            "list_components",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.component.list@v1",
        ),
    );
    schema.methods.insert(
        "get_component_schema".to_string(),
        method_decl_from_schemars_with_output::<ComponentLookupInput, ComponentOutput>(
            "get_component_schema",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.component.get@v1",
        ),
    );
    schema.methods.insert(
        "list_actions".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, ActionsOutput>(
            "list_actions",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.action.list@v1",
        ),
    );
    schema.methods.insert(
        "list_core_exports".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, CoreExportsOutput>(
            "list_core_exports",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.core-export.list@v1",
        ),
    );
    schema.methods.insert(
        "list_renderers".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, RenderersOutput>(
            "list_renderers",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.renderer.list@v1",
        ),
    );
    schema.methods.insert(
        "get_spec_schema".to_string(),
        method_decl_from_schemars_with_output::<EmptyJsonRenderInput, SpecContractOutput>(
            "get_spec_schema",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.spec.get@v1",
        ),
    );
    schema.methods.insert(
        "validate_spec".to_string(),
        method_decl_from_schemars_with_output::<SpecValidationInput, ValidationResultOutput>(
            "validate_spec",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.spec.validate@v1",
        ),
    );
    schema.methods.insert(
        "export_json_schema".to_string(),
        method_decl_from_schemars_with_output::<SchemaExportInput, SchemaExportOutput>(
            "export_json_schema",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.schema.export@v1",
        ),
    );
    schema.methods.insert(
        "build_prompt_surface".to_string(),
        method_decl_from_schemars_with_output::<PromptSurfaceInput, PromptSurfaceOutput>(
            "build_prompt_surface",
            SideEffect::Read,
            true,
            "json_render.read",
            "obs.software.plugin.json-render.prompt.build@v1",
        ),
    );

    schema
}

fn default_packages() -> Vec<JsonRenderPackage> {
    [
        (
            "core",
            "@json-render/core",
            "0.19.0",
            "schema, catalog, spec, validation, stream, diff, merge, prompt core",
        ),
        (
            "react",
            "@json-render/react",
            "0.19.0",
            "React renderer, registry, providers, hooks",
        ),
        (
            "next",
            "@json-render/next",
            "0.19.0",
            "Next.js app renderer, routes, layouts, metadata, SSR",
        ),
        (
            "shadcn",
            "@json-render/shadcn",
            "0.19.0",
            "React shadcn/ui catalog",
        ),
        (
            "shadcn-svelte",
            "@json-render/shadcn-svelte",
            "0.19.0",
            "Svelte shadcn catalog",
        ),
        ("vue", "@json-render/vue", "0.19.0", "Vue renderer"),
        ("svelte", "@json-render/svelte", "0.19.0", "Svelte renderer"),
        ("solid", "@json-render/solid", "0.19.0", "Solid renderer"),
        (
            "react-native",
            "@json-render/react-native",
            "0.19.0",
            "React Native renderer",
        ),
        (
            "react-pdf",
            "@json-render/react-pdf",
            "0.19.0",
            "PDF renderer",
        ),
        (
            "react-email",
            "@json-render/react-email",
            "0.19.0",
            "HTML email renderer",
        ),
        (
            "image",
            "@json-render/image",
            "0.19.0",
            "SVG and PNG image renderer via Satori",
        ),
        (
            "remotion",
            "@json-render/remotion",
            "0.19.0",
            "Video composition renderer",
        ),
        ("ink", "@json-render/ink", "0.19.0", "Terminal UI renderer"),
        (
            "react-three-fiber",
            "@json-render/react-three-fiber",
            "0.19.0",
            "3D scene renderer",
        ),
        (
            "directives",
            "@json-render/directives",
            "0.19.0",
            "pre-built directive pack",
        ),
        (
            "codegen",
            "@json-render/codegen",
            "0.19.0",
            "code generation utilities",
        ),
        (
            "devtools",
            "@json-render/devtools",
            "0.19.0",
            "framework-agnostic devtools",
        ),
        (
            "devtools-react",
            "@json-render/devtools-react",
            "0.19.0",
            "React devtools adapter",
        ),
        (
            "devtools-vue",
            "@json-render/devtools-vue",
            "0.19.0",
            "Vue devtools adapter",
        ),
        (
            "devtools-svelte",
            "@json-render/devtools-svelte",
            "0.19.0",
            "Svelte devtools adapter",
        ),
        (
            "devtools-solid",
            "@json-render/devtools-solid",
            "0.19.0",
            "Solid devtools adapter",
        ),
        ("mcp", "@json-render/mcp", "0.19.0", "MCP Apps integration"),
        (
            "yaml",
            "@json-render/yaml",
            "0.19.0",
            "YAML wire format and streaming edits",
        ),
        (
            "redux",
            "@json-render/redux",
            "0.19.0",
            "Redux state adapter",
        ),
        (
            "zustand",
            "@json-render/zustand",
            "0.19.0",
            "Zustand state adapter",
        ),
        (
            "jotai",
            "@json-render/jotai",
            "0.19.0",
            "Jotai state adapter",
        ),
        (
            "xstate",
            "@json-render/xstate",
            "0.19.0",
            "XState store adapter",
        ),
    ]
    .into_iter()
    .map(|(folder, package_name, version, role)| JsonRenderPackage {
        folder: folder.to_string(),
        package_name: package_name.to_string(),
        version: version.to_string(),
        role: role.to_string(),
    })
    .collect()
}

fn default_core_exports() -> Vec<ApiExport> {
    [
        ("defineSchema", "@json-render/core", "function", "schema"),
        ("defineCatalog", "@json-render/core", "function", "catalog"),
        ("buildUserPrompt", "@json-render/core", "function", "prompt"),
        (
            "resolveDynamicValue",
            "@json-render/core",
            "function",
            "state",
        ),
        ("getByPath", "@json-render/core", "function", "state"),
        ("setByPath", "@json-render/core", "function", "state"),
        ("addByPath", "@json-render/core", "function", "state"),
        ("removeByPath", "@json-render/core", "function", "state"),
        ("createStateStore", "@json-render/core", "function", "state"),
        (
            "evaluateVisibility",
            "@json-render/core",
            "function",
            "visibility",
        ),
        (
            "conditionUsesItemScope",
            "@json-render/core",
            "function",
            "visibility",
        ),
        (
            "splitRepeatVisibility",
            "@json-render/core",
            "function",
            "visibility",
        ),
        ("resolvePropValue", "@json-render/core", "function", "props"),
        (
            "resolveElementProps",
            "@json-render/core",
            "function",
            "props",
        ),
        ("resolveBindings", "@json-render/core", "function", "props"),
        (
            "resolveActionParam",
            "@json-render/core",
            "function",
            "props",
        ),
        (
            "defineDirective",
            "@json-render/core",
            "function",
            "directives",
        ),
        (
            "createDirectiveRegistry",
            "@json-render/core",
            "function",
            "directives",
        ),
        (
            "findDirective",
            "@json-render/core",
            "function",
            "directives",
        ),
        ("resolveAction", "@json-render/core", "function", "actions"),
        ("executeAction", "@json-render/core", "function", "actions"),
        (
            "interpolateString",
            "@json-render/core",
            "function",
            "actions",
        ),
        (
            "runValidationCheck",
            "@json-render/core",
            "function",
            "validation",
        ),
        (
            "runValidation",
            "@json-render/core",
            "function",
            "validation",
        ),
        (
            "validateSpec",
            "@json-render/core",
            "function",
            "spec-validation",
        ),
        (
            "autoFixSpec",
            "@json-render/core",
            "function",
            "spec-validation",
        ),
        (
            "formatSpecIssues",
            "@json-render/core",
            "function",
            "spec-validation",
        ),
        (
            "parseSpecStreamLine",
            "@json-render/core",
            "function",
            "streaming",
        ),
        (
            "applySpecStreamPatch",
            "@json-render/core",
            "function",
            "streaming",
        ),
        (
            "applySpecPatch",
            "@json-render/core",
            "function",
            "streaming",
        ),
        ("nestedToFlat", "@json-render/core", "function", "streaming"),
        (
            "compileSpecStream",
            "@json-render/core",
            "function",
            "streaming",
        ),
        (
            "createSpecStreamCompiler",
            "@json-render/core",
            "function",
            "streaming",
        ),
        (
            "createMixedStreamParser",
            "@json-render/core",
            "function",
            "streaming",
        ),
        (
            "createJsonRenderTransform",
            "@json-render/core",
            "function",
            "streaming",
        ),
        (
            "pipeJsonRender",
            "@json-render/core",
            "function",
            "streaming",
        ),
        ("deepMergeSpec", "@json-render/core", "function", "merge"),
        ("diffToPatches", "@json-render/core", "function", "diff"),
    ]
    .into_iter()
    .map(|(name, package, kind, category)| ApiExport {
        name: name.to_string(),
        package: package.to_string(),
        kind: kind.to_string(),
        category: category.to_string(),
    })
    .collect()
}

fn default_renderers() -> Vec<RendererSurface> {
    [
        (
            "react",
            "@json-render/react",
            "React",
            vec![
                "defineRegistry",
                "createRenderer",
                "Renderer",
                "JSONUIProvider",
                "useUIStream",
                "useChatUI",
            ],
        ),
        (
            "next",
            "@json-render/next",
            "Next.js",
            vec![
                "createApp",
                "createRouter",
                "PageRenderer",
                "renderMetadata",
            ],
        ),
        (
            "vue",
            "@json-render/vue",
            "Vue",
            vec!["Renderer", "defineRegistry", "useJsonRender"],
        ),
        (
            "svelte",
            "@json-render/svelte",
            "Svelte",
            vec!["Renderer", "createRegistry"],
        ),
        (
            "solid",
            "@json-render/solid",
            "Solid",
            vec!["Renderer", "defineRegistry"],
        ),
        (
            "react-native",
            "@json-render/react-native",
            "React Native",
            vec!["Renderer", "defineRegistry"],
        ),
        (
            "react-pdf",
            "@json-render/react-pdf",
            "PDF",
            vec!["Renderer", "renderToStream", "renderToBuffer"],
        ),
        (
            "react-email",
            "@json-render/react-email",
            "Email",
            vec!["Renderer", "render"],
        ),
        (
            "image",
            "@json-render/image",
            "Image",
            vec!["renderToSvg", "renderToPng"],
        ),
        (
            "remotion",
            "@json-render/remotion",
            "Video",
            vec!["Renderer", "renderComposition"],
        ),
        ("ink", "@json-render/ink", "Terminal", vec!["Renderer"]),
        (
            "react-three-fiber",
            "@json-render/react-three-fiber",
            "3D",
            vec!["Renderer", "catalog"],
        ),
    ]
    .into_iter()
    .map(|(id, package_name, target, exports)| RendererSurface {
        id: id.to_string(),
        package_name: package_name.to_string(),
        target: target.to_string(),
        exports: exports.into_iter().map(str::to_string).collect(),
    })
    .collect()
}

fn default_components() -> Vec<ComponentDecl> {
    [
        (
            "Card",
            "layout",
            vec!["title", "description", "maxWidth", "centered", "className"],
            vec![],
            "Container card for content sections.",
        ),
        (
            "Stack",
            "layout",
            vec!["direction", "gap", "align", "justify", "className"],
            vec![],
            "Flex container for layouts.",
        ),
        (
            "Grid",
            "layout",
            vec!["columns", "gap", "className"],
            vec![],
            "Grid layout.",
        ),
        (
            "Separator",
            "layout",
            vec!["orientation"],
            vec![],
            "Visual separator line.",
        ),
        (
            "Tabs",
            "layout",
            vec!["tabs", "defaultValue", "value"],
            vec!["change"],
            "Tab navigation.",
        ),
        (
            "Accordion",
            "layout",
            vec!["items", "type"],
            vec![],
            "Collapsible sections.",
        ),
        (
            "Collapsible",
            "layout",
            vec!["title", "defaultOpen"],
            vec![],
            "Collapsible section with trigger.",
        ),
        (
            "Dialog",
            "overlay",
            vec!["title", "description", "openPath"],
            vec![],
            "Modal dialog controlled by state.",
        ),
        (
            "Drawer",
            "overlay",
            vec!["title", "description", "openPath"],
            vec![],
            "Bottom sheet drawer controlled by state.",
        ),
        (
            "Carousel",
            "layout",
            vec!["items"],
            vec![],
            "Horizontally scrollable carousel.",
        ),
        (
            "Table",
            "data",
            vec!["columns", "rows", "caption"],
            vec![],
            "Data table.",
        ),
        (
            "Heading",
            "typography",
            vec!["text", "level"],
            vec![],
            "Heading text.",
        ),
        (
            "Text",
            "typography",
            vec!["text", "variant"],
            vec![],
            "Paragraph text.",
        ),
        (
            "Image",
            "media",
            vec!["src", "alt", "width", "height"],
            vec![],
            "Image or placeholder.",
        ),
        (
            "Avatar",
            "data",
            vec!["src", "name", "size"],
            vec![],
            "User avatar with fallback initials.",
        ),
        (
            "Badge",
            "data",
            vec!["text", "variant"],
            vec![],
            "Status badge.",
        ),
        (
            "Alert",
            "feedback",
            vec!["title", "message", "type"],
            vec![],
            "Alert banner.",
        ),
        (
            "Progress",
            "feedback",
            vec!["value", "max", "label"],
            vec![],
            "Progress bar.",
        ),
        (
            "Skeleton",
            "feedback",
            vec!["width", "height", "rounded"],
            vec![],
            "Loading placeholder.",
        ),
        (
            "Spinner",
            "feedback",
            vec!["size"],
            vec![],
            "Loading spinner.",
        ),
        (
            "Tooltip",
            "overlay",
            vec!["content"],
            vec![],
            "Hover or focus tooltip.",
        ),
        (
            "Popover",
            "overlay",
            vec!["trigger", "content"],
            vec![],
            "Floating popover.",
        ),
        (
            "Input",
            "form",
            vec![
                "label",
                "placeholder",
                "value",
                "type",
                "disabled",
                "checks",
                "validateOn",
            ],
            vec!["change", "blur"],
            "Text input.",
        ),
        (
            "Textarea",
            "form",
            vec!["label", "placeholder", "value", "rows", "disabled"],
            vec!["change", "blur"],
            "Multi-line input.",
        ),
        (
            "Select",
            "form",
            vec!["label", "options", "value", "placeholder", "disabled"],
            vec!["change"],
            "Select input.",
        ),
        (
            "Checkbox",
            "form",
            vec!["label", "checked", "disabled"],
            vec!["change"],
            "Checkbox input.",
        ),
        (
            "Radio",
            "form",
            vec!["label", "options", "value", "disabled"],
            vec!["change"],
            "Radio group.",
        ),
        (
            "Switch",
            "form",
            vec!["label", "checked", "disabled"],
            vec!["change"],
            "Boolean switch.",
        ),
        (
            "Slider",
            "form",
            vec!["value", "min", "max", "step"],
            vec!["change"],
            "Numeric slider.",
        ),
        (
            "Button",
            "action",
            vec!["label", "variant", "disabled"],
            vec!["press"],
            "Clickable button.",
        ),
        (
            "Link",
            "navigation",
            vec!["label", "href", "target"],
            vec!["press"],
            "Navigation link.",
        ),
        (
            "DropdownMenu",
            "action",
            vec!["items", "label"],
            vec!["select"],
            "Dropdown action menu.",
        ),
        (
            "Toggle",
            "form",
            vec!["pressed", "label"],
            vec!["change"],
            "Toggle button.",
        ),
        (
            "ToggleGroup",
            "form",
            vec!["options", "value", "type"],
            vec!["change"],
            "Toggle group.",
        ),
        (
            "ButtonGroup",
            "action",
            vec!["buttons", "selected"],
            vec!["press"],
            "Segmented button group.",
        ),
        (
            "Pagination",
            "navigation",
            vec!["page", "pageCount"],
            vec!["change"],
            "Pagination controls.",
        ),
    ]
    .into_iter()
    .map(
        |(name, category, props, events, description)| ComponentDecl {
            name: name.to_string(),
            category: category.to_string(),
            props: props.into_iter().map(str::to_string).collect(),
            events: events.into_iter().map(str::to_string).collect(),
            description: description.to_string(),
        },
    )
    .collect()
}

fn default_actions() -> Vec<ActionDecl> {
    [
        (
            "setState",
            vec!["statePath", "value"],
            true,
            "Set a state value by JSON Pointer path.",
        ),
        (
            "pushState",
            vec!["statePath", "value", "clearStatePath"],
            true,
            "Append an item to an array in state.",
        ),
        (
            "removeState",
            vec!["statePath", "index"],
            true,
            "Remove an array item by index.",
        ),
        (
            "push",
            vec!["screen"],
            true,
            "Push a screen name onto navigation state.",
        ),
        (
            "pop",
            vec![],
            true,
            "Pop the previous screen from navigation state.",
        ),
        (
            "validateForm",
            vec!["statePath"],
            true,
            "Run registered validation and write the result to state.",
        ),
    ]
    .into_iter()
    .map(|(name, params, mutates_state, description)| ActionDecl {
        name: name.to_string(),
        params: params.into_iter().map(str::to_string).collect(),
        mutates_state,
        description: description.to_string(),
    })
    .collect()
}

fn default_validation_checks() -> Vec<NamedCapability> {
    [
        ("required", "validation", "Require non-empty values."),
        ("email", "validation", "Validate email address shape."),
        ("minLength", "validation", "Require minimum string length."),
        ("maxLength", "validation", "Require maximum string length."),
        (
            "pattern",
            "validation",
            "Validate against a regular expression.",
        ),
        ("min", "validation", "Require minimum numeric value."),
        ("max", "validation", "Require maximum numeric value."),
        ("numeric", "validation", "Require numeric input."),
        ("url", "validation", "Validate URL shape."),
        (
            "matches",
            "validation",
            "Require equality with another value.",
        ),
        ("equalTo", "validation", "Alias for cross-field equality."),
        (
            "lessThan",
            "validation",
            "Require value less than another value.",
        ),
        (
            "greaterThan",
            "validation",
            "Require value greater than another value.",
        ),
    ]
    .into_iter()
    .map(|(name, category, description)| NamedCapability {
        name: name.to_string(),
        category: category.to_string(),
        description: description.to_string(),
    })
    .collect()
}

fn default_directives() -> Vec<NamedCapability> {
    [
        ("$state", "built-in-prop", "Resolve a value from state."),
        (
            "$item",
            "built-in-prop",
            "Resolve a value from the current repeat item.",
        ),
        (
            "$index",
            "built-in-prop",
            "Resolve the current repeat index.",
        ),
        (
            "$bindState",
            "built-in-prop",
            "Create a two-way state binding.",
        ),
        (
            "$bindItem",
            "built-in-prop",
            "Create a repeat item binding.",
        ),
        ("$cond", "built-in-prop", "Conditional prop expression."),
        ("$computed", "built-in-prop", "Computed prop expression."),
        (
            "$template",
            "built-in-prop",
            "Template string prop expression.",
        ),
        ("$format", "directive-pack", "Locale-aware formatting."),
        ("$math", "directive-pack", "Math expression helper."),
        ("$concat", "directive-pack", "Concatenate values."),
        ("$count", "directive-pack", "Count collection values."),
        ("$truncate", "directive-pack", "Truncate text."),
        ("$pluralize", "directive-pack", "Pluralize labels."),
        ("$join", "directive-pack", "Join collections."),
        ("$t", "directive-pack", "i18n translation lookup."),
    ]
    .into_iter()
    .map(|(name, category, description)| NamedCapability {
        name: name.to_string(),
        category: category.to_string(),
        description: description.to_string(),
    })
    .collect()
}

fn default_methods() -> Vec<MethodSurface> {
    [
        (
            "get_config",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.config.get@v1",
        ),
        (
            "set_config",
            "mutation",
            "json_render.invoke",
            "mut.software.plugin.json-render.config.set@v1",
        ),
        (
            "get_health",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.health@v1",
        ),
        (
            "list_tools",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.tool.list@v1",
        ),
        (
            "list_packages",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.package.list@v1",
        ),
        (
            "get_package",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.package.get@v1",
        ),
        (
            "list_components",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.component.list@v1",
        ),
        (
            "get_component_schema",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.component.get@v1",
        ),
        (
            "list_actions",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.action.list@v1",
        ),
        (
            "list_core_exports",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.core-export.list@v1",
        ),
        (
            "list_renderers",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.renderer.list@v1",
        ),
        (
            "get_spec_schema",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.spec.get@v1",
        ),
        (
            "validate_spec",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.spec.validate@v1",
        ),
        (
            "export_json_schema",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.schema.export@v1",
        ),
        (
            "build_prompt_surface",
            "read",
            "json_render.read",
            "obs.software.plugin.json-render.prompt.build@v1",
        ),
    ]
    .into_iter()
    .map(
        |(name, side_effect, required_capability, subid)| MethodSurface {
            name: name.to_string(),
            side_effect: side_effect.to_string(),
            required_capability: required_capability.to_string(),
            subid: subid.to_string(),
        },
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use serde_json::Value as JVal;

    fn collect_subids(value: &JVal, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(JVal::String(subid)) = obj.get("x-oscal-subid") {
                out.push(subid.clone());
            }
            for v in obj.values() {
                collect_subids(v, out);
            }
        }
        if let Some(arr) = value.as_array() {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn all_subids_are_valid() {
        let root = serde_json::to_value(schemars::schema_for!(JsonRenderState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            validate_subid(&subid).unwrap_or_else(|error| panic!("invalid subid {subid}: {error}"));
        }
    }

    #[test]
    fn schema_declares_expected_methods() {
        let schema = json_render_schema();
        for method in [
            "get_config",
            "set_config",
            "get_health",
            "list_tools",
            "list_packages",
            "get_package",
            "list_components",
            "get_component_schema",
            "list_actions",
            "list_core_exports",
            "list_renderers",
            "get_spec_schema",
            "validate_spec",
            "export_json_schema",
            "build_prompt_surface",
        ] {
            assert!(
                schema.methods.contains_key(method),
                "missing method {method}"
            );
        }
    }
}

inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(JsonRenderPlugin::new()))
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
    #[schemars(extend("x-oscal-subid" = "sch.software.json-render.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `ts.packages.codegen.src.serialize.interface.SerializeOptions.field.indent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.indent@v1"))]
        pub indent: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.serialize.interface.SerializeOptions.field.options`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.options@v1"))]
        pub options: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.serialize.interface.SerializeOptions.field.quotes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.quotes@v1"))]
        pub quotes: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.serialize.interface.SerializeOptions.field.str`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.str@v1"))]
        pub str: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.serialize.interface.SerializeOptions.field.value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.value@v1"))]
        pub value: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.condition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.condition@v1"))]
        pub condition: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.depth`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.depth@v1"))]
        pub depth: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.element`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.element@v1"))]
        pub element: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.item`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.item@v1"))]
        pub item: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.key@v1"))]
        pub key: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.parent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.parent@v1"))]
        pub parent: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.paths`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.paths@v1"))]
        pub paths: Option<Vec<String>>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.spec`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.spec@v1"))]
        pub spec: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.startKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.startkey@v1"))]
        pub startkey: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.traverse.interface.TreeVisitor.field.visitor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.visitor@v1"))]
        pub visitor: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.types.interface.GeneratedFile.field.content`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.content@v1"))]
        pub content: Option<String>,

        /// Discovered from Repomix path `ts.packages.codegen.src.types.interface.GeneratedFile.field.path`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.path@v1"))]
        pub path: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.action-observer.interface.ActionDispatchInfo.field.at`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.at@v1"))]
        pub at: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.action-observer.interface.ActionObserver.field.onDispatch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.ondispatch@v1"))]
        pub ondispatch: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.action-observer.interface.ActionObserver.field.onSettle`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.onsettle@v1"))]
        pub onsettle: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.action-observer.interface.ActionSettleInfo.field.durationMs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.durationms@v1"))]
        pub durationms: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.action-observer.interface.ActionSettleInfo.field.error`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.error@v1"))]
        pub error: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.action-observer.interface.ActionSettleInfo.field.ok`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.ok@v1"))]
        pub ok: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.action-observer.interface.ActionSettleInfo.field.result`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.result@v1"))]
        pub result: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionBinding.field.action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionBinding.field.confirm`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.confirm@v1"))]
        pub confirm: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionBinding.field.onError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.onerror@v1"))]
        pub onerror: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionBinding.field.onSuccess`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.onsuccess@v1"))]
        pub onsuccess: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionBinding.field.preventDefault`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.preventdefault@v1"))]
        pub preventdefault: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionConfirm.field.cancelLabel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.cancellabel@v1"))]
        pub cancellabel: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionConfirm.field.confirmLabel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.confirmlabel@v1"))]
        pub confirmlabel: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionConfirm.field.message`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.message@v1"))]
        pub message: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionConfirm.field.title`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.title@v1"))]
        pub title: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionConfirm.field.variant`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.variant@v1"))]
        pub variant: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionExecutionContext.field.ctx`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.ctx@v1"))]
        pub ctx: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionExecutionContext.field.executeAction`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.executeaction@v1"))]
        pub executeaction: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionExecutionContext.field.handler`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.handler@v1"))]
        pub handler: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionExecutionContext.field.navigate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.navigate@v1"))]
        pub navigate: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ActionExecutionContext.field.setState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.setstate@v1"))]
        pub setstate: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ResolvedAction.field.binding`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.binding@v1"))]
        pub binding: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ResolvedAction.field.stateModel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.statemodel@v1"))]
        pub statemodel: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.actions.interface.ResolvedAction.field.template`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.template@v1"))]
        pub template: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.directives.interface.DirectiveDefinition.field.resolve`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.resolve@v1"))]
        pub resolve: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.directives.interface.DirectiveDefinition.field.schema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.schema@v1"))]
        pub schema: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.directives.type.checking.field.definition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.definition@v1"))]
        pub definition: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.edit-modes.interface.BuildEditUserPromptOptions.field.currentSpec`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.currentspec@v1"))]
        pub currentspec: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.edit-modes.interface.BuildEditUserPromptOptions.field.format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.format@v1"))]
        pub format: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.edit-modes.interface.BuildEditUserPromptOptions.field.maxPromptLength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.maxpromptlength@v1"))]
        pub maxpromptlength: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.edit-modes.interface.BuildEditUserPromptOptions.field.prompt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.prompt@v1"))]
        pub prompt: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.edit-modes.interface.BuildEditUserPromptOptions.field.serializer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.serializer@v1"))]
        pub serializer: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.edit-modes.interface.EditConfig.field.modes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.modes@v1"))]
        pub modes: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.env.d.interface.so.field.NODE_ENV`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.node-env@v1"))]
        pub node_env: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.editModes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.editmodes@v1"))]
        pub editmodes: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.state`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.state@v1"))]
        pub state: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.props.interface.PropResolutionContext.field.functions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.functions@v1"))]
        pub functions: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.props.interface.PropResolutionContext.field.itemPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.itempath@v1"))]
        pub itempath: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.props.interface.PropResolutionContext.field.repeatBasePath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.repeatbasepath@v1"))]
        pub repeatbasepath: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.props.type.PropExpression.field.$cond`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.cond@v1"))]
        pub cond: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.props.type.PropExpression.field.$else`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.else-field@v1"))]
        pub else_field: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.props.type.PropExpression.field.$then`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.then@v1"))]
        pub then: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.enum.of.field.catalog`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.catalog@v1"))]
        pub catalog: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field._specType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.spectype@v1"))]
        pub spectype: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field.actionNames`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.actionnames@v1"))]
        pub actionnames: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field.componentNames`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.componentnames@v1"))]
        pub componentnames: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field.data`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.data@v1"))]
        pub data: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.CatalogComponentDef.field.example`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.example@v1"))]
        pub example: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.CatalogComponentDef.field.slots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.slots@v1"))]
        pub slots: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.JsonSchemaOptions.field.strict`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.strict@v1"))]
        pub strict: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.PromptOptions.field.customRules`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.customrules@v1"))]
        pub customrules: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.PromptOptions.field.mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.mode@v1"))]
        pub mode: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.PromptOptions.field.system`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.system@v1"))]
        pub system: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Schema.field.builtInActions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.builtinactions@v1"))]
        pub builtinactions: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Schema.field.defaultRules`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.defaultrules@v1"))]
        pub defaultrules: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Schema.field.promptTemplate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.prompttemplate@v1"))]
        pub prompttemplate: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SchemaBuilder.field.entryShape`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.entryshape@v1"))]
        pub entryshape: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SchemaBuilder.field.shape`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.shape@v1"))]
        pub shape: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SpecValidationResult.field.success`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.success@v1"))]
        pub success: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.InferPropsOfType.field.builder`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.builder@v1"))]
        pub builder: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.InferPropsOfType.field.catalogData`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.catalogdata@v1"))]
        pub catalogdata: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.as.field.context`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.context@v1"))]
        pub context: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.as.field.formatZodType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.formatzodtype@v1"))]
        pub formatzodtype: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.representation.field.inner`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.inner@v1"))]
        pub inner: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.representation.field.optional`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.optional@v1"))]
        pub optional: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.AutoFixOptions.field.lossy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.lossy@v1"))]
        pub lossy: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecIssue.field.code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.code@v1"))]
        pub code: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecIssue.field.elementKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.elementkey@v1"))]
        pub elementkey: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecIssue.field.severity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.severity@v1"))]
        pub severity: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecValidationIssues.field.issues`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.issues@v1"))]
        pub issues: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecValidationIssues.field.valid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.valid@v1"))]
        pub valid: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.ValidateSpecOptions.field.checkOrphans`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.checkorphans@v1"))]
        pub checkorphans: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.state-store.interface.StoreAdapterConfig.field._seen`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.seen@v1"))]
        pub seen: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.state-store.interface.StoreAdapterConfig.field._warned`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.warned@v1"))]
        pub warned: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.state-store.interface.StoreAdapterConfig.field.getSnapshot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.getsnapshot@v1"))]
        pub getsnapshot: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.state-store.interface.StoreAdapterConfig.field.obj`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.obj@v1"))]
        pub obj: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.state-store.interface.StoreAdapterConfig.field.setSnapshot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.setsnapshot@v1"))]
        pub setsnapshot: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.state-store.interface.StoreAdapterConfig.field.subscribe`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.subscribe@v1"))]
        pub subscribe: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.state-store.type.StateModel.field.root`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.root@v1"))]
        pub root: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.fieldName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.fieldname@v1"))]
        pub fieldname: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.from`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.from@v1"))]
        pub from: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.op`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.op@v1"))]
        pub op: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.MixedStreamCallbacks.field.onPatch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.onpatch@v1"))]
        pub onpatch: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.MixedStreamCallbacks.field.onText`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.ontext@v1"))]
        pub ontext: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.MixedStreamParser.field.callbacks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.callbacks@v1"))]
        pub callbacks: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.NestedNode.field.children`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.children@v1"))]
        pub children: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.NestedNode.field.type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.Spec.field.elements`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.elements@v1"))]
        pub elements: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.SpecStreamCompiler.field.initial`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.initial@v1"))]
        pub initial: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.get`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.get@v1"))]
        pub get: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.getServerSnapshot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.getserversnapshot@v1"))]
        pub getserversnapshot: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.set`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.set@v1"))]
        pub set: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.update`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.update@v1"))]
        pub update: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.eq`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.eq@v1"))]
        pub eq: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.gt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.gt@v1"))]
        pub gt: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.gte`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.gte@v1"))]
        pub gte: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.lt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.lt@v1"))]
        pub lt: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.lte`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.lte@v1"))]
        pub lte: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.neq`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.neq@v1"))]
        pub neq: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.not`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.not@v1"))]
        pub not: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.SpecStreamLine.field.patch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.patch@v1"))]
        pub patch: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.StreamChunk.field.controller`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.controller@v1"))]
        pub controller: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.StreamChunk.field.delta`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.delta@v1"))]
        pub delta: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.StreamChunk.field.line`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.line@v1"))]
        pub line: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.cast.field.stream`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.stream@v1"))]
        pub stream: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.on`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.on@v1"))]
        pub on: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.parentKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.parentkey@v1"))]
        pub parentkey: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.repeat`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.repeat@v1"))]
        pub repeat: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.visible`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.visible@v1"))]
        pub visible: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.watch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.watch@v1"))]
        pub watch: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationCheck.field.args`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.args@v1"))]
        pub args: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationConfig.field.checks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.checks@v1"))]
        pub checks: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationConfig.field.enabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.enabled@v1"))]
        pub enabled: Option<bool>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationConfig.field.validateOn`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.validateon@v1"))]
        pub validateon: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationContext.field.check`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.check@v1"))]
        pub check: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationContext.field.customFunctions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.customfunctions@v1"))]
        pub customfunctions: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationFunctionDefinition.field.validate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.validate@v1"))]
        pub validate: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationResult.field.errors`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.errors@v1"))]
        pub errors: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.visibility.interface.VisibilityContext.field.repeatIndex`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.repeatindex@v1"))]
        pub repeatindex: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.visibility.interface.VisibilityContext.field.repeatItem`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.repeatitem@v1"))]
        pub repeatitem: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.build-app-html.interface.BuildAppHtmlOptions.field.css`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.css@v1"))]
        pub css: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.build-app-html.interface.BuildAppHtmlOptions.field.head`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.head@v1"))]
        pub head: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.build-app-html.interface.BuildAppHtmlOptions.field.js`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.js@v1"))]
        pub js: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.types.interface.CreateMcpAppOptions.field.html`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.html@v1"))]
        pub html: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.types.interface.CreateMcpAppOptions.field.tool`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.tool@v1"))]
        pub tool: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.types.interface.RegisterResourceOptions.field.resourceUri`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.resourceuri@v1"))]
        pub resourceuri: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.use-json-render-app.interface.ToolResultContent.field.text`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.text@v1"))]
        pub text: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.use-json-render-app.interface.UseJsonRenderAppReturn.field.app`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.app@v1"))]
        pub app: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.use-json-render-app.interface.UseJsonRenderAppReturn.field.callServerTool`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.callservertool@v1"))]
        pub callservertool: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.use-json-render-app.interface.UseJsonRenderAppReturn.field.connected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.connected@v1"))]
        pub connected: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.use-json-render-app.interface.UseJsonRenderAppReturn.field.connecting`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.connecting@v1"))]
        pub connecting: Option<String>,

        /// Discovered from Repomix path `ts.packages.mcp.src.use-json-render-app.interface.UseJsonRenderAppReturn.field.loading`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.loading@v1"))]
        pub loading: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.catalog-types.interface.BaseComponentProps.field.bindings`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.bindings@v1"))]
        pub bindings: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.catalog-types.interface.BaseComponentProps.field.emit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.emit@v1"))]
        pub emit: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.catalog-types.interface.EventHandle.field.bound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.bound@v1"))]
        pub bound: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.catalog-types.interface.EventHandle.field.shouldPreventDefault`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.shouldpreventdefault@v1"))]
        pub shouldpreventdefault: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.catalog-types.type.SetState.field.updater`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.updater@v1"))]
        pub updater: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ActionContextValue.field.cancel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.cancel@v1"))]
        pub cancel: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ActionContextValue.field.execute`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.execute@v1"))]
        pub execute: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ActionContextValue.field.handlers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.handlers@v1"))]
        pub handlers: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ActionContextValue.field.loadingActions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.loadingactions@v1"))]
        pub loadingactions: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ActionContextValue.field.pendingConfirmation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.pendingconfirmation@v1"))]
        pub pendingconfirmation: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ActionContextValue.field.registerHandler`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.registerhandler@v1"))]
        pub registerhandler: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ConfirmDialogProps.field.onCancel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.oncancel@v1"))]
        pub oncancel: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.ConfirmDialogProps.field.onConfirm`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.onconfirm@v1"))]
        pub onconfirm: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.actions.interface.PendingConfirmation.field.reject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.reject@v1"))]
        pub reject: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.repeat-scope.type.ReactNode.field.basePath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.basepath@v1"))]
        pub basepath: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.repeat-scope.type.ReactNode.field.index`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.index@v1"))]
        pub index: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.FieldValidationState.field.touched`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.touched@v1"))]
        pub touched: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.FieldValidationState.field.validated`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.validated@v1"))]
        pub validated: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.ValidationContextValue.field.clear`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.clear@v1"))]
        pub clear: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.ValidationContextValue.field.fieldStates`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.fieldstates@v1"))]
        pub fieldstates: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.ValidationContextValue.field.registerField`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.registerfield@v1"))]
        pub registerfield: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.ValidationContextValue.field.touch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.touch@v1"))]
        pub touch: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.ValidationContextValue.field.validateAll`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.validateall@v1"))]
        pub validateall: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.ValidationProviderProps.field.a`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.a@v1"))]
        pub a: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.validation.interface.ValidationProviderProps.field.b`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.b@v1"))]
        pub b: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.contexts.visibility.interface.VisibilityContextValue.field.isVisible`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.isvisible@v1"))]
        pub isvisible: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.interface.TokenUsage.field.completionTokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.completiontokens@v1"))]
        pub completiontokens: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.interface.TokenUsage.field.promptTokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.prompttokens@v1"))]
        pub prompttokens: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.interface.TokenUsage.field.totalTokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.totaltokens@v1"))]
        pub totaltokens: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.interface.UseChatUIOptions.field.api`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.api@v1"))]
        pub api: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.interface.UseChatUIOptions.field.onComplete`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.oncomplete@v1"))]
        pub oncomplete: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.type.for.field.bindingPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.bindingpath@v1"))]
        pub bindingpath: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.type.for.field.isStreaming`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.isstreaming@v1"))]
        pub isstreaming: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.type.for.field.messages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.messages@v1"))]
        pub messages: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.type.for.field.propValue`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.propvalue@v1"))]
        pub propvalue: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.type.for.field.rawLines`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.rawlines@v1"))]
        pub rawlines: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.type.for.field.send`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.send@v1"))]
        pub send: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.hooks.type.for.field.usage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.usage@v1"))]
        pub usage: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.renderer.class.ElementErrorBoundary.field.actionName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.actionname@v1"))]
        pub actionname: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.renderer.class.ElementErrorBoundary.field.fallback`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.fallback@v1"))]
        pub fallback: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.renderer.class.ElementErrorBoundary.field.getSetState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.getsetstate@v1"))]
        pub getsetstate: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.renderer.class.ElementErrorBoundary.field.getState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.getstate@v1"))]
        pub getstate: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.renderer.class.ElementErrorBoundary.field.registry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.registry@v1"))]
        pub registry: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.renderer.interface.ElementErrorBoundaryProps.field.elementType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.elementtype@v1"))]
        pub elementtype: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.renderer.interface.ElementErrorBoundaryState.field.hasError`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.haserror@v1"))]
        pub haserror: Option<String>,

        /// Discovered from Repomix path `ts.packages.react.src.schema.type.from.field.createCatalog`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.json-render.createcatalog@v1"))]
        pub createcatalog: Option<String>,
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

    /// Typed input candidate for `escapestring` discovered at `ts.packages.codegen.src.serialize.function.escapeString`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EscapestringInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EscapestringOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `serializepropvalue` discovered at `ts.packages.codegen.src.serialize.function.serializePropValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SerializepropvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SerializepropvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `serializeprops` discovered at `ts.packages.codegen.src.serialize.function.serializeProps`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SerializepropsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SerializepropsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `collectactions` discovered at `ts.packages.codegen.src.traverse.function.collectActions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CollectactionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CollectactionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `collectpathfromitem` discovered at `ts.packages.codegen.src.traverse.function.collectPathFromItem`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CollectpathfromitemInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CollectpathfromitemOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `collectpathsfromcondition` discovered at `ts.packages.codegen.src.traverse.function.collectPathsFromCondition`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CollectpathsfromconditionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CollectpathsfromconditionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `collectstatepaths` discovered at `ts.packages.codegen.src.traverse.function.collectStatePaths`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CollectstatepathsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CollectstatepathsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `collectusedcomponents` discovered at `ts.packages.codegen.src.traverse.function.collectUsedComponents`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CollectusedcomponentsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CollectusedcomponentsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `traversespec` discovered at `ts.packages.codegen.src.traverse.function.traverseSpec`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct TraversespecInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct TraversespecOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `visit` discovered at `ts.packages.codegen.src.traverse.function.visit`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct VisitInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct VisitOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `generate` discovered at `ts.packages.codegen.src.types.interface.CodeGenerator.method.generate`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GenerateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GenerateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `nextactiondispatchid` discovered at `ts.packages.core.src.action-observer.function.nextActionDispatchId`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NextactiondispatchidInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NextactiondispatchidOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `notifyactiondispatch` discovered at `ts.packages.core.src.action-observer.function.notifyActionDispatch`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NotifyactiondispatchInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NotifyactiondispatchOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `notifyactionsettle` discovered at `ts.packages.core.src.action-observer.function.notifyActionSettle`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NotifyactionsettleInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NotifyactionsettleOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `registeractionobserver` discovered at `ts.packages.core.src.action-observer.function.registerActionObserver`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RegisteractionobserverInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RegisteractionobserverOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `executeaction` discovered at `ts.packages.core.src.actions.function.executeAction`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ExecuteactionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ExecuteactionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `interpolatestring` discovered at `ts.packages.core.src.actions.function.interpolateString`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct InterpolatestringInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct InterpolatestringOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolveaction` discovered at `ts.packages.core.src.actions.function.resolveAction`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolveactionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolveactionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isdevtoolsactive` discovered at `ts.packages.core.src.devtools-flag.function.isDevtoolsActive`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsdevtoolsactiveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsdevtoolsactiveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `markdevtoolsactive` discovered at `ts.packages.core.src.devtools-flag.function.markDevtoolsActive`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MarkdevtoolsactiveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MarkdevtoolsactiveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `notifydevtoolsactivechange` discovered at `ts.packages.core.src.devtools-flag.function.notifyDevtoolsActiveChange`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NotifydevtoolsactivechangeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NotifydevtoolsactivechangeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `subscribedevtoolsactive` discovered at `ts.packages.core.src.devtools-flag.function.subscribeDevtoolsActive`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SubscribedevtoolsactiveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SubscribedevtoolsactiveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `arraysequal` discovered at `ts.packages.core.src.diff.function.arraysEqual`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ArraysequalInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ArraysequalOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `buildpath` discovered at `ts.packages.core.src.diff.function.buildPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuildpathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuildpathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `difftopatches` discovered at `ts.packages.core.src.diff.function.diffToPatches`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DifftopatchesInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DifftopatchesOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `escapetoken` discovered at `ts.packages.core.src.diff.function.escapeToken`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EscapetokenInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EscapetokenOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isplainobject` discovered at `ts.packages.core.src.diff.function.isPlainObject`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsplainobjectInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsplainobjectOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `createdirectiveregistry` discovered at `ts.packages.core.src.directives.function.createDirectiveRegistry`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CreatedirectiveregistryInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CreatedirectiveregistryOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `finddirective` discovered at `ts.packages.core.src.directives.function.findDirective`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FinddirectiveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FinddirectiveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `addlinenumbers` discovered at `ts.packages.core.src.edit-modes.function.addLineNumbers`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AddlinenumbersInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AddlinenumbersOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `buildeditinstructions` discovered at `ts.packages.core.src.edit-modes.function.buildEditInstructions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuildeditinstructionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuildeditinstructionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `buildedituserprompt` discovered at `ts.packages.core.src.edit-modes.function.buildEditUserPrompt`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuildedituserpromptInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuildedituserpromptOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isnonemptyspec` discovered at `ts.packages.core.src.edit-modes.function.isNonEmptySpec`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsnonemptyspecInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsnonemptyspecOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `jsondiffinstructions` discovered at `ts.packages.core.src.edit-modes.function.jsonDiffInstructions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct JsondiffinstructionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct JsondiffinstructionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `jsonmergeinstructions` discovered at `ts.packages.core.src.edit-modes.function.jsonMergeInstructions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct JsonmergeinstructionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct JsonmergeinstructionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `jsonpatchinstructions` discovered at `ts.packages.core.src.edit-modes.function.jsonPatchInstructions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct JsonpatchinstructionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct JsonpatchinstructionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `modeselectionguidance` discovered at `ts.packages.core.src.edit-modes.function.modeSelectionGuidance`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ModeselectionguidanceInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ModeselectionguidanceOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `normalizemodes` discovered at `ts.packages.core.src.edit-modes.function.normalizeModes`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NormalizemodesInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NormalizemodesOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `yamldiffinstructions` discovered at `ts.packages.core.src.edit-modes.function.yamlDiffInstructions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct YamldiffinstructionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct YamldiffinstructionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `yamlmergeinstructions` discovered at `ts.packages.core.src.edit-modes.function.yamlMergeInstructions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct YamlmergeinstructionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct YamlmergeinstructionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `yamlpatchinstructions` discovered at `ts.packages.core.src.edit-modes.function.yamlPatchInstructions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct YamlpatchinstructionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct YamlpatchinstructionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `deepmergespec` discovered at `ts.packages.core.src.merge.function.deepMergeSpec`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DeepmergespecInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DeepmergespecOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `builduserprompt` discovered at `ts.packages.core.src.prompt.function.buildUserPrompt`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuilduserpromptInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuilduserpromptOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resetwarnedcomputedfns` discovered at `ts.packages.core.src.props.function._resetWarnedComputedFns`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResetwarnedcomputedfnsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResetwarnedcomputedfnsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isbinditemexpression` discovered at `ts.packages.core.src.props.function.isBindItemExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsbinditemexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsbinditemexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isbindstateexpression` discovered at `ts.packages.core.src.props.function.isBindStateExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsbindstateexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsbindstateexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `iscomputedexpression` discovered at `ts.packages.core.src.props.function.isComputedExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IscomputedexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IscomputedexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `iscondexpression` discovered at `ts.packages.core.src.props.function.isCondExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IscondexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IscondexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isindexexpression` discovered at `ts.packages.core.src.props.function.isIndexExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsindexexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsindexexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isitemexpression` discovered at `ts.packages.core.src.props.function.isItemExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsitemexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsitemexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isstateexpression` discovered at `ts.packages.core.src.props.function.isStateExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsstateexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsstateexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `istemplateexpression` discovered at `ts.packages.core.src.props.function.isTemplateExpression`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IstemplateexpressionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IstemplateexpressionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolveactionparam` discovered at `ts.packages.core.src.props.function.resolveActionParam`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolveactionparamInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolveactionparamOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolvebinditempath` discovered at `ts.packages.core.src.props.function.resolveBindItemPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolvebinditempathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolvebinditempathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolvebindings` discovered at `ts.packages.core.src.props.function.resolveBindings`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolvebindingsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolvebindingsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolveelementprops` discovered at `ts.packages.core.src.props.function.resolveElementProps`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolveelementpropsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolveelementpropsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolvepropvalue` discovered at `ts.packages.core.src.props.function.resolvePropValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolvepropvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolvepropvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `buildzodschemafromdefinition` discovered at `ts.packages.core.src.schema.function.buildZodSchemaFromDefinition`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuildzodschemafromdefinitionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuildzodschemafromdefinitionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `buildzodtype` discovered at `ts.packages.core.src.schema.function.buildZodType`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuildzodtypeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuildzodtypeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `createbuilder` discovered at `ts.packages.core.src.schema.function.createBuilder`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CreatebuilderInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CreatebuilderOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `findfirststringprop` discovered at `ts.packages.core.src.schema.function.findFirstStringProp`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FindfirststringpropInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FindfirststringpropOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `formatzodtype` discovered at `ts.packages.core.src.schema.function.formatZodType`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FormatzodtypeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FormatzodtypeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `generateexamplepropsfromzod` discovered at `ts.packages.core.src.schema.function.generateExamplePropsFromZod`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GenerateexamplepropsfromzodInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GenerateexamplepropsfromzodOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `generateexamplevalue` discovered at `ts.packages.core.src.schema.function.generateExampleValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GenerateexamplevalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GenerateexamplevalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getexampleprops` discovered at `ts.packages.core.src.schema.function.getExampleProps`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetexamplepropsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetexamplepropsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getkeysfrompath` discovered at `ts.packages.core.src.schema.function.getKeysFromPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetkeysfrompathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetkeysfrompathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getpropsfrompath` discovered at `ts.packages.core.src.schema.function.getPropsFromPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetpropsfrompathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetpropsfrompathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getzodtypename` discovered at `ts.packages.core.src.schema.function.getZodTypeName`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetzodtypenameInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetzodtypenameOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `normalizetypename` discovered at `ts.packages.core.src.schema.function.normalizeTypeName`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NormalizetypenameInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NormalizetypenameOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `zodtojsonschema` discovered at `ts.packages.core.src.schema.function.zodToJsonSchema`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ZodtojsonschemaInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ZodtojsonschemaOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `zodtypename` discovered at `ts.packages.core.src.schema.function.zodTypeName`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ZodtypenameInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ZodtypenameOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `jsonschema` discovered at `ts.packages.core.src.schema.interface.Catalog.method.jsonSchema`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct JsonschemaInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct JsonschemaOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `prompt` discovered at `ts.packages.core.src.schema.interface.Catalog.method.prompt`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PromptInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PromptOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `validate` discovered at `ts.packages.core.src.schema.interface.Catalog.method.validate`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ValidateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ValidateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `zodschema` discovered at `ts.packages.core.src.schema.interface.Catalog.method.zodSchema`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ZodschemaInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ZodschemaOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `any` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.any`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AnyInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AnyOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `boolean` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.boolean`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BooleanInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BooleanOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `number` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.number`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NumberInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NumberOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `optional` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.optional`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct OptionalInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct OptionalOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `propsof` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.propsOf`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PropsofInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PropsofOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `ref_field` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.ref`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RefFieldInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RefFieldOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `string` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.string`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct StringInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct StringOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `zod` discovered at `ts.packages.core.src.schema.interface.SchemaBuilder.method.zod`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ZodInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ZodOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `autofixspec` discovered at `ts.packages.core.src.spec-validator.function.autoFixSpec`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AutofixspecInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AutofixspecOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `formatspecissues` discovered at `ts.packages.core.src.spec-validator.function.formatSpecIssues`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FormatspecissuesInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FormatspecissuesOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `createstatestore` discovered at `ts.packages.core.src.state-store.function.createStateStore`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CreatestatestoreInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CreatestatestoreOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `createstoreadapter` discovered at `ts.packages.core.src.state-store.function.createStoreAdapter`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CreatestoreadapterInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CreatestoreadapterOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `flattentopointers` discovered at `ts.packages.core.src.state-store.function.flattenToPointers`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FlattentopointersInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FlattentopointersOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `immutablesetbypath` discovered at `ts.packages.core.src.state-store.function.immutableSetByPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ImmutablesetbypathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ImmutablesetbypathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `notify` discovered at `ts.packages.core.src.state-store.function.notify`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NotifyInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NotifyOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `get` discovered at `ts.packages.core.src.state-store.type.StateModel.method.get`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getserversnapshot` discovered at `ts.packages.core.src.state-store.type.StateModel.method.getServerSnapshot`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetserversnapshotInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetserversnapshotOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getsnapshot` discovered at `ts.packages.core.src.state-store.type.StateModel.method.getSnapshot`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetsnapshotInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetsnapshotOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `set` discovered at `ts.packages.core.src.state-store.type.StateModel.method.set`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SetInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SetOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `update` discovered at `ts.packages.core.src.state-store.type.StateModel.method.update`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UpdateInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UpdateOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `addbypath` discovered at `ts.packages.core.src.types.function.addByPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct AddbypathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AddbypathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `applyspecpatch` discovered at `ts.packages.core.src.types.function.applySpecPatch`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ApplyspecpatchInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ApplyspecpatchOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `closetextblock` discovered at `ts.packages.core.src.types.function.closeTextBlock`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ClosetextblockInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ClosetextblockOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `createjsonrendertransform` discovered at `ts.packages.core.src.types.function.createJsonRenderTransform`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CreatejsonrendertransformInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CreatejsonrendertransformOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `createmixedstreamparser` discovered at `ts.packages.core.src.types.function.createMixedStreamParser`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CreatemixedstreamparserInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CreatemixedstreamparserOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `deepequal` discovered at `ts.packages.core.src.types.function.deepEqual`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DeepequalInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DeepequalOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `emitpatch` discovered at `ts.packages.core.src.types.function.emitPatch`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EmitpatchInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EmitpatchOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `emittextdelta` discovered at `ts.packages.core.src.types.function.emitTextDelta`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EmittextdeltaInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EmittextdeltaOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `ensuretextblock` discovered at `ts.packages.core.src.types.function.ensureTextBlock`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EnsuretextblockInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EnsuretextblockOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `findformvalue` discovered at `ts.packages.core.src.types.function.findFormValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FindformvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FindformvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `flushbuffer` discovered at `ts.packages.core.src.types.function.flushBuffer`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FlushbufferInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FlushbufferOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getbypath` discovered at `ts.packages.core.src.types.function.getByPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetbypathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetbypathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isnumericindex` discovered at `ts.packages.core.src.types.function.isNumericIndex`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsnumericindexInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsnumericindexOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `nestedtoflat` discovered at `ts.packages.core.src.types.function.nestedToFlat`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct NestedtoflatInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct NestedtoflatOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `parsejsonpointer` discovered at `ts.packages.core.src.types.function.parseJsonPointer`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ParsejsonpointerInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ParsejsonpointerOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `parsespecstreamline` discovered at `ts.packages.core.src.types.function.parseSpecStreamLine`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ParsespecstreamlineInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ParsespecstreamlineOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `processcompleteline` discovered at `ts.packages.core.src.types.function.processCompleteLine`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ProcesscompletelineInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ProcesscompletelineOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `processline` discovered at `ts.packages.core.src.types.function.processLine`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ProcesslineInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ProcesslineOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `removebypath` discovered at `ts.packages.core.src.types.function.removeByPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RemovebypathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RemovebypathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `setbypath` discovered at `ts.packages.core.src.types.function.setByPath`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SetbypathInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SetbypathOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `unescapejsonpointer` discovered at `ts.packages.core.src.types.function.unescapeJsonPointer`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UnescapejsonpointerInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UnescapejsonpointerOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `walk` discovered at `ts.packages.core.src.types.function.walk`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct WalkInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct WalkOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `flush` discovered at `ts.packages.core.src.types.interface.MixedStreamParser.method.flush`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FlushInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FlushOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `push` discovered at `ts.packages.core.src.types.interface.MixedStreamParser.method.push`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct PushInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PushOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getpatches` discovered at `ts.packages.core.src.types.interface.SpecStreamCompiler.method.getPatches`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetpatchesInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetpatchesOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getresult` discovered at `ts.packages.core.src.types.interface.SpecStreamCompiler.method.getResult`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetresultInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetresultOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `reset` discovered at `ts.packages.core.src.types.interface.SpecStreamCompiler.method.reset`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResetInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResetOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `runvalidation` discovered at `ts.packages.core.src.validation.function.runValidation`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RunvalidationInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RunvalidationOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `runvalidationcheck` discovered at `ts.packages.core.src.validation.function.runValidationCheck`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RunvalidationcheckInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RunvalidationcheckOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `conditionusesitemscope` discovered at `ts.packages.core.src.visibility.function.conditionUsesItemScope`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ConditionusesitemscopeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConditionusesitemscopeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `evaluatecondition` discovered at `ts.packages.core.src.visibility.function.evaluateCondition`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EvaluateconditionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EvaluateconditionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `evaluatevisibility` discovered at `ts.packages.core.src.visibility.function.evaluateVisibility`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EvaluatevisibilityInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EvaluatevisibilityOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isandcondition` discovered at `ts.packages.core.src.visibility.function.isAndCondition`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsandconditionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsandconditionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isindexcondition` discovered at `ts.packages.core.src.visibility.function.isIndexCondition`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsindexconditionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsindexconditionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isitemcondition` discovered at `ts.packages.core.src.visibility.function.isItemCondition`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsitemconditionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsitemconditionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isorcondition` discovered at `ts.packages.core.src.visibility.function.isOrCondition`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsorconditionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsorconditionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolvecomparisonvalue` discovered at `ts.packages.core.src.visibility.function.resolveComparisonValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolvecomparisonvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolvecomparisonvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `resolveconditionvalue` discovered at `ts.packages.core.src.visibility.function.resolveConditionValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ResolveconditionvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolveconditionvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `splitrepeatvisibility` discovered at `ts.packages.core.src.visibility.function.splitRepeatVisibility`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SplitrepeatvisibilityInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SplitrepeatvisibilityOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `mcpappview` discovered at `ts.packages.mcp.src.app.function.McpAppView`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct McpappviewInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct McpappviewOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `buildapphtml` discovered at `ts.packages.mcp.src.build-app-html.function.buildAppHtml`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuildapphtmlInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuildapphtmlOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `escapehtml` discovered at `ts.packages.mcp.src.build-app-html.function.escapeHtml`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct EscapehtmlInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct EscapehtmlOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `createmcpapp` discovered at `ts.packages.mcp.src.server.function.createMcpApp`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct CreatemcpappInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct CreatemcpappOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getextapps` discovered at `ts.packages.mcp.src.server.function.getExtApps`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetextappsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetextappsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `registerjsonrenderresource` discovered at `ts.packages.mcp.src.server.function.registerJsonRenderResource`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RegisterjsonrenderresourceInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RegisterjsonrenderresourceOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `registerjsonrendertool` discovered at `ts.packages.mcp.src.server.function.registerJsonRenderTool`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RegisterjsonrendertoolInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RegisterjsonrendertoolOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `parsespecfromtoolresult` discovered at `ts.packages.mcp.src.use-json-render-app.function.parseSpecFromToolResult`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ParsespecfromtoolresultInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ParsespecfromtoolresultOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usejsonrenderapp` discovered at `ts.packages.mcp.src.use-json-render-app.function.useJsonRenderApp`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsejsonrenderappInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsejsonrenderappOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `actionprovider` discovered at `ts.packages.react.src.contexts.actions.function.ActionProvider`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ActionproviderInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ActionproviderOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `deepresolvevalue` discovered at `ts.packages.react.src.contexts.actions.function.deepResolveValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DeepresolvevalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DeepresolvevalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `generateuniqueid` discovered at `ts.packages.react.src.contexts.actions.function.generateUniqueId`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GenerateuniqueidInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GenerateuniqueidOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `useaction` discovered at `ts.packages.react.src.contexts.actions.function.useAction`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UseactionInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UseactionOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `useactions` discovered at `ts.packages.react.src.contexts.actions.function.useActions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UseactionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UseactionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `repeatscopeprovider` discovered at `ts.packages.react.src.contexts.repeat-scope.function.RepeatScopeProvider`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RepeatscopeproviderInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RepeatscopeproviderOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `userepeatscope` discovered at `ts.packages.react.src.contexts.repeat-scope.function.useRepeatScope`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UserepeatscopeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UserepeatscopeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `validationprovider` discovered at `ts.packages.react.src.contexts.validation.function.ValidationProvider`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ValidationproviderInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ValidationproviderOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `dynamicargsequal` discovered at `ts.packages.react.src.contexts.validation.function.dynamicArgsEqual`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct DynamicargsequalInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct DynamicargsequalOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usefieldvalidation` discovered at `ts.packages.react.src.contexts.validation.function.useFieldValidation`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsefieldvalidationInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsefieldvalidationOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `useoptionalvalidation` discovered at `ts.packages.react.src.contexts.validation.function.useOptionalValidation`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UseoptionalvalidationInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UseoptionalvalidationOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usevalidation` discovered at `ts.packages.react.src.contexts.validation.function.useValidation`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsevalidationInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsevalidationOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `validationconfigequal` discovered at `ts.packages.react.src.contexts.validation.function.validationConfigEqual`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ValidationconfigequalInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ValidationconfigequalOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `visibilityprovider` discovered at `ts.packages.react.src.contexts.visibility.function.VisibilityProvider`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct VisibilityproviderInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct VisibilityproviderOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `useisvisible` discovered at `ts.packages.react.src.contexts.visibility.function.useIsVisible`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UseisvisibleInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UseisvisibleOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usevisibility` discovered at `ts.packages.react.src.contexts.visibility.function.useVisibility`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsevisibilityInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsevisibilityOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `messagebubble` discovered at `ts.packages.react.src.hooks.function.MessageBubble`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct MessagebubbleInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct MessagebubbleOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `applypatch` discovered at `ts.packages.react.src.hooks.function.applyPatch`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ApplypatchInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ApplypatchOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `buildspecfromparts` discovered at `ts.packages.react.src.hooks.function.buildSpecFromParts`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct BuildspecfrompartsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct BuildspecfrompartsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `flattotree` discovered at `ts.packages.react.src.hooks.function.flatToTree`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct FlattotreeInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct FlattotreeOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `generatechatid` discovered at `ts.packages.react.src.hooks.function.generateChatId`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GeneratechatidInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GeneratechatidOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `getspecvalue` discovered at `ts.packages.react.src.hooks.function.getSpecValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GetspecvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetspecvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `gettextfromparts` discovered at `ts.packages.react.src.hooks.function.getTextFromParts`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct GettextfrompartsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GettextfrompartsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `isspecdatapart` discovered at `ts.packages.react.src.hooks.function.isSpecDataPart`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct IsspecdatapartInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct IsspecdatapartOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `parseline` discovered at `ts.packages.react.src.hooks.function.parseLine`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ParselineInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ParselineOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `removespecvalue` discovered at `ts.packages.react.src.hooks.function.removeSpecValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct RemovespecvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RemovespecvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `setspecvalue` discovered at `ts.packages.react.src.hooks.function.setSpecValue`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct SetspecvalueInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SetspecvalueOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usechatui` discovered at `ts.packages.react.src.hooks.function.useChatUI`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsechatuiInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsechatuiOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usejsonrendermessage` discovered at `ts.packages.react.src.hooks.function.useJsonRenderMessage`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsejsonrendermessageInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsejsonrendermessageOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `useuistream` discovered at `ts.packages.react.src.hooks.function.useUIStream`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UseuistreamInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UseuistreamOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usedevtoolsactive` discovered at `ts.packages.react.src.renderer.function.useDevtoolsActive`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsedevtoolsactiveInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsedevtoolsactiveOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usedirectives` discovered at `ts.packages.react.src.renderer.function.useDirectives`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsedirectivesInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsedirectivesOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    /// Typed input candidate for `usefunctions` discovered at `ts.packages.react.src.renderer.function.useFunctions`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct UsefunctionsInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct UsefunctionsOutput {
        /// Human-readable operation result.
        pub message: String,
        pub changed: bool,
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[
        MethodCandidate {
            name: "escapestring",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.escapestring@v1",
            repomix_path: "ts.packages.codegen.src.serialize.function.escapeString",
            command: &[],
        },
        MethodCandidate {
            name: "serializepropvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.serializepropvalue@v1",
            repomix_path: "ts.packages.codegen.src.serialize.function.serializePropValue",
            command: &[],
        },
        MethodCandidate {
            name: "serializeprops",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.serializeprops@v1",
            repomix_path: "ts.packages.codegen.src.serialize.function.serializeProps",
            command: &[],
        },
        MethodCandidate {
            name: "collectactions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.collectactions@v1",
            repomix_path: "ts.packages.codegen.src.traverse.function.collectActions",
            command: &[],
        },
        MethodCandidate {
            name: "collectpathfromitem",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.collectpathfromitem@v1",
            repomix_path: "ts.packages.codegen.src.traverse.function.collectPathFromItem",
            command: &[],
        },
        MethodCandidate {
            name: "collectpathsfromcondition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.collectpathsfromcondition@v1",
            repomix_path: "ts.packages.codegen.src.traverse.function.collectPathsFromCondition",
            command: &[],
        },
        MethodCandidate {
            name: "collectstatepaths",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.collectstatepaths@v1",
            repomix_path: "ts.packages.codegen.src.traverse.function.collectStatePaths",
            command: &[],
        },
        MethodCandidate {
            name: "collectusedcomponents",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.collectusedcomponents@v1",
            repomix_path: "ts.packages.codegen.src.traverse.function.collectUsedComponents",
            command: &[],
        },
        MethodCandidate {
            name: "traversespec",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.traversespec@v1",
            repomix_path: "ts.packages.codegen.src.traverse.function.traverseSpec",
            command: &[],
        },
        MethodCandidate {
            name: "visit",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.visit@v1",
            repomix_path: "ts.packages.codegen.src.traverse.function.visit",
            command: &[],
        },
        MethodCandidate {
            name: "generate",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.generate@v1",
            repomix_path: "ts.packages.codegen.src.types.interface.CodeGenerator.method.generate",
            command: &[],
        },
        MethodCandidate {
            name: "nextactiondispatchid",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.nextactiondispatchid@v1",
            repomix_path: "ts.packages.core.src.action-observer.function.nextActionDispatchId",
            command: &[],
        },
        MethodCandidate {
            name: "notifyactiondispatch",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.notifyactiondispatch@v1",
            repomix_path: "ts.packages.core.src.action-observer.function.notifyActionDispatch",
            command: &[],
        },
        MethodCandidate {
            name: "notifyactionsettle",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.notifyactionsettle@v1",
            repomix_path: "ts.packages.core.src.action-observer.function.notifyActionSettle",
            command: &[],
        },
        MethodCandidate {
            name: "registeractionobserver",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.registeractionobserver@v1",
            repomix_path: "ts.packages.core.src.action-observer.function.registerActionObserver",
            command: &[],
        },
        MethodCandidate {
            name: "executeaction",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.executeaction@v1",
            repomix_path: "ts.packages.core.src.actions.function.executeAction",
            command: &[],
        },
        MethodCandidate {
            name: "interpolatestring",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.interpolatestring@v1",
            repomix_path: "ts.packages.core.src.actions.function.interpolateString",
            command: &[],
        },
        MethodCandidate {
            name: "resolveaction",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolveaction@v1",
            repomix_path: "ts.packages.core.src.actions.function.resolveAction",
            command: &[],
        },
        MethodCandidate {
            name: "isdevtoolsactive",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isdevtoolsactive@v1",
            repomix_path: "ts.packages.core.src.devtools-flag.function.isDevtoolsActive",
            command: &[],
        },
        MethodCandidate {
            name: "markdevtoolsactive",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.markdevtoolsactive@v1",
            repomix_path: "ts.packages.core.src.devtools-flag.function.markDevtoolsActive",
            command: &[],
        },
        MethodCandidate {
            name: "notifydevtoolsactivechange",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.notifydevtoolsactivechange@v1",
            repomix_path: "ts.packages.core.src.devtools-flag.function.notifyDevtoolsActiveChange",
            command: &[],
        },
        MethodCandidate {
            name: "subscribedevtoolsactive",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.subscribedevtoolsactive@v1",
            repomix_path: "ts.packages.core.src.devtools-flag.function.subscribeDevtoolsActive",
            command: &[],
        },
        MethodCandidate {
            name: "arraysequal",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.arraysequal@v1",
            repomix_path: "ts.packages.core.src.diff.function.arraysEqual",
            command: &[],
        },
        MethodCandidate {
            name: "buildpath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.buildpath@v1",
            repomix_path: "ts.packages.core.src.diff.function.buildPath",
            command: &[],
        },
        MethodCandidate {
            name: "difftopatches",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.difftopatches@v1",
            repomix_path: "ts.packages.core.src.diff.function.diffToPatches",
            command: &[],
        },
        MethodCandidate {
            name: "escapetoken",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.escapetoken@v1",
            repomix_path: "ts.packages.core.src.diff.function.escapeToken",
            command: &[],
        },
        MethodCandidate {
            name: "isplainobject",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isplainobject@v1",
            repomix_path: "ts.packages.core.src.diff.function.isPlainObject",
            command: &[],
        },
        MethodCandidate {
            name: "createdirectiveregistry",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.createdirectiveregistry@v1",
            repomix_path: "ts.packages.core.src.directives.function.createDirectiveRegistry",
            command: &[],
        },
        MethodCandidate {
            name: "finddirective",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.finddirective@v1",
            repomix_path: "ts.packages.core.src.directives.function.findDirective",
            command: &[],
        },
        MethodCandidate {
            name: "addlinenumbers",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.addlinenumbers@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.addLineNumbers",
            command: &[],
        },
        MethodCandidate {
            name: "buildeditinstructions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.buildeditinstructions@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.buildEditInstructions",
            command: &[],
        },
        MethodCandidate {
            name: "buildedituserprompt",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.buildedituserprompt@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.buildEditUserPrompt",
            command: &[],
        },
        MethodCandidate {
            name: "isnonemptyspec",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isnonemptyspec@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.isNonEmptySpec",
            command: &[],
        },
        MethodCandidate {
            name: "jsondiffinstructions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.jsondiffinstructions@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.jsonDiffInstructions",
            command: &[],
        },
        MethodCandidate {
            name: "jsonmergeinstructions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.jsonmergeinstructions@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.jsonMergeInstructions",
            command: &[],
        },
        MethodCandidate {
            name: "jsonpatchinstructions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.jsonpatchinstructions@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.jsonPatchInstructions",
            command: &[],
        },
        MethodCandidate {
            name: "modeselectionguidance",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.modeselectionguidance@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.modeSelectionGuidance",
            command: &[],
        },
        MethodCandidate {
            name: "normalizemodes",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.normalizemodes@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.normalizeModes",
            command: &[],
        },
        MethodCandidate {
            name: "yamldiffinstructions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.yamldiffinstructions@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.yamlDiffInstructions",
            command: &[],
        },
        MethodCandidate {
            name: "yamlmergeinstructions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.yamlmergeinstructions@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.yamlMergeInstructions",
            command: &[],
        },
        MethodCandidate {
            name: "yamlpatchinstructions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.yamlpatchinstructions@v1",
            repomix_path: "ts.packages.core.src.edit-modes.function.yamlPatchInstructions",
            command: &[],
        },
        MethodCandidate {
            name: "deepmergespec",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.deepmergespec@v1",
            repomix_path: "ts.packages.core.src.merge.function.deepMergeSpec",
            command: &[],
        },
        MethodCandidate {
            name: "builduserprompt",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.builduserprompt@v1",
            repomix_path: "ts.packages.core.src.prompt.function.buildUserPrompt",
            command: &[],
        },
        MethodCandidate {
            name: "resetwarnedcomputedfns",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resetwarnedcomputedfns@v1",
            repomix_path: "ts.packages.core.src.props.function._resetWarnedComputedFns",
            command: &[],
        },
        MethodCandidate {
            name: "isbinditemexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isbinditemexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isBindItemExpression",
            command: &[],
        },
        MethodCandidate {
            name: "isbindstateexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isbindstateexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isBindStateExpression",
            command: &[],
        },
        MethodCandidate {
            name: "iscomputedexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.iscomputedexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isComputedExpression",
            command: &[],
        },
        MethodCandidate {
            name: "iscondexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.iscondexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isCondExpression",
            command: &[],
        },
        MethodCandidate {
            name: "isindexexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isindexexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isIndexExpression",
            command: &[],
        },
        MethodCandidate {
            name: "isitemexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isitemexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isItemExpression",
            command: &[],
        },
        MethodCandidate {
            name: "isstateexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isstateexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isStateExpression",
            command: &[],
        },
        MethodCandidate {
            name: "istemplateexpression",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.istemplateexpression@v1",
            repomix_path: "ts.packages.core.src.props.function.isTemplateExpression",
            command: &[],
        },
        MethodCandidate {
            name: "resolveactionparam",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolveactionparam@v1",
            repomix_path: "ts.packages.core.src.props.function.resolveActionParam",
            command: &[],
        },
        MethodCandidate {
            name: "resolvebinditempath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolvebinditempath@v1",
            repomix_path: "ts.packages.core.src.props.function.resolveBindItemPath",
            command: &[],
        },
        MethodCandidate {
            name: "resolvebindings",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolvebindings@v1",
            repomix_path: "ts.packages.core.src.props.function.resolveBindings",
            command: &[],
        },
        MethodCandidate {
            name: "resolveelementprops",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolveelementprops@v1",
            repomix_path: "ts.packages.core.src.props.function.resolveElementProps",
            command: &[],
        },
        MethodCandidate {
            name: "resolvepropvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolvepropvalue@v1",
            repomix_path: "ts.packages.core.src.props.function.resolvePropValue",
            command: &[],
        },
        MethodCandidate {
            name: "buildzodschemafromdefinition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.buildzodschemafromdefinition@v1",
            repomix_path: "ts.packages.core.src.schema.function.buildZodSchemaFromDefinition",
            command: &[],
        },
        MethodCandidate {
            name: "buildzodtype",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.buildzodtype@v1",
            repomix_path: "ts.packages.core.src.schema.function.buildZodType",
            command: &[],
        },
        MethodCandidate {
            name: "createbuilder",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.createbuilder@v1",
            repomix_path: "ts.packages.core.src.schema.function.createBuilder",
            command: &[],
        },
        MethodCandidate {
            name: "findfirststringprop",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.findfirststringprop@v1",
            repomix_path: "ts.packages.core.src.schema.function.findFirstStringProp",
            command: &[],
        },
        MethodCandidate {
            name: "formatzodtype",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.formatzodtype@v1",
            repomix_path: "ts.packages.core.src.schema.function.formatZodType",
            command: &[],
        },
        MethodCandidate {
            name: "generateexamplepropsfromzod",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.generateexamplepropsfromzod@v1",
            repomix_path: "ts.packages.core.src.schema.function.generateExamplePropsFromZod",
            command: &[],
        },
        MethodCandidate {
            name: "generateexamplevalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.generateexamplevalue@v1",
            repomix_path: "ts.packages.core.src.schema.function.generateExampleValue",
            command: &[],
        },
        MethodCandidate {
            name: "getexampleprops",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getexampleprops@v1",
            repomix_path: "ts.packages.core.src.schema.function.getExampleProps",
            command: &[],
        },
        MethodCandidate {
            name: "getkeysfrompath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getkeysfrompath@v1",
            repomix_path: "ts.packages.core.src.schema.function.getKeysFromPath",
            command: &[],
        },
        MethodCandidate {
            name: "getpropsfrompath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getpropsfrompath@v1",
            repomix_path: "ts.packages.core.src.schema.function.getPropsFromPath",
            command: &[],
        },
        MethodCandidate {
            name: "getzodtypename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getzodtypename@v1",
            repomix_path: "ts.packages.core.src.schema.function.getZodTypeName",
            command: &[],
        },
        MethodCandidate {
            name: "normalizetypename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.normalizetypename@v1",
            repomix_path: "ts.packages.core.src.schema.function.normalizeTypeName",
            command: &[],
        },
        MethodCandidate {
            name: "zodtojsonschema",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.zodtojsonschema@v1",
            repomix_path: "ts.packages.core.src.schema.function.zodToJsonSchema",
            command: &[],
        },
        MethodCandidate {
            name: "zodtypename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.zodtypename@v1",
            repomix_path: "ts.packages.core.src.schema.function.zodTypeName",
            command: &[],
        },
        MethodCandidate {
            name: "jsonschema",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.jsonschema@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.jsonSchema",
            command: &[],
        },
        MethodCandidate {
            name: "prompt",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.prompt@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.prompt",
            command: &[],
        },
        MethodCandidate {
            name: "validate",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.validate@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.validate",
            command: &[],
        },
        MethodCandidate {
            name: "zodschema",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.zodschema@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.zodSchema",
            command: &[],
        },
        MethodCandidate {
            name: "any",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.any@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.any",
            command: &[],
        },
        MethodCandidate {
            name: "boolean",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.boolean@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.boolean",
            command: &[],
        },
        MethodCandidate {
            name: "number",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.number@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.number",
            command: &[],
        },
        MethodCandidate {
            name: "optional",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.optional@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.optional",
            command: &[],
        },
        MethodCandidate {
            name: "propsof",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.propsof@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.propsOf",
            command: &[],
        },
        MethodCandidate {
            name: "ref_field",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.ref-field@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.ref",
            command: &[],
        },
        MethodCandidate {
            name: "string",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.string@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.string",
            command: &[],
        },
        MethodCandidate {
            name: "zod",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.zod@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.zod",
            command: &[],
        },
        MethodCandidate {
            name: "autofixspec",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.autofixspec@v1",
            repomix_path: "ts.packages.core.src.spec-validator.function.autoFixSpec",
            command: &[],
        },
        MethodCandidate {
            name: "formatspecissues",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.formatspecissues@v1",
            repomix_path: "ts.packages.core.src.spec-validator.function.formatSpecIssues",
            command: &[],
        },
        MethodCandidate {
            name: "createstatestore",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.createstatestore@v1",
            repomix_path: "ts.packages.core.src.state-store.function.createStateStore",
            command: &[],
        },
        MethodCandidate {
            name: "createstoreadapter",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.createstoreadapter@v1",
            repomix_path: "ts.packages.core.src.state-store.function.createStoreAdapter",
            command: &[],
        },
        MethodCandidate {
            name: "flattentopointers",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.flattentopointers@v1",
            repomix_path: "ts.packages.core.src.state-store.function.flattenToPointers",
            command: &[],
        },
        MethodCandidate {
            name: "immutablesetbypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.immutablesetbypath@v1",
            repomix_path: "ts.packages.core.src.state-store.function.immutableSetByPath",
            command: &[],
        },
        MethodCandidate {
            name: "notify",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.notify@v1",
            repomix_path: "ts.packages.core.src.state-store.function.notify",
            command: &[],
        },
        MethodCandidate {
            name: "get",
            side_effect: "read",
            idempotent: true,
            required_capability: "json_render.read",
            subid: "obs.software.json-render.get@v1",
            repomix_path: "ts.packages.core.src.state-store.type.StateModel.method.get",
            command: &[],
        },
        MethodCandidate {
            name: "getserversnapshot",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getserversnapshot@v1",
            repomix_path:
                "ts.packages.core.src.state-store.type.StateModel.method.getServerSnapshot",
            command: &[],
        },
        MethodCandidate {
            name: "getsnapshot",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getsnapshot@v1",
            repomix_path: "ts.packages.core.src.state-store.type.StateModel.method.getSnapshot",
            command: &[],
        },
        MethodCandidate {
            name: "set",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.set@v1",
            repomix_path: "ts.packages.core.src.state-store.type.StateModel.method.set",
            command: &[],
        },
        MethodCandidate {
            name: "update",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.update@v1",
            repomix_path: "ts.packages.core.src.state-store.type.StateModel.method.update",
            command: &[],
        },
        MethodCandidate {
            name: "addbypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.addbypath@v1",
            repomix_path: "ts.packages.core.src.types.function.addByPath",
            command: &[],
        },
        MethodCandidate {
            name: "applyspecpatch",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.applyspecpatch@v1",
            repomix_path: "ts.packages.core.src.types.function.applySpecPatch",
            command: &[],
        },
        MethodCandidate {
            name: "closetextblock",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.closetextblock@v1",
            repomix_path: "ts.packages.core.src.types.function.closeTextBlock",
            command: &[],
        },
        MethodCandidate {
            name: "createjsonrendertransform",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.createjsonrendertransform@v1",
            repomix_path: "ts.packages.core.src.types.function.createJsonRenderTransform",
            command: &[],
        },
        MethodCandidate {
            name: "createmixedstreamparser",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.createmixedstreamparser@v1",
            repomix_path: "ts.packages.core.src.types.function.createMixedStreamParser",
            command: &[],
        },
        MethodCandidate {
            name: "deepequal",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.deepequal@v1",
            repomix_path: "ts.packages.core.src.types.function.deepEqual",
            command: &[],
        },
        MethodCandidate {
            name: "emitpatch",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.emitpatch@v1",
            repomix_path: "ts.packages.core.src.types.function.emitPatch",
            command: &[],
        },
        MethodCandidate {
            name: "emittextdelta",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.emittextdelta@v1",
            repomix_path: "ts.packages.core.src.types.function.emitTextDelta",
            command: &[],
        },
        MethodCandidate {
            name: "ensuretextblock",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.ensuretextblock@v1",
            repomix_path: "ts.packages.core.src.types.function.ensureTextBlock",
            command: &[],
        },
        MethodCandidate {
            name: "findformvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.findformvalue@v1",
            repomix_path: "ts.packages.core.src.types.function.findFormValue",
            command: &[],
        },
        MethodCandidate {
            name: "flushbuffer",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.flushbuffer@v1",
            repomix_path: "ts.packages.core.src.types.function.flushBuffer",
            command: &[],
        },
        MethodCandidate {
            name: "getbypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getbypath@v1",
            repomix_path: "ts.packages.core.src.types.function.getByPath",
            command: &[],
        },
        MethodCandidate {
            name: "isnumericindex",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isnumericindex@v1",
            repomix_path: "ts.packages.core.src.types.function.isNumericIndex",
            command: &[],
        },
        MethodCandidate {
            name: "nestedtoflat",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.nestedtoflat@v1",
            repomix_path: "ts.packages.core.src.types.function.nestedToFlat",
            command: &[],
        },
        MethodCandidate {
            name: "parsejsonpointer",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.parsejsonpointer@v1",
            repomix_path: "ts.packages.core.src.types.function.parseJsonPointer",
            command: &[],
        },
        MethodCandidate {
            name: "parsespecstreamline",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.parsespecstreamline@v1",
            repomix_path: "ts.packages.core.src.types.function.parseSpecStreamLine",
            command: &[],
        },
        MethodCandidate {
            name: "processcompleteline",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.processcompleteline@v1",
            repomix_path: "ts.packages.core.src.types.function.processCompleteLine",
            command: &[],
        },
        MethodCandidate {
            name: "processline",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.processline@v1",
            repomix_path: "ts.packages.core.src.types.function.processLine",
            command: &[],
        },
        MethodCandidate {
            name: "removebypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.removebypath@v1",
            repomix_path: "ts.packages.core.src.types.function.removeByPath",
            command: &[],
        },
        MethodCandidate {
            name: "setbypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.setbypath@v1",
            repomix_path: "ts.packages.core.src.types.function.setByPath",
            command: &[],
        },
        MethodCandidate {
            name: "unescapejsonpointer",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.unescapejsonpointer@v1",
            repomix_path: "ts.packages.core.src.types.function.unescapeJsonPointer",
            command: &[],
        },
        MethodCandidate {
            name: "walk",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.walk@v1",
            repomix_path: "ts.packages.core.src.types.function.walk",
            command: &[],
        },
        MethodCandidate {
            name: "flush",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.flush@v1",
            repomix_path: "ts.packages.core.src.types.interface.MixedStreamParser.method.flush",
            command: &[],
        },
        MethodCandidate {
            name: "push",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.push@v1",
            repomix_path: "ts.packages.core.src.types.interface.MixedStreamParser.method.push",
            command: &[],
        },
        MethodCandidate {
            name: "getpatches",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getpatches@v1",
            repomix_path:
                "ts.packages.core.src.types.interface.SpecStreamCompiler.method.getPatches",
            command: &[],
        },
        MethodCandidate {
            name: "getresult",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getresult@v1",
            repomix_path:
                "ts.packages.core.src.types.interface.SpecStreamCompiler.method.getResult",
            command: &[],
        },
        MethodCandidate {
            name: "reset",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.reset@v1",
            repomix_path: "ts.packages.core.src.types.interface.SpecStreamCompiler.method.reset",
            command: &[],
        },
        MethodCandidate {
            name: "runvalidation",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.runvalidation@v1",
            repomix_path: "ts.packages.core.src.validation.function.runValidation",
            command: &[],
        },
        MethodCandidate {
            name: "runvalidationcheck",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.runvalidationcheck@v1",
            repomix_path: "ts.packages.core.src.validation.function.runValidationCheck",
            command: &[],
        },
        MethodCandidate {
            name: "conditionusesitemscope",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.conditionusesitemscope@v1",
            repomix_path: "ts.packages.core.src.visibility.function.conditionUsesItemScope",
            command: &[],
        },
        MethodCandidate {
            name: "evaluatecondition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.evaluatecondition@v1",
            repomix_path: "ts.packages.core.src.visibility.function.evaluateCondition",
            command: &[],
        },
        MethodCandidate {
            name: "evaluatevisibility",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.evaluatevisibility@v1",
            repomix_path: "ts.packages.core.src.visibility.function.evaluateVisibility",
            command: &[],
        },
        MethodCandidate {
            name: "isandcondition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isandcondition@v1",
            repomix_path: "ts.packages.core.src.visibility.function.isAndCondition",
            command: &[],
        },
        MethodCandidate {
            name: "isindexcondition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isindexcondition@v1",
            repomix_path: "ts.packages.core.src.visibility.function.isIndexCondition",
            command: &[],
        },
        MethodCandidate {
            name: "isitemcondition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isitemcondition@v1",
            repomix_path: "ts.packages.core.src.visibility.function.isItemCondition",
            command: &[],
        },
        MethodCandidate {
            name: "isorcondition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isorcondition@v1",
            repomix_path: "ts.packages.core.src.visibility.function.isOrCondition",
            command: &[],
        },
        MethodCandidate {
            name: "resolvecomparisonvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolvecomparisonvalue@v1",
            repomix_path: "ts.packages.core.src.visibility.function.resolveComparisonValue",
            command: &[],
        },
        MethodCandidate {
            name: "resolveconditionvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.resolveconditionvalue@v1",
            repomix_path: "ts.packages.core.src.visibility.function.resolveConditionValue",
            command: &[],
        },
        MethodCandidate {
            name: "splitrepeatvisibility",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.splitrepeatvisibility@v1",
            repomix_path: "ts.packages.core.src.visibility.function.splitRepeatVisibility",
            command: &[],
        },
        MethodCandidate {
            name: "mcpappview",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.mcpappview@v1",
            repomix_path: "ts.packages.mcp.src.app.function.McpAppView",
            command: &[],
        },
        MethodCandidate {
            name: "buildapphtml",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.buildapphtml@v1",
            repomix_path: "ts.packages.mcp.src.build-app-html.function.buildAppHtml",
            command: &[],
        },
        MethodCandidate {
            name: "escapehtml",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.escapehtml@v1",
            repomix_path: "ts.packages.mcp.src.build-app-html.function.escapeHtml",
            command: &[],
        },
        MethodCandidate {
            name: "createmcpapp",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.createmcpapp@v1",
            repomix_path: "ts.packages.mcp.src.server.function.createMcpApp",
            command: &[],
        },
        MethodCandidate {
            name: "getextapps",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getextapps@v1",
            repomix_path: "ts.packages.mcp.src.server.function.getExtApps",
            command: &[],
        },
        MethodCandidate {
            name: "registerjsonrenderresource",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.registerjsonrenderresource@v1",
            repomix_path: "ts.packages.mcp.src.server.function.registerJsonRenderResource",
            command: &[],
        },
        MethodCandidate {
            name: "registerjsonrendertool",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.registerjsonrendertool@v1",
            repomix_path: "ts.packages.mcp.src.server.function.registerJsonRenderTool",
            command: &[],
        },
        MethodCandidate {
            name: "parsespecfromtoolresult",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.parsespecfromtoolresult@v1",
            repomix_path:
                "ts.packages.mcp.src.use-json-render-app.function.parseSpecFromToolResult",
            command: &[],
        },
        MethodCandidate {
            name: "usejsonrenderapp",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usejsonrenderapp@v1",
            repomix_path: "ts.packages.mcp.src.use-json-render-app.function.useJsonRenderApp",
            command: &[],
        },
        MethodCandidate {
            name: "actionprovider",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.actionprovider@v1",
            repomix_path: "ts.packages.react.src.contexts.actions.function.ActionProvider",
            command: &[],
        },
        MethodCandidate {
            name: "deepresolvevalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.deepresolvevalue@v1",
            repomix_path: "ts.packages.react.src.contexts.actions.function.deepResolveValue",
            command: &[],
        },
        MethodCandidate {
            name: "generateuniqueid",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.generateuniqueid@v1",
            repomix_path: "ts.packages.react.src.contexts.actions.function.generateUniqueId",
            command: &[],
        },
        MethodCandidate {
            name: "useaction",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.useaction@v1",
            repomix_path: "ts.packages.react.src.contexts.actions.function.useAction",
            command: &[],
        },
        MethodCandidate {
            name: "useactions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.useactions@v1",
            repomix_path: "ts.packages.react.src.contexts.actions.function.useActions",
            command: &[],
        },
        MethodCandidate {
            name: "repeatscopeprovider",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.repeatscopeprovider@v1",
            repomix_path:
                "ts.packages.react.src.contexts.repeat-scope.function.RepeatScopeProvider",
            command: &[],
        },
        MethodCandidate {
            name: "userepeatscope",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.userepeatscope@v1",
            repomix_path: "ts.packages.react.src.contexts.repeat-scope.function.useRepeatScope",
            command: &[],
        },
        MethodCandidate {
            name: "validationprovider",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.validationprovider@v1",
            repomix_path: "ts.packages.react.src.contexts.validation.function.ValidationProvider",
            command: &[],
        },
        MethodCandidate {
            name: "dynamicargsequal",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.dynamicargsequal@v1",
            repomix_path: "ts.packages.react.src.contexts.validation.function.dynamicArgsEqual",
            command: &[],
        },
        MethodCandidate {
            name: "usefieldvalidation",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usefieldvalidation@v1",
            repomix_path: "ts.packages.react.src.contexts.validation.function.useFieldValidation",
            command: &[],
        },
        MethodCandidate {
            name: "useoptionalvalidation",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.useoptionalvalidation@v1",
            repomix_path:
                "ts.packages.react.src.contexts.validation.function.useOptionalValidation",
            command: &[],
        },
        MethodCandidate {
            name: "usevalidation",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usevalidation@v1",
            repomix_path: "ts.packages.react.src.contexts.validation.function.useValidation",
            command: &[],
        },
        MethodCandidate {
            name: "validationconfigequal",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.validationconfigequal@v1",
            repomix_path:
                "ts.packages.react.src.contexts.validation.function.validationConfigEqual",
            command: &[],
        },
        MethodCandidate {
            name: "visibilityprovider",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.visibilityprovider@v1",
            repomix_path: "ts.packages.react.src.contexts.visibility.function.VisibilityProvider",
            command: &[],
        },
        MethodCandidate {
            name: "useisvisible",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.useisvisible@v1",
            repomix_path: "ts.packages.react.src.contexts.visibility.function.useIsVisible",
            command: &[],
        },
        MethodCandidate {
            name: "usevisibility",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usevisibility@v1",
            repomix_path: "ts.packages.react.src.contexts.visibility.function.useVisibility",
            command: &[],
        },
        MethodCandidate {
            name: "messagebubble",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.messagebubble@v1",
            repomix_path: "ts.packages.react.src.hooks.function.MessageBubble",
            command: &[],
        },
        MethodCandidate {
            name: "applypatch",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.applypatch@v1",
            repomix_path: "ts.packages.react.src.hooks.function.applyPatch",
            command: &[],
        },
        MethodCandidate {
            name: "buildspecfromparts",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.buildspecfromparts@v1",
            repomix_path: "ts.packages.react.src.hooks.function.buildSpecFromParts",
            command: &[],
        },
        MethodCandidate {
            name: "flattotree",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.flattotree@v1",
            repomix_path: "ts.packages.react.src.hooks.function.flatToTree",
            command: &[],
        },
        MethodCandidate {
            name: "generatechatid",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.generatechatid@v1",
            repomix_path: "ts.packages.react.src.hooks.function.generateChatId",
            command: &[],
        },
        MethodCandidate {
            name: "getspecvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.getspecvalue@v1",
            repomix_path: "ts.packages.react.src.hooks.function.getSpecValue",
            command: &[],
        },
        MethodCandidate {
            name: "gettextfromparts",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.gettextfromparts@v1",
            repomix_path: "ts.packages.react.src.hooks.function.getTextFromParts",
            command: &[],
        },
        MethodCandidate {
            name: "isspecdatapart",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.isspecdatapart@v1",
            repomix_path: "ts.packages.react.src.hooks.function.isSpecDataPart",
            command: &[],
        },
        MethodCandidate {
            name: "parseline",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.parseline@v1",
            repomix_path: "ts.packages.react.src.hooks.function.parseLine",
            command: &[],
        },
        MethodCandidate {
            name: "removespecvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.removespecvalue@v1",
            repomix_path: "ts.packages.react.src.hooks.function.removeSpecValue",
            command: &[],
        },
        MethodCandidate {
            name: "setspecvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.setspecvalue@v1",
            repomix_path: "ts.packages.react.src.hooks.function.setSpecValue",
            command: &[],
        },
        MethodCandidate {
            name: "usechatui",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usechatui@v1",
            repomix_path: "ts.packages.react.src.hooks.function.useChatUI",
            command: &[],
        },
        MethodCandidate {
            name: "usejsonrendermessage",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usejsonrendermessage@v1",
            repomix_path: "ts.packages.react.src.hooks.function.useJsonRenderMessage",
            command: &[],
        },
        MethodCandidate {
            name: "useuistream",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.useuistream@v1",
            repomix_path: "ts.packages.react.src.hooks.function.useUIStream",
            command: &[],
        },
        MethodCandidate {
            name: "usedevtoolsactive",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usedevtoolsactive@v1",
            repomix_path: "ts.packages.react.src.renderer.function.useDevtoolsActive",
            command: &[],
        },
        MethodCandidate {
            name: "usedirectives",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usedirectives@v1",
            repomix_path: "ts.packages.react.src.renderer.function.useDirectives",
            command: &[],
        },
        MethodCandidate {
            name: "usefunctions",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "json_render.write",
            subid: "mut.software.json-render.usefunctions@v1",
            repomix_path: "ts.packages.react.src.renderer.function.useFunctions",
            command: &[],
        },
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
