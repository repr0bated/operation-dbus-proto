//! Cognitive Tools for MCP
//!
//! MCP tools for transitional memory operations and authoritative graph inspection.

use crate::graph_store::{KnowledgeGraphStore, ProjectedEvent};
use anyhow::Result;
use async_trait::async_trait;
use op_blockchain::PluginFootprint;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct CognitiveToolRegistry;

impl CognitiveToolRegistry {
    pub async fn register_all(
        registry: &ToolRegistry,
        graph_store: Arc<KnowledgeGraphStore>,
    ) -> Result<()> {
        registry
            .register(Arc::new(MemoryTool::new(graph_store.clone())) as BoxedTool)
            .await?;
        registry
            .register(Arc::new(GraphTool::new(graph_store)) as BoxedTool)
            .await?;
        Ok(())
    }
}

pub struct MemoryTool {
    graph_store: Arc<KnowledgeGraphStore>,
}

impl MemoryTool {
    pub fn new(graph_store: Arc<KnowledgeGraphStore>) -> Self {
        Self { graph_store }
    }
}

pub struct GraphTool {
    store: Arc<KnowledgeGraphStore>,
}

impl GraphTool {
    pub fn new(store: Arc<KnowledgeGraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "cognitive_memory"
    }

    fn description(&self) -> &str {
        "Manage cognitive memory with graph-authoritative reads and transitional compatibility writes. Operations: store, retrieve, query, delete, list_namespaces, stats."
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
                    "enum": ["store", "retrieve", "query", "delete", "list_namespaces", "stats", "graph_retrieve", "graph_query", "graph_list_namespaces", "graph_stats"],
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
            "graph_retrieve" => self.op_graph_retrieve(&input).await,
            "graph_query" => self.op_graph_query(&input).await,
            "graph_list_namespaces" => self.op_graph_list_namespaces().await,
            "graph_stats" => self.op_graph_stats().await,
            other => Err(anyhow::anyhow!("unknown operation: {}", other)),
        }
    }
}

#[async_trait]
impl Tool for GraphTool {
    fn name(&self) -> &str {
        "cognitive_graph"
    }

    fn description(&self) -> &str {
        "Inspect the authoritative Cozo knowledge graph projected from immutable blockchain footprints. Operations: list_events, list_links, list_namespaces, stats."
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "graph".to_string(),
            "cognitive".to_string(),
            "knowledge".to_string(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["list_events", "list_links", "list_namespaces", "stats"],
                    "description": "Graph operation to perform"
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
            "list_events" => {
                let events = self.store.list_projected_events()?;
                Ok(json!({
                    "count": events.len(),
                    "events": events
                }))
            }
            "list_links" => {
                let links = self.store.list_links()?;
                Ok(json!({
                    "count": links.len(),
                    "links": links
                }))
            }
            "list_namespaces" => {
                let namespaces = self.store.list_namespaces()?;
                Ok(json!({
                    "count": namespaces.len(),
                    "namespaces": namespaces
                }))
            }
            "stats" => {
                let events = self.store.list_projected_events()?;
                let links = self.store.list_links()?;
                let namespaces = self.store.list_namespaces()?;
                Ok(json!({
                    "event_count": events.len(),
                    "link_count": links.len(),
                    "namespace_count": namespaces.len()
                }))
            }
            other => Err(anyhow::anyhow!("unknown operation: {}", other)),
        }
    }
}

impl MemoryTool {
    async fn op_store(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;
        let block_hash = format!("memory-{}", Uuid::new_v4());
        self.project_memory_footprint("store", namespace, key, &input["value"], &block_hash)?;
        Ok(json!({
            "ok": true,
            "block_hash": block_hash,
            "namespace": namespace,
            "key": key,
            "source": "graph"
        }))
    }

    async fn op_retrieve(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        if let Some(event) = self.graph_store.find_latest_event(namespace, key, None)? {
            if event.operation == "delete" {
                return Ok(json!({
                    "found": false,
                    "namespace": namespace,
                    "key": key,
                    "source": "graph"
                }));
            }
            let payload = parse_payload_value(&event.payload_json)?;
            return Ok(json!({
                "found": true,
                "block_hash": event.block_hash,
                "namespace": namespace,
                "key": key,
                "value": payload["value"].clone(),
                "source": "graph",
                "timestamp": event.timestamp
            }));
        }
        Ok(json!({
            "found": false,
            "namespace": namespace,
            "key": key,
            "source": "graph"
        }))
    }

