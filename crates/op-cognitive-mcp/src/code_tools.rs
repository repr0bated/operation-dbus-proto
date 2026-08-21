//! Code-Context MCP Tools
//!
//! Coding-aware retrieval and indexing surfaces backed by the [`RagPipeline`]
//! (Voyage embeddings + Qdrant) and the [`ContextAwarenessEngine`]. These are
//! the tools an external coding client (Droid, Cursor, Codex) calls to pull
//! repository context.
//!
//! Tools:
//! - `code_search`  — filtered semantic search over indexed repos.
//!   subid: `obs.service.code-rag.search@v1`
//! - `code_context` — session-aware fused retrieval + awareness signals.
//!   subid: `exp.service.code-context.render@v1`
//! - `code_index`   — live indexing of a source file / repomix entry.
//!   subid: `src.software.workspace.index@v1`

use crate::context_awareness::{ActivityType, ContextAwarenessEngine};
use crate::ingress::validate_query;
use crate::rag_pipeline::{CodeFilter, RagPipeline, RagResult, RetrievalMode, RetrievalProfile};
use anyhow::Result;
use async_trait::async_trait;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolReadiness, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tracing::warn;

const RAG_INGEST_ROOTS_ENV: &str = "COGNITIVE_MCP_RAG_INGEST_ROOTS";
const RAG_WRITE_COLLECTIONS_ENV: &str = "COGNITIVE_MCP_RAG_WRITE_COLLECTIONS";
const RAG_MAX_SOURCE_BYTES_ENV: &str = "COGNITIVE_MCP_RAG_MAX_SOURCE_BYTES";
const DEFAULT_MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;

/// Register all code-context tools against the shared registry.
/// Returns the number of tools registered.
pub async fn register_code_tools(
    registry: &ToolRegistry,
    rag: Arc<RagPipeline>,
    engine: Arc<ContextAwarenessEngine>,
    collection: String,
) -> Result<usize> {
    registry
        .register(Arc::new(CodeSearchTool {
            rag: rag.clone(),
            collection: collection.clone(),
        }) as BoxedTool)
        .await?;
    registry
        .register(Arc::new(CodeContextTool {
            rag: rag.clone(),
            engine,
            collection: collection.clone(),
        }) as BoxedTool)
        .await?;
    registry
        .register(Arc::new(CodeIndexTool { rag, collection }) as BoxedTool)
        .await?;
    Ok(3)
}

/// Keep the Cognitive MCP catalog truthful when its RAG dependencies are not
/// configured.  The sealed plugin methods still exist, so silently omitting
/// their backing tools makes a client see a misleading "not found" error.
/// Disabled tools preserve discovery and explain exactly what must be restored.
pub async fn register_disabled_code_tools(
    registry: &ToolRegistry,
    reason: impl Into<String>,
) -> Result<usize> {
    let reason = reason.into();
    for (name, schema_field) in [
        ("code_search", "code_search"),
        ("code_context", "code_context"),
        ("code_index", "code_index"),
    ] {
        registry
            .register(Arc::new(DisabledCodeTool {
                name,
                schema_field,
                reason: reason.clone(),
            }) as BoxedTool)
            .await?;
    }
    Ok(3)
}

struct DisabledCodeTool {
    name: &'static str,
    schema_field: &'static str,
    reason: String,
}

#[async_trait]
impl Tool for DisabledCodeTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        "DISABLED: code-RAG dependency is unavailable. See readiness_reason for the required repair."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn tags(&self) -> Vec<String> {
        vec!["code".into(), "rag".into(), "disabled".into()]
    }

    fn readiness(&self) -> ToolReadiness {
        ToolReadiness::Disabled {
            reason: self.reason.clone(),
        }
    }

    fn input_schema(&self) -> Value {
        tool_input_schema(self.schema_field)
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        Err(anyhow::anyhow!(
            "{} is disabled: {}",
            self.name,
            self.reason
        ))
    }
}

// ─── code_search ────────────────────────────────────────────────────────────

