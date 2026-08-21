//! Cognitive Tools for MCP
//!
//! MCP tools backed by the SQLite namespace/entry memory store.
//! Operations: store, retrieve, query, delete, list_namespaces, stats.

use crate::agent_tools::register_agent_tools;
use crate::blob_catalog_tool::register_blob_catalog_tool;
use crate::blob_vectors_tool::register_blob_vectors_tools;
use crate::development_ledger::DevelopmentLedger;
use crate::memory_store::{CognitiveMemoryStore, EntryQuery, NamespaceKind};
use crate::notebooklm::register_notebooklm_tools;
use crate::qdrant_shuttle::QdrantSemanticShuttle;
use anyhow::Result;
use async_trait::async_trait;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolReadiness, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::Mutex;

const DYNAMIC_TOOL_NAMESPACE: &str = "project:3tched-cognative";
const DYNAMIC_TOOL_KEY_PREFIX: &str = "_tool_alias:";

/// Safe field access for simd_json values.
///
/// simd_json's `Index` impl **panics** when the key is absent, unlike `serde_json`
/// which yields `Null`. Every optional-field read must therefore go through `get`.
/// This wrapper keeps the ergonomic `field(input, "k").as_str()` shape while
/// returning a Null sentinel rather than unwinding the worker task.
///
/// Reading an absent optional argument used to panic the tokio worker and reset the
/// client connection, which surfaced as an empty HTTP response rather than an error.
pub(crate) fn field<'a>(input: &'a Value, key: &str) -> &'a Value {
    static NULL: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    input
        .get(key)
        .unwrap_or_else(|| NULL.get_or_init(|| json!(null)))
}

pub struct CognitiveToolRegistry;

impl CognitiveToolRegistry {
    pub async fn register_all(
        registry: Arc<ToolRegistry>,
        store: Arc<CognitiveMemoryStore>,
        qdrant: Option<Arc<QdrantSemanticShuttle>>,
    ) -> Result<()> {
        let dynamic_tools = Arc::new(DynamicToolCatalog::new(registry.clone(), store.clone()));
        registry
            .register(Arc::new(MemoryTool::new(store.clone(), qdrant.clone())) as BoxedTool)
            .await?;
        registry
            .register(Arc::new(RegisterToolTool::new(dynamic_tools.clone())) as BoxedTool)
            .await?;
        registry
            .register(Arc::new(DevelopmentLedgerTool::new(store.clone())) as BoxedTool)
            .await?;
        register_agent_tools(registry.as_ref()).await?;
        register_notebooklm_tools(registry.as_ref()).await?;
        register_blob_catalog_tool(registry.as_ref()).await?;
        register_blob_vectors_tools(registry.as_ref(), qdrant).await?;
        Ok(())
    }

    /// Restore declarative aliases only after every optional runtime tool has
    /// been registered.  That lets an alias target a live code-RAG tool while
    /// still refusing it if its dependency is disabled in this boot.
    pub async fn restore_dynamic_tools(
        registry: Arc<ToolRegistry>,
        store: Arc<CognitiveMemoryStore>,
    ) -> Result<usize> {
        DynamicToolCatalog::new(registry, store).restore().await
    }
}

pub struct DevelopmentLedgerTool {
    ledger: DevelopmentLedger,
}

impl DevelopmentLedgerTool {
    fn new(store: Arc<CognitiveMemoryStore>) -> Self {
        // The memory store and ledger intentionally share the same Cozo DB.
        Self {
            ledger: DevelopmentLedger::new(store.shuttle()),
        }
    }
}

