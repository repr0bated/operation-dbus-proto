//! Tools backed by plugin-created D-Bus projection objects.
//!
//! Every object published below `/opdbus/v1/plugins` is exposed as a
//! read-only tool. Execution reads the live `org.opdbus.ProjectedObjectV1`
//! object rather than scraping procfs or rebuilding state locally.

use crate::tool::Tool;
use crate::ToolRegistry;
use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;

const PLUGIN_ROOT: &str = "/org/opdbus/v1/plugins";
const OPDBUS_DEST: &str = "org.opdbus.v1";
const PROJECTED_IFACE: &str = "org.opdbus.ProjectedObjectV1";

#[derive(Clone)]
pub struct PluginProjectionTool {
    name: String,
    service: String,
    plugin_name: String,
    object_path: String,
}

impl PluginProjectionTool {
    pub fn new(plugin_name: &str, object_path: String) -> Self {
        Self {
            name: tool_name_for_path(&object_path),
            service: OPDBUS_DEST.to_string(),
            plugin_name: plugin_name.to_string(),
            object_path,
        }
    }

    pub fn new_generic(service: &str, object_path: String) -> Self {
        let name = format!(
            "projection_{}_{}",
            sanitize_segment(service.split('.').last().unwrap_or(service)).to_ascii_lowercase(),
            sanitize_segment(object_path.split('/').last().unwrap_or(&object_path))
                .to_ascii_lowercase()
        );
        Self {
            name,
            service: service.to_string(),
            plugin_name: "generic".to_string(),
            object_path,
        }
    }

    async fn connection() -> Result<zbus::Connection> {
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok() {
            zbus::Connection::session()
                .await
                .map_err(|e| anyhow::anyhow!("failed to connect to shared session bus: {}", e))
        } else {
            zbus::Connection::system()
                .await
                .map_err(|e| anyhow::anyhow!("failed to connect to system bus: {}", e))
        }
    }
}

#[async_trait]
impl Tool for PluginProjectionTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Read a live object created by a PluginSchema-backed plugin projection."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "property": {
                    "type": "string",
                    "description": "Optional top-level JSON property to read from the projected object"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let connection = Self::connection().await?;
        let proxy: zbus::Proxy<'_> = zbus::proxy::Builder::new(&connection)
            .destination(self.service.as_str())?
            .path(self.object_path.as_str())?
            .interface(PROJECTED_IFACE)?
            .build()
            .await?;

        let json_text: String =
            if let Some(property) = input.get("property").and_then(|v| v.as_str()) {
                proxy
                    .call::<_, _, String>("get_property", &(property.to_string(),))
                    .await?
            } else {
                proxy.get_property::<String>("json_data").await?
            };

        let mut buf = json_text.into_bytes();
        let data = simd_json::from_slice::<Value>(&mut buf).unwrap_or_else(|_| Value::null());
        Ok(json!({
            "plugin": self.plugin_name,
            "service": self.service,
            "object_path": self.object_path,
            "data": data
        }))
    }

    fn category(&self) -> &str {
        "plugin"
    }

    fn namespace(&self) -> &str {
        "plugin-projection"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "plugin".to_string(),
            "projection".to_string(),
            self.plugin_name.clone(),
        ]
    }
}

pub async fn register_plugin_projection_tools(
    registry: &ToolRegistry,
    plugin_state: &HashMap<String, Value>,
) -> Result<usize> {
    let mut count = 0;

    for (plugin_name, state) in plugin_state {
        let root_path = plugin_path(plugin_name);
        let mut paths = vec![root_path.clone()];
        collect_child_paths(&root_path, state, &mut paths);

        for path in paths {
            registry
                .register_tool(Arc::new(PluginProjectionTool::new(plugin_name, path)))
                .await?;
            count += 1;
        }
    }

    Ok(count)
}

fn collect_child_paths(root_path: &str, data: &Value, out: &mut Vec<String>) {
    match data {
        Value::Object(map) => {
            for (key, value) in map.iter() {
                let child_path = format!("{}/{}", root_path, sanitize_segment(key.as_str()));
                out.push(child_path.clone());
                collect_child_paths(&child_path, value, out);
            }
        }
        Value::Array(items) => {
            for (idx, value) in items.iter().enumerate() {
                let child_path = format!("{}/{}", root_path, idx);
                out.push(child_path.clone());
                collect_child_paths(&child_path, value, out);
            }
        }
        _ => {}
    }
}

fn plugin_path(plugin_name: &str) -> String {
    format!("{}/{}", PLUGIN_ROOT, sanitize_segment(plugin_name))
}

fn sanitize_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for ch in segment.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

fn tool_name_for_path(path: &str) -> String {
    let suffix = path
        .strip_prefix(PLUGIN_ROOT)
        .unwrap_or(path)
        .trim_matches('/');
    let mut name = String::from("plugin_projection");
    for part in suffix.split('/').filter(|part| !part.is_empty()) {
        name.push('_');
        name.push_str(&sanitize_segment(part).to_ascii_lowercase());
    }
    name
}