pub struct CodeSearchTool {
    rag: Arc<RagPipeline>,
    collection: String,
}

#[async_trait]
impl Tool for CodeSearchTool {
    fn name(&self) -> &str {
        "code_search"
    }

    fn description(&self) -> &str {
        "Semantic code search over indexed repositories. Filter by repo, language, file_type, \
         path substring, or symbol. Returns ranked code chunks with symbols, imports, and line spans."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn tags(&self) -> Vec<String> {
        vec!["code".into(), "search".into(), "rag".into()]
    }

    fn input_schema(&self) -> Value {
        tool_input_schema("code_search")
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let query = required_text(&input, "query")?;
        validate_query(query).map_err(anyhow::Error::msg)?;
        let profile = retrieval_profile_from(&input, RetrievalMode::Search);
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(profile.limit)
            .clamp(1, 50);
        let filter = filter_from(&input, false);
        let collections = collections_from(&input, &self.collection, &profile)?;

        let results = if collections.len() > 1 {
            self.rag
                .query_fused_collections(&collections, query, limit, profile.fetch_limit, &filter)
                .await?
        } else if input.get("fused").and_then(Value::as_bool).unwrap_or(true) {
            self.rag
                .query_fused(&collections[0], query, limit, &filter)
                .await?
        } else {
            self.rag
                .query_filtered(&collections[0], query, limit, &filter)
                .await?
        };

        Ok(json!({
            "count": results.len(),
            "collections": collections,
            "retrieval_mode": profile.mode.as_str(),
            "rerank_enabled": profile.rerank_enabled,
            "kiro_lsp_state_dir": profile.kiro_lsp_state_dir,
            "results": results_to_simd(&results),
        }))
    }
}

// ─── code_context ───────────────────────────────────────────────────────────

pub struct CodeContextTool {
    rag: Arc<RagPipeline>,
    engine: Arc<ContextAwarenessEngine>,
    collection: String,
}

#[async_trait]
impl Tool for CodeContextTool {
    fn name(&self) -> &str {
        "code_context"
    }

    fn description(&self) -> &str {
        "Session-aware coding context. Records the current activity (query, edit, build_error, \
         test_failure, etc.) into the awareness engine, runs fused retrieval, and returns relevant \
         code plus session signals (stuck, recent errors, topics)."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn tags(&self) -> Vec<String> {
        vec!["code".into(), "context".into(), "session".into()]
    }

    fn input_schema(&self) -> Value {
        tool_input_schema("code_context")
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let query = required_text(&input, "query")?;
        validate_query(query).map_err(anyhow::Error::msg)?;
        let session_id = input
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("default");
        let activity_type = input
            .get("activity_type")
            .and_then(Value::as_str)
            .map(ActivityType::parse)
            .unwrap_or(ActivityType::Query);
        let profile = retrieval_profile_from(&input, RetrievalMode::Completion);
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(profile.limit)
            .clamp(1, 50);

        // Record the activity so stuck/error/topic detection stays current.
        self.engine
            .record_activity(
                session_id,
                activity_type,
                query.to_string(),
                serde_json::json!({ "source": "code_context" }),
            )
            .await;

        let filter = filter_from(&input, false);
        let collections = collections_from(&input, &self.collection, &profile)?;
        let (results, retrieval_error) = match self
            .rag
            .query_fused_collections(&collections, query, limit, profile.fetch_limit, &filter)
            .await
        {
            Ok(results) => (results, None),
            Err(err) => {
                warn!(
                    error = %err,
                    "Code-context vector retrieval unavailable; returning awareness signals only"
                );
                (Vec::new(), Some(err.to_string()))
            }
        };

        let signals = self
            .engine
            .get_session_signals(session_id)
            .await
            .unwrap_or_else(|| serde_json::json!({ "session_id": session_id }));

        Ok(json!({
            "count": results.len(),
            "collections": collections,
            "retrieval_mode": profile.mode.as_str(),
            "rerank_enabled": profile.rerank_enabled,
            "kiro_lsp_state_dir": profile.kiro_lsp_state_dir,
            "session_id": session_id,
            "signals": serde_to_simd(&signals),
            "results": results_to_simd(&results),
            "retrieval_error": retrieval_error,
        }))
    }
}

// ─── code_index ─────────────────────────────────────────────────────────────

pub struct CodeIndexTool {
    rag: Arc<RagPipeline>,
    collection: String,
}

#[async_trait]
impl Tool for CodeIndexTool {
    fn name(&self) -> &str {
        "code_index"
    }

