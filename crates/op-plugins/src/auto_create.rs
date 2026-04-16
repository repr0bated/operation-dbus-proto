//! Auto-Discovery and Creation of Plugins
//!
//! This module provides the capability to automatically discover system services
//! and create corresponding state plugins.

use anyhow::Result;
use async_trait::async_trait;
use op_state::StatePlugin;
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Auto-creator for systemd-based plugins
pub struct SystemdAutoCreator;

impl SystemdAutoCreator {
    /// Discover systemd units and create plugins
    pub async fn discover_units() -> Result<Vec<(String, Value)>> {
        let mut plugins = Vec::new();

        // Example discovery: find all active .service units
        // In a real implementation, this would query systemd via D-Bus
        let discovered_units = vec!["nginx.service", "redis.service", "postgresql.service"];

        for unit in discovered_units {
            plugins.push((
                unit.to_string(),
                json!({
                    "type": "systemd",
                    "name": unit,
                    "state": "active",
                    "enabled": true
                }),
            ));
        }

        Ok(plugins)
    }
}

/// Generic auto-plugin that can wrap discovered services
pub struct AutoPlugin {
    name: String,
    _category: String,
    current_state: Arc<RwLock<Value>>,
    schema: Value,
}

impl AutoPlugin {
    pub fn new(name: &str, category: &str, initial_state: Value) -> Self {
        let schema = Self::generate_schema(name, category, &initial_state);
        Self {
            name: name.to_string(),
            _category: category.to_string(),
            current_state: Arc::new(RwLock::new(initial_state)),
            schema,
        }
    }

    /// Generate a schema for this auto-plugin
    /// Every auto-created plugin gets a schema with schema_derived=true
    fn generate_schema(name: &str, category: &str, _initial_state: &Value) -> Value {
        let object_type_name = format!("{}Object", to_pascal_case(name));
        let base_path = format!("/org/opdbus/auto/{}", name);
        let interface = format!("org.opdbus.auto.{}.{}", name, object_type_name);

        let mut object_types = HashMap::new();
        object_types.insert(
            object_type_name.clone(),
            json!({
                "description": format!("Auto-generated {} object", name),
                "base_path": base_path,
                "interface": interface,
                "schema_derived": true,
                "category": category
            }),
        );

        json!({
            "version": "1.0.0",
            "plugin_type": category,
            "type": object_type_name,
            "description": format!("Auto-generated plugin for {}", name),
            "auto_generated": true,
            "object_types": object_types,
            "common_properties": {
                "id": {"type": "string", "required": true},
                "object_type": {"type": "string", "required": true},
                "name": {"type": "string", "required": true}
            }
        })
    }

    /// Get the generated schema for this plugin
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    /// Convert raw state to schema-compliant format with object_type
    fn normalize_state(&self, state: &Value) -> Value {
        // If state is already an array, ensure each item has object_type
        if let Some(arr) = state.as_array() {
            let mut normalized = Vec::new();
            for (idx, item) in arr.iter().enumerate() {
                let mut obj = item.clone();
                // Use simd_json's Object API
                let obj_ref = obj.as_object_mut();
                if let Some(obj_map) = obj_ref {
                    let has_type = obj_map.iter().any(|(k, _)| k == "object_type");
                    let has_id = obj_map.iter().any(|(k, _)| k == "id");
                    if !has_type {
                        obj_map.insert("object_type".to_string(), json!("AutoObject"));
                    }
                    if !has_id {
                        obj_map.insert("id".to_string(), json!(format!("{}-{}", self.name, idx)));
                    }
                }
                normalized.push(obj);
            }
            return json!(normalized);
        }

        // If state is a single object, wrap it in an array with proper fields
        let mut obj = state.clone();
        let obj_ref = obj.as_object_mut();
        if let Some(obj_map) = obj_ref {
            let has_type = obj_map.iter().any(|(k, _)| k == "object_type");
            let has_id = obj_map.iter().any(|(k, _)| k == "id");
            let has_name = obj_map.iter().any(|(k, _)| k == "name");
            if !has_type {
                obj_map.insert("object_type".to_string(), json!("AutoObject"));
            }
            if !has_id {
                obj_map.insert("id".to_string(), json!(format!("{}-0", self.name)));
            }
            if !has_name {
                obj_map.insert("name".to_string(), json!(self.name.clone()));
            }
        }
        json!(vec![obj])
    }
}

/// Convert snake_case to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[async_trait]
impl StatePlugin for AutoPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = self.current_state.read().await.clone();
        Ok(self.normalize_state(&state))
    }

    async fn calculate_diff(
        &self,
        current: &Value,
        desired: &Value,
    ) -> Result<op_state::StateDiff> {
        // Simple generic diff: if not equal, replace
        let mut actions = Vec::new();
        if current != desired {
            actions.push(op_state::StateAction::Create {
                resource: self.name.clone(),
                config: desired.clone(),
            });
        }

        Ok(op_state::StateDiff {
            plugin: self.name.clone(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &op_state::StateDiff) -> Result<op_state::ApplyResult> {
        let changes = Vec::new();
        let errors = Vec::new();

        for action in &diff.actions {
            if let op_state::StateAction::Create { config, .. } = action {
                let mut state = self.current_state.write().await;
                *state = config.clone();
            }
        }

        Ok(op_state::ApplyResult {
            success: true,
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.current_state.read().await;
        Ok(&*current == desired)
    }

    async fn create_checkpoint(&self) -> Result<op_state::Checkpoint> {
        let state = self.current_state.read().await;
        Ok(op_state::Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state.clone(),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &op_state::Checkpoint) -> Result<()> {
        let mut state = self.current_state.write().await;
        *state = checkpoint.state_snapshot.clone();
        Ok(())
    }

    fn capabilities(&self) -> op_state::PluginCapabilities {
        op_state::PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }
}
