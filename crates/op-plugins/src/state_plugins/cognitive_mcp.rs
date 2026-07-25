//! Cognitive MCP state plugin — GB.CognitiveMcp.
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
use op_state_store::SideEffect;
use serde::{Deserialize, Serialize};
use simd_json::prelude::ValueAsScalar;
use simd_json::{prelude::*, OwnedValue as Value};

use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;

// =============================================================================
// PLUGIN ENTRY: identity and typed schema seed
// =============================================================================

const PLUGIN_NAME: &str = "cognitive_mcp";
const PLUGIN_VERSION: &str = "2.0.0";
const PLUGIN_CATEGORY: &str = "service";
const PLUGIN_DESCRIPTION: &str = "Cognitive MCP server — memory, gRPC CognitiveToolService. THE PLUGIN IS THE SCHEMA: every method, tool, property, and field is declared here. Downstream inherits.";
const PLUGIN_DISPLAY_NAME: &str = "GB.CognitiveMcp";

const S6_SV_PATH: &str = "/run/service/op-cognitive-mcp";
const ENV_DIR: &str = "/etc/s6/sv/op-cognitive-mcp/env";
const RUNTIME_ENV_DIR: &str = "/run/service/op-cognitive-mcp/env";
const DEFAULT_HTTP: &str = "100.90.37.254:3003";
const DEFAULT_GRPC: &str = "100.90.37.254:50052";
const DEFAULT_WG: &str = "netmaker";

// ── Deployment config (tunable via env-dir / apply_state) ──────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitiveMcpConfig {
    #[serde(default = "default_http")]
    pub http: String,
    #[serde(default = "default_grpc")]
    pub grpc: String,
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
            wg_interface: default_wg(),
            http_enabled: true,
            grpc_enabled: true,
            dbus_enabled: true,
        }
    }
}

// ── Plugin struct + service helpers ─────────────────────────────────────────

// =============================================================================
// PLUGIN BODY: D-Bus-backed behavior only
// =============================================================================

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
                "/org/opdbus/v1/plugins/s6/systemctl",
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
        PLUGIN_NAME
    }
    fn version(&self) -> &str {
        PLUGIN_VERSION
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

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = simd_json::json!(null);
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
        let current = simd_json::json!(null);
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
        description = "Override the Qdrant collection (see qdrant plugin collections) for this search",
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
        description = "Override the Qdrant collection (see qdrant plugin collections) for this context request",
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

/// Output for GetConfig method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetConfigOutput {
    pub http: String,
    pub grpc: String,
    pub wg_interface: String,
    pub http_enabled: bool,
    pub grpc_enabled: bool,
    pub dbus_enabled: bool,
}

/// Output for SetConfig method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetConfigOutput {
    pub success: bool,
    pub message: String,
}

/// Output for GetHealth method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetHealthOutput {
    pub healthy: bool,
    pub running: bool,
    pub auth_status: String,
    pub queries_remaining: i64,
    pub queries_limit: i64,
}

/// Output for ListTools method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListToolsOutput {
    pub tools: Vec<String>,
}

/// Output for RegisterTool method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RegisterToolOutput {
    pub success: bool,
    pub tool_name: String,
}

/// Output for MemoryStore method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryStoreOutput {
    pub success: bool,
    pub key: String,
}

/// Output for MemoryRetrieve method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryRetrieveOutput {
    pub success: bool,
    pub value: Option<String>,
    pub namespace: String,
}

/// Output for MemoryQuery method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryQueryOutput {
    pub results: Vec<String>,
    pub count: usize,
}

/// Output for MemoryDelete method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryDeleteOutput {
    pub success: bool,
    pub keys_deleted: usize,
}

/// Output for MemoryListNamespaces method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryListNamespacesOutput {
    pub namespaces: Vec<String>,
}

/// Output for CodeSearch method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeSearchOutput {
    pub results: Vec<String>,
    pub count: usize,
}

/// Output for CodeIndex method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeIndexOutput {
    pub success: bool,
    pub files_indexed: usize,
}

/// Output for CodeContext method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeContextOutput {
    pub context: String,
    pub sources: Vec<String>,
}

/// Output for GeminiQuery method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeminiQueryOutput {
    pub response: String,
    pub citations: Vec<String>,
}

/// Output for RestartService method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RestartServiceOutput {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.cognitive-mcp.schema@v1"))]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct CognitiveMcpState {
    #[serde(default = "default_cognitive_http")]
    #[schemars(description = "HTTP/SSE bind address for the MCP protocol endpoint", example = example_cognitive_http(), extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.http@v1"))]
    pub http: String,
    #[serde(default = "default_cognitive_grpc")]
    #[schemars(description = "gRPC bind address for the CognitiveToolService endpoint", example = example_cognitive_grpc(), extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.grpc@v1"))]
    pub grpc: String,
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

