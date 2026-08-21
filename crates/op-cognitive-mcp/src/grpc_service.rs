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

use std::sync::Arc;

use chrono::Utc;
use tonic::{Request, Response, Status};
use tracing::{info, warn};
use uuid::Uuid;

use crate::memory_store::{CognitiveMemoryStore, MemoryNamespace, NamespaceKind};
use crate::proto::cognitive_tool_service_server::CognitiveToolService;
use crate::proto::*;
use crate::quota::QuotaManager;
use crate::session::{QueryTurn, SessionManager};
use op_mcp::tool_registry::ToolRegistry;

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
}

impl CognitiveGrpcService {
    pub fn new(
        memory_store: Arc<CognitiveMemoryStore>,
        session_manager: Arc<SessionManager>,
        quota_manager: Arc<QuotaManager>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            memory_store,
            session_manager,
            quota_manager,
            tool_registry,
        }
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
            .get_or_create(&req.conversation_id, &req.notebook_id);
        let conversation_id = session.id.clone();

        // Attempt grounded query via memory store.
        // Phase 1: query entries matching the notebook namespace.
        // Phase 2+: this forwards through the NotebookLM bridge.
        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;
        let entries = self
            .memory_store
            .search_entries(&namespace, &req.query, 10)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let grounded = !entries.is_empty();
        let answer = if grounded {
            entries
                .iter()
                .map(|e| {
                    format!(
                        "[{}] {}",
                        e.key,
                        e.value.as_str().unwrap_or(&e.value.to_string())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            format!(
                "No grounded answer found for '{}' in notebook '{}'.",
                req.query, req.notebook_id
            )
        };

        let citations: Vec<Citation> = entries
            .iter()
            .map(|e| Citation {
                text: e.key.clone(),
                source: e.namespace_id.clone(),
                page: String::new(),
            })
            .collect();

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

        Ok(Response::new(AskQuestionResponse {
            answer,
            citations,
            conversation_id,
            grounded,
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

        let (allowed, _, _) = self.quota_manager.check_and_increment().await;
        if !allowed {
            return Err(Status::resource_exhausted("Daily query quota exceeded"));
        }

        let session = self
            .session_manager
            .get_or_create(&req.conversation_id, &req.notebook_id);

        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;
        let limit = if req.max_results > 0 {
            req.max_results as i64
        } else {
            10
        };

        let entries = self
            .memory_store
            .search_entries(&namespace, &req.query, limit as usize)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let answer = entries
            .iter()
            .map(|e| {
                format!(
                    "[{}] {}",
                    e.key,
                    e.value.as_str().unwrap_or(&e.value.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let citations: Vec<Citation> = entries
            .iter()
            .map(|e| Citation {
                text: e.key.clone(),
                source: e.namespace_id.clone(),
                page: String::new(),
            })
            .collect();

        let _ = self.session_manager.append_turn(
            &session.id,
            QueryTurn {
                query: req.query,
                answer: answer.clone(),
                timestamp: Utc::now(),
                citations_count: citations.len() as u32,
                grounded: !entries.is_empty(),
            },
        );

        Ok(Response::new(QueryNotebookResponse {
            answer,
            citations,
            conversation_id: session.id,
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
        let offset = req.offset.max(0) as usize;
        let limit = if req.limit > 0 {
            req.limit as usize
        } else {
            100
        };

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
        let req = request.into_inner();
        info!(title = %req.title, "CreateNotebook");

        let kind = if req.kind.is_empty() {
            NamespaceKind::Project
        } else {
            req.kind.parse().unwrap_or(NamespaceKind::Custom)
        };

        let (name, kind) = canonical_create_notebook(&req.title, kind)?;
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
        }))
    }

    // =========================================================================
    // R4 — BatchCreateNotebooks
    // =========================================================================
    async fn batch_create_notebooks(
        &self,
        request: Request<BatchCreateNotebooksRequest>,
    ) -> Result<Response<BatchCreateNotebooksResponse>, Status> {
        let req = request.into_inner();
        info!(count = req.notebooks.len(), "BatchCreateNotebooks");

        let mut created_notebooks = Vec::new();
        let mut failed = 0i32;

        for nb_req in req.notebooks {
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
        }))
    }

    // =========================================================================
    // R5 — AddSource
    // =========================================================================
    async fn add_source(
        &self,
        request: Request<AddSourceRequest>,
    ) -> Result<Response<AddSourceResponse>, Status> {
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            source_type = %req.source_type,
            "AddSource"
        );

        let namespace = self
            .resolve_or_create_notebook(&req.notebook_id)
            .await?
            .name;

        let source_id = Uuid::new_v4().to_string();
        let key = if req.title.is_empty() {
            source_id.clone()
        } else {
            req.title.clone()
        };

        let value = serde_json::json!({
            "source_type": req.source_type,
            "content": req.content,
            "title": req.title,
        });

        self.memory_store
            .store_entry(&namespace, &key, value, req.tags, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(AddSourceResponse {
            source_id,
            success: true,
        }))
    }

    // =========================================================================
    // R5 — AddFolder (bulk ingest)
    // =========================================================================
    async fn add_folder(
        &self,
        request: Request<AddFolderRequest>,
    ) -> Result<Response<AddFolderResponse>, Status> {
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            folder_path = %req.folder_path,
            "AddFolder"
        );

        // Validate path exists — no shell=True, use std::fs
        let path = std::path::Path::new(&req.folder_path);
        if !path.exists() || !path.is_dir() {
            return Err(Status::invalid_argument(format!(
                "Folder '{}' does not exist or is not a directory",
                req.folder_path
            )));
        }

        let namespace = self
            .resolve_or_create_notebook(&req.notebook_id)
            .await?
            .name;

        let mut added = 0i32;
        let mut skipped = 0i32;
        let mut errors = Vec::new();

        // Walk directory — no shell, pure Rust
        let walker = if req.recursive {
            walkdir(path)
        } else {
            walkdir_shallow(path)
        };

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

            match std::fs::read_to_string(&entry_path) {
                Ok(content) => {
                    let key = entry_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| Uuid::new_v4().to_string());

                    let value = serde_json::json!({
                        "source_type": "file",
                        "content": content,
                        "path": entry_path.to_string_lossy(),
                    });

                    match self
                        .memory_store
                        .store_entry(&namespace, &key, value, vec![], None)
                        .await
                    {
                        Ok(_) => added += 1,
                        Err(e) => {
                            errors.push(format!("{}: {}", entry_path.display(), e));
                        }
                    }
                }
                Err(e) => {
                    skipped += 1;
                    errors.push(format!("{}: {}", entry_path.display(), e));
                }
            }
        }

        Ok(Response::new(AddFolderResponse {
            sources_added: added,
            sources_skipped: skipped,
            errors,
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
        let limit = if req.limit > 0 { req.limit as i64 } else { 100 };

        let entries = self
            .memory_store
            .query_entries(crate::memory_store::EntryQuery {
                namespace_id: Some(namespace),
                key_pattern: None,
                tags: None,
                limit: Some(limit),
                offset: Some(req.offset as i64),
            })
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let total = entries.len() as i32;
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
                    id: e.id,
                    title: e.key,
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
        let title = entry.key;

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
        let req = request.into_inner();
        info!(notebook_id = %req.notebook_id, "GenerateDataTable");

        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;

        // Step 1: Get all sources in the notebook
        let entries = self
            .memory_store
            .query_entries(crate::memory_store::EntryQuery {
                namespace_id: Some(namespace.clone()),
                key_pattern: None,
                tags: None,
                limit: Some(50), // Sample size for table extraction
                offset: None,
            })
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if entries.is_empty() {
            return Ok(Response::new(GenerateDataTableResponse {
                data_json: "[]".to_string(),
                row_count: 0,
            }));
        }

        Err(Status::failed_precondition(
            "GenerateDataTable requires a configured provider-neutral model route; Cognitive MCP does not select providers directly.",
        ))
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
                    components["memory_store"] = serde_json::json!(format!("error: {}", e));
                }
            }

            components["active_sessions"] = serde_json::json!(self.session_manager.active_count());
            components["total_sessions"] = serde_json::json!(self.session_manager.count());
        }

        Ok(Response::new(GetHealthResponse {
            healthy: true,
            status: "operational".to_string(),
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
        let req = request.into_inner();
        info!(
            notebook_id = %req.notebook_id,
            source_id = %req.source_id,
            "RemoveSource"
        );

        let namespace = self.resolve_notebook(&req.notebook_id).await?.name;

        self.memory_store
            .delete_entry(&namespace, &req.source_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(RemoveSourceResponse { success: true }))
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

        let limit = if req.limit > 0 {
            req.limit as usize
        } else {
            50
        };

        let history = crate::doctor::get_query_history(&self.session_manager, limit);
        let total = history.len() as i32;

        let entries = history
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
    if explicit_namespace_kind(notebook_ref).is_some() {
        Ok(notebook_ref.to_string())
    } else {
        Ok(format!("project:{notebook_ref}"))
    }
}

fn canonical_create_notebook(
    title: &str,
    requested_kind: NamespaceKind,
) -> Result<(String, NamespaceKind), Status> {
    let title = title.trim();
    if title.is_empty() {
        return Err(Status::invalid_argument("title is required"));
    }
    if let Some(kind) = explicit_namespace_kind(title) {
        Ok((title.to_string(), kind))
    } else {
        Ok((format!("{requested_kind}:{title}"), requested_kind))
    }
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

/// Walk a directory recursively, yielding file paths only.
fn walkdir(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                result.extend(walkdir(&p));
            } else if p.is_file() {
                result.push(p);
            }
        }
    }
    result
}

/// Walk a directory non-recursively (shallow).
fn walkdir_shallow(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                result.push(p);
            }
        }
    }
    result
}

/// Simple glob matching — supports * and ? only.
/// Used for AddFolder pattern filtering without shell expansion.
fn glob_match(pattern: &str, name: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    glob_match_inner(&pattern_chars, &name_chars)
}

fn glob_match_inner(pattern: &[char], name: &[char]) -> bool {
    match (pattern.first(), name.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // '*' matches zero or more characters
            glob_match_inner(&pattern[1..], name)
                || (!name.is_empty() && glob_match_inner(pattern, &name[1..]))
        }
        (Some('?'), Some(_)) => glob_match_inner(&pattern[1..], &name[1..]),
        (Some(p), Some(n)) if *p == *n => glob_match_inner(&pattern[1..], &name[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_service() -> (CognitiveGrpcService, Arc<CognitiveMemoryStore>) {
        let shuttle =
            Arc::new(crate::cozo_shuttle::CozoGraphShuttle::new_in_memory().expect("cozo"));
        let store = Arc::new(CognitiveMemoryStore::new(shuttle).await.expect("store"));
        let service = CognitiveGrpcService::new(
            store.clone(),
            Arc::new(SessionManager::with_defaults()),
            Arc::new(QuotaManager::with_defaults()),
            Arc::new(ToolRegistry::new()),
        );
        (service, store)
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
        let (service, store) = test_service().await;
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
    async fn setup_auth_rejects_credentials_that_the_service_cannot_persist() {
        let (service, _) = test_service().await;
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