#[async_trait]
impl Tool for DevelopmentLedgerTool {
    fn name(&self) -> &str {
        "cognitive_development"
    }
    fn description(&self) -> &str {
        "Track Cognitive capability implementation, verification, deployment, dependencies, and blockers."
    }
    fn category(&self) -> &str {
        "cognitive"
    }
    fn tags(&self) -> Vec<String> {
        vec!["development".into(), "ledger".into(), "verification".into()]
    }
    fn input_schema(&self) -> Value {
        json!({
            "type":"object",
            "properties": {
                "operation":{"type":"string","enum":["upsert","list","summary","history","record_verification","categories"]},
                "capability_id":{"type":"string"},
                "category":{"type":"string"},
                "title":{"type":"string"},
                "description":{"type":"string"},
                "owner":{"type":"string"},
                "status":{"type":"string"},
                "schema_surface":{"type":"string"},
                "required_capability":{"type":"string"},
                "subid":{"type":"string"},
                "dependencies":{"type":"array","items":{"type":"string"}},
                "tests":{"type":"array","items":{"type":"string"}},
                "live_verified":{"type":"boolean"},
                "deployed_commit":{"type":"string"},
                "commit":{"type":"string"},
                "blocker":{"type":"string"},
                "details":{"type":"string"},
                "checks":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"passed":{"type":"boolean"},"details":{"type":"string"}},"required":["name","passed"]}}
            }
        })
    }
    async fn execute(&self, input: Value) -> Result<Value> {
        let json_input: serde_json::Value = serde_json::from_str(&input.to_string())?;
        let output = self.ledger.execute(&json_input)?;
        let mut bytes = serde_json::to_vec(&output)?;
        Ok(simd_json::to_owned_value(&mut bytes)?)
    }
}

pub struct MemoryTool {
    store: Arc<CognitiveMemoryStore>,
    qdrant: Option<Arc<QdrantSemanticShuttle>>,
}

impl MemoryTool {
    pub fn new(
        store: Arc<CognitiveMemoryStore>,
        qdrant: Option<Arc<QdrantSemanticShuttle>>,
    ) -> Self {
        Self { store, qdrant }
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "cognitive_memory"
    }

    fn description(&self) -> &str {
        "OPERATOR-ONLY: cognitive memory lifecycle is controlled through the canonical orchestrator and gRPC ingress, not direct model calls."
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

    fn readiness(&self) -> ToolReadiness {
        ToolReadiness::Disabled {
            reason: "Memory namespace lifecycle and mutation are owned by the canonical orchestrator/gRPC ingress."
                .to_string(),
        }
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["ensure_soul", "store", "retrieve", "query", "semantic_query", "delete", "list_namespaces", "stats"],
                    "description": "Operation to perform"
                },
                "owner": {
                    "type": "string",
                    "enum": ["chatbot", "user_container"],
                    "description": "Soul owner for ensure_soul"
                },
                "container_id": {
                    "type": "string",
                    "description": "User container identity. When namespace is omitted, memory is scoped to container:<container_id>."
                },
                "identity_id": {
                    "type": "string",
                    "description": "Canonical identity bound to a user container soul and namespace"
                },
                "wireguard_pubkey": {
                    "type": "string",
                    "description": "WireGuard public key bound to the identity, when known"
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
                "query": {
                    "type": "string",
                    "description": "Semantic query text for semantic_query"
                },
                "semantic": {
                    "type": "boolean",
                    "description": "Mirror text content into Qdrant user_memory when available"
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
        let op = field(&input, "operation")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing operation"))?;

        match op {
            "ensure_soul" => self.op_ensure_soul(&input).await,
            "store" => self.op_store(&input).await,
            "retrieve" => self.op_retrieve(&input).await,
            "query" => self.op_query(&input).await,
            "semantic_query" => self.op_semantic_query(&input).await,
            "delete" => self.op_delete(&input).await,
            "list_namespaces" => self.op_list_namespaces(&input).await,
            "stats" => self.op_stats().await,
            other => Err(anyhow::anyhow!("unknown operation: {}", other)),
        }
    }
}

/// A persisted declaration for a safe runtime tool alias.  This does not carry
/// code, shell, URL, or provider fields: dynamic registration can only expose
/// a second catalog name for an already-live, allow-listed registry tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ToolAliasRegistration {
    version: u8,
    name: String,
    target: String,
    description: Option<String>,
}

/// Bridge-owned registry for declarative aliases.  The admission gate prevents
/// a model from creating executable code or escalating into a disabled/mock
/// tool.  Successful aliases are persisted so the catalog survives a restart.
struct DynamicToolCatalog {
    registry: Arc<ToolRegistry>,
    store: Arc<CognitiveMemoryStore>,
    gate: Mutex<()>,
}

impl DynamicToolCatalog {
    fn new(registry: Arc<ToolRegistry>, store: Arc<CognitiveMemoryStore>) -> Self {
        Self {
            registry,
            store,
            gate: Mutex::new(()),
        }
    }

