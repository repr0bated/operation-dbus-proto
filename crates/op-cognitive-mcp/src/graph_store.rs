//! Graph-native projection store for persistent memory.
//!
//! This is the first graph layer replacing ad-hoc key/value memory ownership.
//! It projects immutable blockchain footprints into Cozo relations so the
//! knowledge graph can be queried and rebuilt from the ledger.

use anyhow::{Context, Result};
use cozo::{DataValue, DbInstance, ScriptMutability};
use op_blockchain::PluginFootprint;
use serde::Serialize;
use simd_json::prelude::ValueAsScalar;
use simd_json::OwnedValue;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectedEvent {
    pub block_hash: String,
    pub plugin_id: String,
    pub operation: String,
    pub timestamp: i64,
    pub content_hash: String,
    pub data_hash: String,
    pub namespace: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EventLink {
    pub source: String,
    pub relation: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NamespaceNode {
    pub name: String,
    pub kind: String,
}

#[derive(Clone)]
pub struct KnowledgeGraphStore {
    db: Arc<Mutex<DbInstance>>,
}

impl KnowledgeGraphStore {
    pub fn new_in_memory() -> Result<Self> {
        let db = DbInstance::new("mem", "", Default::default())
            .map_err(|err| anyhow::anyhow!("failed to create in-memory Cozo database: {err}"))?;
        let store = Self {
            db: Arc::new(Mutex::new(db)),
        };
        store.initialize_schema()?;
        Ok(store)
    }

    pub fn new_on_disk(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).with_context(|| {
            format!(
                "failed to create graph store directory at {}",
                path.display()
            )
        })?;
        let db = DbInstance::new("sled", path, Default::default()).map_err(|err| {
            anyhow::anyhow!(
                "failed to create persistent Cozo graph database at {}: {err}",
                path.display()
            )
        })?;
        let store = Self {
            db: Arc::new(Mutex::new(db)),
        };
        store.initialize_schema()?;
        Ok(store)
    }

    fn initialize_schema(&self) -> Result<()> {
        let schemas = [
            r#":create events {
  block_hash: String =>
  plugin_id: String,
  operation: String,
  timestamp: Int,
  content_hash: String,
  data_hash: String,
  namespace: String,
  payload_json: String
}"#,
            r#":create event_links {
  source: String,
  relation: String =>
  target: String
}"#,
            r#":create namespaces {
  name: String =>
  kind: String
}"#,
        ];

        for schema in schemas {
            match self.run_script(schema, BTreeMap::new(), ScriptMutability::Mutable) {
                Ok(_) => {}
                Err(err) => {
                    let msg = err.to_string();
                    if !(msg.contains("stored relation") && msg.contains("exists")) {
                        return Err(err).context("failed to initialize Cozo graph schema");
                    }
                }
            }
        }
        Ok(())
    }

    pub fn project_footprint(&self, block_hash: &str, footprint: &PluginFootprint) -> Result<()> {
        let namespace = namespace_from_metadata(&footprint.metadata);
        let namespace_kind = namespace_kind(&namespace);
        let payload_json =
            serde_json::to_string(&footprint.metadata).context("serialize footprint metadata")?;

        let mut params = BTreeMap::new();
        params.insert("block_hash".to_string(), DataValue::from(block_hash.to_string()));
        params.insert(
            "plugin_id".to_string(),
            DataValue::from(footprint.plugin_id.clone()),
        );
        params.insert(
            "operation".to_string(),
            DataValue::from(footprint.operation.clone()),
        );
        params.insert(
            "timestamp".to_string(),
            DataValue::from(footprint.timestamp as i64),
        );
        params.insert(
            "content_hash".to_string(),
            DataValue::from(footprint.content_hash.clone()),
        );
        params.insert(
            "data_hash".to_string(),
            DataValue::from(footprint.data_hash.clone()),
        );
        params.insert("namespace".to_string(), DataValue::from(namespace.clone()));
        params.insert(
            "namespace_kind".to_string(),
            DataValue::from(namespace_kind.to_string()),
        );
        params.insert("payload_json".to_string(), DataValue::from(payload_json));

        self.run_script(
            r#"?[name, kind] <- [[ $namespace, $namespace_kind ]]
:put namespaces { name => kind }"#,
            params.clone(),
            ScriptMutability::Mutable,
        )
        .context("failed to store namespace in Cozo graph")?;

        self.run_script(
            r#"?[block_hash, plugin_id, operation, timestamp, content_hash, data_hash, namespace, payload_json] <-
  [[ $block_hash, $plugin_id, $operation, $timestamp, $content_hash, $data_hash, $namespace, $payload_json ]]
:put events { block_hash => plugin_id, operation, timestamp, content_hash, data_hash, namespace, payload_json }
"#,
            params.clone(),
            ScriptMutability::Mutable,
        )
        .context("failed to store event in Cozo graph")?;

        self.run_script(
            r#"?[source, relation, target] <- [[ $block_hash, "in_namespace", $namespace ]]
:put event_links { source, relation => target }
"#,
            params,
            ScriptMutability::Mutable,
        )
        .context("failed to store event link in Cozo graph")?;

        Ok(())
    }

    pub fn list_projected_events(&self) -> Result<Vec<ProjectedEvent>> {
        let rows = self
            .run_script(
                "?[block_hash, plugin_id, operation, timestamp, content_hash, data_hash, namespace, payload_json] := *events{block_hash, plugin_id, operation, timestamp, content_hash, data_hash, namespace, payload_json}",
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .context("query projected events")?;

        rows.rows
            .into_iter()
            .map(|row| {
                Ok(ProjectedEvent {
                    block_hash: data_value_to_string(row.first())?,
                    plugin_id: data_value_to_string(row.get(1))?,
                    operation: data_value_to_string(row.get(2))?,
                    timestamp: data_value_to_i64(row.get(3))?,
                    content_hash: data_value_to_string(row.get(4))?,
                    data_hash: data_value_to_string(row.get(5))?,
                    namespace: data_value_to_string(row.get(6))?,
                    payload_json: data_value_to_string(row.get(7))?,
                })
            })
            .collect()
    }

    pub fn list_links(&self) -> Result<Vec<EventLink>> {
        let rows = self
            .run_script(
                "?[source, relation, target] := *event_links{source, relation, target}",
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .context("query event links")?;

        rows.rows
            .into_iter()
            .map(|row| {
                Ok(EventLink {
                    source: data_value_to_string(row.first())?,
                    relation: data_value_to_string(row.get(1))?,
                    target: data_value_to_string(row.get(2))?,
                })
            })
            .collect()
    }

    pub fn list_namespaces(&self) -> Result<Vec<NamespaceNode>> {
        let rows = self
            .run_script(
                "?[name, kind] := *namespaces{name, kind}",
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .context("query namespaces")?;

        rows.rows
            .into_iter()
            .map(|row| {
                Ok(NamespaceNode {
                    name: data_value_to_string(row.first())?,
                    kind: data_value_to_string(row.get(1))?,
                })
            })
            .collect()
    }

    pub fn find_latest_event(
        &self,
        namespace: &str,
        key: &str,
        operation: Option<&str>,
    ) -> Result<Option<ProjectedEvent>> {
        let mut events = self.list_projected_events()?;
        events.retain(|event| {
            if event.namespace != namespace {
                return false;
            }
            if let Some(expected_operation) = operation {
                if event.operation != expected_operation {
                    return false;
                }
            }
            event_payload_key(event).as_deref() == Some(key)
        });
        events.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));
        Ok(events.into_iter().next())
    }

    pub fn query_events(
        &self,
        namespace: Option<&str>,
        key_pattern: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<ProjectedEvent>> {
        let mut events = self.list_projected_events()?;
        events.retain(|event| {
            if let Some(expected_namespace) = namespace {
                if event.namespace != expected_namespace {
                    return false;
                }
            }
            if let Some(pattern) = key_pattern {
                let Some(key) = event_payload_key(event) else {
                    return false;
                };
                if !key.contains(pattern) {
                    return false;
                }
            }
            true
        });
        events.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));
        if let Some(limit) = limit {
            events.truncate(limit);
        }
        Ok(events)
    }

    fn run_script(
        &self,
        script: &str,
        params: BTreeMap<String, DataValue>,
        mutability: ScriptMutability,
    ) -> Result<cozo::NamedRows> {
        let db = self.db.lock().expect("graph db mutex poisoned");
        db.run_script(script, params, mutability)
            .map_err(|err| anyhow::anyhow!(err.to_string()))
    }
}

