use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// Runtime state of the schema renderer plugin.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.schema-renderer.schema@v1"))]
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
    use crate::state_plugins::schemars_adapter::schema_diffs;
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
    fn derived_schema_matches_hand_rolled() {
        let golden = super::schema_renderer_schema_golden();
        let derived = super::schema_renderer_schema();
        let diffs = schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
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

#[cfg(test)]
pub(crate) fn schema_renderer_schema_golden() -> PluginSchema {
    use op_state_store::{FieldSchema, FieldType};
    use simd_json::json;

    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "status".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Operational status.".to_string(),
            default: Some(json!("active")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "element_types".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Supported UI element types.".to_string(),
            default: Some(json!([
                {"id": "form", "label": "Form", "icon": "Form", "schema_type": "object", "default_layout": "grid", "description": "Input form with validation from schema"},
                {"id": "table", "label": "Table", "icon": "Table", "schema_type": "object", "default_layout": "list", "description": "Tabular data display with sort/filter"},
                {"id": "card", "label": "Card", "icon": "Card", "schema_type": "object", "default_layout": "grid", "description": "Summary card with key fields"},
                {"id": "dashboard", "label": "Dashboard", "icon": "Dashboard", "schema_type": "object", "default_layout": "masonry", "description": "Multi-element dashboard combining sub-views"},
                {"id": "chart", "label": "Chart", "icon": "Chart", "schema_type": "object", "default_layout": "grid", "description": "Visual chart from numeric fields"},
                {"id": "timeline", "label": "Timeline", "icon": "Timeline", "schema_type": "object", "default_layout": "list", "description": "Chronological event display"},
                {"id": "tree", "label": "Tree", "icon": "Tree", "schema_type": "object", "default_layout": "list", "description": "Hierarchical nested data view"}
            ])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "field_mappings".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "JSON Schema type to component mappings.".to_string(),
            default: Some(json!({
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
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "layouts".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Supported layout modes.".to_string(),
            default: Some(json!([
                {"id": "grid", "name": "Grid", "columns": [1,2,3,4], "default_columns": 3, "gap": "1rem"},
                {"id": "list", "name": "List", "density": ["compact","comfortable","spacious"], "default_density": "comfortable"},
                {"id": "tabs", "name": "Tabbed", "tab_position": ["top","left","right"], "default_position": "top"},
                {"id": "masonry", "name": "Masonry", "column_width": 300, "gap": "1rem"},
                {"id": "carousel", "name": "Carousel", "items_per_view": [1,2,3], "autoplay": false}
            ])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "render_config".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Rendering configuration.".to_string(),
            default: Some(json!({
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
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "gallery_config".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Auto-gallery configuration.".to_string(),
            default: Some(json!({
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
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "sub_views".to_string(),
        FieldSchema {
            field_type: FieldType::Any,
            required: false,
            description: "Sub-view templates.".to_string(),
            default: Some(json!([
                {"id": "table-view", "name": "Table View", "target_type": "table", "description": "Renders object arrays as sortable data tables"},
                {"id": "chart-view", "name": "Chart View", "target_type": "chart", "description": "Renders numeric data as bar/line/pie charts"},
                {"id": "timeline-view", "name": "Timeline View", "target_type": "timeline", "description": "Renders timestamped data chronologically"},
                {"id": "tree-view", "name": "Tree View", "target_type": "tree", "description": "Renders hierarchical data as expandable tree"},
                {"id": "dashboard-view", "name": "Dashboard View", "target_type": "dashboard", "description": "Composite view combining multiple sub-views"},
                {"id": "form-view", "name": "Form View", "target_type": "form", "description": "Interactive form with validation"},
                {"id": "card-view", "name": "Card View", "target_type": "card", "description": "Summary card views in grid/masonry"}
            ])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schema = PluginSchema::builder("schema_renderer")
        .version("1.0.0")
        .description(
            "Schema Renderer - dynamic JSON Schema to React form generation with auto-gallery",
        )
        .build();
    schema.fields = fields;
    schema.subids = std::collections::HashMap::from([
        (
            "__schema__".to_string(),
            "sch.software.plugin.schema-renderer.schema@v1".to_string(),
        ),
        (
            "status".to_string(),
            "obs.software.plugin.schema-renderer.status@v1".to_string(),
        ),
        (
            "element_types".to_string(),
            "exp.software.plugin.schema-renderer.element-types@v1".to_string(),
        ),
        (
            "field_mappings".to_string(),
            "exp.software.plugin.schema-renderer.field-mappings@v1".to_string(),
        ),
        (
            "layouts".to_string(),
            "exp.software.plugin.schema-renderer.layouts@v1".to_string(),
        ),
        (
            "render_config".to_string(),
            "sch.software.plugin.schema-renderer.render-config@v1".to_string(),
        ),
        (
            "gallery_config".to_string(),
            "exp.software.plugin.schema-renderer.gallery-config@v1".to_string(),
        ),
        (
            "sub_views".to_string(),
            "exp.software.plugin.schema-renderer.sub-views@v1".to_string(),
        ),
    ]);
    let state = simd_json::serde::to_owned_value(&SchemaRendererState::default())
        .expect("SchemaRendererState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("schema_renderer", |_ctx| std::sync::Arc::new(SchemaRendererPlugin::new()))
}
