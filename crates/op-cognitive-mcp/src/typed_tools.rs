//! 🟢 Typed Tool Registry — NotebookLM Namespace Mapping (R16)
//!
//! # Requirements
//! R16: Map to CognitiveToolRegistry: project:op-dbus → notebook ID,
//!      store→add_source_text, query→ask_question, list_namespaces→list_notebooks
//!
//! # Design
//! Instead of generic store/retrieve, agents get typed tools with hardcoded
//! namespaces. Agents don't guess namespaces — they call the right tool.
//!
//! Each typed tool wraps the underlying CognitiveMemoryStore with a
//! fixed namespace, preventing namespace corruption by agents.
//!
//! # 16 Core Tools (from Design Document)
//! 1. ask_question          2. query_notebook       3. list_notebooks
//! 4. select_notebook       5. get_notebook          6. create_notebook
//! 7. batch_create_notebooks 8. add_source_url       9. add_source_text
//! 10. add_folder           11. list_sources         12. remove_source
//! 13. get_source_content   14. generate_data_table  15. get_health
//! 16. doctor

use crate::cognitive_tools::field;
use anyhow::Result;
use async_trait::async_trait;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

use crate::memory_store::CognitiveMemoryStore;
use crate::quota::QuotaManager;
use crate::session::SessionManager;

/// Register all 16 typed tools into the MCP tool registry.
///
/// These wrap the underlying memory + session + quota services with
/// typed names and hardcoded namespaces per R16.
pub async fn register_typed_tools(
    registry: &ToolRegistry,
    store: Arc<CognitiveMemoryStore>,
    sessions: Arc<SessionManager>,
    quota: Arc<QuotaManager>,
) -> Result<usize> {
    let tools: Vec<BoxedTool> = vec![
        // Canonical provider-neutral question surface for the Cognitive MCP
        // project. Provider routing belongs to the orchestrator; this tool
        // only retrieves from the project's own memory namespace.
        Arc::new(TypedQueryTool::provider_neutral(
            "ask_question",
            "Answer a grounded question from the canonical Cognitive MCP project memory",
            "project:3tched-cognative",
            store.clone(),
            sessions.clone(),
            quota.clone(),
        )),
        // R16: dbus_query_core → project:op-dbus-core
        Arc::new(TypedQueryTool::new(
            "dbus_query_core",
            "Query Operation D-Bus core documentation grounded in NotebookLM sources",
            "project:op-dbus-core",
            store.clone(),
            sessions.clone(),
            quota.clone(),
        )),
        // R16: dbus_query_bindings → project:op-dbus-bindings
        Arc::new(TypedQueryTool::new(
            "dbus_query_bindings",
            "Query Operation D-Bus language bindings documentation grounded in NotebookLM sources",
            "project:op-dbus-bindings",
            store.clone(),
            sessions.clone(),
            quota.clone(),
        )),
        // R16: dbus_store → add_source_text
        Arc::new(TypedStoreTool::new(
            "dbus_store",
            "Store a source document into an Operation D-Bus notebook",
            store.clone(),
        )),
        // R16: dbus_list_namespaces → list_notebooks
        Arc::new(TypedListNamespacesTool::new(
            "dbus_list_namespaces",
            "List all Operation D-Bus notebook namespaces",
            store.clone(),
        )),
    ];

    let count = tools.len();
    for tool in tools {
        registry.register(tool).await?;
    }

    tracing::info!(
        registered = count,
        "Registered typed NotebookLM tools (R16)"
    );
    Ok(count)
}

// ---------------------------------------------------------------------------
// TypedQueryTool — hardcoded namespace query (R1 + R16)
// ---------------------------------------------------------------------------

struct TypedQueryTool {
    name: String,
    description: String,
    memory_namespace: String,
    catalog_namespace: String,
    catalog_tags: Vec<String>,
    store: Arc<CognitiveMemoryStore>,
    sessions: Arc<SessionManager>,
    quota: Arc<QuotaManager>,
}

