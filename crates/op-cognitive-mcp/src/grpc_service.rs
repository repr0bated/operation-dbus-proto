//! ⚖️ CognitiveToolService — gRPC ingress for NotebookLM MCP
//!
//! # Requirements
//! Implements R1-R3 (core querying), R9-R11 (resilience/auth), and stubs for
//! R4-R7 (lifecycle, advanced). Traces every RPC to the requirements doc.
//!
//! # Design
//! - gRPC → CognitiveToolRegistry → NotebookLM bridge + CognitiveMemoryStore
//! - CognitiveMemoryStore is the cache; NotebookLM is the source of truth
//! - Sessions tracked via SessionManager for conversation_id follow-ups
//! - Quota enforced via QuotaManager before forwarding to the bridge
//!
//! # Security (R13)
//! - No shell=True, no eval
//! - Credentials stored 0o600
//! - Exponential backoff retries on bridge calls

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use sha2::{Digest, Sha256};
use tonic::{Request, Response, Status};
use tracing::{info, warn};
use uuid::Uuid;

use crate::context_awareness::{ActivityType, ContextAwarenessConfig, ContextAwarenessEngine};
use crate::memory_store::{CognitiveMemoryStore, MemoryEntry, MemoryNamespace, NamespaceKind};
use crate::proto::cognitive_tool_service_server::CognitiveToolService;
use crate::proto::*;
use crate::quota::QuotaManager;
use crate::session::{QueryTurn, SessionManager};
use op_mcp::tool_registry::ToolRegistry;

/// Authenticated request context injected by the bridge after its
/// Ghostbridge identity and capability checks have succeeded. The direct
/// Cognitive service deliberately does not derive this from request fields:
/// callers must not be able to self-assert the actor recorded in the audit
/// chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CognitiveRequestContext {
    pub actor_id: String,
    pub capability_id: String,
}

/// Safe, non-content-bearing description of a cognitive write handed to the
/// bridge's canonical mutation/audit boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct CognitiveMutationAuditRequest {
    pub actor_id: String,
    pub capability_id: String,
    pub operation: String,
    pub arguments: serde_json::Value,
}

/// Immutable chain receipt returned once a cognitive write has been accepted
/// by the bridge's audit boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CognitiveMutationAuditReceipt {
    pub event_id: u64,
    pub event_hash: String,
}

/// Adapter implemented by the hosting bridge. Keeping this trait in the
/// Cognitive crate avoids a dependency cycle while making the bridge—not a
/// raw gRPC handler—the only available mutation ingress.
#[async_trait::async_trait]
pub trait CognitiveMutationAuditor: Send + Sync {
    async fn record_mutation(
        &self,
        request: CognitiveMutationAuditRequest,
    ) -> Result<CognitiveMutationAuditReceipt, Status>;
}

/// Request for one provider-neutral model operation. Cognitive MCP supplies
/// the constrained task and source context; the bridge resolves the live
/// route and is the sole owner of provider/model selection.
#[derive(Clone, Debug, PartialEq)]
pub struct CognitiveModelRequest {
    pub actor_id: String,
    pub capability_id: String,
    pub operation: String,
    pub prompt: String,
}

/// Model output together with the resolved route and audit receipt that made
/// the external operation accountable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CognitiveModelResponse {
    pub content: String,
    pub provider: String,
    pub model: String,
    pub audit_event_id: u64,
    pub audit_event_hash: String,
}

/// Provider-neutral model routing contract implemented only by the bridge.
#[async_trait::async_trait]
pub trait CognitiveModelRouter: Send + Sync {
    async fn generate(
        &self,
        request: CognitiveModelRequest,
    ) -> Result<CognitiveModelResponse, Status>;
}

/// Used by a Cognitive service constructed outside the bridge. Reads remain
/// useful for diagnostics, but writes fail closed rather than bypassing the
/// canonical event-chain path.
#[derive(Default)]
struct UnavailableMutationAuditor;

#[async_trait::async_trait]
impl CognitiveMutationAuditor for UnavailableMutationAuditor {
    async fn record_mutation(
        &self,
        _request: CognitiveMutationAuditRequest,
    ) -> Result<CognitiveMutationAuditReceipt, Status> {
        Err(Status::failed_precondition(
            "Cognitive mutations require the op-grpc-bridge canonical audit ingress.",
        ))
    }
}

#[derive(Default)]
struct UnavailableModelRouter;

#[async_trait::async_trait]
impl CognitiveModelRouter for UnavailableModelRouter {
    async fn generate(
        &self,
        _request: CognitiveModelRequest,
    ) -> Result<CognitiveModelResponse, Status> {
        Err(Status::failed_precondition(
            "Cognitive model generation requires the op-grpc-bridge provider-neutral route.",
        ))
    }
}

const DEFAULT_INGEST_MAX_FILES: usize = 10_000;
const DEFAULT_INGEST_MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_INGEST_ERROR_REPORTS: usize = 100;
const DEFAULT_QUERY_RESULTS: usize = 10;
const MAX_QUERY_RESULTS: usize = 50;
const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 500;
const DEFAULT_HISTORY_PAGE_SIZE: usize = 50;
const MAX_HISTORY_PAGE_SIZE: usize = 200;
const MAX_NOTEBOOK_REF_BYTES: usize = 512;
const MAX_NOTEBOOK_TITLE_BYTES: usize = 256;
const MAX_NOTEBOOK_DESCRIPTION_BYTES: usize = 8 * 1024;
const MAX_BATCH_NOTEBOOKS: usize = 100;
const MAX_SOURCE_CONTENT_BYTES: usize = 5 * 1024 * 1024;
const MAX_SOURCE_TITLE_BYTES: usize = 512;
const MAX_SOURCE_TAGS: usize = 32;
const MAX_SOURCE_TAG_BYTES: usize = 128;
const MAX_FOLDER_PATTERNS: usize = 32;
const MAX_FOLDER_PATTERN_BYTES: usize = 128;
const MAX_DATA_TABLE_PROMPT_BYTES: usize = 8 * 1024;
const MAX_DATA_TABLE_COLUMNS: usize = 100;
const MAX_DATA_TABLE_COLUMN_BYTES: usize = 128;
const MAX_DATA_TABLE_SOURCES: i64 = 50;
const MAX_DATA_TABLE_SOURCE_CONTEXT_BYTES: usize = 256 * 1024;
const MAX_DATA_TABLE_ROWS: usize = 1_000;
const MAX_DATA_TABLE_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

/// The gRPC service implementation.
///
/// Wired into the tonic server alongside health and reflection services.
/// Delegates to CognitiveMemoryStore for namespace/entry ops and to the
/// NotebookLM MCP bridge (via ToolRegistry) for grounded queries.
#[derive(Clone)]
pub struct CognitiveGrpcService {
    memory_store: Arc<CognitiveMemoryStore>,
    session_manager: Arc<SessionManager>,
    quota_manager: Arc<QuotaManager>,
    tool_registry: Arc<ToolRegistry>,
    context_engine: Arc<ContextAwarenessEngine>,
    mutation_auditor: Arc<dyn CognitiveMutationAuditor>,
    model_router: Arc<dyn CognitiveModelRouter>,
}

impl CognitiveGrpcService {
    pub fn new(
        memory_store: Arc<CognitiveMemoryStore>,
        session_manager: Arc<SessionManager>,
        quota_manager: Arc<QuotaManager>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self::with_mutation_auditor(
            memory_store,
            session_manager,
            quota_manager,
            tool_registry,
            Arc::new(UnavailableMutationAuditor),
        )
    }

    /// Construct the bridge-mounted service with an auditor that records each
    /// accepted mutation before the authoritative Cozo write occurs.
    pub fn with_mutation_auditor(
        memory_store: Arc<CognitiveMemoryStore>,
        session_manager: Arc<SessionManager>,
        quota_manager: Arc<QuotaManager>,
        tool_registry: Arc<ToolRegistry>,
        mutation_auditor: Arc<dyn CognitiveMutationAuditor>,
    ) -> Self {
        Self::with_operational_adapters(
            memory_store.clone(),
            session_manager,
            quota_manager,
            tool_registry,
            Arc::new(ContextAwarenessEngine::new(
                ContextAwarenessConfig::default(),
                memory_store,
                None,
            )),
            mutation_auditor,
            Arc::new(UnavailableModelRouter),
        )
    }

    /// Construct the bridge-mounted service with canonical audit and model
    /// routing adapters. Neither adapter can be recreated by an MCP caller.
    pub fn with_operational_adapters(
        memory_store: Arc<CognitiveMemoryStore>,
        session_manager: Arc<SessionManager>,
        quota_manager: Arc<QuotaManager>,
        tool_registry: Arc<ToolRegistry>,
        context_engine: Arc<ContextAwarenessEngine>,
        mutation_auditor: Arc<dyn CognitiveMutationAuditor>,
        model_router: Arc<dyn CognitiveModelRouter>,
    ) -> Self {
        Self {
            memory_store,
            session_manager,
            quota_manager,
            tool_registry,
            context_engine,
            mutation_auditor,
            model_router,
        }
    }

