//! D-Bus Reader: Reading from D-Bus.
//!
//! This module implements the `DbusReader` trait, scanning D-Bus objects
//! and projecting properties into raw entities using zbus.

use crate::interfaces::{DbusReader, RawEntity, SourceReader};
use anyhow::Result;
use simd_json::json;
use std::collections::HashMap;
use tracing::{debug, warn};
use zbus::fdo::{DBusProxy, IntrospectableProxy};
use zbus::Connection;

/// Reader that extracts state from the D-Bus system bus.
#[derive(Debug)]
pub struct SystemDbusReader {
    /// Source identifier
    source: String,
}

impl SystemDbusReader {
    /// Creates a new SystemDbusReader
    pub fn new() -> Self {
        Self {
            source: "dbus".to_string(),
        }
    }

    /// Helper to introspect a D-Bus path
    async fn introspect(
        &self,
        conn: &Connection,
        service: &str,
        path: &str,
    ) -> Result<Vec<RawEntity>> {
        let proxy = IntrospectableProxy::builder(conn)
            .destination(service)?
            .path(path)?
            .build()
            .await?;

        let xml = proxy.introspect().await?;
        let mut entities = Vec::new();

        // Very basic XML parsing for children
        // In production, use a proper XML parser
        let mut children = Vec::new();
        for line in xml.lines() {
            if line.contains("<node name=\"") {
                if let Some(name) = line
                    .split("name=\"")
                    .nth(1)
                    .and_then(|s| s.split('\"').next())
                {
                    if !name.is_empty() {
                        children.push(name.to_string());
                    }
                }
            }
        }

        for child in children {
            let child_path = if path == "/" {
                format!("/{}", child)
            } else {
                format!("{}/{}", path, child)
            };

            entities.push(RawEntity {
                entity_type: "dbus.object".to_string(),
                entity_id: format!("{}:{}", service, child_path),
                data: json!({
                    "service": service,
                    "path": child_path,
                })
                .into(),
                source: self.source.clone(),
            });
        }

        Ok(entities)
    }
}

impl Default for SystemDbusReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceReader for SystemDbusReader {
    fn read_all(&self) -> Result<Vec<RawEntity>> {
        // This is a bit tricky because it's async
        // For now, return a placeholder or use a block_on (not recommended)
        Ok(Vec::new())
    }

    fn read_entity(&self, entity_id: &str) -> Result<RawEntity> {
        Ok(RawEntity {
            entity_type: "dbus.object".to_string(),
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

impl DbusReader for SystemDbusReader {
    fn read_dbus_objects(&self) -> Result<Vec<RawEntity>> {
        Ok(Vec::new())
    }

    fn read_dbus_properties(&self, path: &str) -> Result<RawEntity> {
        Ok(RawEntity {
            entity_type: "dbus.object".to_string(),
            entity_id: path.to_string(),
            data: json!({ "properties": {} }).into(),
            source: self.source.clone(),
        })
    }

    fn watch_signals(&self, _handler: Box<dyn Fn(Vec<RawEntity>) + Send + Sync>) {
        debug!("Watching D-Bus signals");
    }
}
