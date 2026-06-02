//! Plugin Reader: Reading from plugins.
//!
//! This module implements the `PluginReader` trait by loading the default
//! runtime plugins, querying their live state, and emitting both top-level
//! plugin state entities and nested object projections.

use crate::data_models::{Constraint, FieldSchema, FieldType, PluginSchema};
use crate::interfaces::{PluginLifecycleEvent, PluginReader, RawEntity, SourceReader};
use anyhow::{Context, Result};
use op_plugins::DefaultPluginRegistry;
use op_state::StatePlugin;
use op_state_store::{
    builtin_plugin_schema, Constraint as RuntimeConstraint, FieldSchema as RuntimeFieldSchema,
    FieldType as RuntimeFieldType, MemoryStore, PluginSchema as RuntimePluginSchema, StateStore,
};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::future::Future;
use std::sync::Arc;
use tracing::{debug, info, warn};

struct LoadedPlugin {
    name: String,
    schema: Option<RuntimePluginSchema>,
    plugin: Arc<dyn StatePlugin>,
}

/// Reader that extracts live state from runtime plugins.
pub struct SystemPluginReader {
    /// Source identifier
    source: String,
    /// Loaded runtime plugins and their resolved schemas
    plugins: Vec<LoadedPlugin>,
}

impl std::fmt::Debug for SystemPluginReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let plugin_names: Vec<&str> = self
            .plugins
            .iter()
            .map(|plugin| plugin.name.as_str())
            .collect();
        f.debug_struct("SystemPluginReader")
            .field("source", &self.source)
            .field("plugins", &plugin_names)
            .finish()
    }
}

impl SystemPluginReader {
    /// Creates an empty reader when plugin bootstrap is unavailable.
    pub fn empty() -> Self {
        Self {
            source: "plugin".to_string(),
            plugins: Vec::new(),
        }
    }

    /// Creates a new SystemPluginReader backed by the default runtime plugins.
    /// Uses MemoryStore — no SQLite, zero drift. Current state = desired state.
    pub async fn new() -> Result<Self> {
        let state_store: Arc<dyn StateStore> = Arc::new(MemoryStore::new());

        let registry = DefaultPluginRegistry::new(state_store);
        let plugins = registry.load_default_plugins().await?;
        let plugins = plugins
            .into_iter()
            .map(|plugin| {
                let name = plugin.name().to_string();
                let schema = plugin.schema().or_else(|| builtin_plugin_schema(&name));

                if schema.is_none() {
                    warn!(
                        plugin_id = %name,
                        "Plugin has no PluginSchema; top-level state will be projected without plugin-specific validation"
                    );
                }

                LoadedPlugin { name, schema, plugin }
            })
            .collect::<Vec<_>>();

        info!(
            plugin_count = plugins.len(),
            "Initialized plugin projection reader"
        );

        Ok(Self {
            source: "plugin".to_string(),
            plugins,
        })
    }