    fn bridge_request_context<T>(request: &Request<T>) -> Result<CognitiveRequestContext, Status> {
        request
            .extensions()
            .get::<CognitiveRequestContext>()
            .cloned()
            .ok_or_else(|| {
                Status::failed_precondition(
                    "Cognitive request was not admitted through the canonical bridge ingress.",
                )
            })
    }

    async fn audit_mutation(
        &self,
        context: CognitiveRequestContext,
        operation: &str,
        arguments: serde_json::Value,
    ) -> Result<CognitiveMutationAuditReceipt, Status> {
        if context.capability_id != "cognitive_mcp.invoke" {
            return Err(Status::permission_denied(
                "Cognitive mutation requires capability cognitive_mcp.invoke.",
            ));
        }
        self.mutation_auditor
            .record_mutation(CognitiveMutationAuditRequest {
                actor_id: context.actor_id,
                capability_id: context.capability_id,
                operation: operation.to_string(),
                arguments,
            })
            .await
    }

    /// Record grounded-query activity in the shared, ephemeral awareness
    /// engine. This is deliberately not memory ingestion: it has no namespace
    /// write API and only returns bounded session signals to the caller.
    async fn record_query_context(&self, session_id: &str, namespace: &str, query: &str) -> String {
        self.context_engine
            .record_activity(
                session_id,
                ActivityType::Query,
                query.to_string(),
                serde_json::json!({ "namespace": namespace, "source": "grounded_query" }),
            )
            .await;
        self.context_engine
            .get_session_signals(session_id)
            .await
            .and_then(|signals| serde_json::to_string(&signals).ok())
            .unwrap_or_else(|| "{}".to_string())
    }

    /// Resolve either stable UUIDs returned by `ListNotebooks`, canonical
    /// namespace names, or the historic bare project slug accepted by the
    /// original gRPC API. This keeps every RPC on one namespace identity
    /// instead of prepending `project:` independently at each call site.
    async fn resolve_notebook(&self, notebook_ref: &str) -> Result<MemoryNamespace, Status> {
        let notebook_ref = notebook_ref.trim();
        if notebook_ref.is_empty() {
            return Err(Status::invalid_argument("notebook_id is required"));
        }
        require_at_most_bytes(notebook_ref, "notebook_id", MAX_NOTEBOOK_REF_BYTES)?;

        if let Some(namespace) = self
            .memory_store
            .get_namespace_by_name(notebook_ref)
            .await
            .map_err(memory_status)?
        {
            return Ok(namespace);
        }
        if let Some(namespace) = self
            .memory_store
            .get_namespace_by_id(notebook_ref)
            .await
            .map_err(memory_status)?
        {
            return Ok(namespace);
        }
        if !notebook_ref.contains(':') {
            let canonical_name = format!("project:{notebook_ref}");
            if let Some(namespace) = self
                .memory_store
                .get_namespace_by_name(&canonical_name)
                .await
                .map_err(memory_status)?
            {
                return Ok(namespace);
            }
        }

        Err(Status::not_found(format!(
            "Notebook '{notebook_ref}' not found"
        )))
    }

    /// Source-ingest RPCs preserve their legacy create-on-first-use behavior,
    /// but only after resolving every public notebook reference form above.
    async fn resolve_or_create_notebook(
        &self,
        notebook_ref: &str,
    ) -> Result<MemoryNamespace, Status> {
        match self.resolve_notebook(notebook_ref).await {
            Ok(namespace) => Ok(namespace),
            Err(status) if status.code() == tonic::Code::NotFound => {
                let name = canonical_notebook_name(notebook_ref)?;
                let kind = namespace_kind_from_name(&name);
                self.memory_store
                    .upsert_namespace(&name, kind, None, None, None, serde_json::json!({}))
                    .await
                    .map_err(memory_status)
            }
            Err(status) => Err(status),
        }
    }
}

#[tonic::async_trait]
impl CognitiveToolService for CognitiveGrpcService {
    // =========================================================================
    // R1 — AskQuestion (grounded query)
    // =========================================================================
    async fn ask_question(
        &self,
        request: Request<AskQuestionRequest>,
    ) -> Result<Response<AskQuestionResponse>, Status> {
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            conversation_id = %req.conversation_id,
            "AskQuestion"
        );

        require_query(&req.query)?;
        require_conversation_id(&req.conversation_id)?;
        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;

        // R11 — quota check
        let (allowed, remaining, _limit) = self.quota_manager.check_and_increment().await;
        if !allowed {
            return Err(Status::resource_exhausted(format!(
                "Daily query quota exceeded ({} remaining)",
                remaining
            )));
        }

        // R2 — conversation_id session management
        let session = self
            .session_manager
            .get_or_create(&req.conversation_id, &namespace);
        let conversation_id = session.id.clone();

        // Attempt grounded query via memory store.
        // Phase 1: query entries matching the notebook namespace.
        // Phase 2+: this forwards through the NotebookLM bridge.
        let entries = self
            .memory_store
            .search_entries(&namespace, &req.query, 10)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let (answer, citations, grounded) =
            format_grounded_query(&entries, &req.query, &req.notebook_id);

        // Append turn to session history
        let _ = self.session_manager.append_turn(
            &conversation_id,
            QueryTurn {
                query: req.query.clone(),
                answer: answer.clone(),
                timestamp: Utc::now(),
                citations_count: citations.len() as u32,
                grounded,
            },
        );
        let context_json = self
            .record_query_context(&conversation_id, &namespace, &req.query)
            .await;