    async fn restore(&self) -> Result<usize> {
        let entries = self
            .store
            .query_entries(EntryQuery {
                namespace_id: Some(DYNAMIC_TOOL_NAMESPACE.to_string()),
                key_pattern: Some(DYNAMIC_TOOL_KEY_PREFIX.to_string()),
                limit: Some(500),
                ..Default::default()
            })
            .await?;
        let mut restored = 0;
        for entry in entries {
            let registration = match serde_json::from_value::<ToolAliasRegistration>(entry.value) {
                Ok(registration) if registration.version == 1 => registration,
                Ok(registration) => {
                    tracing::warn!(
                        name = %registration.name,
                        version = registration.version,
                        "skipping unsupported persisted Cognitive tool alias version"
                    );
                    continue;
                }
                Err(error) => {
                    tracing::warn!(key = %entry.key, %error, "skipping malformed persisted Cognitive tool alias");
                    continue;
                }
            };
            match self.install(&registration).await {
                Ok(()) => restored += 1,
                Err(error) => tracing::warn!(
                    name = %registration.name,
                    target = %registration.target,
                    %error,
                    "skipping unavailable persisted Cognitive tool alias"
                ),
            }
        }
        if restored > 0 {
            tracing::info!(restored, "restored persisted Cognitive tool aliases");
        }
        Ok(restored)
    }

    async fn register(&self, registration: ToolAliasRegistration) -> Result<()> {
        let _guard = self.gate.lock().await;
        self.assert_installable(&registration).await?;
        self.store
            .upsert_namespace(
                DYNAMIC_TOOL_NAMESPACE,
                NamespaceKind::Project,
                Some("Cognitive operational state; not model-controlled memory"),
                None,
                None,
                serde_json::json!({
                    "owner": "canonical_orchestrator",
                    "purpose": "declarative_tool_aliases",
                }),
            )
            .await?;
        // Persist before exposing the alias. A storage failure therefore
        // fails closed rather than creating a live alias that disappears on
        // the next process restart.
        self.store
            .store_entry(
                DYNAMIC_TOOL_NAMESPACE,
                &format!("{DYNAMIC_TOOL_KEY_PREFIX}{}", registration.name),
                serde_json::to_value(&registration)?,
                vec!["internal".to_string(), "tool-alias".to_string()],
                None,
            )
            .await?;
        self.install(&registration).await?;
        Ok(())
    }

    async fn assert_installable(&self, registration: &ToolAliasRegistration) -> Result<()> {
        validate_alias_registration(registration)?;
        if self.registry.get(&registration.name).await.is_some() {
            anyhow::bail!("tool name '{}' is already registered", registration.name);
        }
        let target = self
            .registry
            .get(&registration.target)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("target tool '{}' is not registered", registration.target)
            })?;
        if !matches!(target.readiness(), ToolReadiness::Live) {
            anyhow::bail!(
                "target tool '{}' is not live and cannot be exposed by an alias",
                registration.target
            );
        }
        if target.tags().iter().any(|tag| tag == "dynamic_alias") {
            anyhow::bail!("dynamic aliases may not target another alias");
        }
        Ok(())
    }

    async fn install(&self, registration: &ToolAliasRegistration) -> Result<()> {
        self.assert_installable(registration).await?;
        let definition = self
            .registry
            .get_definition(&registration.target)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "target tool '{}' has no catalog definition",
                    registration.target
                )
            })?;
        let description = registration
            .description
            .clone()
            .unwrap_or_else(|| format!("Declared alias for {}", registration.target));
        self.registry
            .register(Arc::new(RegisteredAliasTool {
                name: registration.name.clone(),
                target: registration.target.clone(),
                description,
                input_schema: definition.input_schema,
                category: definition.category,
                namespace: definition.namespace,
                registry: self.registry.clone(),
            }) as BoxedTool)
            .await
    }
}