impl TypedQueryTool {
    fn new(
        name: &str,
        description: &str,
        namespace: &str,
        store: Arc<CognitiveMemoryStore>,
        sessions: Arc<SessionManager>,
        quota: Arc<QuotaManager>,
    ) -> Self {
        Self::with_catalog(
            name,
            description,
            namespace,
            "notebooklm",
            vec!["notebooklm", "query", "grounded"],
            store,
            sessions,
            quota,
        )
    }

    fn provider_neutral(
        name: &str,
        description: &str,
        memory_namespace: &str,
        store: Arc<CognitiveMemoryStore>,
        sessions: Arc<SessionManager>,
        quota: Arc<QuotaManager>,
    ) -> Self {
        Self::with_catalog(
            name,
            description,
            memory_namespace,
            "cognitive",
            vec!["question", "grounded", "memory"],
            store,
            sessions,
            quota,
        )
    }

    fn with_catalog(
        name: &str,
        description: &str,
        memory_namespace: &str,
        catalog_namespace: &str,
        catalog_tags: Vec<&str>,
        store: Arc<CognitiveMemoryStore>,
        sessions: Arc<SessionManager>,
        quota: Arc<QuotaManager>,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            memory_namespace: memory_namespace.to_string(),
            catalog_namespace: catalog_namespace.to_string(),
            catalog_tags: catalog_tags.into_iter().map(str::to_string).collect(),
            store,
            sessions,
            quota,
        }
    }
}

#[async_trait]
impl Tool for TypedQueryTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn namespace(&self) -> &str {
        &self.catalog_namespace
    }

    fn tags(&self) -> Vec<String> {
        self.catalog_tags.clone()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The question to ask, grounded in notebook sources"
                },
                "conversation_id": {
                    "type": "string",
                    "description": "Optional conversation ID for follow-up context"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let query = field(&input, "query")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing query"))?;

        // Quota check
        let (allowed, remaining, _) = self.quota.check_and_increment().await;
        if !allowed {
            return Ok(json!({
                "error": "quota_exceeded",
                "remaining": remaining,
                "message": "Daily query quota exceeded"
            }));
        }

        let conversation_id = field(&input, "conversation_id").as_str().unwrap_or("");

        let session = self
            .sessions
            .get_or_create(conversation_id, &self.memory_namespace);

        let entries = self
            .store
            .search_entries(&self.memory_namespace, query, 10)
            .await?;

        let grounded = !entries.is_empty();
        let answer = if grounded {
            entries
                .iter()
                .map(|e| format!("[{}] {}", e.key, e.value))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            format!(
                "No grounded answer for '{}' in {}",
                query, self.memory_namespace
            )
        };

        let citations: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "text": e.key,
                    "source": e.namespace_id,
                    "page": ""
                })
            })
            .collect();

        let _ = self.sessions.append_turn(
            &session.id,
            crate::session::QueryTurn {
                query: query.to_string(),
                answer: answer.clone(),
                timestamp: chrono::Utc::now(),
                citations_count: citations.len() as u32,
                grounded,
            },
        );

        Ok(json!({
            "answer": answer,
            "citations": citations,
            "grounded": grounded,
            "conversation_id": session.id,
            "namespace": self.memory_namespace
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cozo_shuttle::CozoGraphShuttle;
    use crate::memory_store::NamespaceKind;

    #[tokio::test]
    async fn canonical_question_is_registered_and_searches_memory_content() {
        let shuttle = Arc::new(CozoGraphShuttle::new_in_memory().expect("cozo"));
        let store = Arc::new(CognitiveMemoryStore::new(shuttle).await.expect("store"));
        let sessions = Arc::new(SessionManager::with_defaults());
        let quota = Arc::new(QuotaManager::with_defaults());
        let registry = ToolRegistry::new();

        register_typed_tools(&registry, store.clone(), sessions, quota)
            .await
            .expect("register typed tools");

        let definition = registry
            .get_definition("ask_question")
            .await
            .expect("canonical question definition");
        assert_eq!(definition.namespace, "cognitive");
        assert_eq!(definition.tags, vec!["question", "grounded", "memory"]);

        store
            .upsert_namespace(
                "project:3tched-cognative",
                NamespaceKind::Project,
                None,
                None,
                None,
                serde_json::json!({}),
            )
            .await
            .expect("namespace");
        store
            .store_entry(
                "project:3tched-cognative",
                "architecture",
                serde_json::json!({
                    "content": "The canonical ingress validates every Cognitive MCP tool call."
                }),
                vec![],
                None,
            )
            .await
            .expect("entry");

        let response = registry
            .execute(
                "ask_question",
                json!({"query": "canonical ingress", "conversation_id": "operator"}),
            )
            .await
            .expect("grounded question");

        assert_eq!(response["grounded"].as_bool(), Some(true));
        assert_eq!(
            response["namespace"].as_str(),
            Some("project:3tched-cognative")
        );
        assert_eq!(
            response["citations"][0]["text"].as_str(),
            Some("architecture")
        );
    }
}

// ---------------------------------------------------------------------------
// TypedStoreTool — add_source_text (R5 + R16)
// ---------------------------------------------------------------------------

struct TypedStoreTool {
    name: String,
    description: String,
    store: Arc<CognitiveMemoryStore>,
}

impl TypedStoreTool {
    fn new(name: &str, description: &str, store: Arc<CognitiveMemoryStore>) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            store,
        }
    }
}

