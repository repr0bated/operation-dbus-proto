//! Cognitive MCP state plugin
//!
//! Tracks and manages the op-cognitive-mcp server: bind addresses, WireGuard
//! identity, tool registrations, and gRPC/HTTP health.  Publishes live state
//! to D-Bus under `/opdbus/v1/plugins/cognitive_mcp` so that
//! `register_plugin_projection_tools` can expose it as MCP tools.
//!
//! The canonical schema (every gRPC method, every MCP tool, every
//! request/response field) lives in the `cognitive_mcp_schema()` function below.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::PluginSchema;
#[cfg(test)]
use op_state_store::{Constraint, FieldSchema, FieldType};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use simd_json::json;
use simd_json::{prelude::*, OwnedValue as Value};
#[cfg(test)]
use std::collections::HashMap;

const S6_SV_PATH: &str = "/run/service/op-cognitive-mcp";
const ENV_DIR: &str = "/etc/s6/sv/op-cognitive-mcp/env";
const RUNTIME_ENV_DIR: &str = "/run/service/op-cognitive-mcp/env";
const DEFAULT_HTTP: &str = "100.90.37.254:3003";
const DEFAULT_GRPC: &str = "100.90.37.254:50052";
const DEFAULT_DB: &str = "/var/lib/op-dbus/cognitive.db";
const DEFAULT_WG: &str = "netmaker";

// ── Deployment config (tunable via env-dir / apply_state) ──────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitiveMcpConfig {
    #[serde(default = "default_http")]
    pub http: String,
    #[serde(default = "default_grpc")]
    pub grpc: String,
    #[serde(default = "default_db")]
    pub db_path: String,
    #[serde(default = "default_wg")]
    pub wg_interface: String,
    #[serde(default = "default_true")]
    pub http_enabled: bool,
    #[serde(default = "default_true")]
    pub grpc_enabled: bool,
    #[serde(default = "default_true")]
    pub dbus_enabled: bool,
}

fn default_http() -> String {
    DEFAULT_HTTP.into()
}
fn default_grpc() -> String {
    DEFAULT_GRPC.into()
}
fn default_db() -> String {
    DEFAULT_DB.into()
}
fn default_wg() -> String {
    DEFAULT_WG.into()
}
fn default_true() -> bool {
    true
}

impl Default for CognitiveMcpConfig {
    fn default() -> Self {
        Self {
            http: default_http(),
            grpc: default_grpc(),
            db_path: default_db(),
            wg_interface: default_wg(),
            http_enabled: true,
            grpc_enabled: true,
            dbus_enabled: true,
        }
    }
}

// ── Plugin struct + service helpers ─────────────────────────────────────────

pub struct CognitiveMcpPlugin;

impl CognitiveMcpPlugin {
    pub fn new() -> Self {
        Self
    }

    fn service_running() -> bool {
        let sv = std::path::Path::new(S6_SV_PATH);
        sv.exists() && !sv.join("down").exists()
    }

