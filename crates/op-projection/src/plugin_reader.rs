//! Plugin Reader: Reading from plugins.
//!
//! This module implements the `PluginReader` trait by loading the default
//! runtime plugins, querying their live state, and emitting both top-level
//! plugin state entities and nested object projections.

use crate::data_models::{FieldSchema, FieldType, PluginSchema};
use crate::interfaces::{PluginLifecycleEvent, PluginReader, RawEntity, SourceReader};
use anyhow::{Context, Result};
use op_plugins::DefaultPluginRegistry;
use op_state::StatePlugin;
use op_state_store::{builtin_plugin_schema, MemoryStore, StateStore};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::future::Future;
use std::sync::Arc;
use tracing::{debug, info, warn};

struct LoadedPlugin {
    name: String,
    schema: Option<PluginSchema>,
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
        let read_only_string = |description: &str| FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: description.to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        };

        PluginSchema::builder("plugin.object")
            .version("1.0.0")
            .category("plugin")
            .field("plugin_id", read_only_string("Owning plugin identifier"))
            .field("parent_id", read_only_string("Parent projection entity ID"))
            .field(
                "object_path",
                read_only_string("JSON pointer-like path to the nested object"),
            )
            .field(
                "value",
                FieldSchema {
                    field_type: FieldType::Any,
                    required: true,
                    description: "Nested object value mirrored from plugin state".to_string(),
                    default: None,
                    example: None,
                    constraints: Vec::new(),
                    read_only: true,
                    read_only_when: None,
                },
            )
            .build()
    }

    /// Returns all schemas required for plugin state projection.
    ///
    /// Plugin schemas are the canonical `PluginSchema` directly — no conversion:
    /// the plugin is the schema.
    pub fn projection_schemas(&self) -> Vec<PluginSchema> {
        let mut schemas = self
            .plugins
            .iter()
            .filter_map(|plugin| plugin.schema.clone())
            .collect::<Vec<_>>();
        schemas.push(Self::nested_object_projection_schema());
        schemas
    }

    /// Returns `(plugin_id, Option<PluginSchema>)` pairs for every loaded
    /// plugin.  Plugins returning `None` from `schema()` are included with
    /// `None` so the SHM writer can skip them.
    pub fn plugin_schemas_with_ids(&self) -> Vec<(String, Option<PluginSchema>)> {
        self.plugins
            .iter()
            .map(|plugin| (plugin.name.clone(), plugin.schema.clone()))
            .collect()
    }

    /// Queries the present-state of every loaded plugin and returns it as
    /// `(plugin_id, serde_json::Value)` pairs.  Plugins whose state query
    /// fails are skipped with a warning.
    pub async fn plugin_present_states(&self) -> Vec<(String, serde_json::Value)> {
        let mut result = Vec::new();
        for plugin in &self.plugins {
            match plugin.plugin.query_current_state().await {
                Ok(state) => {
                    // Convert simd_json::OwnedValue to serde_json::Value via
                    // string round-trip.
                    let json_str = match simd_json::to_string(&state) {
                        Ok(s) => s,
                        Err(e) => {
                            warn!(
                                plugin_id = %plugin.name,
                                error = %e,
                                "Failed to serialize plugin state for SHM"
                            );
                            continue;
                        }
                    };
                    let value: serde_json::Value = match serde_json::from_str(&json_str) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                plugin_id = %plugin.name,
                                error = %e,
                                "Failed to parse plugin state as serde_json"
                            );
                            continue;
                        }
                    };
                    result.push((plugin.name.clone(), value));
                }
                Err(e) => {
                    warn!(
                        plugin_id = %plugin.name,
                        error = %e,
                        "Failed to query plugin state for SHM present-state"
                    );
                }
            }
        }
        result
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
                        }),
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
                        }),
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
    fn should_consume_canonical_plugin_schema_directly() {
        // The plugin is the schema: projection uses the canonical PluginSchema
        // with no conversion. Fields are keyed in a HashMap.
        let schema = PluginSchema::builder("net")
            .version("1.2.3")
            .category("network")
            .description("Network schema")
            .field(
                "interfaces",
                FieldSchema {
                    field_type: FieldType::Array(Box::new(FieldType::String)),
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

        assert_eq!(schema.name, "net");
        assert_eq!(schema.version, "1.2.3");
        assert_eq!(schema.fields.len(), 1);
        let interfaces = schema.fields.get("interfaces").expect("interfaces field");
        assert_eq!(
            interfaces.field_type,
            FieldType::Array(Box::new(FieldType::String))
        );
        assert!(interfaces.read_only);
    }
}