    async fn op_query(&self, input: &Value) -> Result<Value> {
        let namespace = input.get("namespace").and_then(|value| value.as_str());
        let key_pattern = input.get("key_pattern").and_then(|value| value.as_str());
        let limit = input
            .get("limit")
            .and_then(|value| value.as_i64())
            .map(|value| value as usize);

        let graph_events = self.graph_store.query_events(namespace, key_pattern, None)?;
        let latest = collapse_latest_memory_events(graph_events)?;
        let items: Vec<Value> = latest
            .into_iter()
            .filter_map(|event| {
                if event.operation == "delete" {
                    return None;
                }
                let payload = parse_payload_value(&event.payload_json).ok()?;
                Some(json!({
                    "block_hash": event.block_hash,
                    "namespace": event.namespace,
                    "key": payload["key"].clone(),
                    "value": payload["value"].clone(),
                    "operation": event.operation,
                    "timestamp": event.timestamp,
                    "source": "graph"
                }))
            })
            .collect();
        let items = if let Some(limit) = limit {
            items.into_iter().take(limit).collect::<Vec<_>>()
        } else {
            items
        };

        Ok(json!({ "count": items.len(), "entries": items, "source": "graph" }))
    }

    async fn op_delete(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        let block_hash = format!("memory-delete-{}", Uuid::new_v4());
        self.project_memory_footprint(
            "delete",
            namespace,
            key,
            &Value::Static(simd_json::StaticNode::Null),
            &block_hash,
        )?;
        Ok(json!({
            "ok": true,
            "namespace": namespace,
            "key": key,
            "block_hash": block_hash,
            "source": "graph"
        }))
    }

    async fn op_list_namespaces(&self, input: &Value) -> Result<Value> {
        let kind_filter = input["namespace_kind"].as_str();
        let graph_namespaces = self.graph_store.list_namespaces()?;
        let filtered_graph: Vec<_> = graph_namespaces
            .into_iter()
            .filter(|ns| kind_filter.map(|kind| ns.kind == kind).unwrap_or(true))
            .collect();
        if !filtered_graph.is_empty() {
            let count = filtered_graph.len();
            let items: Vec<Value> = filtered_graph
                .into_iter()
                .map(|ns| {
                    json!({
                        "name": ns.name,
                        "kind": ns.kind,
                        "source": "graph"
                    })
                })
                .collect();
            return Ok(json!({ "count": count, "namespaces": items, "source": "graph" }));
        }
        Ok(json!({ "count": 0, "namespaces": [], "source": "graph" }))
    }

    async fn op_stats(&self) -> Result<Value> {
        let events = self.graph_store.list_projected_events()?;
        let links = self.graph_store.list_links()?;
        let namespaces = self.graph_store.list_namespaces()?;
        Ok(json!({
            "event_count": events.len(),
            "link_count": links.len(),
            "namespace_count": namespaces.len(),
            "source": "graph"
        }))
    }