        Ok(Response::new(AskQuestionResponse {
            answer,
            citations,
            conversation_id,
            grounded,
            context_json,
        }))
    }

    // =========================================================================
    // QueryNotebook
    // =========================================================================
    async fn query_notebook(
        &self,
        request: Request<QueryNotebookRequest>,
    ) -> Result<Response<QueryNotebookResponse>, Status> {
        let req = request.into_inner();
        info!(notebook_id = %req.notebook_id, "QueryNotebook");

        require_query(&req.query)?;
        require_conversation_id(&req.conversation_id)?;
        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;

        let (allowed, _, _) = self.quota_manager.check_and_increment().await;
        if !allowed {
            return Err(Status::resource_exhausted("Daily query quota exceeded"));
        }

        let session = self
            .session_manager
            .get_or_create(&req.conversation_id, &namespace);

        let limit = bounded_limit(req.max_results, DEFAULT_QUERY_RESULTS, MAX_QUERY_RESULTS);

        let entries = self
            .memory_store
            .search_entries(&namespace, &req.query, limit)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let (answer, citations, grounded) =
            format_grounded_query(&entries, &req.query, &req.notebook_id);

        let query = req.query;
        let _ = self.session_manager.append_turn(
            &session.id,
            QueryTurn {
                query: query.clone(),
                answer: answer.clone(),
                timestamp: Utc::now(),
                citations_count: citations.len() as u32,
                grounded,
            },
        );
        let context_json = self
            .record_query_context(&session.id, &namespace, &query)
            .await;

        Ok(Response::new(QueryNotebookResponse {
            answer,
            citations,
            conversation_id: session.id,
            context_json,
        }))
    }

    // =========================================================================
    // R3 — ListNotebooks
    // =========================================================================
    async fn list_notebooks(
        &self,
        request: Request<ListNotebooksRequest>,
    ) -> Result<Response<ListNotebooksResponse>, Status> {
        let req = request.into_inner();
        info!(kind_filter = %req.kind_filter, "ListNotebooks");

        let kind = if req.kind_filter.is_empty() {
            None
        } else {
            req.kind_filter.parse::<NamespaceKind>().ok()
        };

        let namespaces = self
            .memory_store
            .list_namespaces(kind)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let total = namespaces.len() as i32;
        let offset = nonnegative_offset(req.offset);
        let limit = bounded_limit(req.limit, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE);

        let mut notebooks = Vec::new();
        for ns in namespaces.into_iter().skip(offset).take(limit) {
            let source_count = self
                .memory_store
                .count_entries(&ns.name)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            notebooks.push(NotebookInfo {
                id: ns.id,
                name: ns.name,
                kind: ns.kind.to_string(),
                description: ns.description.unwrap_or_default(),
                source_count,
                created_at: ns.created_at.to_rfc3339(),
                updated_at: ns.updated_at.to_rfc3339(),
            });
        }

        Ok(Response::new(ListNotebooksResponse { notebooks, total }))
    }

    // =========================================================================
    // R3 — GetNotebook
    // =========================================================================
    async fn get_notebook(
        &self,
        request: Request<GetNotebookRequest>,
    ) -> Result<Response<GetNotebookResponse>, Status> {
        let req = request.into_inner();
        info!(notebook_id = %req.notebook_id, "GetNotebook");

        let ns = self.resolve_notebook(&req.notebook_id).await?;

        let metadata_json =
            serde_json::to_string(&ns.metadata).unwrap_or_else(|_| "{}".to_string());

        let source_count = self
            .memory_store
            .count_entries(&ns.name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(GetNotebookResponse {
            notebook: Some(NotebookInfo {
                id: ns.id,
                name: ns.name,
                kind: ns.kind.to_string(),
                description: ns.description.unwrap_or_default(),
                source_count,
                created_at: ns.created_at.to_rfc3339(),
                updated_at: ns.updated_at.to_rfc3339(),
            }),
            metadata_json,
        }))
    }

    // =========================================================================
    // R4 — CreateNotebook
    // =========================================================================
    async fn create_notebook(
        &self,
        request: Request<CreateNotebookRequest>,
    ) -> Result<Response<CreateNotebookResponse>, Status> {
        let context = Self::bridge_request_context(&request)?;
        let req = request.into_inner();
        info!(title = %req.title, "CreateNotebook");
        require_at_most_bytes(
            &req.description,
            "description",
            MAX_NOTEBOOK_DESCRIPTION_BYTES,
        )?;

        let kind = if req.kind.is_empty() {
            NamespaceKind::Project
        } else {
            req.kind.parse().unwrap_or(NamespaceKind::Custom)
        };

        let (name, kind) = canonical_create_notebook(&req.title, kind)?;
        let receipt = self
            .audit_mutation(
                context,
                "create_notebook",
                serde_json::json!({
                    "notebook": &name,
                    "kind": kind.to_string(),
                    "description_sha256": content_fingerprint(&req.description),
                }),
            )
            .await?;
        let ns = self
            .memory_store
            .upsert_namespace(
                &name,
                kind,
                if req.description.is_empty() {
                    None
                } else {
                    Some(req.description.as_str())
                },
                None,
                None,
                serde_json::json!({}),
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let source_count = self
            .memory_store
            .count_entries(&ns.name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateNotebookResponse {
            notebook: Some(NotebookInfo {
                id: ns.id,
                name: ns.name,
                kind: ns.kind.to_string(),
                description: ns.description.unwrap_or_default(),
                source_count,
                created_at: ns.created_at.to_rfc3339(),
                updated_at: ns.updated_at.to_rfc3339(),
            }),
            audit_event_id: receipt.event_id,
            audit_event_hash: receipt.event_hash,
        }))
    }

    // =========================================================================
    // R4 — BatchCreateNotebooks
    // =========================================================================
    async fn batch_create_notebooks(
        &self,
        request: Request<BatchCreateNotebooksRequest>,
    ) -> Result<Response<BatchCreateNotebooksResponse>, Status> {
        let context = Self::bridge_request_context(&request)?;
        let req = request.into_inner();
        info!(count = req.notebooks.len(), "BatchCreateNotebooks");
        if req.notebooks.len() > MAX_BATCH_NOTEBOOKS {
            return Err(Status::invalid_argument(format!(
                "notebooks exceeds the {MAX_BATCH_NOTEBOOKS}-item batch limit"
            )));
        }

        let receipt = self
            .audit_mutation(
                context,
                "batch_create_notebooks",
                serde_json::json!({
                    "count": req.notebooks.len(),
                    "request_sha256": batch_request_fingerprint(&req.notebooks),
                }),
            )
            .await?;

        let mut created_notebooks = Vec::new();
        let mut failed = 0i32;

        for nb_req in req.notebooks {
            if let Err(error) = require_at_most_bytes(
                &nb_req.description,
                "description",
                MAX_NOTEBOOK_DESCRIPTION_BYTES,
            ) {
                warn!(title = %nb_req.title, error = %error, "Failed to validate notebook description");
                failed += 1;
                continue;
            }
            let kind = if nb_req.kind.is_empty() {
                NamespaceKind::Project
            } else {
                nb_req.kind.parse().unwrap_or(NamespaceKind::Custom)
            };

            let (name, kind) = match canonical_create_notebook(&nb_req.title, kind) {
                Ok(value) => value,
                Err(status) => {
                    warn!(title = %nb_req.title, error = %status, "Failed to validate notebook name");
                    failed += 1;
                    continue;
                }
            };
            match self
                .memory_store
                .upsert_namespace(
                    &name,
                    kind,
                    if nb_req.description.is_empty() {
                        None
                    } else {
                        Some(nb_req.description.as_str())
                    },
                    None,
                    None,
                    serde_json::json!({}),
                )
                .await
            {
                Ok(ns) => {
                    let source_count = self.memory_store.count_entries(&ns.name).await.unwrap_or(0);
                    created_notebooks.push(NotebookInfo {
                        id: ns.id,
                        name: ns.name,
                        kind: ns.kind.to_string(),
                        description: ns.description.unwrap_or_default(),
                        source_count,
                        created_at: ns.created_at.to_rfc3339(),
                        updated_at: ns.updated_at.to_rfc3339(),
                    });
                }
                Err(e) => {
                    warn!(title = %nb_req.title, error = %e, "Failed to create notebook");
                    failed += 1;
                }
            }
        }

        let created = created_notebooks.len() as i32;
        Ok(Response::new(BatchCreateNotebooksResponse {
            notebooks: created_notebooks,
            created,
            failed,
            audit_event_id: receipt.event_id,
            audit_event_hash: receipt.event_hash,
        }))
    }

    // =========================================================================
    // R5 — AddSource
    // =========================================================================
    async fn add_source(
        &self,
        request: Request<AddSourceRequest>,
    ) -> Result<Response<AddSourceResponse>, Status> {
        let context = Self::bridge_request_context(&request)?;
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            source_type = %req.source_type,
            "AddSource"
        );
        validate_source_request(&req)?;

        let receipt = self
            .audit_mutation(
                context,
                "add_source",
                serde_json::json!({
                    "notebook_id": &req.notebook_id,
                    "source_type": &req.source_type,
                    "title": &req.title,
                    "tags": &req.tags,
                    "content_bytes": req.content.len(),
                    "content_sha256": content_fingerprint(&req.content),
                }),
            )
            .await?;

        let namespace = self
            .resolve_or_create_notebook(&req.notebook_id)
            .await?
            .name;

        let source_id = Uuid::new_v4().to_string();

        let value = serde_json::json!({
            "source_type": req.source_type,
            "content": req.content,
            "title": req.title,
        });

        self.memory_store
            .store_entry(&namespace, &source_id, value, req.tags, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(AddSourceResponse {
            source_id,
            success: true,
            audit_event_id: receipt.event_id,
            audit_event_hash: receipt.event_hash,
        }))
    }

    // =========================================================================
    // R5 — AddFolder (bulk ingest)
    // =========================================================================
    async fn add_folder(
        &self,
        request: Request<AddFolderRequest>,
    ) -> Result<Response<AddFolderResponse>, Status> {
        let context = Self::bridge_request_context(&request)?;
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            folder_path = %req.folder_path,
            "AddFolder"
        );
        validate_folder_patterns(&req.patterns)?;

        let receipt = self
            .audit_mutation(
                context,
                "add_folder",
                serde_json::json!({
                    "notebook_id": &req.notebook_id,
                    "folder_path_sha256": content_fingerprint(&req.folder_path),
                    "patterns": &req.patterns,
                    "recursive": req.recursive,
                }),
            )
            .await?;

        // Bulk import is intentionally opt-in. The gRPC client never gets to
        // turn an arbitrary readable host directory into retrievable notebook
        // data; the operator supplies the bounded roots through service config.
        let path = resolve_ingest_folder(&req.folder_path)?;

        let namespace = self
            .resolve_or_create_notebook(&req.notebook_id)
            .await?
            .name;

        let mut added = 0i32;
        let mut skipped = 0i32;
        let mut errors = Vec::new();
        let max_files = ingest_limit("COGNITIVE_MCP_INGEST_MAX_FILES", DEFAULT_INGEST_MAX_FILES);
        let max_file_bytes = ingest_limit_u64(
            "COGNITIVE_MCP_INGEST_MAX_FILE_BYTES",
            DEFAULT_INGEST_MAX_FILE_BYTES,
        );

        // Walk directory — no shell, pure Rust
        // Discover only one more file than can be imported. The former helper
        // collected every path before applying `max_files`, which meant a huge
        // tree could exhaust process memory even though its contents were never
        // read. One look-ahead file makes truncation truthful without scanning
        // or storing the rest of the tree.
        let discovery_limit = max_files.saturating_add(1);
        let mut walker = walkdir_bounded(&path, req.recursive, discovery_limit);
        let truncated = walker.len() > max_files;
        if truncated {
            walker.truncate(max_files);
            skipped += 1;
            record_ingest_error(
                &mut errors,
                format!(
                    "Folder import reached its {max_files}-file limit; additional files were not read."
                ),
            );
        }

        for entry_path in walker {
            // Apply glob patterns if specified
            if !req.patterns.is_empty() {
                let file_name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let matches = req.patterns.iter().any(|pat| glob_match(pat, &file_name));
                if !matches {
                    skipped += 1;
                    continue;
                }
            }

            match entry_path.metadata() {
                Ok(metadata) if metadata.len() > max_file_bytes => {
                    skipped += 1;
                    record_ingest_error(
                        &mut errors,
                        format!(
                            "{}: file is larger than the {}-byte ingest limit",
                            entry_path.display(),
                            max_file_bytes
                        ),
                    );
                    continue;
                }
                Ok(_) => {}
                Err(error) => {
                    skipped += 1;
                    record_ingest_error(
                        &mut errors,
                        format!("{}: {}", entry_path.display(), error),
                    );
                    continue;
                }
            }

            match std::fs::read_to_string(&entry_path) {
                Ok(content) => {
                    // Keys must be unique within a notebook. A basename would
                    // silently overwrite `src/lib.rs` with `tests/lib.rs`; use
                    // a stable hash of the root-relative path instead.
                    let relative_path = entry_path
                        .strip_prefix(&path)
                        .unwrap_or(&entry_path)
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/");
                    let key = folder_source_key(&relative_path);

                    let value = serde_json::json!({
                        "source_type": "file",
                        "content": content,
                        "title": relative_path,
                        // Do not project a host absolute path into a notebook
                        // source; a relative name is enough for retrieval.
                        "path": relative_path,
                    });

                    match self
                        .memory_store
                        .store_entry(&namespace, &key, value, vec![], None)
                        .await
                    {
                        Ok(_) => added += 1,
                        Err(e) => {
                            record_ingest_error(
                                &mut errors,
                                format!("{}: {}", entry_path.display(), e),
                            );
                        }
                    }
                }
                Err(e) => {
                    skipped += 1;
                    record_ingest_error(&mut errors, format!("{}: {}", entry_path.display(), e));
                }
            }
        }

        Ok(Response::new(AddFolderResponse {
            sources_added: added,
            sources_skipped: skipped,
            errors,
            audit_event_id: receipt.event_id,
            audit_event_hash: receipt.event_hash,
        }))
    }

    // =========================================================================
    // R6 — ListSources
    // =========================================================================
    async fn list_sources(
        &self,
        request: Request<ListSourcesRequest>,
    ) -> Result<Response<ListSourcesResponse>, Status> {
        let req = request.into_inner();
        info!(notebook_id = %req.notebook_id, "ListSources");

        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;
        let limit = bounded_limit(req.limit, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE) as i64;
        let offset = nonnegative_offset(req.offset) as i64;

        let total = self
            .memory_store
            .count_entries(&namespace)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let entries = self
            .memory_store
            .query_entries(crate::memory_store::EntryQuery {
                namespace_id: Some(namespace),
                key_pattern: None,
                tags: None,
                limit: Some(limit),
                offset: Some(offset),
            })
            .await
            .map_err(|e| Status::internal(format!("{e:#}")))?;

        let sources: Vec<SourceInfo> = entries
            .into_iter()
            .map(|e| {
                let source_type = e
                    .value
                    .get("source_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("text")
                    .to_string();

                SourceInfo {
                    // The public source identifier is the entry key. Older
                    // records may have title-shaped keys; preserving that
                    // value keeps them addressable by Get/RemoveSource too.
                    id: e.key.clone(),
                    title: e
                        .value
                        .get("title")
                        .and_then(|value| value.as_str())
                        .filter(|title| !title.is_empty())
                        .unwrap_or(&e.key)
                        .to_string(),
                    source_type,
                    tags: e.tags,
                    created_at: e.created_at.to_rfc3339(),
                }
            })
            .collect();

        Ok(Response::new(ListSourcesResponse { sources, total }))
    }

    // =========================================================================
    // R6 — GetSourceContent
    // =========================================================================
    async fn get_source_content(
        &self,
        request: Request<GetSourceContentRequest>,
    ) -> Result<Response<GetSourceContentResponse>, Status> {
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            source_id = %req.source_id,
            "GetSourceContent"
        );

        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;
        let entry = self
            .memory_store
            .retrieve_entry(&namespace, &req.source_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("Source '{}' not found", req.source_id)))?;

        let content = entry
            .value
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let source_type = entry
            .value
            .get("source_type")
            .and_then(|v| v.as_str())
            .unwrap_or("text")
            .to_string();
        let title = entry
            .value
            .get("title")
            .and_then(|value| value.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or(&entry.key)
            .to_string();

        Ok(Response::new(GetSourceContentResponse {
            content,
            source_type,
            title,
        }))
    }

    // =========================================================================
    // R7 — GenerateDataTable (Phase 3)
    // =========================================================================
    async fn generate_data_table(
        &self,
        request: Request<GenerateDataTableRequest>,
    ) -> Result<Response<GenerateDataTableResponse>, Status> {
        let context = Self::bridge_request_context(&request)?;
        let req = request.into_inner();
        info!(notebook_id = %req.notebook_id, "GenerateDataTable");
        validate_data_table_request(&req)?;

        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;

        // Step 1: Get all sources in the notebook
        let entries = self
            .memory_store
            .query_entries(crate::memory_store::EntryQuery {
                namespace_id: Some(namespace.clone()),
                key_pattern: None,
                tags: None,
                limit: Some(MAX_DATA_TABLE_SOURCES),
                offset: None,
            })
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if entries.is_empty() {
            return Ok(Response::new(GenerateDataTableResponse {
                data_json: "[]".to_string(),
                row_count: 0,
                model_provider: String::new(),
                model: String::new(),
                audit_event_id: 0,
                audit_event_hash: String::new(),
            }));
        }

        let prompt = build_data_table_prompt(&req, &entries)?;
        let model = self
            .model_router
            .generate(CognitiveModelRequest {
                actor_id: context.actor_id,
                capability_id: context.capability_id,
                operation: "generate_data_table".to_string(),
                prompt,
            })
            .await?;
        let rows = parse_data_table_output(&model.content, &req.columns)?;
        let row_count = i32::try_from(rows.len())
            .map_err(|_| Status::internal("data table row count exceeds gRPC range"))?;
        let data_json = serde_json::to_string(&rows).map_err(|error| {
            Status::internal(format!("serialize generated data table: {error}"))
        })?;

        Ok(Response::new(GenerateDataTableResponse {
            data_json,
            row_count,
            model_provider: model.provider,
            model: model.model,
            audit_event_id: model.audit_event_id,
            audit_event_hash: model.audit_event_hash,
        }))
    }

    // =========================================================================
    // R10 — GetHealth
    // =========================================================================
    async fn get_health(
        &self,
        request: Request<GetHealthRequest>,
    ) -> Result<Response<GetHealthResponse>, Status> {
        let req = request.into_inner();
        info!(deep_check = req.deep_check, "GetHealth");

        let (remaining, limit) = self.quota_manager.status().await;

        let mut healthy = true;
        let mut components = serde_json::json!({
            "memory_store": "ok",
            "session_manager": "ok",
            "quota_manager": "ok",
        });

        if req.deep_check {
            // Deep check — verify memory store connectivity
            match self.memory_store.get_stats().await {
                Ok(stats) => {
                    components["memory_store_stats"] = serde_json::json!({
                        "total_namespaces": stats.total_namespaces,
                        "total_entries": stats.total_entries,
                    });
                }
                Err(e) => {
                    healthy = false;
                    components["memory_store"] = serde_json::json!(format!("error: {}", e));
                }
            }

            components["active_sessions"] = serde_json::json!(self.session_manager.active_count());
            components["total_sessions"] = serde_json::json!(self.session_manager.count());
        }

        Ok(Response::new(GetHealthResponse {
            healthy,
            status: if healthy { "operational" } else { "degraded" }.to_string(),
            components_json: serde_json::to_string(&components)
                .unwrap_or_else(|_| "{}".to_string()),
            queries_remaining: remaining as i32,
            queries_limit: limit as i32,
            auth_status: crate::notebooklm::notebooklm_auth_status().to_string(),
        }))
    }

    // =========================================================================
    // R9 — SetupAuth
    // =========================================================================
    async fn setup_auth(
        &self,
        request: Request<SetupAuthRequest>,
    ) -> Result<Response<SetupAuthResponse>, Status> {
        let req = request.into_inner();
        info!(auth_method = %req.auth_method, "SetupAuth");

        if !matches!(req.auth_method.as_str(), "chrome_profile" | "cookie") {
            return Err(Status::invalid_argument(
                "auth_method must be chrome_profile or cookie",
            ));
        }

        if req.credential.is_empty() {
            return Err(Status::invalid_argument(
                "credential is required (path to Chrome profile or cookie value)",
            ));
        }

        // This service has no secret-store write path and no ability to reload
        // the sidecar. Returning success here previously claimed that a raw
        // credential had been persisted when it had only been discarded.
        // Authentication remains an operator-controlled runit secret.
        Err(Status::failed_precondition(
            "SetupAuth cannot persist credentials. Provision NotebookLM authentication through the operator-managed secret and restart path.",
        ))
    }

    // =========================================================================
    // R6 — RemoveSource
    // =========================================================================
    async fn remove_source(
        &self,
        request: Request<RemoveSourceRequest>,
    ) -> Result<Response<RemoveSourceResponse>, Status> {
        let context = Self::bridge_request_context(&request)?;
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            source_id = %req.source_id,
            "RemoveSource"
        );

        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;
        let receipt = self
            .audit_mutation(
                context,
                "remove_source",
                serde_json::json!({
                    "notebook_id": &req.notebook_id,
                    "source_id": &req.source_id,
                }),
            )
            .await?;

        let deleted = self
            .memory_store
            .delete_entry(&namespace, &req.source_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        if !deleted {
            return Err(Status::not_found(format!(
                "Source '{}' not found",
                req.source_id
            )));
        }

        Ok(Response::new(RemoveSourceResponse {
            success: true,
            audit_event_id: receipt.event_id,
            audit_event_hash: receipt.event_hash,
        }))
    }

    // =========================================================================
    // R14 — GetToolProfile
    // =========================================================================
    async fn get_tool_profile(
        &self,
        request: Request<GetToolProfileRequest>,
    ) -> Result<Response<GetToolProfileResponse>, Status> {
        let req = request.into_inner();
        info!("GetToolProfile");

        let profile = if req.profile_name.is_empty() {
            crate::tool_profiles::current_profile()
        } else {
            req.profile_name
                .parse()
                .unwrap_or(crate::tool_profiles::current_profile())
        };

        let resolved =
            crate::tool_profiles::resolve_live_profile(&self.tool_registry, profile).await;

        Ok(Response::new(GetToolProfileResponse {
            current_profile: profile.to_string(),
            tool_count: resolved.tool_count() as i32,
            schema_tokens: resolved.schema_tokens as i32,
            savings_percent: resolved.savings_percent as i32,
            tools: resolved.tools,
        }))
    }

    // =========================================================================
    // R15 — Doctor
    // =========================================================================
    async fn doctor(
        &self,
        _request: Request<DoctorRequest>,
    ) -> Result<Response<DoctorResponse>, Status> {
        info!("Doctor");

        let report = crate::doctor::run_diagnostics(
            &self.memory_store,
            &self.session_manager,
            &self.quota_manager,
            &self.tool_registry,
        )
        .await;

        let components_json =
            serde_json::to_string(&report.components).unwrap_or_else(|_| "[]".to_string());

        Ok(Response::new(DoctorResponse {
            overall_status: report.overall_status,
            timestamp: report.timestamp,
            components_json,
            recommendations: report.recommendations,
        }))
    }

    // =========================================================================
    // R15 — GetQueryHistory
    // =========================================================================
    async fn get_query_history(
        &self,
        request: Request<GetQueryHistoryRequest>,
    ) -> Result<Response<GetQueryHistoryResponse>, Status> {
        let req = request.into_inner();
        info!("GetQueryHistory");

        let limit = bounded_limit(req.limit, DEFAULT_HISTORY_PAGE_SIZE, MAX_HISTORY_PAGE_SIZE);

        let history = crate::doctor::get_query_history(&self.session_manager, limit);

        let entries: Vec<QueryHistoryEntry> = history
            .into_iter()
            .map(|v| QueryHistoryEntry {
                conversation_id: v["conversation_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                notebook_id: v["notebook_id"].as_str().unwrap_or_default().to_string(),
                query: v["query"].as_str().unwrap_or_default().to_string(),
                answer_preview: v["answer_preview"].as_str().unwrap_or_default().to_string(),
                timestamp: v["timestamp"].as_str().unwrap_or_default().to_string(),
                citations_count: v["citations_count"].as_i64().unwrap_or(0) as i32,
                grounded: v["grounded"].as_bool().unwrap_or(false),
            })
            // Filter by conversation_id if provided
            .filter(|e| req.conversation_id.is_empty() || e.conversation_id == req.conversation_id)
            .collect();
        let total = entries.len().min(i32::MAX as usize) as i32;

        Ok(Response::new(GetQueryHistoryResponse { entries, total }))
    }
}