fn validate_alias_registration(registration: &ToolAliasRegistration) -> Result<()> {
    if registration.version != 1 {
        anyhow::bail!("unsupported tool alias version {}", registration.version);
    }
    let valid_name = |value: &str| {
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    if !valid_name(&registration.name) {
        anyhow::bail!(
            "tool alias name must be 1-64 lowercase ASCII letters, digits, or underscores"
        );
    }
    if !valid_name(&registration.target) {
        anyhow::bail!("tool alias target must be an existing lowercase registry tool name");
    }
    if registration.name == registration.target || registration.target == "register_tool" {
        anyhow::bail!("tool aliases cannot target themselves or register_tool");
    }
    if registration
        .description
        .as_deref()
        .is_some_and(|description| description.trim().is_empty() || description.len() > 512)
    {
        anyhow::bail!("tool alias description must be non-empty and at most 512 bytes");
    }
    Ok(())
}

struct RegisteredAliasTool {
    name: String,
    target: String,
    description: String,
    input_schema: Value,
    category: String,
    namespace: String,
    registry: Arc<ToolRegistry>,
}

#[async_trait]
impl Tool for RegisteredAliasTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn category(&self) -> &str {
        &self.category
    }

    fn namespace(&self) -> &str {
        &self.namespace
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "dynamic_alias".to_string(),
            format!("target:{}", self.target),
        ]
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        self.registry.execute(&self.target, input).await
    }
}

/// Declaratively register a persisted alias for an already-live tool.
/// Runtime registration never interprets schema text as code and cannot
/// re-enable disabled or mock integrations.
pub struct RegisterToolTool {
    dynamic_tools: Arc<DynamicToolCatalog>,
}

impl RegisterToolTool {
    fn new(dynamic_tools: Arc<DynamicToolCatalog>) -> Self {
        Self { dynamic_tools }
    }
}

#[async_trait]
impl Tool for RegisterToolTool {
    fn name(&self) -> &str {
        "register_tool"
    }

    fn description(&self) -> &str {
        "Register a persisted, declarative alias for an existing live Cognitive tool. Aliases cannot add code, providers, or permissions."
    }

    fn category(&self) -> &str {
        "cognitive"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "tool".to_string(),
            "registry".to_string(),
            "registration".to_string(),
            "declarative".to_string(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "name": {"type": "string", "description": "New lowercase alias name"},
                "target": {"type": "string", "description": "Existing live registry tool to expose"},
                "description": {"type": "string", "description": "Optional operator-facing alias description"}
            },
            "required": ["name", "target"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let registration = ToolAliasRegistration {
            version: 1,
            name: field(&input, "name")
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("register_tool requires name"))?
                .to_string(),
            target: field(&input, "target")
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("register_tool requires target"))?
                .to_string(),
            description: field(&input, "description").as_str().map(str::to_string),
        };
        self.dynamic_tools.register(registration.clone()).await?;
        Ok(json!({
            "success": true,
            "tool_name": registration.name,
            "target": registration.target,
            "persisted": true,
        }))
    }
}

