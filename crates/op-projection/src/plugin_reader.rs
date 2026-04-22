//! Plugin Reader: Reading from plugins.
//!
//! This module implements the `PluginReader` trait, reading plugin objects
//! and handling plugin lifecycle events.

use crate::interfaces::{PluginLifecycleEvent, PluginReader, RawEntity, SourceReader};
use anyhow::Result;
use simd_json::json;
use tracing::{debug, info};

/// Reader that extracts state from plugins.
#[derive(Debug)]
pub struct SystemPluginReader {
    /// Source identifier
    source: String,
}

impl SystemPluginReader {
    /// Creates a new SystemPluginReader
    pub fn new() -> Self {
        Self {
            source: "plugin".to_string(),
        }
    }
}

impl Default for SystemPluginReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for SystemPluginReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        debug!("Reading all plugin objects");
        Ok(Vec::new())
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        debug!(entity_id = entity_id, "Reading plugin entity");
        Ok(RawEntity {
            entity_type: "plugin.object".to_string(),
            entity_id: entity_id.to_string(),
            data: json!({ "properties": {} }).into(),
            source: self.source.clone(),
        })
    }

    fn source_id(&self) -> &str {
        &self.source
    }

    fn is_available(&self) -> bool {
        true
    }
}

impl PluginReader for SystemPluginReader {
    fn read_plugin_objects(&self, plugin_id: &str) -> Result<Vec<RawEntity>> {
        debug!(plugin_id = plugin_id, "Reading plugin objects");
        Ok(Vec::new())
    }

    fn read_nested_objects(&self, plugin_id: &str, parent_id: &str) -> Result<Vec<RawEntity>> {
        debug!(
            plugin_id = plugin_id,
            parent_id = parent_id,
            "Reading nested plugin objects"
        );
        Ok(Vec::new())
    }

    fn handle_lifecycle(&self, plugin_id: &str, event: PluginLifecycleEvent) {
        info!(
            plugin_id = plugin_id,
            event = ?event,
            "Plugin lifecycle event"
        );
    }
}