    fn description(&self) -> &str {
        "Index code into the semantic store. mode='source' indexes a single in-memory file \
         (repo, file_path, content) for live workspace updates; mode='repomix_zip' ingests an \
         entry from a repomix zip (zip_path, entry)."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn tags(&self) -> Vec<String> {
        vec!["code".into(), "index".into(), "ingest".into()]
    }

    fn input_schema(&self) -> Value {
        tool_input_schema("code_index")
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let collection = writable_collection_from(&input, &self.collection)?;
        let mode = input
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("source");

        let stats = match mode {
            "source" => {
                let repo = required_text(&input, "repo")?;
                let file_path = required_text(&input, "file_path")?;
                let content = required_text(&input, "content")?;
                require_relative_file_path(file_path)?;
                require_at_most_bytes(
                    content,
                    "content",
                    configured_positive_usize(RAG_MAX_SOURCE_BYTES_ENV, DEFAULT_MAX_SOURCE_BYTES),
                )?;
                self.rag
                    .ingest_source_text(repo, file_path, content, &collection)
                    .await?
            }
            "repomix_zip" => {
                let zip_path = required_text(&input, "zip_path")?;
                let entry = required_text(&input, "entry")?;
                let zip_path = resolve_repomix_zip(zip_path)?;
                self.rag
                    .ingest_repomix_entry(&zip_path, entry, &collection)
                    .await?
            }
            other => return Err(anyhow::anyhow!("unknown mode: {other}")),
        };

        Ok(json!({
            "ok": true,
            "mode": mode,
            "collection": collection,
            "files_parsed": stats.files_parsed as u64,
            "chunks_created": stats.chunks_created as u64,
            "chunks_upserted": stats.chunks_upserted as u64,
            "chunks_skipped": stats.chunks_skipped as u64,
            "errors": stats.errors as u64,
        }))
    }
}

// ─── helpers ──────────────────────────────────────────────────────────────────

/// Derive a tool's JSON input schema from the single canonical authority
/// (`op_plugins::cognitive_mcp_plugin_schema`). The tool never declares its
/// schema inline — the OSCAL subid and field contract live in the PluginSchema.
fn tool_input_schema(field: &str) -> Value {
    op_plugins::cognitive_mcp_plugin_schema()
        .field_input_schema(field)
        .unwrap_or_else(|| json!({ "type": "object", "properties": {} }))
}

fn required_text<'a>(input: &'a Value, field: &str) -> Result<&'a str> {
    let value = input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing {field}"))?;
    if value.trim().is_empty() {
        anyhow::bail!("{field} must not be empty");
    }
    Ok(value)
}

/// Source buffers are sent to an embedding provider. Bound them before any
/// provider work so a malformed client cannot turn one index request into an
/// unbounded bill or payload.
fn require_at_most_bytes(value: &str, field: &str, limit: usize) -> Result<()> {
    if value.len() > limit {
        anyhow::bail!("{field} exceeds the configured {limit}-byte limit");
    }
    Ok(())
}

/// `file_path` is stored as repository-relative code metadata. It is never a
/// host path for source-mode indexing, so reject absolute and traversing paths
/// rather than preserving misleading or unsafe identifiers in the corpus.
fn require_relative_file_path(file_path: &str) -> Result<()> {
    let path = Path::new(file_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        anyhow::bail!("file_path must be a relative path within the repo");
    }
    Ok(())
}

/// Filesystem-backed zip ingest is opt-in. Source-mode indexing remains
/// available for an editor's in-memory buffer, but a model cannot use the MCP
/// process as a general host-file reader.
fn resolve_repomix_zip(zip_path: &str) -> Result<PathBuf> {
    let configured = std::env::var_os(RAG_INGEST_ROOTS_ENV).ok_or_else(|| {
        anyhow::anyhow!(
            "repomix zip ingest is disabled; configure {RAG_INGEST_ROOTS_ENV} with approved directories"
        )
    })?;
    let roots: Vec<PathBuf> = std::env::split_paths(&configured)
        .filter_map(|root| root.canonicalize().ok())
        .filter(|root| root.is_dir())
        .collect();
    if roots.is_empty() {
        anyhow::bail!(
            "repomix zip ingest is disabled because {RAG_INGEST_ROOTS_ENV} has no usable directories"
        );
    }

    let candidate = Path::new(zip_path)
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("zip_path does not exist"))?;
    if !candidate.is_file() {
        anyhow::bail!("zip_path is not a regular file");
    }
    if roots.iter().any(|root| candidate.starts_with(root)) {
        return Ok(candidate);
    }