impl MemoryTool {
    async fn op_ensure_soul(&self, input: &Value) -> Result<Value> {
        let owner = field(input, "owner").as_str().unwrap_or("user_container");
        let container_id = field(input, "container_id").as_str();
        let identity = soul_identity(owner, input)?;
        let namespace = soul_namespace(owner, container_id)?;
        let kind = if owner == "chatbot" {
            NamespaceKind::Agent
        } else {
            NamespaceKind::Custom
        };
        let metadata = soul_metadata(owner, container_id, identity.as_deref(), input);

        self.store
            .upsert_namespace(
                &namespace,
                kind,
                Some("Durable soul state"),
                None,
                None,
                metadata,
            )
            .await?;

        let value = if field(input, "value").is_null() {
            serde_json::json!({
                "owner": owner,
                "container_id": container_id,
                "identity_id": identity,
                "wireguard_pubkey": field(input, "wireguard_pubkey").as_str(),
                "purpose": "soul"
            })
        } else {
            simd_json_to_serde(field(input, "value"))
        };
        let entry = self
            .store
            .store_entry(
                &namespace,
                "soul",
                value,
                soul_tags(owner, container_id, identity.as_deref()),
                None,
            )
            .await?;

        if owner == "user_container" {
            let namespace = memory_namespace(input)?;
            self.ensure_namespace(&namespace, Some("custom")).await?;
            self.store
                .store_entry(
                    &namespace,
                    "_identity",
                    identity_link_value(input, identity.as_deref()),
                    scoped_tags(vec!["identity".to_string()], container_id),
                    None,
                )
                .await?;
        }

        Ok(json!({
            "ok": true,
            "namespace": namespace,
            "key": entry.key,
            "id": entry.id,
            "identity_id": identity
        }))
    }

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
        let namespace = memory_namespace(input)?;
        let key = field(input, "key")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;
        let value = simd_json_to_serde(field(input, "value"));
        let tags: Vec<String> = field(input, "tags")
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let tags = scoped_tags(tags, field(input, "container_id").as_str());

        self.ensure_namespace(&namespace, field(input, "namespace_kind").as_str())
            .await?;

        if field(input, "container_id").as_str().is_some()
            && self
                .store
                .retrieve_entry(&namespace, "_identity")
                .await?
                .is_none()
        {
            let identity = soul_identity("user_container", input)?;
            self.store
                .store_entry(
                    &namespace,
                    "_identity",
                    identity_link_value(input, identity.as_deref()),
                    scoped_tags(
                        vec!["identity".to_string()],
                        field(input, "container_id").as_str(),
                    ),
                    None,
                )
                .await?;
        }

        let entry = self
            .store
            .store_entry(&namespace, key, value.clone(), tags, None)
            .await?;
        let semantic_mirrored = self
            .mirror_semantic_memory(input, &namespace, key, &value)
            .await?;