// =============================================================================
// PLUGIN EXIT: publish the single PluginSchema contract
// =============================================================================

// Handler audit (all 15 schema methods below) — where each is actually backed:
// get_config              → cognitive_mcp.rs:current_config() (this file, StatePlugin)
// set_config              → cognitive_mcp.rs:apply_state() (this file, StatePlugin)
// get_health              → op-cognitive-mcp/grpc_service.rs:get_health (tonic RPC)
// list_tools              → op-cognitive-mcp/dbus_interface.rs:list_tools (D-Bus)
// register_tool           → op-cognitive-mcp/cognitive_tools.rs:RegisterToolTool
//                           (stub — no dynamic tool-registration mechanism exists yet)
// memory_store            → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_store
// memory_retrieve         → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_retrieve
// memory_query            → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_query
// memory_delete           → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_delete
// memory_list_namespaces  → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_list_namespaces
// code_search             → op-cognitive-mcp/code_tools.rs:CodeSearchTool
// code_index              → op-cognitive-mcp/code_tools.rs:CodeIndexTool
// code_context            → op-cognitive-mcp/code_tools.rs:CodeContextTool
// gemini_query            → op-cognitive-mcp/grpc_service.rs:gemini_query (tonic RPC)
// restart_service         → cognitive_mcp.rs:reload_service_dbus() (this file, StatePlugin)
pub(crate) fn cognitive_mcp_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(CognitiveMcpState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;

    schema.methods.insert(
        "get_config".to_string(),
        method_decl_from_schemars_with_output::<(), GetConfigOutput>(
            "get_config",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.config.get@v1",
        ),
    );
    schema.methods.insert(
        "set_config".to_string(),
        method_decl_from_schemars_with_output::<(), SetConfigOutput>(
            "set_config",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.config.set@v1",
        ),
    );
    schema.methods.insert(
        "get_health".to_string(),
        method_decl_from_schemars_with_output::<(), GetHealthOutput>(
            "get_health",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.health@v1",
        ),
    );
    schema.methods.insert(
        "list_tools".to_string(),
        method_decl_from_schemars_with_output::<(), ListToolsOutput>(
            "list_tools",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.tool.list@v1",
        ),
    );
    schema.methods.insert(
        "register_tool".to_string(),
        method_decl_from_schemars_with_output::<(), RegisterToolOutput>(
            "register_tool",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.tool.register@v1",
        ),
    );
    schema.methods.insert(
        "memory_store".to_string(),
        method_decl_from_schemars_with_output::<(), MemoryStoreOutput>(
            "memory_store",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.memory.store@v1",
        ),
    );
    schema.methods.insert(
        "memory_retrieve".to_string(),
        method_decl_from_schemars_with_output::<(), MemoryRetrieveOutput>(
            "memory_retrieve",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.memory.retrieve@v1",
        ),
    );
    schema.methods.insert(
        "memory_query".to_string(),
        method_decl_from_schemars_with_output::<(), MemoryQueryOutput>(
            "memory_query",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.memory.query@v1",
        ),
    );
    schema.methods.insert(
        "memory_delete".to_string(),
        method_decl_from_schemars_with_output::<(), MemoryDeleteOutput>(
            "memory_delete",
            SideEffect::Mutation,
            true,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.memory.delete@v1",
        ),
    );
    schema.methods.insert(
        "memory_list_namespaces".to_string(),
        method_decl_from_schemars_with_output::<(), MemoryListNamespacesOutput>(
            "memory_list_namespaces",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.memory.namespace.list@v1",
        ),
    );
    schema.methods.insert(
        "code_search".to_string(),
        method_decl_from_schemars_with_output::<(), CodeSearchOutput>(
            "code_search",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.code-rag.search@v1",
        ),
    );
    schema.methods.insert(
        "code_index".to_string(),
        method_decl_from_schemars_with_output::<(), CodeIndexOutput>(
            "code_index",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.code-rag.index@v1",
        ),
    );
    schema.methods.insert(
        "code_context".to_string(),
        method_decl_from_schemars_with_output::<(), CodeContextOutput>(
            "code_context",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.code-context.get@v1",
        ),
    );
    schema.methods.insert(
        "gemini_query".to_string(),
        method_decl_from_schemars_with_output::<(), GeminiQueryOutput>(
            "gemini_query",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.gemini.query@v1",
        ),
    );
    schema.methods.insert(
        "restart_service".to_string(),
        method_decl_from_schemars_with_output::<(), RestartServiceOutput>(
            "restart_service",
            SideEffect::Mutation,
            true,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.restart@v1",
        ),
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
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
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(CognitiveMcpPlugin::new()))
}