    anyhow::bail!("zip_path is outside the configured repomix ingest roots")
}

fn configured_positive_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn filter_from(input: &Value, default_exclude_tests: bool) -> CodeFilter {
    CodeFilter {
        repo: input.get("repo").and_then(Value::as_str).map(String::from),
        language: input
            .get("language")
            .and_then(Value::as_str)
            .map(String::from),
        file_type: input
            .get("file_type")
            .and_then(Value::as_str)
            .map(String::from),
        path_contains: input
            .get("path_contains")
            .and_then(Value::as_str)
            .map(String::from),
        symbol_contains: input
            .get("symbol_contains")
            .and_then(Value::as_str)
            .map(String::from),
        exclude_tests: input
            .get("exclude_tests")
            .and_then(Value::as_bool)
            .unwrap_or(default_exclude_tests),
    }
}

fn retrieval_profile_from(input: &Value, default_mode: RetrievalMode) -> RetrievalProfile {
    let mode = input
        .get("retrieval_mode")
        .or_else(|| input.get("mode"))
        .and_then(Value::as_str)
        .map(RetrievalMode::parse)
        .unwrap_or(default_mode);
    RetrievalProfile::from_env(mode)
}

fn collections_from(
    input: &Value,
    fallback: &str,
    profile: &RetrievalProfile,
) -> Result<Vec<String>> {
    let allowed = configured_read_collections(fallback, profile);
    if let Some(collection) = input.get("collection").and_then(Value::as_str) {
        return validate_collections([collection], &allowed);
    }

    if let Some(collections) = input.get("collections").and_then(Value::as_array) {
        let parsed: Vec<&str> = collections.iter().filter_map(Value::as_str).collect();
        if !parsed.is_empty() {
            return validate_collections(parsed, &allowed);
        }
    }

    if !profile.collections.is_empty() {
        return Ok(profile.collections.clone());
    }

    Ok(vec![fallback.to_string()])
}

fn writable_collection_from(input: &Value, fallback: &str) -> Result<String> {
    let allowed = std::env::var(RAG_WRITE_COLLECTIONS_ENV)
        .ok()
        .map(|value| split_collection_list(&value))
        .filter(|collections| !collections.is_empty())
        .unwrap_or_else(|| vec![fallback.to_string()]);
    let requested = input
        .get("collection")
        .and_then(Value::as_str)
        .unwrap_or(fallback);
    Ok(validate_collections([requested], &allowed)?.remove(0))
}

fn configured_read_collections(fallback: &str, profile: &RetrievalProfile) -> Vec<String> {
    let mut allowed = profile.collections.clone();
    if !allowed.iter().any(|collection| collection == fallback) {
        allowed.push(fallback.to_string());
    }
    allowed
}