        Ok(json!({
            "ok": true,
            "id": entry.id,
            "namespace": namespace,
            "key": key,
            "semantic_mirrored": semantic_mirrored
        }))
    }

    async fn op_retrieve(&self, input: &Value) -> Result<Value> {
        let namespace = memory_namespace(input)?;
        let key = field(input, "key")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        match self.store.retrieve_entry(&namespace, key).await? {
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
            namespace_id: optional_memory_namespace(input)?,
            key_pattern: field(input, "key_pattern").as_str().map(String::from),
            tags: field(input, "tags").as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
            limit: field(input, "limit").as_i64(),
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

    async fn op_semantic_query(&self, input: &Value) -> Result<Value> {
        let qdrant = self.qdrant.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "semantic memory unavailable: Qdrant Semantic Shuttle is not configured"
            )
        })?;
        let container_id = field(input, "container_id")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing container_id"))?;
        let query = field(input, "query")
            .as_str()
            .or_else(|| field(input, "key_pattern").as_str())
            .ok_or_else(|| anyhow::anyhow!("missing query"))?;
        let limit = field(input, "limit").as_i64().unwrap_or(10).max(1) as u64;

        let embedding = qdrant.embed_query_text(query).await?;
        let results = qdrant
            .search_user_memory(embedding, container_id, limit)
            .await?;
        let count = results.len();
        let items: Vec<Value> = results
            .into_iter()
            .map(|point| {
                json!({
                    "id": format!("{:?}", point.id),
                    "score": point.score,
                    "payload": format!("{:?}", point.payload)
                })
            })
            .collect();

        Ok(json!({
            "count": count,
            "container_id": container_id,
            "results": items
        }))
    }

    async fn op_delete(&self, input: &Value) -> Result<Value> {
        let namespace = memory_namespace(input)?;
        let key = field(input, "key")
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing key"))?;

        let deleted = self.store.delete_entry(&namespace, key).await?;
        Ok(json!({ "ok": deleted, "namespace": namespace, "key": key }))
    }

    async fn op_list_namespaces(&self, input: &Value) -> Result<Value> {
        // `namespace_kind` is optional. `input[key]` on simd_json's OwnedValue panics
        // when the key is absent (unlike serde_json, which yields Null), so this must
        // go through `get`. Calling list_namespaces without a kind filter is the
        // common case and previously panicked the worker task.
        let kind = input
            .get("namespace_kind")
            .and_then(|v| v.as_str())
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

    async fn mirror_semantic_memory(
        &self,
        input: &Value,
        namespace: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<bool> {
        if !field(input, "semantic").as_bool().unwrap_or(false) {
            return Ok(false);
        }

        let Some(qdrant) = &self.qdrant else {
            tracing::warn!(
                namespace = namespace,
                key = key,
                "semantic memory requested but Qdrant Semantic Shuttle is unavailable"
            );
            return Ok(false);
        };

        let Some(container_id) = field(input, "container_id").as_str() else {
            tracing::warn!(
                namespace = namespace,
                key = key,
                "semantic memory requested without container_id"
            );
            return Ok(false);
        };

        let content = memory_content(value);
        if content.trim().is_empty() {
            return Ok(false);
        }

        let vector = qdrant.embed_document(&content).await?;
        qdrant
            .upsert_user_memory(
                uuid::Uuid::new_v4().to_string(),
                vector,
                container_id,
                key,
                &content,
            )
            .await?;
        Ok(true)
    }
}

fn simd_json_to_serde(v: &Value) -> serde_json::Value {
    let s = simd_json::to_string(v).unwrap_or_default();
    serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)
}

fn serde_to_simd_json(v: serde_json::Value) -> Value {
    let s = serde_json::to_string(&v).unwrap_or_default();
    let mut buf = s.into_bytes();
    simd_json::from_slice(&mut buf).unwrap_or(Value::Static(simd_json::StaticNode::Null))
}

fn memory_namespace(input: &Value) -> Result<String> {
    optional_memory_namespace(input)?
        .ok_or_else(|| anyhow::anyhow!("missing namespace or container_id"))
}

fn optional_memory_namespace(input: &Value) -> Result<Option<String>> {
    if let Some(namespace) = field(input, "namespace").as_str() {
        return Ok(Some(namespace.to_string()));
    }
    Ok(field(input, "container_id")
        .as_str()
        .map(container_namespace))
}

fn container_namespace(container_id: &str) -> String {
    format!("container:{}", container_id)
}

fn soul_namespace(owner: &str, container_id: Option<&str>) -> Result<String> {
    match owner {
        "chatbot" => Ok("soul:chatbot".to_string()),
        "user_container" => {
            let container_id =
                container_id.ok_or_else(|| anyhow::anyhow!("missing container_id"))?;
            Ok(format!("soul:user-container:{container_id}"))
        }
        other => Err(anyhow::anyhow!("unknown soul owner: {}", other)),
    }
}