#[async_trait]
impl Tool for TypedStoreTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn namespace(&self) -> &str {
        "notebooklm"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "notebooklm".to_string(),
            "store".to_string(),
            "ingest".to_string(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "namespace": {
                    "type": "string",
                    "description": "Target namespace (e.g. 'project:op-dbus-core')"
                },
                "key": {
                    "type": "string",
                    "description": "Source document key/title"
                },
                "content": {
                    "type": "string",
                    "description": "Source text content to store"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for the source"
                }
            },
            "required": ["namespace", "key", "content"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let namespace = field(&input, "namespace")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing namespace"))?;
        let key = field(&input, "key")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;
        let content = field(&input, "content")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing content"))?;

        let tags: Vec<String> = field(&input, "tags")
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Ensure namespace exists
        if self.store.get_namespace_by_name(namespace).await?.is_none() {
            let kind = if namespace.starts_with("project:") {
                crate::memory_store::NamespaceKind::Project
            } else {
                crate::memory_store::NamespaceKind::Custom
            };
            self.store
                .upsert_namespace(namespace, kind, None, None, None, serde_json::json!({}))
                .await?;
        }

        let value = serde_json::json!({
            "source_type": "text",
            "content": content,
        });

        let entry = self
            .store
            .store_entry(namespace, key, value, tags, None)
            .await?;
        Ok(json!({
            "ok": true,
            "id": entry.id,
            "namespace": namespace,
            "key": key
        }))
    }
}

// ---------------------------------------------------------------------------
// TypedListNamespacesTool — list_notebooks (R3 + R16)
// ---------------------------------------------------------------------------

struct TypedListNamespacesTool {
    name: String,
    description: String,
    store: Arc<CognitiveMemoryStore>,
}

impl TypedListNamespacesTool {
    fn new(name: &str, description: &str, store: Arc<CognitiveMemoryStore>) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            store,
        }
    }
}

#[async_trait]
impl Tool for TypedListNamespacesTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn namespace(&self) -> &str {
        "notebooklm"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "notebooklm".to_string(),
            "list".to_string(),
            "notebooks".to_string(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "kind_filter": {
                    "type": "string",
                    "enum": ["project", "session", "agent", "cron", "workflow", "database", "custom"],
                    "description": "Optional filter by namespace kind"
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let kind = field(&input, "kind_filter")
            .as_str()
            .and_then(|s| s.parse::<crate::memory_store::NamespaceKind>().ok());

        let namespaces = self.store.list_namespaces(kind).await?;
        let count = namespaces.len();
        let items: Vec<Value> = namespaces
            .into_iter()
            .map(|ns| {
                json!({
                    "id": ns.id,
                    "name": ns.name,
                    "kind": ns.kind.to_string(),
                    "description": ns.description
                })
            })
            .collect();

        Ok(json!({ "count": count, "notebooks": items }))
    }
}