    /// The schema used to validate nested plugin object projections.
    pub fn nested_object_projection_schema() -> PluginSchema {
        PluginSchema {
            name: "plugin.object".to_string(),
            version: "1.0.0".to_string(),
            fields: vec![
                FieldSchema {
                    name: "plugin_id".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("Owning plugin identifier".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
                FieldSchema {
                    name: "parent_id".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("Parent projection entity ID".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
                FieldSchema {
                    name: "object_path".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("JSON pointer-like path to the nested object".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
                FieldSchema {
                    name: "value".to_string(),
                    field_type: FieldType::Any,
                    required: true,
                    description: Some("Nested object value mirrored from plugin state".to_string()),
                    constraints: Vec::new(),
                    example: None,
                    read_only: true,
                },
            ],
            category: Some("plugin".to_string()),
            examples: None,
            secret_paths: Vec::new(),
            pii_paths: Vec::new(),
        }
    }

    /// Returns all schemas required for plugin state projection.
    pub fn projection_schemas(&self) -> Vec<PluginSchema> {
        let mut schemas = self
            .plugins
            .iter()
            .filter_map(|plugin| plugin.schema.as_ref().map(convert_schema))
            .collect::<Vec<_>>();
        schemas.push(Self::nested_object_projection_schema());
        schemas
    }

    /// Reads all plugin-backed projection entities asynchronously.
    pub async fn read_all_async(&self) -> Result<Vec<RawEntity>> {
        let mut entities = Vec::new();

        for plugin in &self.plugins {
            entities.extend(self.read_loaded_plugin(plugin).await?);
        }

        debug!(
            entity_count = entities.len(),
            "Read plugin projection entities"
        );
        Ok(entities)
    }

    /// Reads all projection entities for a single plugin asynchronously.
    pub async fn read_plugin_objects_async(&self, plugin_id: &str) -> Result<Vec<RawEntity>> {
        let requested_id = DefaultPluginRegistry::resolve_requested_plugin_name(plugin_id)
            .with_context(|| format!("invalid plugin request '{}'", plugin_id))?;
        let requested_plugin;
        let plugin = match self
            .plugins
            .iter()
            .find(|plugin| plugin.name == requested_id)
        {
            Some(plugin) => plugin,
            None => {
                requested_plugin = Self::load_requested_plugin(&requested_id).await?;
                &requested_plugin
            }
        };

        self.read_loaded_plugin(plugin).await
    }

    async fn load_requested_plugin(plugin_id: &str) -> Result<LoadedPlugin> {
        let state_store: Arc<dyn StateStore> = Arc::new(MemoryStore::new());
        let registry = DefaultPluginRegistry::new(state_store);
        let plugin = registry
            .load_plugin(plugin_id)
            .await
            .with_context(|| format!("failed to auto-load requested plugin '{}'", plugin_id))?;
        let name = plugin.name().to_string();
        let schema = plugin.schema().or_else(|| builtin_plugin_schema(&name));

        Ok(LoadedPlugin {
            name,
            schema,
            plugin,
        })
    }

    /// Reads nested object projections for a single plugin asynchronously.
    pub async fn read_nested_objects_async(
        &self,
        plugin_id: &str,
        parent_id: &str,
    ) -> Result<Vec<RawEntity>> {
        let entities = self.read_plugin_objects_async(plugin_id).await?;

        Ok(entities
            .into_iter()
            .filter(|entity| {
                entity.entity_type == "plugin.object"
                    && entity
                        .data
                        .get("parent_id")
                        .and_then(|value| value.as_str())
                        == Some(parent_id)
            })
            .collect())
    }

    async fn read_loaded_plugin(&self, plugin: &LoadedPlugin) -> Result<Vec<RawEntity>> {
        let state = match plugin.plugin.query_current_state().await {
            Ok(state) => state,
            Err(error) => {
                warn!(
                    plugin_id = %plugin.name,
                    error = %error,
                    "Skipping plugin projection because state query failed"
                );
                return Ok(Vec::new());
            }
        };

        let entity_type = plugin
            .schema
            .as_ref()
            .map(|schema| schema.name.clone())
            .unwrap_or_else(|| plugin.name.clone());
        let mut entities = vec![RawEntity {
            entity_type,
            entity_id: plugin.name.clone(),
            data: state.clone(),
            source: self.source.clone(),
        }];

        entities.extend(Self::collect_nested_entities(
            &plugin.name,
            &state,
            &self.source,
        ));

        Ok(entities)
    }

    fn collect_nested_entities(plugin_id: &str, state: &Value, source: &str) -> Vec<RawEntity> {
        let mut entities = Vec::new();
        Self::collect_nested_entities_recursive(
            &mut entities,
            plugin_id,
            plugin_id,
            "",
            state,
            source,
        );
        entities
    }

    fn collect_nested_entities_recursive(
        entities: &mut Vec<RawEntity>,
        plugin_id: &str,
        parent_id: &str,
        path: &str,
        value: &Value,
        source: &str,
    ) {
        match value {
            Value::Object(map) => {
                if !path.is_empty() {
                    entities.push(RawEntity {
                        entity_type: "plugin.object".to_string(),
                        entity_id: Self::nested_entity_id(plugin_id, path),
                        data: json!({
                            "plugin_id": plugin_id,
                            "parent_id": parent_id,
                            "object_path": path,
                            "value": value.clone(),
                        })
                        .into(),
                        source: source.to_string(),
                    });
                }

                let current_id = if path.is_empty() {
                    plugin_id.to_string()
                } else {
                    Self::nested_entity_id(plugin_id, path)
                };

                for (key, child) in map.iter() {
                    if child.is_object() || child.is_array() {
                        let child_path = format!("{}/{}", path, key);
                        Self::collect_nested_entities_recursive(
                            entities,
                            plugin_id,
                            &current_id,
                            &child_path,
                            child,
                            source,
                        );
                    }
                }
            }
            Value::Array(array) => {
                if !path.is_empty() {
                    entities.push(RawEntity {
                        entity_type: "plugin.object".to_string(),
                        entity_id: Self::nested_entity_id(plugin_id, path),
                        data: json!({
                            "plugin_id": plugin_id,
                            "parent_id": parent_id,
                            "object_path": path,
                            "value": value.clone(),
                        })
                        .into(),
                        source: source.to_string(),
                    });
                }

                let current_id = if path.is_empty() {
                    plugin_id.to_string()
                } else {
                    Self::nested_entity_id(plugin_id, path)
                };

                for (index, child) in array.iter().enumerate() {
                    if child.is_object() || child.is_array() {
                        let child_path = format!("{}/{}", path, index);
                        Self::collect_nested_entities_recursive(
                            entities,
                            plugin_id,
                            &current_id,
                            &child_path,
                            child,
                            source,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn nested_entity_id(plugin_id: &str, path: &str) -> String {
        format!("{}:{}", plugin_id, path)
    }

    fn block_on<F, T>(&self, future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
            Err(_) => {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("failed to build a tokio runtime for plugin projection")?;
                runtime.block_on(future)
            }
        }
    }
}

impl Default for SystemPluginReader {
    fn default() -> Self {
        Self::empty()
    }
}

impl SourceReader for SystemPluginReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        self.block_on(self.read_all_async())
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        let entities = self.block_on(self.read_all_async())?;
        entities
            .into_iter()
            .find(|entity| entity.entity_id == entity_id)
            .with_context(|| format!("unknown plugin entity '{}'", entity_id))
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        !self.plugins.is_empty()
    }
}

impl PluginReader for SystemPluginReader {
    fn read_plugin_objects(&self, plugin_id: &str) -> Result<Vec<RawEntity>> {
        debug!(plugin_id = plugin_id, "Reading plugin objects");
        self.block_on(self.read_plugin_objects_async(plugin_id))
    }

    fn read_nested_objects(&self, plugin_id: &str, parent_id: &str) -> Result<Vec<RawEntity>> {
        debug!(
            plugin_id = plugin_id,
            parent_id = parent_id,
            "Reading nested plugin objects"
        );
        self.block_on(self.read_nested_objects_async(plugin_id, parent_id))
    }

    fn handle_lifecycle(&self, plugin_id: &str, event: PluginLifecycleEvent) {
        info!(
            plugin_id = plugin_id,
            event = ?event,
            "Plugin lifecycle event"
        );
    }
}

/// Convert an `op_state_store::PluginSchema` into an `op_projection::PluginSchema`.
pub fn convert_schema(schema: &RuntimePluginSchema) -> PluginSchema {
    PluginSchema {
        name: schema.name.clone(),
        version: schema.version.clone(),
        fields: schema
            .fields
            .iter()
            .map(|(name, field)| convert_field(name, field))
            .collect(),
        category: Some(schema.category.clone()),
        examples: schema.example.clone().map(|example| vec![example]),
        secret_paths: Vec::new(),
        pii_paths: Vec::new(),
    }
}

fn convert_field(name: &str, field: &RuntimeFieldSchema) -> FieldSchema {
    FieldSchema {
        name: name.to_string(),
        field_type: convert_field_type(&field.field_type),
        required: field.required,
        description: Some(field.description.clone()).filter(|description| !description.is_empty()),
        constraints: field
            .constraints
            .iter()
            .filter_map(convert_constraint)
            .collect(),
        example: field.example.clone(),
        read_only: field.read_only,
    }
}

fn convert_field_type(field_type: &RuntimeFieldType) -> FieldType {
    match field_type {
        RuntimeFieldType::String => FieldType::String,
        RuntimeFieldType::Integer => FieldType::Integer,
        RuntimeFieldType::Float => FieldType::Number,
        RuntimeFieldType::Boolean => FieldType::Boolean,
        RuntimeFieldType::Array(inner) => FieldType::Array(Box::new(convert_field_type(inner))),
        RuntimeFieldType::Object(_) => FieldType::Object,
        RuntimeFieldType::Enum(values) => FieldType::Enum(values.clone()),
        RuntimeFieldType::Any => FieldType::Any,
    }
}

fn convert_constraint(constraint: &RuntimeConstraint) -> Option<Constraint> {
    match constraint {
        RuntimeConstraint::Min { value } => Some(Constraint::MinValue(*value as i64)),
        RuntimeConstraint::Max { value } => Some(Constraint::MaxValue(*value as i64)),
        RuntimeConstraint::Pattern { regex } => Some(Constraint::Pattern(regex.clone())),
        RuntimeConstraint::OneOf { values } => {
            let values = values
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect::<Vec<_>>();

            if values.is_empty() {
                None
            } else {
                Some(Constraint::Enum(values))
            }
        }
        RuntimeConstraint::RequiresField { .. } | RuntimeConstraint::Custom { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;

    #[test]
    fn should_project_nested_plugin_objects() {
        let state = json!({
            "interfaces": [
                {
                    "name": "wg0",
                    "peers": [
                        { "name": "peer-a" }
                    ]
                }
            ],
            "metadata": {
                "enabled": true
            }
        });

        let entities = SystemPluginReader::collect_nested_entities("wireguard", &state, "plugin");
        let entity_ids = entities
            .iter()
            .map(|entity| entity.entity_id.clone())
            .collect::<Vec<_>>();

        assert!(entity_ids.contains(&"wireguard:/interfaces".to_string()));
        assert!(entity_ids.contains(&"wireguard:/interfaces/0".to_string()));
        assert!(entity_ids.contains(&"wireguard:/interfaces/0/peers".to_string()));
        assert!(entity_ids.contains(&"wireguard:/metadata".to_string()));

        let peers = entities
            .iter()
            .find(|entity| entity.entity_id == "wireguard:/interfaces/0/peers")
            .expect("peers projection");
        assert_eq!(peers.data["parent_id"], "wireguard:/interfaces/0");
        assert_eq!(peers.data["plugin_id"], "wireguard");
    }

    #[test]
    fn should_convert_runtime_schema_to_projection_schema() {
        let schema = RuntimePluginSchema::builder("net")
            .version("1.2.3")
            .category("network")
            .description("Network schema")
            .field(
                "interfaces",
                RuntimeFieldSchema {
                    field_type: RuntimeFieldType::Array(Box::new(RuntimeFieldType::String)),
                    required: true,
                    description: "Interface names".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: true,
                    read_only_when: None,
                },
            )
            .build();

        let converted = convert_schema(&schema);
        assert_eq!(converted.name, "net");
        assert_eq!(converted.version, "1.2.3");
        assert_eq!(converted.fields.len(), 1);
        assert_eq!(converted.fields[0].name, "interfaces");
        assert_eq!(
            converted.fields[0].field_type,
            FieldType::Array(Box::new(FieldType::String))
        );
        assert!(converted.fields[0].read_only);
    }
}
