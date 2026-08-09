use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// Runtime state of the schema renderer plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.schema-renderer.schema@v1"))]
#[schemars(extend("x-oscal-category" = "software"))]
pub struct SchemaRendererState {
    /// Operational status.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.schema-renderer.status@v1"))]
    pub status: String,
    /// Supported UI element types.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.schema-renderer.element-types@v1"))]
    pub element_types: serde_json::Value,
    /// JSON Schema type to component mappings.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.schema-renderer.field-mappings@v1"))]
    pub field_mappings: serde_json::Value,
    /// Supported layout modes.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.schema-renderer.layouts@v1"))]
    pub layouts: serde_json::Value,
    /// Rendering configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.schema-renderer.render-config@v1"))]
    pub render_config: serde_json::Value,
    /// Auto-gallery configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.schema-renderer.gallery-config@v1"))]
    pub gallery_config: serde_json::Value,
    /// Sub-view templates.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.plugin.schema-renderer.sub-views@v1"))]
    pub sub_views: serde_json::Value,
}

impl Default for SchemaRendererState {
    fn default() -> Self {
        Self {
            status: "active".to_string(),
            element_types: serde_json::json!([
                {"id": "form", "label": "Form", "icon": "Form", "schema_type": "object", "default_layout": "grid", "description": "Input form with validation from schema"},
                {"id": "table", "label": "Table", "icon": "Table", "schema_type": "object", "default_layout": "list", "description": "Tabular data display with sort/filter"},
                {"id": "card", "label": "Card", "icon": "Card", "schema_type": "object", "default_layout": "grid", "description": "Summary card with key fields"},
                {"id": "dashboard", "label": "Dashboard", "icon": "Dashboard", "schema_type": "object", "default_layout": "masonry", "description": "Multi-element dashboard combining sub-views"},
                {"id": "chart", "label": "Chart", "icon": "Chart", "schema_type": "object", "default_layout": "grid", "description": "Visual chart from numeric fields"},
                {"id": "timeline", "label": "Timeline", "icon": "Timeline", "schema_type": "object", "default_layout": "list", "description": "Chronological event display"},
                {"id": "tree", "label": "Tree", "icon": "Tree", "schema_type": "object", "default_layout": "list", "description": "Hierarchical nested data view"}
            ]),
            field_mappings: serde_json::json!({
                "string": {"component": "Input", "props": {"variant": "text"}, "long_component": "Textarea", "long_threshold": 80},
                "number": {"component": "NumberInput", "props": {"step": 1}},
                "integer": {"component": "NumberInput", "props": {"step": 1, "decimal": false}},
                "boolean": {"component": "Switch", "props": {}},
                "enum": {"component": "Select", "props": {}},
                "object": {"component": "ObjectView", "props": {"collapsible": true}},
                "array": {"component": "ArrayView", "props": {"repeatable": true, "sortable": false}},
                "date": {"component": "DatePicker", "props": {}},
                "datetime": {"component": "DateTimePicker", "props": {}},
                "color": {"component": "ColorPicker", "props": {}},
                "url": {"component": "Input", "props": {"variant": "url"}},
                "email": {"component": "Input", "props": {"variant": "email"}}
            }),
            layouts: serde_json::json!([
                {"id": "grid", "name": "Grid", "columns": [1,2,3,4], "default_columns": 3, "gap": "1rem"},
                {"id": "list", "name": "List", "density": ["compact","comfortable","spacious"], "default_density": "comfortable"},
                {"id": "tabs", "name": "Tabbed", "tab_position": ["top","left","right"], "default_position": "top"},
                {"id": "masonry", "name": "Masonry", "column_width": 300, "gap": "1rem"},
                {"id": "carousel", "name": "Carousel", "items_per_view": [1,2,3], "autoplay": false}
            ]),
            render_config: serde_json::json!({
                "modes": ["compact", "full", "table", "chart"],
                "default_mode": "compact",
                "max_fields_per_card": 8,
                "collapse_depth": 2,
                "show_field_types": true,
                "show_examples": true,
                "show_descriptions": true,
                "show_required_markers": true,
                "theme": "system",
                "animation": "none"
            }),
            gallery_config: serde_json::json!({
                "auto_generate": true,
                "categories_as_tabs": true,
                "show_preview": true,
                "max_preview_fields": 3,
                "search_enabled": true,
                "deploy_action": "navigate",
                "deploy_target": "/plugin/{plugin_id}",
                "default_element_type": "card",
                "default_layout": "grid",
                "group_by_category": true
            }),
            sub_views: serde_json::json!([
                {"id": "table-view", "name": "Table View", "target_type": "table", "description": "Renders object arrays as sortable data tables"},
                {"id": "chart-view", "name": "Chart View", "target_type": "chart", "description": "Renders numeric data as bar/line/pie charts"},
                {"id": "timeline-view", "name": "Timeline View", "target_type": "timeline", "description": "Renders timestamped data chronologically"},
                {"id": "tree-view", "name": "Tree View", "target_type": "tree", "description": "Renders hierarchical data as expandable tree"},
                {"id": "dashboard-view", "name": "Dashboard View", "target_type": "dashboard", "description": "Composite view combining multiple sub-views"},
                {"id": "form-view", "name": "Form View", "target_type": "form", "description": "Interactive form with validation"},
                {"id": "card-view", "name": "Card View", "target_type": "card", "description": "Summary card views in grid/masonry"}
            ]),
        }
    }
}

