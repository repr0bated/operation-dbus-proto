//! Cognitive Tools for MCP
//!
//! MCP tools backed by the SQLite namespace/entry memory store.
//! Operations: store, retrieve, query, delete, list_namespaces, stats.

use crate::memory_store::{CognitiveMemoryStore, EntryQuery, NamespaceKind};
use anyhow::Result;
use async_trait::async_trait;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub struct CognitiveToolRegistry;

impl CognitiveToolRegistry {
    pub async fn register_all(
        registry: &ToolRegistry,
        store: Arc<CognitiveMemoryStore>,
    ) -> Result<()> {
        registry
            .register(Arc::new(MemoryTool::new(store.clone())) as BoxedTool)
            .await?;
        Ok(())
    }
}

pub struct MemoryTool {
    store: Arc<CognitiveMemoryStore>,
}

impl MemoryTool {
    pub fn new(store: Arc<CognitiveMemoryStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "cognitive_memory"
    }

    fn description(&self) -> &str {
        "Manage cognitive memory namespaces and entries. Operations: store, retrieve, query, delete, list_namespaces, stats."
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "memory".to_string(),
            "cognitive".to_string(),
            "storage".to_string(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["store", "retrieve", "query", "delete", "list_namespaces", "stats"],
                    "description": "Operation to perform"
                },
                "namespace": {
                    "type": "string",
                    "description": "Namespace name (e.g. 'project:op-dbus', 'session:abc', 'agent:planner')"
                },
                "namespace_kind": {
                    "type": "string",
                    "enum": ["project", "session", "database", "workflow", "agent", "cron", "custom"],
                    "description": "Kind of namespace (used when creating)"
                },
                "key": {
                    "type": "string",
                    "description": "Entry key within namespace"
                },
                "value": {
                    "description": "Value to store (any JSON)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for the entry"
                },
                "key_pattern": {
                    "type": "string",
                    "description": "Substring pattern for key search (used in query)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results (default 50)"
                }
            },
            "required": ["operation"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let op = input["operation"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing operation"))?;

        match op {
            "store" => self.op_store(&input).await,
            "retrieve" => self.op_retrieve(&input).await,
            "query" => self.op_query(&input).await,
            "delete" => self.op_delete(&input).await,
            "list_namespaces" => self.op_list_namespaces(&input).await,
            "stats" => self.op_stats().await,
            other => Err(anyhow::anyhow!("unknown operation: {}", other)),
        }
    }
}

impl MemoryTool {
    async fn ensure_namespace(&self, name: &str, kind_str: Option<&str>) -> Result<()> {
        let kind = kind_str
            .and_then(|s| s.parse::<NamespaceKind>().ok())
            .unwrap_or_else(|| {
                if name.starts_with("project:") {
                    NamespaceKind::Project
                } else if name.starts_with("session:") {
                    NamespaceKind::Session
                } else if name.starts_with("agent:") {
                    NamespaceKind::Agent
                } else if name.starts_with("cron:") {
                    NamespaceKind::Cron
                } else if name.starts_with("workflow:") {
                    NamespaceKind::Workflow
                } else if name.starts_with("db:") {
                    NamespaceKind::Database
                } else {
                    NamespaceKind::Custom
                }
            });

        if self.store.get_namespace_by_name(name).await?.is_none() {
            self.store
                .upsert_namespace(name, kind, None, None, None, serde_json::json!({}))
                .await?;
        }
        Ok(())
    }

    async fn op_store(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;
        let value = simd_json_to_serde(&input["value"]);
        let tags: Vec<String> = input["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        self.ensure_namespace(namespace, input["namespace_kind"].as_str())
            .await?;

        let entry = self
            .store
            .store_entry(namespace, key, value, tags, None)
            .await?;
        Ok(json!({ "ok": true, "id": entry.id, "namespace": namespace, "key": key }))
    }

    async fn op_retrieve(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        match self.store.retrieve_entry(namespace, key).await? {
            Some(e) => {
                let val = serde_to_simd_json(e.value);
                Ok(json!({
                    "found": true,
                    "id": e.id,
                    "namespace": namespace,
                    "key": e.key,
                    "value": val,
                    "tags": e.tags,
                    "access_count": e.access_count,
                    "updated_at": e.updated_at.to_rfc3339()
                }))
            }
            None => Ok(json!({ "found": false, "namespace": namespace, "key": key })),
        }
    }

    async fn op_query(&self, input: &Value) -> Result<Value> {
        let q = EntryQuery {
            namespace_id: input["namespace"].as_str().map(String::from),
            key_pattern: input["key_pattern"].as_str().map(String::from),
            tags: input["tags"].as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
            limit: input["limit"].as_i64(),
            offset: None,
        };

        let entries = self.store.query_entries(q).await?;
        let count = entries.len();
        let items: Vec<Value> = entries
            .into_iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "namespace_id": e.namespace_id,
                    "key": e.key,
                    "tags": e.tags,
                    "access_count": e.access_count,
                    "updated_at": e.updated_at.to_rfc3339()
                })
            })
            .collect();

        Ok(json!({ "count": count, "entries": items }))
    }

    async fn op_delete(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        let deleted = self.store.delete_entry(namespace, key).await?;
        Ok(json!({ "ok": deleted, "namespace": namespace, "key": key }))
    }

    async fn op_list_namespaces(&self, input: &Value) -> Result<Value> {
        let kind = input["namespace_kind"]
            .as_str()
            .and_then(|s| s.parse::<NamespaceKind>().ok());

        let namespaces = self.store.list_namespaces(kind).await?;
        let count = namespaces.len();
        let items: Vec<Value> = namespaces
            .into_iter()
            .map(|ns| {
                json!({
                    "id": ns.id,
                    "name": ns.name,
                    "kind": ns.kind.to_string(),
                    "description": ns.description,
                    "linked_task_id": ns.linked_task_id,
                    "linked_cron": ns.linked_cron
                })
            })
            .collect();

        Ok(json!({ "count": count, "namespaces": items }))
    }

    async fn op_stats(&self) -> Result<Value> {
        let stats = self.store.get_stats().await?;
        Ok(json!({
            "total_namespaces": stats.total_namespaces,
            "total_entries": stats.total_entries,
            "entries_by_kind": stats.entries_by_kind
        }))
    }
}

fn simd_json_to_serde(v: &Value) -> serde_json::Value {
    let s = simd_json::to_string(v).unwrap_or_default();
    serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)
}

fn serde_to_simd_json(v: serde_json::Value) -> Value {
    let s = serde_json::to_string(&v).unwrap_or_default();
    let mut buf = s.into_bytes();
    unsafe { simd_json::from_slice(&mut buf) }.unwrap_or(Value::Static(simd_json::StaticNode::Null))
}