fn namespace_from_metadata(metadata: &HashMap<String, simd_json::OwnedValue>) -> String {
    metadata
        .get("namespace")
        .and_then(|value| value.as_str())
        .or_else(|| metadata.get("memory_store").and_then(|value| value.as_str()))
        .unwrap_or("control:default")
        .to_string()
}

fn namespace_kind(namespace: &str) -> &str {
    namespace.split(':').next().unwrap_or("custom")
}

fn data_value_to_string(value: Option<&DataValue>) -> Result<String> {
    match value.context("missing data value")? {
        DataValue::Str(s) => Ok(s.to_string()),
        other => Ok(other.to_string()),
    }
}

fn data_value_to_i64(value: Option<&DataValue>) -> Result<i64> {
    match value.context("missing numeric value")? {
        DataValue::Num(num) => match num {
            cozo::Num::Int(i) => Ok(*i),
            cozo::Num::Float(f) => Ok(*f as i64),
        },
        other => other
            .to_string()
            .parse::<i64>()
            .context("parse numeric Cozo value"),
    }
}

fn event_payload_key(event: &ProjectedEvent) -> Option<String> {
    parse_payload_json(&event.payload_json)
        .ok()
        .and_then(|payload| {
            payload
                .get("key")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
        })
}

fn parse_payload_json(payload_json: &str) -> Result<HashMap<String, OwnedValue>> {
    serde_json::from_str(payload_json).context("parse projected payload json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;

    #[test]
    fn should_project_footprint_into_graph_relations() {
        let store = KnowledgeGraphStore::new_in_memory().unwrap();
        let footprint = PluginFootprint {
            plugin_id: "ctl-plane-chatbot".to_string(),
            operation: "decision".to_string(),
            timestamp: 1_710_000_000,
            data_hash: "data_hash_123".to_string(),
            content_hash: "content_hash_456".to_string(),
            metadata: simd_json::serde::from_owned_value(json!({
                "namespace": "work:project-x",
                "conversation_id": "conv-1"
            }))
            .unwrap(),
            vector_features: vec![0.1, 0.2, 0.3],
        };

        store.project_footprint("block-1", &footprint).unwrap();

        let events = store.list_projected_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].block_hash, "block-1");
        assert_eq!(events[0].namespace, "work:project-x");

        let links = store.list_links().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].relation, "in_namespace");
        assert_eq!(links[0].target, "work:project-x");
    }

    #[test]
    fn should_query_latest_event_by_namespace_and_key() {
        let store = KnowledgeGraphStore::new_in_memory().unwrap();

        let mut first = PluginFootprint::new(
            "cognitive_memory",
            "store",
            &json!({"key": "database_url", "value": "postgres://old"}),
        );
        first.timestamp = 100;
        first.metadata = simd_json::serde::from_owned_value(json!({
            "namespace": "project:alpha",
            "key": "database_url",
            "value": "postgres://old"
        }))
        .unwrap();
        store.project_footprint("block-old", &first).unwrap();

        let mut second = PluginFootprint::new(
            "cognitive_memory",
            "store",
            &json!({"key": "database_url", "value": "postgres://new"}),
        );
        second.timestamp = 200;
        second.metadata = simd_json::serde::from_owned_value(json!({
            "namespace": "project:alpha",
            "key": "database_url",
            "value": "postgres://new"
        }))
        .unwrap();
        store.project_footprint("block-new", &second).unwrap();

        let latest = store
            .find_latest_event("project:alpha", "database_url", Some("store"))
            .unwrap()
            .unwrap();
        assert_eq!(latest.block_hash, "block-new");

        let events = store
            .query_events(Some("project:alpha"), Some("database_"), Some(10))
            .unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].block_hash, "block-new");
    }
}