pub struct SchemaRendererPlugin;
impl Default for SchemaRendererPlugin {
    fn default() -> Self {
        Self
    }
}
impl SchemaRendererPlugin {
    pub fn new() -> Self {
        Self
    }
    pub(crate) fn current_state() -> SchemaRendererState {
        SchemaRendererState::default()
    }
}

#[async_trait]
impl StatePlugin for SchemaRendererPlugin {
    fn name(&self) -> &str {
        "schema_renderer"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn schema(&self) -> Option<PluginSchema> {
        Some(schema_renderer_schema())
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

/// Derived `schema_renderer` schema from the typed [`SchemaRendererState`] struct via schemars.
pub(crate) fn schema_renderer_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(SchemaRendererState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "schema_renderer",
        "1.0.0",
        "Schema Renderer - dynamic JSON Schema to React form generation with auto-gallery",
        &root,
    );
    let state = simd_json::serde::to_owned_value(&SchemaRendererState::default())
        .expect("SchemaRendererState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RenderSchemaOutput {
        pub rendering: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListGalleriesOutput {
        pub galleries: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetRenderingOutput {
        pub rendering: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ValidateSchemaOutput {
        pub valid: bool,
        pub errors: Vec<String>,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "render_schema".to_string(),
        method_decl_from_schemars_with_output::<(), RenderSchemaOutput>(
            "render_schema",
            SideEffect::Read,
            true,
            "schema_renderer.read",
            "obs.service.schema.renderer.render@v1",
        ),
    );
    schema.methods.insert(
        "list_galleries".to_string(),
        method_decl_from_schemars_with_output::<(), ListGalleriesOutput>(
            "list_galleries",
            SideEffect::Read,
            true,
            "schema_renderer.read",
            "obs.service.schema.gallery.list@v1",
        ),
    );
    schema.methods.insert(
        "get_rendering".to_string(),
        method_decl_from_schemars_with_output::<(), GetRenderingOutput>(
            "get_rendering",
            SideEffect::Read,
            true,
            "schema_renderer.read",
            "obs.service.schema.rendering.get@v1",
        ),
    );
    schema.methods.insert(
        "validate_schema".to_string(),
        method_decl_from_schemars_with_output::<(), ValidateSchemaOutput>(
            "validate_schema",
            SideEffect::Read,
            true,
            "schema_renderer.read",
            "obs.service.schema.validate@v1",
        ),
    );

    schema
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
        let root = serde_json::to_value(schemars::schema_for!(SchemaRendererState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("schema_renderer", |_ctx| std::sync::Arc::new(SchemaRendererPlugin::new()))
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
    #[schemars(extend("x-oscal-subid" = "sch.software.schema-renderer.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.currentSpec`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.currentspec@v1"))]
        pub currentspec: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.editModes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.editmodes@v1"))]
        pub editmodes: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.format@v1"))]
        pub format: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.maxPromptLength`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.maxpromptlength@v1"))]
        pub maxpromptlength: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.prompt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.prompt@v1"))]
        pub prompt: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.serializer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.serializer@v1"))]
        pub serializer: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.prompt.interface.UserPromptOptions.field.state`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.state@v1"))]
        pub state: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.enum.of.field.catalog`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.catalog@v1"))]
        pub catalog: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.enum.of.field.options`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.options@v1"))]
        pub options: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.BuiltInAction.field.description`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.description@v1"))]
        pub description: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.BuiltInAction.field.name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.name@v1"))]
        pub name: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field._specType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.spectype@v1"))]
        pub spectype: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field.actionNames`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.actionnames@v1"))]
        pub actionnames: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field.componentNames`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.componentnames@v1"))]
        pub componentnames: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field.data`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.data@v1"))]
        pub data: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Catalog.field.schema`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.schema@v1"))]
        pub schema: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.CatalogComponentDef.field.events`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.events@v1"))]
        pub events: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.CatalogComponentDef.field.example`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.example@v1"))]
        pub example: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.CatalogComponentDef.field.props`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.props@v1"))]
        pub props: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.CatalogComponentDef.field.slots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.slots@v1"))]
        pub slots: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.JsonSchemaOptions.field.strict`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.strict@v1"))]
        pub strict: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.PromptOptions.field.customRules`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.customrules@v1"))]
        pub customrules: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.PromptOptions.field.directives`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.directives@v1"))]
        pub directives: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.PromptOptions.field.mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.mode@v1"))]
        pub mode: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.PromptOptions.field.system`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.system@v1"))]
        pub system: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Schema.field.builtInActions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.builtinactions@v1"))]
        pub builtinactions: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Schema.field.defaultRules`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.defaultrules@v1"))]
        pub defaultrules: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Schema.field.definition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.definition@v1"))]
        pub definition: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.Schema.field.promptTemplate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.prompttemplate@v1"))]
        pub prompttemplate: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SchemaBuilder.field.entryShape`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.entryshape@v1"))]
        pub entryshape: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SchemaBuilder.field.shape`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.shape@v1"))]
        pub shape: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SchemaDefinition.field.spec`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.spec@v1"))]
        pub spec: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SpecValidationResult.field.error`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.error@v1"))]
        pub error: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.interface.SpecValidationResult.field.success`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.success@v1"))]
        pub success: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.InferPropsOfType.field.builder`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.builder@v1"))]
        pub builder: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.InferPropsOfType.field.catalogData`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.catalogdata@v1"))]
        pub catalogdata: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.as.field.context`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.context@v1"))]
        pub context: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.as.field.formatZodType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.formatzodtype@v1"))]
        pub formatzodtype: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.representation.field.inner`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.inner@v1"))]
        pub inner: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.representation.field.kind`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.kind@v1"))]
        pub kind: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.schema.type.representation.field.optional`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.optional@v1"))]
        pub optional: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.AutoFixOptions.field.lossy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.lossy@v1"))]
        pub lossy: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecFix.field.message`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.message@v1"))]
        pub message: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecIssue.field.code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.code@v1"))]
        pub code: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecIssue.field.elementKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.elementkey@v1"))]
        pub elementkey: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecIssue.field.severity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.severity@v1"))]
        pub severity: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecValidationIssues.field.issues`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.issues@v1"))]
        pub issues: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.SpecValidationIssues.field.valid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.valid@v1"))]
        pub valid: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.spec-validator.interface.ValidateSpecOptions.field.checkOrphans`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.checkorphans@v1"))]
        pub checkorphans: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.fieldName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.fieldname@v1"))]
        pub fieldname: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.from`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.from@v1"))]
        pub from: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.obj`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.obj@v1"))]
        pub obj: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.op`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.op@v1"))]
        pub op: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.params`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.params@v1"))]
        pub params: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.path`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.path@v1"))]
        pub path: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.stateModel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.statemodel@v1"))]
        pub statemodel: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.JsonPatch.field.value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.value@v1"))]
        pub value: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.MixedStreamCallbacks.field.onPatch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.onpatch@v1"))]
        pub onpatch: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.MixedStreamCallbacks.field.onText`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.ontext@v1"))]
        pub ontext: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.MixedStreamParser.field.callbacks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.callbacks@v1"))]
        pub callbacks: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.NestedNode.field.children`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.children@v1"))]
        pub children: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.NestedNode.field.type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.Spec.field.elements`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.elements@v1"))]
        pub elements: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.Spec.field.root`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.root@v1"))]
        pub root: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.SpecStreamCompiler.field.initial`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.initial@v1"))]
        pub initial: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.get`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.get@v1"))]
        pub get: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.getServerSnapshot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.getserversnapshot@v1"))]
        pub getserversnapshot: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.getSnapshot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.getsnapshot@v1"))]
        pub getsnapshot: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.set`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.set@v1"))]
        pub set: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.subscribe`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.subscribe@v1"))]
        pub subscribe: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.interface.StateStore.field.update`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.update@v1"))]
        pub update: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.eq`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.eq@v1"))]
        pub eq: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.gt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.gt@v1"))]
        pub gt: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.gte`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.gte@v1"))]
        pub gte: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.lt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.lt@v1"))]
        pub lt: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.lte`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.lte@v1"))]
        pub lte: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.neq`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.neq@v1"))]
        pub neq: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.ComparisonOperators.field.not`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.not@v1"))]
        pub not: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.SpecStreamLine.field.patch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.patch@v1"))]
        pub patch: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.StreamChunk.field.controller`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.controller@v1"))]
        pub controller: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.StreamChunk.field.delta`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.delta@v1"))]
        pub delta: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.StreamChunk.field.line`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.line@v1"))]
        pub line: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.cast.field.stream`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.stream@v1"))]
        pub stream: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.key@v1"))]
        pub key: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.on`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.on@v1"))]
        pub on: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.parentKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.parentkey@v1"))]
        pub parentkey: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.repeat`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.repeat@v1"))]
        pub repeat: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.visible`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.visible@v1"))]
        pub visible: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.types.type.from.field.watch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.watch@v1"))]
        pub watch: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationCheck.field.args`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.args@v1"))]
        pub args: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationConfig.field.checks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.checks@v1"))]
        pub checks: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationConfig.field.enabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.enabled@v1"))]
        pub enabled: Option<bool>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationConfig.field.validateOn`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.validateon@v1"))]
        pub validateon: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationContext.field.check`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.check@v1"))]
        pub check: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationContext.field.config`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.config@v1"))]
        pub config: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationContext.field.ctx`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.ctx@v1"))]
        pub ctx: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationContext.field.customFunctions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.customfunctions@v1"))]
        pub customfunctions: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationFunctionDefinition.field.validate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.validate@v1"))]
        pub validate: Option<String>,

        /// Discovered from Repomix path `ts.packages.core.src.validation.interface.ValidationResult.field.errors`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.schema-renderer.errors@v1"))]
        pub errors: Option<String>,

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

    /// Typed input candidate for `validatespec` discovered at `ts.packages.core.src.spec-validator.function.validateSpec`.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    pub struct ValidatespecInput {
        /// String-valued options discovered from the external surface.
        #[serde(default)]
        pub options: std::collections::BTreeMap<String, String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ValidatespecOutput {
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

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[
        MethodCandidate {
            name: "builduserprompt",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.builduserprompt@v1",
            repomix_path: "ts.packages.core.src.prompt.function.buildUserPrompt",
            command: &[],
        },
        MethodCandidate {
            name: "buildzodschemafromdefinition",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.buildzodschemafromdefinition@v1",
            repomix_path: "ts.packages.core.src.schema.function.buildZodSchemaFromDefinition",
            command: &[],
        },
        MethodCandidate {
            name: "buildzodtype",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.buildzodtype@v1",
            repomix_path: "ts.packages.core.src.schema.function.buildZodType",
            command: &[],
        },
        MethodCandidate {
            name: "createbuilder",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.createbuilder@v1",
            repomix_path: "ts.packages.core.src.schema.function.createBuilder",
            command: &[],
        },
        MethodCandidate {
            name: "findfirststringprop",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.findfirststringprop@v1",
            repomix_path: "ts.packages.core.src.schema.function.findFirstStringProp",
            command: &[],
        },
        MethodCandidate {
            name: "formatzodtype",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.formatzodtype@v1",
            repomix_path: "ts.packages.core.src.schema.function.formatZodType",
            command: &[],
        },
        MethodCandidate {
            name: "generateexamplepropsfromzod",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.generateexamplepropsfromzod@v1",
            repomix_path: "ts.packages.core.src.schema.function.generateExamplePropsFromZod",
            command: &[],
        },
        MethodCandidate {
            name: "generateexamplevalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.generateexamplevalue@v1",
            repomix_path: "ts.packages.core.src.schema.function.generateExampleValue",
            command: &[],
        },
        MethodCandidate {
            name: "getexampleprops",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.getexampleprops@v1",
            repomix_path: "ts.packages.core.src.schema.function.getExampleProps",
            command: &[],
        },
        MethodCandidate {
            name: "getkeysfrompath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.getkeysfrompath@v1",
            repomix_path: "ts.packages.core.src.schema.function.getKeysFromPath",
            command: &[],
        },
        MethodCandidate {
            name: "getpropsfrompath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.getpropsfrompath@v1",
            repomix_path: "ts.packages.core.src.schema.function.getPropsFromPath",
            command: &[],
        },
        MethodCandidate {
            name: "getzodtypename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.getzodtypename@v1",
            repomix_path: "ts.packages.core.src.schema.function.getZodTypeName",
            command: &[],
        },
        MethodCandidate {
            name: "normalizetypename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.normalizetypename@v1",
            repomix_path: "ts.packages.core.src.schema.function.normalizeTypeName",
            command: &[],
        },
        MethodCandidate {
            name: "zodtojsonschema",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.zodtojsonschema@v1",
            repomix_path: "ts.packages.core.src.schema.function.zodToJsonSchema",
            command: &[],
        },
        MethodCandidate {
            name: "zodtypename",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.zodtypename@v1",
            repomix_path: "ts.packages.core.src.schema.function.zodTypeName",
            command: &[],
        },
        MethodCandidate {
            name: "jsonschema",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.jsonschema@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.jsonSchema",
            command: &[],
        },
        MethodCandidate {
            name: "prompt",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.prompt@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.prompt",
            command: &[],
        },
        MethodCandidate {
            name: "validate",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.validate@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.validate",
            command: &[],
        },
        MethodCandidate {
            name: "zodschema",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.zodschema@v1",
            repomix_path: "ts.packages.core.src.schema.interface.Catalog.method.zodSchema",
            command: &[],
        },
        MethodCandidate {
            name: "any",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.any@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.any",
            command: &[],
        },
        MethodCandidate {
            name: "boolean",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.boolean@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.boolean",
            command: &[],
        },
        MethodCandidate {
            name: "number",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.number@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.number",
            command: &[],
        },
        MethodCandidate {
            name: "optional",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.optional@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.optional",
            command: &[],
        },
        MethodCandidate {
            name: "propsof",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.propsof@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.propsOf",
            command: &[],
        },
        MethodCandidate {
            name: "ref_field",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.ref-field@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.ref",
            command: &[],
        },
        MethodCandidate {
            name: "string",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.string@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.string",
            command: &[],
        },
        MethodCandidate {
            name: "zod",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.zod@v1",
            repomix_path: "ts.packages.core.src.schema.interface.SchemaBuilder.method.zod",
            command: &[],
        },
        MethodCandidate {
            name: "autofixspec",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.autofixspec@v1",
            repomix_path: "ts.packages.core.src.spec-validator.function.autoFixSpec",
            command: &[],
        },
        MethodCandidate {
            name: "formatspecissues",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.formatspecissues@v1",
            repomix_path: "ts.packages.core.src.spec-validator.function.formatSpecIssues",
            command: &[],
        },
        MethodCandidate {
            name: "validatespec",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.validatespec@v1",
            repomix_path: "ts.packages.core.src.spec-validator.function.validateSpec",
            command: &[],
        },
        MethodCandidate {
            name: "addbypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.addbypath@v1",
            repomix_path: "ts.packages.core.src.types.function.addByPath",
            command: &[],
        },
        MethodCandidate {
            name: "applyspecpatch",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.applyspecpatch@v1",
            repomix_path: "ts.packages.core.src.types.function.applySpecPatch",
            command: &[],
        },
        MethodCandidate {
            name: "closetextblock",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.closetextblock@v1",
            repomix_path: "ts.packages.core.src.types.function.closeTextBlock",
            command: &[],
        },
        MethodCandidate {
            name: "createjsonrendertransform",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.createjsonrendertransform@v1",
            repomix_path: "ts.packages.core.src.types.function.createJsonRenderTransform",
            command: &[],
        },
        MethodCandidate {
            name: "createmixedstreamparser",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.createmixedstreamparser@v1",
            repomix_path: "ts.packages.core.src.types.function.createMixedStreamParser",
            command: &[],
        },
        MethodCandidate {
            name: "deepequal",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.deepequal@v1",
            repomix_path: "ts.packages.core.src.types.function.deepEqual",
            command: &[],
        },
        MethodCandidate {
            name: "emitpatch",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.emitpatch@v1",
            repomix_path: "ts.packages.core.src.types.function.emitPatch",
            command: &[],
        },
        MethodCandidate {
            name: "emittextdelta",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.emittextdelta@v1",
            repomix_path: "ts.packages.core.src.types.function.emitTextDelta",
            command: &[],
        },
        MethodCandidate {
            name: "ensuretextblock",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.ensuretextblock@v1",
            repomix_path: "ts.packages.core.src.types.function.ensureTextBlock",
            command: &[],
        },
        MethodCandidate {
            name: "findformvalue",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.findformvalue@v1",
            repomix_path: "ts.packages.core.src.types.function.findFormValue",
            command: &[],
        },
        MethodCandidate {
            name: "flushbuffer",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.flushbuffer@v1",
            repomix_path: "ts.packages.core.src.types.function.flushBuffer",
            command: &[],
        },
        MethodCandidate {
            name: "getbypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.getbypath@v1",
            repomix_path: "ts.packages.core.src.types.function.getByPath",
            command: &[],
        },
        MethodCandidate {
            name: "isnumericindex",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.isnumericindex@v1",
            repomix_path: "ts.packages.core.src.types.function.isNumericIndex",
            command: &[],
        },
        MethodCandidate {
            name: "nestedtoflat",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.nestedtoflat@v1",
            repomix_path: "ts.packages.core.src.types.function.nestedToFlat",
            command: &[],
        },
        MethodCandidate {
            name: "parsejsonpointer",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.parsejsonpointer@v1",
            repomix_path: "ts.packages.core.src.types.function.parseJsonPointer",
            command: &[],
        },
        MethodCandidate {
            name: "parsespecstreamline",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.parsespecstreamline@v1",
            repomix_path: "ts.packages.core.src.types.function.parseSpecStreamLine",
            command: &[],
        },
        MethodCandidate {
            name: "processcompleteline",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.processcompleteline@v1",
            repomix_path: "ts.packages.core.src.types.function.processCompleteLine",
            command: &[],
        },
        MethodCandidate {
            name: "processline",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.processline@v1",
            repomix_path: "ts.packages.core.src.types.function.processLine",
            command: &[],
        },
        MethodCandidate {
            name: "removebypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.removebypath@v1",
            repomix_path: "ts.packages.core.src.types.function.removeByPath",
            command: &[],
        },
        MethodCandidate {
            name: "setbypath",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.setbypath@v1",
            repomix_path: "ts.packages.core.src.types.function.setByPath",
            command: &[],
        },
        MethodCandidate {
            name: "unescapejsonpointer",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.unescapejsonpointer@v1",
            repomix_path: "ts.packages.core.src.types.function.unescapeJsonPointer",
            command: &[],
        },
        MethodCandidate {
            name: "walk",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.walk@v1",
            repomix_path: "ts.packages.core.src.types.function.walk",
            command: &[],
        },
        MethodCandidate {
            name: "flush",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.flush@v1",
            repomix_path: "ts.packages.core.src.types.interface.MixedStreamParser.method.flush",
            command: &[],
        },
        MethodCandidate {
            name: "push",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.push@v1",
            repomix_path: "ts.packages.core.src.types.interface.MixedStreamParser.method.push",
            command: &[],
        },
        MethodCandidate {
            name: "getpatches",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.getpatches@v1",
            repomix_path: "ts.packages.core.src.types.interface.SpecStreamCompiler.method.getPatches",
            command: &[],
        },
        MethodCandidate {
            name: "getresult",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.getresult@v1",
            repomix_path: "ts.packages.core.src.types.interface.SpecStreamCompiler.method.getResult",
            command: &[],
        },
        MethodCandidate {
            name: "reset",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.reset@v1",
            repomix_path: "ts.packages.core.src.types.interface.SpecStreamCompiler.method.reset",
            command: &[],
        },
        MethodCandidate {
            name: "runvalidation",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.runvalidation@v1",
            repomix_path: "ts.packages.core.src.validation.function.runValidation",
            command: &[],
        },
        MethodCandidate {
            name: "runvalidationcheck",
            side_effect: "mutation",
            idempotent: false,
            required_capability: "schema_renderer.write",
            subid: "mut.software.schema-renderer.runvalidationcheck@v1",
            repomix_path: "ts.packages.core.src.validation.function.runValidationCheck",
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