fn validate_collections<'a>(
    requested: impl IntoIterator<Item = &'a str>,
    allowed: &[String],
) -> Result<Vec<String>> {
    let mut selected = Vec::new();
    for collection in requested {
        let collection = collection.trim();
        if collection.is_empty() {
            anyhow::bail!("collection must not be empty");
        }
        if !allowed.iter().any(|configured| configured == collection) {
            anyhow::bail!("collection '{collection}' is not configured for this tool");
        }
        if !selected.iter().any(|selected| selected == collection) {
            selected.push(collection.to_string());
        }
    }
    if selected.is_empty() {
        anyhow::bail!("at least one collection is required");
    }
    Ok(selected)
}

fn split_collection_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|collection| !collection.is_empty())
        .map(String::from)
        .collect()
}

fn results_to_simd(results: &[RagResult]) -> Value {
    let serde_val = serde_json::to_value(results).unwrap_or(serde_json::Value::Null);
    serde_to_simd(&serde_val)
}

fn serde_to_simd(v: &serde_json::Value) -> Value {
    let s = serde_json::to_string(v).unwrap_or_default();
    let mut buf = s.into_bytes();
    simd_json::from_slice(&mut buf).unwrap_or(Value::Static(simd_json::StaticNode::Null))
}

#[cfg(test)]
mod tests {
    use super::{
        configured_read_collections, register_disabled_code_tools, require_at_most_bytes,
        require_relative_file_path, required_text, validate_collections,
    };
    use crate::rag_pipeline::{RetrievalMode, RetrievalProfile};
    use op_mcp::tool_registry::ToolRegistry;
    use simd_json::json;

    #[test]
    fn required_text_rejects_blank_values() {
        assert_eq!(
            required_text(&json!({ "query": "find ingress" }), "query").expect("query"),
            "find ingress"
        );
        assert!(required_text(&json!({ "query": " \t " }), "query")
            .expect_err("blank text must fail")
            .to_string()
            .contains("query must not be empty"));
        assert!(required_text(&json!({}), "query")
            .expect_err("missing text must fail")
            .to_string()
            .contains("missing query"));
    }

    #[test]
    fn source_indexing_requires_bounded_repo_relative_paths() {
        require_relative_file_path("crates/op-cognitive-mcp/src/code_tools.rs")
            .expect("relative source path");
        assert!(require_relative_file_path("/etc/shadow")
            .expect_err("absolute paths must fail")
            .to_string()
            .contains("relative path"));
        assert!(require_relative_file_path("../secret.rs")
            .expect_err("traversal must fail")
            .to_string()
            .contains("relative path"));
        assert!(require_at_most_bytes("12345", "content", 4)
            .expect_err("oversize source must fail")
            .to_string()
            .contains("4-byte limit"));
    }

    #[test]
    fn collection_overrides_stay_within_the_operator_profile() {
        let profile = RetrievalProfile {
            mode: RetrievalMode::Search,
            collections: vec!["configured-a".to_string(), "configured-b".to_string()],
            limit: 8,
            fetch_limit: 16,
            rerank_enabled: false,
            kiro_lsp_state_dir: String::new(),
        };
        let allowed = configured_read_collections("default", &profile);
        assert_eq!(
            validate_collections(["configured-b", "configured-b"], &allowed)
                .expect("configured collection"),
            vec!["configured-b"]
        );
        assert!(validate_collections(["foreign-collection"], &allowed)
            .expect_err("unconfigured collection must fail")
            .to_string()
            .contains("not configured"));
    }

    #[tokio::test]
    async fn unavailable_rag_tools_are_explicitly_disabled_and_discoverable() {
        let registry = ToolRegistry::new();
        register_disabled_code_tools(&registry, "VOYAGE_API_KEY is not configured")
            .await
            .expect("register disabled code tools");

        let catalog = registry.catalog(0, 10, Some("code")).await;
        assert_eq!(catalog.len(), 3);
        for entry in catalog {
            assert_eq!(entry.readiness.status(), "disabled");
            assert!(entry
                .readiness
                .reason()
                .is_some_and(|reason| reason.contains("VOYAGE_API_KEY")));
        }
    }
}
