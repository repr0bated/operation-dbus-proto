use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use op_state_store::{PluginSchema};
use super::plugin_schema_defs::{schema_from_state};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaRendererState {
    pub status: String,
    pub element_types: Value,
    pub field_mappings: Value,
    pub layouts: Value,
    pub render_config: Value,
    pub gallery_config: Value,
    pub sub_views: Value,
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
        SchemaRendererState {
            status: "active".to_string(),
            element_types: json!([
                {"id": "form", "label": "Form", "icon": "Form", "schema_type": "object", "default_layout": "grid", "description": "Input form with validation from schema"},
                {"id": "table", "label": "Table", "icon": "Table", "schema_type": "object", "default_layout": "list", "description": "Tabular data display with sort/filter"},
                {"id": "card", "label": "Card", "icon": "Card", "schema_type": "object", "default_layout": "grid", "description": "Summary card with key fields"},
                {"id": "dashboard", "label": "Dashboard", "icon": "Dashboard", "schema_type": "object", "default_layout": "masonry", "description": "Multi-element dashboard combining sub-views"},
                {"id": "chart", "label": "Chart", "icon": "Chart", "schema_type": "object", "default_layout": "grid", "description": "Visual chart from numeric fields"},
                {"id": "timeline", "label": "Timeline", "icon": "Timeline", "schema_type": "object", "default_layout": "list", "description": "Chronological event display"},
                {"id": "tree", "label": "Tree", "icon": "Tree", "schema_type": "object", "default_layout": "list", "description": "Hierarchical nested data view"}
            ]),
            field_mappings: json!({
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
            layouts: json!([
                {"id": "grid", "name": "Grid", "columns": [1,2,3,4], "default_columns": 3, "gap": "1rem"},
                {"id": "list", "name": "List", "density": ["compact","comfortable","spacious"], "default_density": "comfortable"},
                {"id": "tabs", "name": "Tabbed", "tab_position": ["top","left","right"], "default_position": "top"},
                {"id": "masonry", "name": "Masonry", "column_width": 300, "gap": "1rem"},
                {"id": "carousel", "name": "Carousel", "items_per_view": [1,2,3], "autoplay": false}
            ]),
            render_config: json!({
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
            gallery_config: json!({
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
            sub_views: json!([
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
    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(Self::current_state())?)
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

pub(crate) fn schema_renderer_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(
        super::schema_renderer::SchemaRendererPlugin::current_state(),
    )
    .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "schema_renderer",
        "ui",
        "1.0.0",
        "Schema Renderer - dynamic JSON Schema to React form generation with auto-gallery",
        &state,
    )
}