    async fn op_graph_retrieve(&self, input: &Value) -> Result<Value> {
        let namespace = input["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = input["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        match self
            .graph_store
            .find_latest_event(namespace, key, Some("store"))?
        {
            Some(event) => Ok(json!({
                "found": true,
                "block_hash": event.block_hash,
                "namespace": event.namespace,
                "key": key,
                "plugin_id": event.plugin_id,
                "timestamp": event.timestamp,
                "payload": parse_payload_value(&event.payload_json)?
            })),
            None => Ok(json!({ "found": false, "namespace": namespace, "key": key })),
        }
    }

    async fn op_graph_query(&self, input: &Value) -> Result<Value> {
        let namespace = input.get("namespace").and_then(|value| value.as_str());
        let key_pattern = input.get("key_pattern").and_then(|value| value.as_str());
        let limit = input
            .get("limit")
            .and_then(|value| value.as_i64())
            .map(|value| value as usize);

        let events = self
            .graph_store
            .query_events(namespace, key_pattern, limit)?;
        let count = events.len();
        let items: Vec<Value> = events
            .into_iter()
            .map(|event| {
                let payload = parse_payload_value(&event.payload_json).unwrap_or(Value::Static(simd_json::StaticNode::Null));
                json!({
                    "block_hash": event.block_hash,
                    "namespace": event.namespace,
                    "operation": event.operation,
                    "timestamp": event.timestamp,
                    "payload": payload
                })
            })
            .collect();

        Ok(json!({ "count": count, "events": items }))
    }

    async fn op_graph_list_namespaces(&self) -> Result<Value> {
        let namespaces = self.graph_store.list_namespaces()?;
        Ok(json!({
            "count": namespaces.len(),
            "namespaces": namespaces
        }))
    }

    async fn op_graph_stats(&self) -> Result<Value> {
        let events = self.graph_store.list_projected_events()?;
        let links = self.graph_store.list_links()?;
        let namespaces = self.graph_store.list_namespaces()?;
        Ok(json!({
            "event_count": events.len(),
            "link_count": links.len(),
            "namespace_count": namespaces.len()
        }))
    }

    fn project_memory_footprint(
        &self,
        operation: &str,
        namespace: &str,
        key: &str,
        value: &Value,
        block_hash: &str,
    ) -> Result<()> {
        let mut footprint = PluginFootprint::new(
            "cognitive_memory",
            operation,
            &json!({
                "namespace": namespace,
                "key": key,
                "value": value.clone()
            }),
        );
        footprint.metadata = simd_json::serde::from_owned_value(json!({
            "namespace": namespace,
            "memory_store": "control:memory",
            "key": key,
            "value": value.clone()
        }))?;
        self.graph_store.project_footprint(block_hash, &footprint)
    }
}

fn parse_payload_value(payload_json: &str) -> Result<Value> {
    let mut buf = payload_json.as_bytes().to_vec();
    simd_json::from_slice(&mut buf).map_err(Into::into)
}

fn collapse_latest_memory_events(mut events: Vec<ProjectedEvent>) -> Result<Vec<ProjectedEvent>> {
    events.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));
    let mut seen = std::collections::HashSet::new();
    let mut latest = Vec::new();
    for event in events {
        let payload = parse_payload_value(&event.payload_json)?;
        let key = payload["key"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("graph event missing key"))?;
        let dedupe_key = format!("{}:{}", event.namespace, key);
        if seen.insert(dedupe_key) {
            latest.push(event);
        }
    }
    Ok(latest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_prefer_graph_for_default_retrieve() {
        let graph_store = Arc::new(KnowledgeGraphStore::new_in_memory().unwrap());
        let tool = MemoryTool::new(graph_store.clone());

        let mut footprint = PluginFootprint::new(
            "cognitive_memory",
            "store",
            &json!({
                "namespace": "project:alpha",
                "key": "database_url",
                "value": "graph-value"
            }),
        );
        footprint.timestamp = 200;
        footprint.metadata = simd_json::serde::from_owned_value(json!({
            "namespace": "project:alpha",
            "memory_store": "control:memory",
            "key": "database_url",
            "value": "graph-value"
        }))
        .unwrap();
        graph_store.project_footprint("block-graph", &footprint).unwrap();

        let result = tool
            .execute(json!({
                "operation": "retrieve",
                "namespace": "project:alpha",
                "key": "database_url"
            }))
            .await
            .unwrap();

        assert_eq!(result["found"].as_bool(), Some(true));
        assert_eq!(result["source"].as_str(), Some("graph"));
        assert_eq!(result["value"].as_str(), Some("graph-value"));
    }

    #[tokio::test]
    async fn should_hide_deleted_entries_from_default_retrieve_and_query() {
        let graph_store = Arc::new(KnowledgeGraphStore::new_in_memory().unwrap());
        let tool = MemoryTool::new(graph_store.clone());

        let mut store_footprint = PluginFootprint::new(
            "cognitive_memory",
            "store",
            &json!({
                "namespace": "project:alpha",
                "key": "api_key",
                "value": "secret"
            }),
        );
        store_footprint.timestamp = 100;
        store_footprint.metadata = simd_json::serde::from_owned_value(json!({
            "namespace": "project:alpha",
            "memory_store": "control:memory",
            "key": "api_key",
            "value": "secret"
        }))
        .unwrap();
        graph_store.project_footprint("block-store", &store_footprint).unwrap();

        let mut delete_footprint = PluginFootprint::new(
            "cognitive_memory",
            "delete",
            &json!({
                "namespace": "project:alpha",
                "key": "api_key",
                "value": null
            }),
        );
        delete_footprint.timestamp = 200;
        delete_footprint.metadata = simd_json::serde::from_owned_value(json!({
            "namespace": "project:alpha",
            "memory_store": "control:memory",
            "key": "api_key",
            "value": null
        }))
        .unwrap();
        graph_store
            .project_footprint("block-delete", &delete_footprint)
            .unwrap();

        let retrieve = tool
            .execute(json!({
                "operation": "retrieve",
                "namespace": "project:alpha",
                "key": "api_key"
            }))
            .await
            .unwrap();
        assert_eq!(retrieve["found"].as_bool(), Some(false));

        let query = tool
            .execute(json!({
                "operation": "query",
                "namespace": "project:alpha"
            }))
            .await
            .unwrap();
        assert_eq!(query["count"].as_u64(), Some(0));
    }
}