fn memory_status(error: anyhow::Error) -> Status {
    Status::internal(error.to_string())
}

fn canonical_notebook_name(notebook_ref: &str) -> Result<String, Status> {
    let notebook_ref = notebook_ref.trim();
    if notebook_ref.is_empty() {
        return Err(Status::invalid_argument("notebook_id is required"));
    }
    require_at_most_bytes(notebook_ref, "notebook_id", MAX_NOTEBOOK_REF_BYTES)?;
    if explicit_namespace_kind(notebook_ref).is_some() {
        Ok(notebook_ref.to_string())
    } else {
        Ok(format!("project:{notebook_ref}"))
    }
}

fn require_query(query: &str) -> Result<(), Status> {
    crate::ingress::validate_query(query).map_err(Status::invalid_argument)
}

fn require_conversation_id(conversation_id: &str) -> Result<(), Status> {
    crate::ingress::validate_conversation_id(conversation_id).map_err(Status::invalid_argument)
}

fn require_at_most_bytes(value: &str, field: &str, limit: usize) -> Result<(), Status> {
    if value.len() > limit {
        return Err(Status::invalid_argument(format!(
            "{field} exceeds the {limit}-byte limit"
        )));
    }
    Ok(())
}

fn content_fingerprint(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn batch_request_fingerprint(requests: &[CreateNotebookRequest]) -> String {
    let mut hasher = Sha256::new();
    for request in requests {
        for value in [&request.title, &request.description, &request.kind] {
            hasher.update(value.as_bytes());
            // Length framing prevents distinct sequences from sharing an
            // ambiguous concatenated representation.
            hasher.update((value.len() as u64).to_be_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn validate_data_table_request(request: &GenerateDataTableRequest) -> Result<(), Status> {
    if request.prompt.trim().is_empty() {
        return Err(Status::invalid_argument("prompt is required"));
    }
    require_at_most_bytes(&request.prompt, "prompt", MAX_DATA_TABLE_PROMPT_BYTES)?;
    if request.columns.len() > MAX_DATA_TABLE_COLUMNS {
        return Err(Status::invalid_argument(format!(
            "columns exceeds the {MAX_DATA_TABLE_COLUMNS}-item limit"
        )));
    }
    let mut seen = std::collections::HashSet::new();
    for column in &request.columns {
        if column.trim().is_empty() {
            return Err(Status::invalid_argument(
                "columns must not contain empty values",
            ));
        }
        require_at_most_bytes(column, "column", MAX_DATA_TABLE_COLUMN_BYTES)?;
        if !seen.insert(column.as_str()) {
            return Err(Status::invalid_argument(
                "columns must not contain duplicates",
            ));
        }
    }
    Ok(())
}

fn build_data_table_prompt(
    request: &GenerateDataTableRequest,
    entries: &[MemoryEntry],
) -> Result<String, Status> {
    let mut remaining = MAX_DATA_TABLE_SOURCE_CONTEXT_BYTES;
    let mut sources = Vec::new();
    for entry in entries {
        if remaining == 0 {
            break;
        }
        let content = memory_entry_content(entry);
        let snippet = truncate_utf8(&content, remaining);
        if snippet.is_empty() {
            continue;
        }
        remaining = remaining.saturating_sub(snippet.len());
        sources.push(serde_json::json!({
            "source_id": entry.key,
            "title": entry.value.get("title").and_then(serde_json::Value::as_str),
            "content": snippet,
        }));
    }
    let source_json = serde_json::to_string(&sources)
        .map_err(|error| Status::internal(format!("serialize table sources: {error}")))?;
    let columns = if request.columns.is_empty() {
        "No fixed columns were requested; infer a compact, clearly named schema.".to_string()
    } else {
        format!("Use only these columns: {}.", request.columns.join(", "))
    };

    Ok(format!(
        "You are a structured data extraction route. Return only a valid JSON array, never Markdown, prose, or code fences. Each array item must be an object. Use only information present in the supplied sources; use null when a requested value is absent. Return at most {MAX_DATA_TABLE_ROWS} rows. {columns}\n\nExtraction request:\n{}\n\nSources:\n{}",
        request.prompt.trim(),
        source_json,
    ))
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn parse_data_table_output(
    content: &str,
    columns: &[String],
) -> Result<Vec<serde_json::Value>, Status> {
    let content = content.trim();
    if content.len() > MAX_DATA_TABLE_OUTPUT_BYTES {
        return Err(Status::resource_exhausted(format!(
            "provider-neutral model output exceeds the {MAX_DATA_TABLE_OUTPUT_BYTES}-byte table limit"
        )));
    }
    let content = content
        .strip_prefix("```json")
        .or_else(|| content.strip_prefix("```"))
        .map(str::trim_start)
        .and_then(|value| value.strip_suffix("```").map(str::trim_end))
        .unwrap_or(content);
    let value: serde_json::Value = serde_json::from_str(content).map_err(|error| {
        Status::failed_precondition(format!(
            "provider-neutral model did not return valid JSON for GenerateDataTable: {error}"
        ))
    })?;
    let rows = value.as_array().ok_or_else(|| {
        Status::failed_precondition(
            "provider-neutral model must return a JSON array for GenerateDataTable",
        )
    })?;
    if rows.len() > MAX_DATA_TABLE_ROWS {
        return Err(Status::failed_precondition(format!(
            "provider-neutral model returned more than {MAX_DATA_TABLE_ROWS} table rows"
        )));
    }

    let allowed: std::collections::HashSet<&str> = columns.iter().map(String::as_str).collect();
    for row in rows {
        let object = row.as_object().ok_or_else(|| {
            Status::failed_precondition("provider-neutral model returned a non-object table row")
        })?;
        if !allowed.is_empty() && object.keys().any(|key| !allowed.contains(key.as_str())) {
            return Err(Status::failed_precondition(
                "provider-neutral model returned a column outside the requested schema",
            ));
        }
    }
    Ok(rows.clone())
}

fn bounded_limit(requested: i32, default: usize, max: usize) -> usize {
    if requested > 0 {
        (requested as usize).min(max)
    } else {
        default
    }
}

fn nonnegative_offset(offset: i32) -> usize {
    offset.max(0) as usize
}

fn canonical_create_notebook(
    title: &str,
    requested_kind: NamespaceKind,
) -> Result<(String, NamespaceKind), Status> {
    let title = title.trim();
    if title.is_empty() {
        return Err(Status::invalid_argument("title is required"));
    }
    require_at_most_bytes(title, "title", MAX_NOTEBOOK_TITLE_BYTES)?;
    if let Some(kind) = explicit_namespace_kind(title) {
        Ok((title.to_string(), kind))
    } else {
        Ok((format!("{requested_kind}:{title}"), requested_kind))
    }
}

fn validate_source_request(request: &AddSourceRequest) -> Result<(), Status> {
    if !matches!(request.source_type.as_str(), "url" | "text" | "file") {
        return Err(Status::invalid_argument(
            "source_type must be url, text, or file",
        ));
    }
    if request.content.trim().is_empty() {
        return Err(Status::invalid_argument("content must not be empty"));
    }
    require_at_most_bytes(&request.content, "content", MAX_SOURCE_CONTENT_BYTES)?;
    require_at_most_bytes(&request.title, "title", MAX_SOURCE_TITLE_BYTES)?;
    if request.tags.len() > MAX_SOURCE_TAGS {
        return Err(Status::invalid_argument(format!(
            "tags exceeds the {MAX_SOURCE_TAGS}-item limit"
        )));
    }
    for tag in &request.tags {
        if tag.trim().is_empty() {
            return Err(Status::invalid_argument(
                "tags must not contain empty values",
            ));
        }
        require_at_most_bytes(tag, "tag", MAX_SOURCE_TAG_BYTES)?;
    }
    Ok(())
}

fn validate_folder_patterns(patterns: &[String]) -> Result<(), Status> {
    if patterns.len() > MAX_FOLDER_PATTERNS {
        return Err(Status::invalid_argument(format!(
            "patterns exceeds the {MAX_FOLDER_PATTERNS}-item limit"
        )));
    }
    for pattern in patterns {
        if pattern.trim().is_empty() {
            return Err(Status::invalid_argument(
                "patterns must not contain empty values",
            ));
        }
        require_at_most_bytes(pattern, "pattern", MAX_FOLDER_PATTERN_BYTES)?;
    }
    Ok(())
}

fn format_grounded_query(
    entries: &[MemoryEntry],
    query: &str,
    notebook_ref: &str,
) -> (String, Vec<Citation>, bool) {
    if entries.is_empty() {
        return (
            format!(
                "No grounded answer found for '{}' in notebook '{}'.",
                query, notebook_ref
            ),
            Vec::new(),
            false,
        );
    }

    let answer = entries
        .iter()
        .map(|entry| format!("[{}] {}", entry.key, memory_entry_content(entry)))
        .collect::<Vec<_>>()
        .join("\n\n");
    let citations = entries
        .iter()
        .map(|entry| Citation {
            text: entry.key.clone(),
            source: entry.namespace_id.clone(),
            page: String::new(),
        })
        .collect();
    (answer, citations, true)
}

fn memory_entry_content(entry: &MemoryEntry) -> String {
    entry
        .value
        .get("content")
        .and_then(serde_json::Value::as_str)
        .or_else(|| entry.value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| entry.value.to_string())
}

fn namespace_kind_from_name(name: &str) -> NamespaceKind {
    explicit_namespace_kind(name).unwrap_or(NamespaceKind::Project)
}

fn explicit_namespace_kind(name: &str) -> Option<NamespaceKind> {
    match name.split_once(':').map(|(prefix, _)| prefix) {
        Some("project") => Some(NamespaceKind::Project),
        Some("session") => Some(NamespaceKind::Session),
        Some("database") | Some("db") => Some(NamespaceKind::Database),
        Some("workflow") => Some(NamespaceKind::Workflow),
        Some("agent") => Some(NamespaceKind::Agent),
        Some("cron") => Some(NamespaceKind::Cron),
        Some("custom") => Some(NamespaceKind::Custom),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Filesystem helpers — no shell=True, pure Rust (R13)
// ---------------------------------------------------------------------------

/// Resolve an import folder inside one of the operator-configured roots.
///
/// `COGNITIVE_MCP_INGEST_ROOTS` uses the host path-list separator (`:` on
/// Linux). Leaving it unset disables filesystem ingest rather than granting
/// the gRPC caller access to every file readable by this process.
fn resolve_ingest_folder(folder_path: &str) -> Result<PathBuf, Status> {
    let configured = std::env::var_os("COGNITIVE_MCP_INGEST_ROOTS").ok_or_else(|| {
        Status::failed_precondition(
            "Filesystem ingest is disabled; configure COGNITIVE_MCP_INGEST_ROOTS with approved directories.",
        )
    })?;
    let roots: Vec<PathBuf> = std::env::split_paths(&configured)
        .filter_map(|root| root.canonicalize().ok())
        .filter(|root| root.is_dir())
        .collect();
    if roots.is_empty() {
        return Err(Status::failed_precondition(
            "Filesystem ingest is disabled because COGNITIVE_MCP_INGEST_ROOTS has no usable directories.",
        ));
    }

    let candidate = Path::new(folder_path)
        .canonicalize()
        .map_err(|_| Status::invalid_argument(format!("Folder '{folder_path}' does not exist")))?;
    if !candidate.is_dir() {
        return Err(Status::invalid_argument(format!(
            "Folder '{folder_path}' is not a directory"
        )));
    }
    if is_within_ingest_roots(&candidate, &roots) {
        return Ok(candidate);
    }

    Err(Status::permission_denied(
        "Folder is outside the configured filesystem ingest roots.",
    ))
}

fn is_within_ingest_roots(candidate: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| candidate.starts_with(root))
}

fn folder_source_key(relative_path: &str) -> String {
    format!(
        "file:{}",
        hex::encode(Sha256::digest(relative_path.as_bytes()))
    )
}

fn ingest_limit(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .as_deref()
        .and_then(parse_positive_usize)
        .unwrap_or(default)
}

fn ingest_limit_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .as_deref()
        .and_then(parse_positive_u64)
        .unwrap_or(default)
}

fn parse_positive_usize(value: &str) -> Option<usize> {
    value.parse::<usize>().ok().filter(|value| *value > 0)
}

fn parse_positive_u64(value: &str) -> Option<u64> {
    value.parse::<u64>().ok().filter(|value| *value > 0)
}

fn record_ingest_error(errors: &mut Vec<String>, error: String) {
    if errors.len() < MAX_INGEST_ERROR_REPORTS {
        errors.push(error);
    } else if errors.len() == MAX_INGEST_ERROR_REPORTS {
        errors.push("Further folder-import errors were omitted.".to_string());
    }
}

/// Walk a directory only until `limit` regular files have been discovered.
/// Symlinks are deliberately skipped so a link created after root validation
/// cannot escape the configured ingest root. Directory entries are ordered at
/// each level to keep the accepted prefix deterministic.
fn walkdir_bounded(path: &Path, recursive: bool, limit: usize) -> Vec<PathBuf> {
    let mut result = Vec::new();
    collect_files_bounded(path, recursive, limit, &mut result);
    result
}

fn collect_files_bounded(path: &Path, recursive: bool, limit: usize, result: &mut Vec<PathBuf>) {
    if result.len() >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if result.len() >= limit {
            return;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_file() {
            result.push(entry.path());
        } else if recursive && file_type.is_dir() {
            collect_files_bounded(&entry.path(), true, limit, result);
        }
    }
}

/// Simple glob matching — supports * and ? only.
/// Used for AddFolder pattern filtering without shell expansion.
fn glob_match(pattern: &str, name: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let mut previous = vec![false; name_chars.len() + 1];
    previous[0] = true;

    for token in pattern_chars {
        let mut current = vec![false; name_chars.len() + 1];
        if token == '*' {
            current[0] = previous[0];
        }
        for (index, character) in name_chars.iter().enumerate() {
            current[index + 1] = match token {
                '*' => previous[index + 1] || current[index],
                '?' => previous[index],
                literal => previous[index] && literal == *character,
            };
        }
        previous = current;
    }
    previous[name_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingress::{MAX_CONVERSATION_ID_BYTES, MAX_QUERY_BYTES};

    #[derive(Default)]
    struct RecordingMutationAuditor {
        calls: std::sync::Mutex<Vec<CognitiveMutationAuditRequest>>,
    }

    #[async_trait::async_trait]
    impl CognitiveMutationAuditor for RecordingMutationAuditor {
        async fn record_mutation(
            &self,
            request: CognitiveMutationAuditRequest,
        ) -> Result<CognitiveMutationAuditReceipt, Status> {
            let mut calls = self.calls.lock().expect("test auditor lock");
            calls.push(request);
            Ok(CognitiveMutationAuditReceipt {
                event_id: calls.len() as u64,
                event_hash: format!("test-event-{}", calls.len()),
            })
        }
    }

    fn admitted_mutation_request<T>(message: T) -> Request<T> {
        let mut request = Request::new(message);
        request.extensions_mut().insert(CognitiveRequestContext {
            actor_id: "test-session".to_string(),
            capability_id: "cognitive_mcp.invoke".to_string(),
        });
        request
    }

    fn admitted_read_request<T>(message: T) -> Request<T> {
        let mut request = Request::new(message);
        request.extensions_mut().insert(CognitiveRequestContext {
            actor_id: "test-session".to_string(),
            capability_id: "cognitive_mcp.read".to_string(),
        });
        request
    }

    struct RecordingModelRouter {
        content: String,
        calls: std::sync::Mutex<Vec<CognitiveModelRequest>>,
    }

    #[async_trait::async_trait]
    impl CognitiveModelRouter for RecordingModelRouter {
        async fn generate(
            &self,
            request: CognitiveModelRequest,
        ) -> Result<CognitiveModelResponse, Status> {
            self.calls.lock().expect("test model lock").push(request);
            Ok(CognitiveModelResponse {
                content: self.content.clone(),
                provider: "test-provider".to_string(),
                model: "test-model".to_string(),
                audit_event_id: 99,
                audit_event_hash: "model-event-99".to_string(),
            })
        }
    }

    async fn test_service() -> (
        CognitiveGrpcService,
        Arc<CognitiveMemoryStore>,
        Arc<RecordingMutationAuditor>,
    ) {
        let shuttle =
            Arc::new(crate::cozo_shuttle::CozoGraphShuttle::new_in_memory().expect("cozo"));
        let store = Arc::new(CognitiveMemoryStore::new(shuttle).await.expect("store"));
        let auditor = Arc::new(RecordingMutationAuditor::default());
        let service = CognitiveGrpcService::with_mutation_auditor(
            store.clone(),
            Arc::new(SessionManager::with_defaults()),
            Arc::new(QuotaManager::with_defaults()),
            Arc::new(ToolRegistry::new()),
            auditor.clone(),
        );
        (service, store, auditor)
    }

    #[test]
    fn should_glob_match_star() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(glob_match("*.rs", "lib.rs"));
        assert!(!glob_match("*.rs", "main.py"));
    }

    #[test]
    fn should_glob_match_question() {
        assert!(glob_match("?.rs", "a.rs"));
        assert!(!glob_match("?.rs", "ab.rs"));
    }

    #[test]
    fn should_glob_match_exact() {
        assert!(glob_match("main.rs", "main.rs"));
        assert!(!glob_match("main.rs", "lib.rs"));
    }

    #[test]
    fn glob_matching_stays_linear_for_many_wildcards() {
        let pattern = format!("{}b", "*a".repeat(64));
        let name = format!("{}b", "a".repeat(64));
        assert!(glob_match(&pattern, &name));
    }

    #[test]
    fn bounded_walk_stops_discovery_without_building_a_full_tree() {
        let root = std::env::temp_dir().join(format!("cognitive-walk-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("nested")).expect("create fixture");
        for file in ["a.txt", "b.txt", "nested/c.txt", "nested/d.txt"] {
            std::fs::write(root.join(file), "fixture").expect("write fixture");
        }

        let files = walkdir_bounded(&root, true, 2);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|file| file.starts_with(&root)));
        std::fs::remove_dir_all(&root).expect("remove fixture");
    }

    #[test]
    fn ingest_root_check_accepts_children_without_prefix_confusion() {
        let base = std::env::temp_dir().join(format!("cognitive-ingest-{}", Uuid::new_v4()));
        let allowed = base.join("approved");
        let child = allowed.join("nested/document.md");
        let lookalike = base.join("approved-elsewhere/document.md");

        assert!(is_within_ingest_roots(
            &child,
            std::slice::from_ref(&allowed)
        ));
        assert!(!is_within_ingest_roots(&lookalike, &[allowed]));
    }

    #[test]
    fn folder_source_keys_do_not_collide_for_matching_basenames() {
        assert_ne!(
            folder_source_key("src/lib.rs"),
            folder_source_key("tests/lib.rs")
        );
        assert_eq!(
            folder_source_key("src/lib.rs"),
            folder_source_key("src/lib.rs")
        );
    }

    #[test]
    fn ingest_limits_reject_zero_and_malformed_values() {
        // Parsing stays pure so service-level environment is read only at
        // runtime rather than mutated by parallel tests.
        assert_eq!(parse_positive_usize("20"), Some(20));
        assert_eq!(parse_positive_usize("0"), None);
        assert_eq!(parse_positive_usize("invalid"), None);
        assert_eq!(parse_positive_u64("1048576"), Some(1_048_576));
        assert_eq!(parse_positive_u64("0"), None);
    }

    #[test]
    fn query_and_page_ingress_limits_are_bounded_before_storage_access() {
        assert!(require_query("grounded question").is_ok());
        assert_eq!(
            require_query(&"q".repeat(MAX_QUERY_BYTES + 1))
                .expect_err("oversized query must fail")
                .code(),
            tonic::Code::InvalidArgument
        );
        assert_eq!(
            require_conversation_id(&"s".repeat(MAX_CONVERSATION_ID_BYTES + 1))
                .expect_err("oversized conversation id must fail")
                .code(),
            tonic::Code::InvalidArgument
        );
        assert_eq!(
            bounded_limit(0, DEFAULT_QUERY_RESULTS, MAX_QUERY_RESULTS),
            DEFAULT_QUERY_RESULTS
        );
        assert_eq!(
            bounded_limit(i32::MAX, DEFAULT_QUERY_RESULTS, MAX_QUERY_RESULTS),
            MAX_QUERY_RESULTS
        );
        assert_eq!(nonnegative_offset(-5), 0);
    }

    #[test]
    fn source_and_folder_mutations_require_bounded_structured_input() {
        let valid = AddSourceRequest {
            notebook_id: "project:3tched-cognative".to_string(),
            source_type: "text".to_string(),
            content: "operator-approved source".to_string(),
            title: "Source".to_string(),
            tags: vec!["cognitive".to_string()],
        };
        validate_source_request(&valid).expect("valid source request");

        let mut invalid_type = valid.clone();
        invalid_type.source_type = "shell".to_string();
        assert_eq!(
            validate_source_request(&invalid_type)
                .expect_err("unknown source type must fail")
                .code(),
            tonic::Code::InvalidArgument
        );

        let mut empty_tag = valid;
        empty_tag.tags = vec![" ".to_string()];
        assert!(validate_source_request(&empty_tag).is_err());
        assert!(validate_folder_patterns(&["*.rs".to_string(), "*.md".to_string()]).is_ok());
        assert!(validate_folder_patterns(&["".to_string()]).is_err());
    }

    #[test]
    fn grounded_queries_render_source_content_and_explain_empty_results() {
        let now = Utc::now();
        let entry = MemoryEntry {
            id: "entry-1".to_string(),
            namespace_id: "project:3tched-cognative".to_string(),
            key: "architecture".to_string(),
            value: serde_json::json!({
                "source_type": "text",
                "content": "Canonical ingress is the only question path."
            }),
            tags: vec![],
            created_at: now,
            updated_at: now,
            expires_at: None,
            access_count: 0,
            last_accessed: now,
        };

        let (answer, citations, grounded) =
            format_grounded_query(&[entry], "question path", "project:3tched-cognative");
        assert!(grounded);
        assert_eq!(
            answer,
            "[architecture] Canonical ingress is the only question path."
        );
        assert_eq!(citations.len(), 1);

        let (empty_answer, empty_citations, grounded) =
            format_grounded_query(&[], "missing", "project:3tched-cognative");
        assert!(!grounded);
        assert!(empty_answer.contains("No grounded answer"));
        assert!(empty_citations.is_empty());
    }

    #[test]
    fn canonical_create_preserves_explicit_namespace_and_requested_kind() {
        assert_eq!(
            canonical_create_notebook("project:3tched-cognative", NamespaceKind::Custom)
                .expect("canonical name"),
            (
                "project:3tched-cognative".to_string(),
                NamespaceKind::Project
            )
        );
        assert_eq!(
            canonical_create_notebook("agent-session", NamespaceKind::Agent)
                .expect("agent namespace"),
            ("agent:agent-session".to_string(), NamespaceKind::Agent)
        );
    }

    #[tokio::test]
    async fn grpc_resolves_notebook_uuid_and_canonical_name_without_double_prefixing() {
        let (service, store, _) = test_service().await;
        let namespace = store
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
                &namespace.name,
                "ingress",
                serde_json::json!({"content":"canonical ingress is unified"}),
                vec![],
                None,
            )
            .await
            .expect("entry");

        let by_id = service
            .ask_question(Request::new(AskQuestionRequest {
                notebook_id: namespace.id.clone(),
                query: "canonical ingress".to_string(),
                conversation_id: String::new(),
            }))
            .await
            .expect("query by UUID")
            .into_inner();
        assert!(by_id.grounded);
        assert!(by_id.answer.contains("canonical ingress is unified"));
        let context: serde_json::Value =
            serde_json::from_str(&by_id.context_json).expect("bounded context signals");
        assert_eq!(context["session_id"], by_id.conversation_id);
        assert_eq!(context["activity_count"], 1);
        assert!(context["current_topics"].is_array());
        let session = service
            .session_manager
            .get_session(&by_id.conversation_id)
            .expect("UUID query session");
        assert_eq!(session.notebook_id, "project:3tched-cognative");

        let by_name = service
            .ask_question(Request::new(AskQuestionRequest {
                notebook_id: namespace.name.clone(),
                query: "canonical ingress".to_string(),
                conversation_id: String::new(),
            }))
            .await
            .expect("query by canonical namespace")
            .into_inner();
        assert!(by_name.grounded);

        let notebook = service
            .get_notebook(Request::new(GetNotebookRequest {
                notebook_id: namespace.id,
            }))
            .await
            .expect("notebook by UUID")
            .into_inner()
            .notebook
            .expect("notebook payload");
        assert_eq!(notebook.name, "project:3tched-cognative");
    }

    #[tokio::test]
    async fn invalid_queries_do_not_consume_quota_or_create_sessions() {
        let (service, _, _) = test_service().await;
        let quota_before = service.quota_manager.status().await;

        let blank = service
            .ask_question(Request::new(AskQuestionRequest {
                notebook_id: "project:3tched-cognative".to_string(),
                query: " \t ".to_string(),
                conversation_id: String::new(),
            }))
            .await
            .expect_err("blank query must fail");
        assert_eq!(blank.code(), tonic::Code::InvalidArgument);
        assert_eq!(service.quota_manager.status().await, quota_before);
        assert_eq!(service.session_manager.count(), 0);

        let missing = service
            .query_notebook(Request::new(QueryNotebookRequest {
                notebook_id: "project:not-present".to_string(),
                query: "find this".to_string(),
                conversation_id: String::new(),
                max_results: 10,
            }))
            .await
            .expect_err("unknown notebook must fail");
        assert_eq!(missing.code(), tonic::Code::NotFound);
        assert_eq!(service.quota_manager.status().await, quota_before);
        assert_eq!(service.session_manager.count(), 0);
    }

    #[tokio::test]
    async fn deep_health_reports_checked_memory_state() {
        let (service, _, _) = test_service().await;

        let health = service
            .get_health(Request::new(GetHealthRequest { deep_check: true }))
            .await
            .expect("deep health")
            .into_inner();
        assert!(health.healthy);
        assert_eq!(health.status, "operational");
        let components: serde_json::Value =
            serde_json::from_str(&health.components_json).expect("health JSON");
        assert_eq!(components["memory_store"], "ok");
        assert!(components["memory_store_stats"].is_object());
    }

    #[tokio::test]
    async fn data_table_uses_the_injected_provider_neutral_route_and_validates_json() {
        let shuttle =
            Arc::new(crate::cozo_shuttle::CozoGraphShuttle::new_in_memory().expect("cozo"));
        let store = Arc::new(CognitiveMemoryStore::new(shuttle).await.expect("store"));
        let model = Arc::new(RecordingModelRouter {
            content: r#"[{"service":"Cognitive MCP","status":"active"}]"#.to_string(),
            calls: std::sync::Mutex::new(Vec::new()),
        });
        let service = CognitiveGrpcService::with_operational_adapters(
            store.clone(),
            Arc::new(SessionManager::with_defaults()),
            Arc::new(QuotaManager::with_defaults()),
            Arc::new(ToolRegistry::new()),
            Arc::new(ContextAwarenessEngine::new(
                ContextAwarenessConfig::default(),
                store.clone(),
                None,
            )),
            Arc::new(RecordingMutationAuditor::default()),
            model.clone(),
        );
        let namespace = store
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
                &namespace.name,
                "service-source",
                serde_json::json!({"title":"Service", "content":"Cognitive MCP is active."}),
                vec![],
                None,
            )
            .await
            .expect("source");

        let response = service
            .generate_data_table(admitted_read_request(GenerateDataTableRequest {
                notebook_id: namespace.name,
                prompt: "List the service and its current status.".to_string(),
                columns: vec!["service".to_string(), "status".to_string()],
            }))
            .await
            .expect("provider-neutral table generation")
            .into_inner();
        assert_eq!(response.row_count, 1);
        assert_eq!(response.model_provider, "test-provider");
        assert_eq!(response.model, "test-model");
        assert_eq!(response.audit_event_id, 99);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response.data_json)
                .expect("returned table JSON")[0]["status"],
            "active"
        );

        let calls = model.calls.lock().expect("test model lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].actor_id, "test-session");
        assert_eq!(calls[0].capability_id, "cognitive_mcp.read");
        assert!(calls[0].prompt.contains("Cognitive MCP is active."));
    }

    #[tokio::test]
    async fn source_rpc_ids_round_trip_through_list_get_and_remove() {
        let (service, _, auditor) = test_service().await;
        let notebook_id = "project:3tched-cognative".to_string();

        let added = service
            .add_source(admitted_mutation_request(AddSourceRequest {
                notebook_id: notebook_id.clone(),
                source_type: "text".to_string(),
                content: "Canonical ingress source content".to_string(),
                title: "Ingress design".to_string(),
                tags: vec!["cognitive".to_string()],
            }))
            .await
            .expect("add source")
            .into_inner();
        assert!(added.success);
        assert_eq!(added.audit_event_id, 1);
        assert_eq!(added.audit_event_hash, "test-event-1");

        service
            .add_source(admitted_mutation_request(AddSourceRequest {
                notebook_id: notebook_id.clone(),
                source_type: "text".to_string(),
                content: "Second source".to_string(),
                title: "Second source".to_string(),
                tags: vec![],
            }))
            .await
            .expect("add second source");

        let listed = service
            .list_sources(Request::new(ListSourcesRequest {
                notebook_id: notebook_id.clone(),
                limit: 1,
                offset: 0,
            }))
            .await
            .expect("list sources")
            .into_inner();
        assert_eq!(listed.total, 2, "total counts all sources, not page length");
        assert_eq!(listed.sources.len(), 1, "request page size is respected");

        let content = service
            .get_source_content(Request::new(GetSourceContentRequest {
                notebook_id: notebook_id.clone(),
                source_id: added.source_id.clone(),
            }))
            .await
            .expect("get source by returned id")
            .into_inner();
        assert_eq!(content.title, "Ingress design");
        assert_eq!(content.content, "Canonical ingress source content");

        let removed = service
            .remove_source(admitted_mutation_request(RemoveSourceRequest {
                notebook_id,
                source_id: added.source_id,
            }))
            .await
            .expect("remove source by returned id")
            .into_inner();
        assert!(removed.success);
        assert_eq!(removed.audit_event_id, 3);

        let calls = auditor.calls.lock().expect("test auditor lock");
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].actor_id, "test-session");
        assert_eq!(calls[0].operation, "add_source");
        assert_eq!(
            calls[0].arguments["content_sha256"],
            content_fingerprint("Canonical ingress source content")
        );
        assert!(
            calls[0]
                .arguments
                .to_string()
                .contains("Canonical ingress source content")
                == false
        );
    }

    #[tokio::test]
    async fn standalone_service_cannot_bypass_canonical_mutation_ingress() {
        let (_, store, _) = test_service().await;
        let service = CognitiveGrpcService::new(
            store.clone(),
            Arc::new(SessionManager::with_defaults()),
            Arc::new(QuotaManager::with_defaults()),
            Arc::new(ToolRegistry::new()),
        );

        let error = service
            .add_source(Request::new(AddSourceRequest {
                notebook_id: "project:3tched-cognative".to_string(),
                source_type: "text".to_string(),
                content: "must not be written".to_string(),
                title: "rejected source".to_string(),
                tags: vec![],
            }))
            .await
            .expect_err("standalone gRPC service must fail closed for writes");
        assert_eq!(error.code(), tonic::Code::FailedPrecondition);
        assert!(error.message().contains("canonical bridge ingress"));
        assert!(
            store
                .get_namespace_by_name("project:3tched-cognative")
                .await
                .expect("namespace query")
                .is_none(),
            "the rejected call must not create a namespace as a side effect"
        );
    }

    #[tokio::test]
    async fn setup_auth_rejects_credentials_that_the_service_cannot_persist() {
        let (service, _, _) = test_service().await;
        let error = service
            .setup_auth(Request::new(SetupAuthRequest {
                auth_method: "cookie".to_string(),
                credential: "not-a-real-cookie".to_string(),
            }))
            .await
            .expect_err("raw credentials must not be falsely accepted");
        assert_eq!(error.code(), tonic::Code::FailedPrecondition);
        assert!(error.message().contains("cannot persist credentials"));
    }
}