    fn read_env(key: &str) -> Option<String> {
        std::fs::read_to_string(format!("{ENV_DIR}/{key}"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn current_config() -> CognitiveMcpConfig {
        CognitiveMcpConfig {
            http: Self::read_env("COGNITIVE_MCP_BIND").unwrap_or_else(default_http),
            grpc: Self::read_env("COGNITIVE_MCP_GRPC_BIND").unwrap_or_else(default_grpc),
            db_path: Self::read_env("COGNITIVE_MCP_DB_PATH").unwrap_or_else(default_db),
            wg_interface: Self::read_env("WG_INTERFACE").unwrap_or_else(default_wg),
            http_enabled: Self::read_env("COGNITIVE_MCP_HTTP_DISABLED").is_none(),
            grpc_enabled: Self::read_env("COGNITIVE_MCP_GRPC_DISABLED").is_none(),
            dbus_enabled: Self::read_env("COGNITIVE_MCP_DBUS_DISABLED").is_none(),
        }
    }

    async fn write_env(key: &str, value: &str) -> Result<()> {
        tokio::fs::create_dir_all(ENV_DIR)
            .await
            .context("create env dir")?;
        tokio::fs::write(format!("{ENV_DIR}/{key}"), value)
            .await
            .with_context(|| format!("write env {key}"))?;
        if let Ok(()) = tokio::fs::create_dir_all(RUNTIME_ENV_DIR).await {
            let _ = tokio::fs::write(format!("{RUNTIME_ENV_DIR}/{key}"), value).await;
        }
        Ok(())
    }

    async fn reload_service() -> Result<()> {
        // D-Bus only per AGENTS.md §4 - no subprocess fallbacks
        Self::reload_service_dbus().await
    }

    async fn reload_service_dbus() -> Result<()> {
        let conn = zbus::Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        let reply = conn
            .call_method(
                Some("opdbus.v1"),
                "/opdbus/v1/s6/systemctl",
                Some("opdbus.v1.S6.Systemctl"),
                "reload",
                &("op-cognitive-mcp",),
            )
            .await
            .context("Failed to call reload on s6-systemctl D-Bus service")?;

        let (success, message): (bool, String) = reply.body().deserialize().map_err(|e| {
            anyhow::anyhow!("Failed to deserialize s6-systemctl reload response: {}", e)
        })?;

        if success {
            tracing::info!("Reloaded op-cognitive-mcp via D-Bus: {}", message);
            Ok(())
        } else {
            Err(anyhow::anyhow!("s6-systemctl reload failed: {}", message))
        }
    }
}

impl Default for CognitiveMcpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// ── StatePlugin impl ─────────────────────────────────────────────────────────

#[async_trait]
impl StatePlugin for CognitiveMcpPlugin {
    fn name(&self) -> &str {
        "cognitive_mcp"
    }
    fn version(&self) -> &str {
        "2.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = cognitive_mcp_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/etc/s6/sv/op-cognitive-mcp").exists()
    }

    fn unavailable_reason(&self) -> String {
        "op-cognitive-mcp s6 service definition not found at /etc/s6/sv/op-cognitive-mcp".into()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut cfg = simd_json::serde::to_owned_value(Self::current_config())?;
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert(
                "running".into(),
                simd_json::OwnedValue::from(Self::service_running()),
            );
        }
        Ok(cfg)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_cfg: CognitiveMcpConfig = simd_json::serde::from_owned_value(current.clone())?;
        let desired_cfg: CognitiveMcpConfig = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();
        macro_rules! field_diff {
            ($field:ident, $key:expr) => {
                if current_cfg.$field != desired_cfg.$field {
                    actions.push(StateAction::Modify {
                        resource: $key.into(),
                        changes: simd_json::serde::to_owned_value(&desired_cfg.$field)?,
                    });
                }
            };
        }
        field_diff!(http, "http");
        field_diff!(grpc, "grpc");
        field_diff!(db_path, "db_path");
        field_diff!(wg_interface, "wg_interface");
        field_diff!(http_enabled, "http_enabled");
        field_diff!(grpc_enabled, "grpc_enabled");
        field_diff!(dbus_enabled, "dbus_enabled");

        Ok(StateDiff {
            plugin: self.name().into(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes = Vec::new();
        let mut errors = Vec::new();
        let mut needs_reload = false;

        for action in &diff.actions {
            if let StateAction::Modify {
                resource,
                changes: val,
            } = action
            {
                let result: Result<()> = match resource.as_str() {
                    "http" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_BIND", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("http must be a string"))
                        }
                    }
                    "grpc" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_GRPC_BIND", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("grpc must be a string"))
                        }
                    }
                    "db_path" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("COGNITIVE_MCP_DB_PATH", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("db_path must be a string"))
                        }
                    }
                    "wg_interface" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("WG_INTERFACE", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("wg_interface must be a string"))
                        }
                    }
                    "http_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_HTTP_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(format!(
                                "{ENV_DIR}/COGNITIVE_MCP_HTTP_DISABLED"
                            ))
                            .await;
                        }
                        needs_reload = true;
                        Ok(())
                    }
                    "grpc_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_GRPC_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(format!(
                                "{ENV_DIR}/COGNITIVE_MCP_GRPC_DISABLED"
                            ))
                            .await;
                        }
                        needs_reload = true;
                        Ok(())
                    }
                    "dbus_enabled" => {
                        if val.as_bool() == Some(false) {
                            Self::write_env("COGNITIVE_MCP_DBUS_DISABLED", "1").await?;
                        } else {
                            let _ = tokio::fs::remove_file(format!(
                                "{ENV_DIR}/COGNITIVE_MCP_DBUS_DISABLED"
                            ))
                            .await;
                        }
                        needs_reload = true;
                        Ok(())
                    }
                    other => Err(anyhow::anyhow!("unknown cognitive_mcp field: {other}")),
                };
                match result {
                    Ok(()) => changes.push(format!("cognitive_mcp.{resource} updated")),
                    Err(e) => errors.push(format!("cognitive_mcp.{resource}: {e}")),
                }
            }
        }

        if needs_reload && errors.is_empty() {
            if let Err(e) = Self::reload_service().await {
                tracing::warn!("cognitive_mcp reload: {e}");
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let cur: CognitiveMcpConfig = simd_json::serde::from_owned_value(current)?;
        let des: CognitiveMcpConfig = simd_json::serde::from_owned_value(desired.clone())?;
        Ok(cur == des)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("cognitive_mcp-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old: CognitiveMcpConfig =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        let desired = simd_json::serde::to_owned_value(&old)?;
        let current = self.query_current_state().await?;
        let diff = self.calculate_diff(&current, &desired).await?;
        self.apply_state(&diff).await?;
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

// ── Schema-derived state for cognitive_mcp ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    None,
    ChromeProfile,
    Cookie,
    ApiKey,
}

impl Default for AuthStatus {
    fn default() -> Self {
        Self::None
    }
}

fn default_auth_status() -> AuthStatus {
    AuthStatus::default()
}

fn example_auth_status() -> AuthStatus {
    AuthStatus::ChromeProfile
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GeminiMode {
    Query,
    DeepResearch,
}

impl Default for GeminiMode {
    fn default() -> Self {
        Self::Query
    }
}

fn default_gemini_mode() -> GeminiMode {
    GeminiMode::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOperation {
    Store,
    Retrieve,
    Query,
    Delete,
    ListNamespaces,
    Stats,
}

fn example_memory_operation() -> MemoryOperation {
    MemoryOperation::Store
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceKind {
    Project,
    Session,
    Database,
    Workflow,
    Agent,
    Cron,
    Custom,
}

fn example_memory_namespace_kind() -> Option<NamespaceKind> {
    Some(NamespaceKind::Project)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Url,
    Text,
    File,
}

fn example_source_type() -> SourceType {
    SourceType::Url
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    Source,
    Test,
    Config,
    Docs,
    Build,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    ToolCall,
    Query,
    ContextSwitch,
    Error,
    Idle,
    ReturnFromIdle,
    FileOpened,
    EditApplied,
    BuildError,
    TestFailure,
    DiffViewed,
    SymbolNavigated,
}

impl Default for ActivityType {
    fn default() -> Self {
        Self::Query
    }
}

fn default_activity_type() -> ActivityType {
    ActivityType::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CodeIndexMode {
    Source,
    RepomixZip,
}

impl Default for CodeIndexMode {
    fn default() -> Self {
        Self::Source
    }
}

fn default_code_index_mode() -> CodeIndexMode {
    CodeIndexMode::default()
}

// Defaults matching the hand-rolled schema contract
fn default_cognitive_http() -> String {
    "0.0.0.0:3003".to_string()
}

fn default_cognitive_grpc() -> String {
    "0.0.0.0:50052".to_string()
}

fn default_running() -> bool {
    false
}

fn default_healthy() -> bool {
    false
}

fn default_queries_remaining() -> i64 {
    0
}

fn default_queries_limit() -> i64 {
    50
}

fn default_notebook_count() -> i64 {
    0
}

fn default_depth() -> u8 {
    3
}

fn default_memory_limit() -> i64 {
    50
}

fn default_code_search_limit() -> u8 {
    8
}

fn default_code_context_limit() -> u8 {
    6
}

fn default_session_id() -> String {
    "default".to_string()
}

// Examples
fn example_cognitive_http() -> String {
    "100.90.37.254:3003".to_string()
}

fn example_cognitive_grpc() -> String {
    "100.90.37.254:50052".to_string()
}

fn default_cognitive_db_path() -> String {
    "/var/lib/op-dbus/cognitive.db".to_string()
}

fn example_db_path() -> String {
    "/var/lib/op-dbus/cognitive.db".to_string()
}

fn example_wg_interface() -> String {
    "netmaker".to_string()
}

fn example_memory_namespace() -> Option<String> {
    Some("project:op-dbus".to_string())
}

fn example_code_search_query() -> String {
    "how is wireguard identity verified".to_string()
}

fn example_code_search_language() -> Option<String> {
    Some("rust".to_string())
}

fn example_collection() -> Option<String> {
    Some("repomix_rag".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Citation {
    #[schemars(description = "Cited text passage")]
    pub text: Option<String>,
    #[schemars(description = "Source document identifier")]
    pub source: Option<String>,
    #[schemars(description = "Page or location within source")]
    pub page: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceInfo {
    #[schemars(description = "Unique source identifier", extend("readOnly" = true))]
    pub id: String,
    #[schemars(description = "Source title")]
    pub title: Option<String>,
    #[schemars(description = "Source transport type", example = example_source_type())]
    pub source_type: SourceType,
    #[serde(default)]
    #[schemars(description = "Tags attached to this source")]
    pub tags: Vec<String>,
    #[schemars(description = "ISO-8601 creation timestamp", extend("readOnly" = true))]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeminiQueryRequest {
    #[schemars(description = "Natural-language query")]
    pub query: String,
    #[schemars(description = "Optional grounding context")]
    pub context: Option<String>,
    #[serde(default = "default_gemini_mode")]
    #[schemars(description = "Query mode")]
    pub mode: GeminiMode,
    #[serde(default = "default_depth")]
    #[schemars(
        range(min = 1, max = 5),
        description = "Deep-research depth (1-5, default 3)"
    )]
    pub depth: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryToolInput {
    #[schemars(
        description = "Memory operation to perform",
        example = example_memory_operation()
    )]
    pub operation: MemoryOperation,
    #[schemars(
        description = "Namespace name (e.g. project:op-dbus, session:abc)",
        example = example_memory_namespace()
    )]
    pub namespace: Option<String>,
    #[schemars(
        description = "Kind of namespace (used when creating)",
        example = example_memory_namespace_kind()
    )]
    pub namespace_kind: Option<NamespaceKind>,
    #[schemars(description = "Entry key within namespace")]
    pub key: Option<String>,
    #[schemars(description = "Value to store (any JSON)")]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    #[schemars(description = "Tags for the entry")]
    pub tags: Vec<String>,
    #[schemars(description = "Substring pattern for key search (used in query)")]
    pub key_pattern: Option<String>,
    #[serde(default = "default_memory_limit")]
    #[schemars(description = "Max results (default 50)")]
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeSearchInput {
    #[schemars(
        description = "Natural-language or code query",
        example = example_code_search_query()
    )]
    pub query: String,
    #[schemars(description = "Restrict to a repo name")]
    pub repo: Option<String>,
    #[schemars(
        description = "Restrict to a language (e.g. rust, typescript)",
        example = example_code_search_language()
    )]
    pub language: Option<String>,
    #[schemars(description = "Restrict to a file classification")]
    pub file_type: Option<FileType>,
    #[schemars(description = "Only files whose path contains this substring")]
    pub path_contains: Option<String>,
    #[schemars(description = "Only chunks whose symbols/path contain this substring")]
    pub symbol_contains: Option<String>,
    #[serde(default)]
    #[schemars(description = "Drop test files from results")]
    pub exclude_tests: bool,
    #[serde(default = "default_true")]
    #[schemars(description = "Fuse semantic+lexical scoring and dedup to one chunk per file")]
    pub fused: bool,
    #[schemars(
        description = "Override the Qdrant repomix/RAG collection for this search",
        example = example_collection()
    )]
    pub collection: Option<String>,
    #[serde(default = "default_code_search_limit")]
    #[schemars(range(min = 1, max = 50), description = "Max results (default 8)")]
    pub limit: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeContextInput {
    #[schemars(description = "Current query / what the agent is working on")]
    pub query: String,
    #[serde(default = "default_session_id")]
    #[schemars(description = "Session identifier (default 'default')")]
    pub session_id: String,
    #[serde(default = "default_activity_type")]
    #[schemars(description = "Kind of activity being recorded (default 'query')")]
    pub activity_type: ActivityType,
    #[schemars(description = "Restrict retrieval to a repo")]
    pub repo: Option<String>,
    #[schemars(description = "Restrict retrieval to a language")]
    pub language: Option<String>,
    #[serde(default)]
    #[schemars(description = "Drop test files from results")]
    pub exclude_tests: bool,
    #[schemars(
        description = "Override the Qdrant repomix/RAG collection for this context request",
        example = example_collection()
    )]
    pub collection: Option<String>,
    #[serde(default = "default_code_context_limit")]
    #[schemars(range(min = 1, max = 50), description = "Max results (default 6)")]
    pub limit: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeIndexInput {
    #[serde(default = "default_code_index_mode")]
    #[schemars(description = "Indexing mode (default 'source')")]
    pub mode: CodeIndexMode,
    #[schemars(description = "Repo name (source mode)")]
    pub repo: Option<String>,
    #[schemars(description = "File path within the repo (source mode)")]
    pub file_path: Option<String>,
    #[schemars(description = "Raw file content (source mode)")]
    pub content: Option<String>,
    #[schemars(description = "Path to repomix zip (repomix_zip mode)")]
    pub zip_path: Option<String>,
    #[schemars(description = "Entry name within the zip (repomix_zip mode)")]
    pub entry: Option<String>,
    #[schemars(description = "Override target collection")]
    pub collection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.cognitive-mcp.schema@v1"))]
pub struct CognitiveMcpState {
    #[serde(default = "default_cognitive_http")]
    #[schemars(description = "HTTP/SSE bind address for the MCP protocol endpoint", example = example_cognitive_http(), extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.http@v1"))]
    pub http: String,
    #[serde(default = "default_cognitive_grpc")]
    #[schemars(description = "gRPC bind address for the CognitiveToolService endpoint", example = example_cognitive_grpc(), extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.grpc@v1"))]
    pub grpc: String,
    #[serde(default = "default_cognitive_db_path")]
    #[schemars(description = "CozoDB database path for persistent memory storage", example = example_db_path(), extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.db-path@v1"))]
    pub db_path: String,
    #[serde(default = "default_wg")]
    #[schemars(description = "WireGuard interface to read identity from", example = example_wg_interface(), extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.wg-interface@v1"))]
    pub wg_interface: String,
    #[serde(default = "default_true")]
    #[schemars(description = "Enable the HTTP/SSE MCP transport", extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.http-enabled@v1"))]
    pub http_enabled: bool,
    #[serde(default = "default_true")]
    #[schemars(description = "Enable the gRPC CognitiveToolService transport", extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.grpc-enabled@v1"))]
    pub grpc_enabled: bool,
    #[serde(default = "default_true")]
    #[schemars(description = "Register on D-Bus as org.opdbus.CognitiveMcp", extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.dbus-enabled@v1"))]
    pub dbus_enabled: bool,
    #[serde(default = "default_running")]
    #[schemars(description = "Whether the s6 service is currently running", extend("readOnly" = true), extend("x-oscal-subid" = "obs.software.plugin.cognitive-mcp.running@v1"))]
    pub running: bool,
    #[serde(default = "default_healthy")]
    #[schemars(description = "Last known health status from GetHealth", extend("readOnly" = true), extend("x-oscal-subid" = "obs.software.plugin.cognitive-mcp.healthy@v1"))]
    pub healthy: bool,
    #[serde(default = "default_auth_status")]
    #[schemars(description = "Current authentication method", example = example_auth_status(), extend("readOnly" = true), extend("x-oscal-subid" = "obs.software.plugin.cognitive-mcp.auth-status@v1"))]
    pub auth_status: AuthStatus,
    #[serde(default = "default_queries_remaining")]
    #[schemars(description = "Queries remaining in current quota period", extend("readOnly" = true), extend("x-oscal-subid" = "obs.software.plugin.cognitive-mcp.queries-remaining@v1"))]
    pub queries_remaining: i64,
    #[serde(default = "default_queries_limit")]
    #[schemars(description = "Total queries allowed per quota period", extend("readOnly" = true), extend("x-oscal-subid" = "obs.software.plugin.cognitive-mcp.queries-limit@v1"))]
    pub queries_limit: i64,
    #[serde(default = "default_notebook_count")]
    #[schemars(description = "Number of notebooks in the library", extend("readOnly" = true), extend("x-oscal-subid" = "obs.software.plugin.cognitive-mcp.notebook-count@v1"))]
    pub notebook_count: i64,
    #[schemars(description = "R12: Gemini fallback query (requires GEMINI_API_KEY)", extend("readOnly" = true), extend("x-oscal-subid" = "exp.software.plugin.cognitive-mcp.gemini-query-request@v1"))]
    pub gemini_query_request: Option<GeminiQueryRequest>,
    #[schemars(description = "MCP MemoryTool: key-value memory store with operations store/retrieve/query/delete/list_namespaces/stats", extend("readOnly" = true), extend("x-oscal-subid" = "exp.software.plugin.cognitive-mcp.memory-tool@v1"))]
    pub memory_tool: Option<MemoryToolInput>,
    #[schemars(description = "Citation sub-object: text, source, page. Inherited by grounded query responses.", extend("readOnly" = true), extend("x-oscal-subid" = "exp.software.plugin.cognitive-mcp.citation@v1"))]
    pub citation: Option<Citation>,
    #[schemars(description = "SourceInfo sub-object: id, title, source_type, tags, created_at. Inherited by source CRUD responses.", extend("readOnly" = true), extend("x-oscal-subid" = "exp.software.plugin.cognitive-mcp.source-info@v1"))]
    pub source_info: Option<SourceInfo>,
    #[schemars(description = "CodeSearchTool input: semantic+lexical search over the indexed code corpus.", extend("readOnly" = true), extend("x-oscal-subid" = "obs.service.code-rag.search@v1"))]
    pub code_search: Option<CodeSearchInput>,
    #[schemars(description = "CodeContextTool input: activity-aware context retrieval for the current session.", extend("readOnly" = true), extend("x-oscal-subid" = "exp.service.code-context.render@v1"))]
    pub code_context: Option<CodeContextInput>,
    #[schemars(description = "CodeIndexTool input: live single-file or repomix-zip indexing into the code corpus.", extend("readOnly" = true), extend("x-oscal-subid" = "src.software.workspace.index@v1"))]
    pub code_index: Option<CodeIndexInput>,
}

pub(crate) fn cognitive_mcp_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(CognitiveMcpState))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "cognitive_mcp",
        "2.0.0",
        "Cognitive MCP server — memory, gRPC CognitiveToolService. THE PLUGIN IS THE SCHEMA: every method, tool, property, and field is declared here. Downstream inherits.",
        &root,
    )
}