fn soul_metadata(
    owner: &str,
    container_id: Option<&str>,
    identity_id: Option<&str>,
    input: &Value,
) -> serde_json::Value {
    serde_json::json!({
        "owner": owner,
        "container_id": container_id,
        "identity_id": identity_id,
        "wireguard_pubkey": field(input, "wireguard_pubkey").as_str(),
        "subid": if owner == "chatbot" {
            "src.software.chatbot-soul.persist@v1"
        } else {
            "src.software.user-container-soul.persist@v1"
        }
    })
}

fn soul_tags(owner: &str, container_id: Option<&str>, identity_id: Option<&str>) -> Vec<String> {
    let mut tags = vec!["soul".to_string(), owner.to_string()];
    if let Some(container_id) = container_id {
        tags.push(format!("container:{container_id}"));
    }
    if let Some(identity_id) = identity_id {
        tags.push(format!("identity:{identity_id}"));
    }
    tags
}

fn scoped_tags(mut tags: Vec<String>, container_id: Option<&str>) -> Vec<String> {
    if let Some(container_id) = container_id {
        let tag = format!("container:{container_id}");
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

fn memory_content(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_default())
}

fn soul_identity(owner: &str, input: &Value) -> Result<Option<String>> {
    let identity = field(input, "identity_id").as_str().map(ToOwned::to_owned);
    if owner == "user_container" && identity.is_none() {
        return Err(anyhow::anyhow!(
            "missing identity_id for user_container soul"
        ));
    }
    Ok(identity)
}

fn identity_link_value(input: &Value, identity_id: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "identity_id": identity_id,
        "container_id": field(input, "container_id").as_str(),
        "wireguard_pubkey": field(input, "wireguard_pubkey").as_str(),
        "soul_namespace": field(input, "container_id")
            .as_str()
            .map(|container_id| format!("soul:user-container:{container_id}")),
        "subid": "src.software.user-container-memory.identity-link@v1"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cozo_shuttle::CozoGraphShuttle;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo_tool"
        }

        fn description(&self) -> &str {
            "Returns its JSON input for alias tests"
        }

        fn input_schema(&self) -> Value {
            json!({"type": "object"})
        }

        async fn execute(&self, input: Value) -> Result<Value> {
            Ok(input)
        }
    }

    async fn test_store() -> Arc<CognitiveMemoryStore> {
        let shuttle = Arc::new(CozoGraphShuttle::new_in_memory().expect("in-memory Cozo"));
        Arc::new(
            CognitiveMemoryStore::new(shuttle)
                .await
                .expect("Cognitive memory store"),
        )
    }

    #[tokio::test]
    async fn dynamic_tool_registration_creates_a_safe_persisted_alias() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(Arc::new(EchoTool)).await.unwrap();
        let store = test_store().await;
        let catalog = DynamicToolCatalog::new(registry.clone(), store.clone());
        let registration = ToolAliasRegistration {
            version: 1,
            name: "project_echo".to_string(),
            target: "echo_tool".to_string(),
            description: Some("Project-local echo alias".to_string()),
        };

        catalog.register(registration).await.unwrap();
        let alias = registry
            .get("project_echo")
            .await
            .expect("registered alias");
        assert_eq!(alias.readiness().status(), "live");
        let mut input = br#"{"hello":"world"}"#.to_vec();
        assert_eq!(
            registry
                .execute(
                    "project_echo",
                    simd_json::to_owned_value(&mut input).unwrap()
                )
                .await
                .unwrap(),
            json!({"hello":"world"})
        );
        assert!(store
            .retrieve_entry(DYNAMIC_TOOL_NAMESPACE, "_tool_alias:project_echo")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn dynamic_tool_registration_rejects_disabled_or_recursive_targets() {
        let registry = Arc::new(ToolRegistry::new());
        let store = test_store().await;
        let catalog = DynamicToolCatalog::new(registry, store);
        let error = catalog
            .register(ToolAliasRegistration {
                version: 1,
                name: "loop".to_string(),
                target: "register_tool".to_string(),
                description: None,
            })
            .await
            .expect_err("register_tool must not be aliasable");
        assert!(error.to_string().contains("register_tool"));
    }
}
