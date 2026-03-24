//! Cognitive Tools for MCP
//!
//! Tools that leverage cognitive memory and dynamic loading capabilities.

use crate::memory_store::{CognitiveMemoryStore, MemoryEntry, MemoryType, MemoryQuery};
use op_mcp::tool_registry::{Tool, ToolContent, ToolResult, ToolMetadata, SecurityLevel};
use simd_json::OwnedValue as Value;
use simd_json::prelude::*;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

pub struct MemoryTool {
    memory_store: Arc<CognitiveMemoryStore>,
}

impl MemoryTool {
    pub fn new(memory_store: Arc<CognitiveMemoryStore>) -> Self {
        Self { memory_store }
    }
}

#[async_trait::async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "cognitive_memory"
    }

    fn description(&self) -> &str {
        "Manage cognitive memory: store, retrieve, query, and manage memory entries"
    }

    fn input_schema(&self) -> Value {
        simd_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["store", "retrieve", "query", "delete", "cleanup", "stats"],
                    "description": "Memory operation to perform"
                },
                "key": {
                    "type": "string",
                    "description": "Memory key (required for store/retrieve)"
                },
                "value": {
                    "description": "Value to store (required for store)"
                },
                "memory_type": {
                    "type": "string",
                    "enum": ["ephemeral", "persistent", "shared"],
                    "description": "Type of memory (default: ephemeral)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for memory entry"
                },
                "query": {
                    "type": "object",
                    "description": "Query parameters for search operations"
                }
            },
            "required": ["operation"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value, anyhow::Error> {
        let operation = params["operation"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing operation parameter"))?;

        let result = match operation {
            "store" => self.store_memory(params).await,
            "retrieve" => self.retrieve_memory(params).await,
            "query" => self.query_memory(params).await,
            "delete" => self.delete_memory(params).await,
            "cleanup" => self.cleanup_memory().await,
            "stats" => self.get_memory_stats().await,
            _ => Err(anyhow::anyhow!("Unknown operation: {}", operation)),
        }?;
        Ok(Value::from(result))
    }
}

impl MemoryTool {
    pub fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            name: "cognitive_memory".to_string(),
            description: "Advanced memory management for cognitive operations".to_string(),
            category: "cognitive".to_string(),
            tags: vec!["memory".to_string(), "cognitive".to_string(), "storage".to_string()],
            author: Some("op-dbus".to_string()),
            version: "1.0.0".to_string(),
            security_level: SecurityLevel::Elevated,
            requires_auth: true,
        }
    }
}

use op_mcp::tool_registry::ToolRegistry;

pub struct CognitiveToolRegistry {
    registry: ToolRegistry,
}

impl CognitiveToolRegistry {
    pub fn new(memory_store: Arc<CognitiveMemoryStore>) -> Self {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(MemoryTool::new(memory_store)));
        Self { registry }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}

impl MemoryTool {
    async fn store_memory(&self, params: Value) -> Result<ToolResult, anyhow::Error> {
        let key = params["key"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing key parameter"))?;
        let value = params["value"].clone();

        let memory_type = match params["memory_type"].as_str() {
            Some("persistent") => MemoryType::Persistent,
            Some("shared") => MemoryType::Shared,
            _ => MemoryType::Ephemeral,
        };

        let tags = params["tags"].as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        let entry = MemoryEntry {
            id: Uuid::new_v4().to_string(),
            key: key.to_string(),
            value,
            memory_type,
            tags,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            expires_at: None, // TODO: Add expiration support
            access_count: 0,
            last_accessed: Utc::now(),
        };

        self.memory_store.store(entry.clone()).await
            .map_err(|e| anyhow::anyhow!("Failed to store memory: {}", e))?;

        Ok(ToolResult::success(ToolContent::text(format!(
            "Stored memory entry with key '{}' and ID '{}'",
            key, entry.id
        ))))
    }

    async fn retrieve_memory(&self, params: Value) -> Result<ToolResult, anyhow::Error> {
        let key = params["key"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing key parameter"))?;

        // Find by key (assuming keys are unique)
        let query = MemoryQuery {
            key_pattern: Some(key.to_string()),
            limit: Some(1),
            ..Default::default()
        };

        let results = self.memory_store.query(query).await;

        if let Some(entry) = results.into_iter().next() {
            Ok(ToolResult::success(ToolContent::json(simd_json::json!({
                "id": entry.id,
                "key": entry.key,
                "value": entry.value,
                "memory_type": entry.memory_type,
                "tags": entry.tags,
                "created_at": entry.created_at,
                "access_count": entry.access_count
            }))))
        } else {
            Ok(ToolResult::success(ToolContent::text(format!("No memory entry found for key '{}'", key))))
        }
    }

    async fn query_memory(&self, params: Value) -> Result<ToolResult, anyhow::Error> {
        let query_params = params["query"].as_object()
            .ok_or_else(|| anyhow::anyhow!("Missing query parameter"))?;

        let query = MemoryQuery {
            key_pattern: query_params.get("key_pattern").and_then(|v| v.as_str()).map(|s| s.to_string()),
            memory_type: query_params.get("memory_type").and_then(|v| v.as_str()).map(|s| match s {
                "persistent" => MemoryType::Persistent,
                "shared" => MemoryType::Shared,
                _ => MemoryType::Ephemeral,
            }),
            tags: query_params.get("tags").and_then(|v| v.as_array()).map(|arr|
                arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
            ),
            limit: query_params.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize),
            offset: query_params.get("offset").and_then(|v| v.as_u64()).map(|n| n as usize),
        };

        let results = self.memory_store.query(query).await;

        Ok(ToolResult::success(ToolContent::json(simd_json::json!({
            "count": results.len(),
            "entries": results.into_iter().map(|entry| simd_json::json!({
                "id": entry.id,
                "key": entry.key,
                "memory_type": entry.memory_type,
                "tags": entry.tags,
                "created_at": entry.created_at,
                "access_count": entry.access_count
            })).collect::<Vec<_>>()
        }))))
    }

    async fn delete_memory(&self, params: Value) -> Result<ToolResult, anyhow::Error> {
        let key = params["key"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing key parameter"))?;

        // Find and delete by key
        let query = MemoryQuery {
            key_pattern: Some(key.to_string()),
            limit: Some(1),
            ..Default::default()
        };

        let results = self.memory_store.query(query).await;

        if let Some(entry) = results.into_iter().next() {
            let deleted = self.memory_store.delete(&entry.id).await;
            if deleted {
                Ok(ToolResult::success(ToolContent::text(format!("Deleted memory entry '{}'", key))))
            } else {
                Ok(ToolResult::success(ToolContent::text(format!("Failed to delete memory entry '{}'", key))))
            }
        } else {
            Ok(ToolResult::success(ToolContent::text(format!("No memory entry found for key '{}'", key))))
        }
    }

    async fn cleanup_memory(&self) -> Result<ToolResult, anyhow::Error> {
        let cleaned = self.memory_store.cleanup_expired().await;
        Ok(ToolResult::success(ToolContent::text(format!("Cleaned up {} expired memory entries", cleaned))))
    }

    async fn get_memory_stats(&self) -> Result<ToolResult, anyhow::Error> {
        let stats = self.memory_store.get_stats().await;
        Ok(ToolResult::success(ToolContent::json(simd_json::json!(stats))))
    }
}