#[cfg(test)]
pub(crate) fn cognitive_mcp_schema_golden() -> PluginSchema {
    let citation_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "text".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Cited text passage".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "source".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source document identifier".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "page".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Page or location within source".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let source_info_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unique source identifier".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "title".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source title".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "source_type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "url".to_string(),
                    "text".to_string(),
                    "file".to_string(),
                ]),
                required: true,
                description: "Source transport type".to_string(),
                default: None,
                example: Some(json!("url")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "tags".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Tags attached to this source".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "created_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "ISO-8601 creation timestamp".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields
    };

    let gemini_query_request_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "query".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Natural-language query".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "context".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional grounding context".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "mode".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["query".to_string(), "deep_research".to_string()]),
                required: false,
                description: "Query mode".to_string(),
                default: Some(json!("query")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "depth".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Deep-research depth (1-5, default 3)".to_string(),
                default: Some(json!(3)),
                example: None,
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 5.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let memory_tool_input_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "operation".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "store".to_string(),
                    "retrieve".to_string(),
                    "query".to_string(),
                    "delete".to_string(),
                    "list_namespaces".to_string(),
                    "stats".to_string(),
                ]),
                required: true,
                description: "Memory operation to perform".to_string(),
                default: None,
                example: Some(json!("store")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "namespace".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Namespace name (e.g. project:op-dbus, session:abc)".to_string(),
                default: None,
                example: Some(json!("project:op-dbus")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "namespace_kind".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "project".to_string(),
                    "session".to_string(),
                    "database".to_string(),
                    "workflow".to_string(),
                    "agent".to_string(),
                    "cron".to_string(),
                    "custom".to_string(),
                ]),
                required: false,
                description: "Kind of namespace (used when creating)".to_string(),
                default: None,
                example: Some(json!("project")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Entry key within namespace".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "value".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Value to store (any JSON)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "tags".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Tags for the entry".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "key_pattern".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Substring pattern for key search (used in query)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "limit".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Max results (default 50)".to_string(),
                default: Some(json!(50)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    // ── code_search tool input (subid obs.service.code-rag.search@v1) ──────
    let code_search_input_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "query".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Natural-language or code query".to_string(),
                default: None,
                example: Some(json!("how is wireguard identity verified")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "repo".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Restrict to a repo name".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "language".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Restrict to a language (e.g. rust, typescript)".to_string(),
                default: None,
                example: Some(json!("rust")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "file_type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "source".to_string(),
                    "test".to_string(),
                    "config".to_string(),
                    "docs".to_string(),
                    "build".to_string(),
                    "other".to_string(),
                ]),
                required: false,
                description: "Restrict to a file classification".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "path_contains".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Only files whose path contains this substring".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "symbol_contains".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Only chunks whose symbols/path contain this substring".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "exclude_tests".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Drop test files from results".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "fused".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Fuse semantic+lexical scoring and dedup to one chunk per file"
                    .to_string(),
                default: Some(json!(true)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "collection".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Override the Qdrant repomix/RAG collection for this search"
                    .to_string(),
                default: None,
                example: Some(json!("repomix_rag")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "limit".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Max results (default 8)".to_string(),
                default: Some(json!(8)),
                example: None,
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 50.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    // ── code_context tool input (subid exp.service.code-context.render@v1) ─
    let code_context_input_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "query".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Current query / what the agent is working on".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "session_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Session identifier (default 'default')".to_string(),
                default: Some(json!("default")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "activity_type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "tool_call".to_string(),
                    "query".to_string(),
                    "context_switch".to_string(),
                    "error".to_string(),
                    "idle".to_string(),
                    "return_from_idle".to_string(),
                    "file_opened".to_string(),
                    "edit_applied".to_string(),
                    "build_error".to_string(),
                    "test_failure".to_string(),
                    "diff_viewed".to_string(),
                    "symbol_navigated".to_string(),
                ]),
                required: false,
                description: "Kind of activity being recorded (default 'query')".to_string(),
                default: Some(json!("query")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "repo".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Restrict retrieval to a repo".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "language".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Restrict retrieval to a language".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "exclude_tests".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Drop test files from results".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "collection".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Override the Qdrant repomix/RAG collection for this context request"
                    .to_string(),
                default: None,
                example: Some(json!("repomix_rag")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "limit".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Max results (default 6)".to_string(),
                default: Some(json!(6)),
                example: None,
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 50.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    // ── code_index tool input (subid src.software.workspace.index@v1) ──────
    let code_index_input_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "mode".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["source".to_string(), "repomix_zip".to_string()]),
                required: false,
                description: "Indexing mode (default 'source')".to_string(),
                default: Some(json!("source")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "repo".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Repo name (source mode)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "file_path".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "File path within the repo (source mode)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "content".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Raw file content (source mode)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "zip_path".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Path to repomix zip (repomix_zip mode)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "entry".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Entry name within the zip (repomix_zip mode)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "collection".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Override target collection".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("cognitive_mcp")
        .version("2.0.0")
        .description("Cognitive MCP server — memory, gRPC CognitiveToolService. THE PLUGIN IS THE SCHEMA: every method, tool, property, and field is declared here. Downstream inherits.")
        .subid("__schema__", "sch.software.plugin.cognitive-mcp.schema@v1")
        .field("http", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "HTTP/SSE bind address for the MCP protocol endpoint".to_string(),
            default: Some(json!("0.0.0.0:3003")), example: Some(json!("100.90.37.254:3003")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .subid("http", "mut.software.plugin.cognitive-mcp.http@v1")
        .field("grpc", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "gRPC bind address for the CognitiveToolService endpoint".to_string(),
            default: Some(json!("0.0.0.0:50052")), example: Some(json!("100.90.37.254:50052")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .subid("grpc", "mut.software.plugin.cognitive-mcp.grpc@v1")
        .field("db_path", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "CozoDB database path for persistent memory storage".to_string(),
            default: Some(json!("/var/lib/op-dbus/cognitive.db")), example: Some(json!("/var/lib/op-dbus/cognitive.db")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .subid("db_path", "mut.software.plugin.cognitive-mcp.db-path@v1")
        .field("wg_interface", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "WireGuard interface to read identity from".to_string(),
            default: Some(json!("netmaker")), example: Some(json!("netmaker")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .subid("wg_interface", "mut.software.plugin.cognitive-mcp.wg-interface@v1")
        .field("http_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable the HTTP/SSE MCP transport".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .subid("http_enabled", "mut.software.plugin.cognitive-mcp.http-enabled@v1")
        .field("grpc_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable the gRPC CognitiveToolService transport".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .subid("grpc_enabled", "mut.software.plugin.cognitive-mcp.grpc-enabled@v1")
        .field("dbus_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Register on D-Bus as org.opdbus.CognitiveMcp".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .subid("dbus_enabled", "mut.software.plugin.cognitive-mcp.dbus-enabled@v1")
        .field("running", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the s6 service is currently running".to_string(),
            default: Some(json!(false)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("running", "obs.software.plugin.cognitive-mcp.running@v1")
        .field("healthy", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Last known health status from GetHealth".to_string(),
            default: Some(json!(false)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("healthy", "obs.software.plugin.cognitive-mcp.healthy@v1")
        .field("auth_status", FieldSchema {
            field_type: FieldType::Enum(vec![
                "none".to_string(), "chrome_profile".to_string(),
                "cookie".to_string(), "api_key".to_string(),
            ]),
            required: false,
            description: "Current authentication method".to_string(),
            default: Some(json!("none")), example: Some(json!("chrome_profile")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .subid("auth_status", "obs.software.plugin.cognitive-mcp.auth-status@v1")
        .field("queries_remaining", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Queries remaining in current quota period".to_string(),
            default: Some(json!(0)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("queries_remaining", "obs.software.plugin.cognitive-mcp.queries-remaining@v1")
        .field("queries_limit", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Total queries allowed per quota period".to_string(),
            default: Some(json!(50)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("queries_limit", "obs.software.plugin.cognitive-mcp.queries-limit@v1")
        .field("notebook_count", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Number of notebooks in the library".to_string(),
            default: Some(json!(0)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("notebook_count", "obs.software.plugin.cognitive-mcp.notebook-count@v1")
        .field("gemini_query_request", FieldSchema {
            field_type: FieldType::Object(gemini_query_request_fields), required: false,
            description: "R12: Gemini fallback query (requires GEMINI_API_KEY)".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("gemini_query_request", "exp.software.plugin.cognitive-mcp.gemini-query-request@v1")
        .field("memory_tool", FieldSchema {
            field_type: FieldType::Object(memory_tool_input_fields), required: false,
            description: "MCP MemoryTool: key-value memory store with operations store/retrieve/query/delete/list_namespaces/stats".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("memory_tool", "exp.software.plugin.cognitive-mcp.memory-tool@v1")
        .field("citation", FieldSchema {
            field_type: FieldType::Object(citation_fields), required: false,
            description: "Citation sub-object: text, source, page. Inherited by grounded query responses.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("citation", "exp.software.plugin.cognitive-mcp.citation@v1")
        .field("source_info", FieldSchema {
            field_type: FieldType::Object(source_info_fields), required: false,
            description: "SourceInfo sub-object: id, title, source_type, tags, created_at. Inherited by source CRUD responses.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("source_info", "exp.software.plugin.cognitive-mcp.source-info@v1")
        .field("code_search", FieldSchema {
            field_type: FieldType::Object(code_search_input_fields), required: false,
            description: "CodeSearchTool input: semantic+lexical search over the indexed code corpus.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("code_search", "obs.service.code-rag.search@v1")
        .field("code_context", FieldSchema {
            field_type: FieldType::Object(code_context_input_fields), required: false,
            description: "CodeContextTool input: activity-aware context retrieval for the current session.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("code_context", "exp.service.code-context.render@v1")
        .field("code_index", FieldSchema {
            field_type: FieldType::Object(code_index_input_fields), required: false,
            description: "CodeIndexTool input: live single-file or repomix-zip indexing into the code corpus.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .subid("code_index", "src.software.workspace.index@v1")
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use crate::state_plugins::schemars_adapter::schema_diffs;
    use serde_json::Value as JVal;

    fn collect_subids(node: &JVal, out: &mut Vec<String>) {
        if let Some(subid) = node.get("x-oscal-subid").and_then(JVal::as_str) {
            out.push(subid.to_string());
        }
        if let Some(props) = node.get("properties").and_then(JVal::as_object) {
            for v in props.values() {
                collect_subids(v, out);
            }
        }
        if let Some(defs) = node
            .get("$defs")
            .or_else(|| node.get("definitions"))
            .and_then(JVal::as_object)
        {
            for v in defs.values() {
                collect_subids(v, out);
            }
        }
        if let Some(items) = node.get("items") {
            collect_subids(items, out);
        }
        if let Some(alternatives) = node
            .get("anyOf")
            .or_else(|| node.get("oneOf"))
            .and_then(JVal::as_array)
        {
            for v in alternatives {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let diffs = schema_diffs(&cognitive_mcp_schema_golden(), &cognitive_mcp_schema());
        assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(CognitiveMcpState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(
            !subids.is_empty(),
            "expected at least one x-oscal-subid in the derived schema"
        );
        for subid in subids {
            validate_subid(&subid).expect("invalid subid: {subid}");
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("cognitive_mcp", |_ctx| std::sync::Arc::new(CognitiveMcpPlugin::new()))
}
