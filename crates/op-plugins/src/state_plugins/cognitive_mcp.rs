//! Cognitive MCP state plugin — GB.CognitiveMcp.
//!
//! Tracks and manages the op-cognitive-mcp server: WireGuard identity, tool
//! registrations, and health.  Publishes live state to D-Bus under
//! `/opdbus/v1/plugins/cognitive_mcp` for introspection by clients.
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
const PLUGIN_VERSION: &str = "3.4.0";
const PLUGIN_CATEGORY: &str = "service";
const PLUGIN_DESCRIPTION: &str = "Cognitive MCP server — memory, gRPC CognitiveToolService. THE PLUGIN IS THE SCHEMA: every method, tool, property, and field is declared here. Downstream inherits.";
const PLUGIN_DISPLAY_NAME: &str = "GB.CognitiveMcp";

/// Live supervised path (`/run/runit/service/op-cognitive-mcp`).
const SUPERVISED_PATH: &str = "/run/runit/service/op-cognitive-mcp";
const ENV_DIR: &str = "/etc/runit/sv/op-cognitive-mcp/env";
const RUNTIME_ENV_DIR: &str = "/run/runit/service/op-cognitive-mcp/env";
const RUNIT_SYSTEMCTL_SERVICE: &str = "org.opdbus.v1.Runit.Systemctl";
const RUNIT_SYSTEMCTL_PATH: &str = "/org/opdbus/v1/plugins/runit/systemctl";
const RUNIT_SYSTEMCTL_INTERFACE: &str = "org.opdbus.v1.Runit.Systemctl";
const DEFAULT_WG: &str = "netmaker";

// ── Deployment config (tunable via env-dir / apply_state) ──────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitiveMcpConfig {
    #[serde(default = "default_wg")]
    pub wg_interface: String,
    #[serde(default = "default_true")]
    pub dbus_enabled: bool,
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
            wg_interface: default_wg(),
            dbus_enabled: true,
        }
    }
}

/// Complete desired configuration accepted by the sealed `set_config` method.
///
/// The mutation intentionally requires both fields.  Supplying the whole
/// configuration makes a request self-contained, so the bridge never has to
/// guess whether an omitted value means "leave unchanged" or "reset it".
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq)]
pub struct SetConfigInput {
    /// WireGuard interface from which the Cognitive MCP service reads identity.
    pub wg_interface: String,
    /// Whether the service should expose its D-Bus interface.
    pub dbus_enabled: bool,
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
        let sv = std::path::Path::new(SUPERVISED_PATH);
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
            wg_interface: Self::read_env("WG_INTERFACE").unwrap_or_else(default_wg),
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
        // Application lifecycle calls use the audited runit D-Bus service;
        // agents themselves continue to use `sudo sv` for host operations.
        Self::reload_service_dbus().await
    }

    async fn reload_service_dbus() -> Result<()> {
        let conn = zbus::Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        let reply = conn
            .call_method(
                Some(RUNIT_SYSTEMCTL_SERVICE),
                RUNIT_SYSTEMCTL_PATH,
                Some(RUNIT_SYSTEMCTL_INTERFACE),
                "reload",
                &("op-cognitive-mcp",),
            )
            .await
            .context("Failed to call reload on runit-systemctl D-Bus service")?;

        let (success, message): (bool, String) = reply.body().deserialize().map_err(|e| {
            anyhow::anyhow!(
                "Failed to deserialize runit-systemctl reload response: {}",
                e
            )
        })?;

        if success {
            tracing::info!(
                "Reloaded op-cognitive-mcp through runit D-Bus control: {}",
                message
            );
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "runit-systemctl reload failed: {}",
                message
            ))
        }
    }
}

impl Default for CognitiveMcpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply a complete Cognitive MCP configuration through the plugin's normal
/// StatePlugin diff/apply path.
///
/// This is deliberately the only bridge-facing write helper: callers do not
/// write runit environment files or signal services directly.  The method is
/// still authorized, audited, and schema-validated by the bridge before it
/// reaches this helper.
pub async fn set_cognitive_mcp_config(input: SetConfigInput) -> Result<SetConfigOutput> {
    let plugin = CognitiveMcpPlugin::new();
    let current = simd_json::serde::to_owned_value(&CognitiveMcpPlugin::current_config())?;
    let desired_config = CognitiveMcpConfig {
        wg_interface: input.wg_interface,
        dbus_enabled: input.dbus_enabled,
    };
    let desired = simd_json::serde::to_owned_value(&desired_config)?;
    let diff = plugin.calculate_diff(&current, &desired).await?;

    if diff.actions.is_empty() {
        return Ok(SetConfigOutput {
            success: true,
            message: "Cognitive MCP configuration already matches the requested values".into(),
        });
    }

    let applied = plugin.apply_state(&diff).await?;
    if !applied.success {
        return Err(anyhow::anyhow!(
            "Cognitive MCP configuration was not fully applied: {}",
            applied.errors.join("; ")
        ));
    }

    Ok(SetConfigOutput {
        success: true,
        message: format!(
            "Applied Cognitive MCP configuration: {}",
            applied.changes_applied.join(", ")
        ),
    })
}

/// Reload the Cognitive MCP service through the canonical audited runit D-Bus
/// control plane.  The legacy method name is retained for schema compatibility;
/// runit receives a reload signal, not an unsupervised process restart.
pub async fn reload_cognitive_mcp_service() -> Result<RestartServiceOutput> {
    CognitiveMcpPlugin::reload_service().await?;
    Ok(RestartServiceOutput {
        success: true,
        message: "Reloaded op-cognitive-mcp through the runit D-Bus control plane".into(),
    })
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

    /// Whether the supervised service definition exists on this host.
    ///
    /// Availability gates more than reporting: `freeze_plugin_method_reflection`
    /// skips unavailable plugins, so a false negative here means none of this
    /// plugin's frozen per-method gRPC services get activated — the sealed blob is
    /// advertised in the reflection catalog but nothing is mounted to serve it.
    ///
    /// Checks the host's runit layout. The runit definition and supervised
    /// service are the only lifecycle observations used on this host.
    fn is_available(&self) -> bool {
        ["/etc/runit/sv/op-cognitive-mcp", SUPERVISED_PATH]
            .iter()
            .any(|p| std::path::Path::new(p).exists())
    }

    fn unavailable_reason(&self) -> String {
        "op-cognitive-mcp supervised service definition not found under \
         /etc/runit/sv or /run/runit/service"
            .into()
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
        field_diff!(wg_interface, "wg_interface");
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
                    "wg_interface" => {
                        if let Some(s) = val.as_str() {
                            Self::write_env("WG_INTERFACE", s).await?;
                            needs_reload = true;
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("wg_interface must be a string"))
                        }
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
                // The environment files may have been updated, but callers
                // must not receive a successful configuration response until
                // the supervised service has accepted the reload as well.
                errors.push(format!("cognitive_mcp reload: {e}"));
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
    500
}

fn default_notebook_count() -> i64 {
    0
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
pub struct AskQuestionInput {
    #[schemars(
        description = "Natural-language question to answer from the configured notebook namespace"
    )]
    pub query: String,
    #[schemars(description = "Optional session identifier for follow-up question context")]
    pub conversation_id: Option<String>,
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

/// Input for a controlled memory write.  The operation selector is injected by
/// the bridge, so callers only provide the data that belongs to this operation.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryStoreInput {
    /// Explicit system-owned namespace, such as `project:3tched-cognative`.
    pub namespace: String,
    pub key: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
    pub namespace_kind: Option<NamespaceKind>,
    #[serde(default)]
    pub semantic: bool,
    /// Optional system container identity used for a linked memory namespace.
    pub container_id: Option<String>,
    pub identity_id: Option<String>,
    pub wireguard_pubkey: Option<String>,
}

/// Input for a point lookup in a system-owned memory namespace.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryRetrieveInput {
    pub namespace: String,
    pub key: String,
}

/// Input for a bounded metadata query over cognitive memory.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryQueryInput {
    pub namespace: Option<String>,
    pub key_pattern: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_memory_limit")]
    pub limit: i64,
}

/// Input for deleting one named memory entry.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryDeleteInput {
    pub namespace: String,
    pub key: String,
}

/// Input for listing memory namespaces, optionally filtered by their kind.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryListNamespacesInput {
    pub namespace_kind: Option<NamespaceKind>,
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

/// Typed empty argument set for methods that take no parameters.
///
/// Distinct from `()`, which schemars renders as `{"type":"null"}` and therefore
/// rejects the `{}` that D-Bus/gRPC callers send for a no-argument method. This
/// renders as an object schema, so both a bare `{}` and an absent body validate.
///
/// OSCAL subid: sch.software.cognitive-mcp.no-args@v1
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NoArgs {}

/// Input for the generic `invoke_tool` method.
///
/// OSCAL subid: sch.software.cognitive-mcp.invoke-tool-input@v1
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InvokeToolInput {
    #[schemars(
        description = "Name of the tool to invoke; must exist in the cognitive_mcp ToolRegistry (e.g. cognitive_memory, search_blob_vectors)"
    )]
    pub tool_name: String,
    #[serde(default)]
    #[schemars(
        description = "Tool-specific arguments, passed verbatim to the tool executor. Shape is validated by the tool, not by the bridge arg gate."
    )]
    pub arguments: serde_json::Value,
}

/// Output for the generic `invoke_tool` method.
///
/// OSCAL subid: sch.software.cognitive-mcp.invoke-tool-output@v1
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InvokeToolOutput {
    pub success: bool,
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Output for GetConfig method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetConfigOutput {
    pub wg_interface: String,
    pub dbus_enabled: bool,
}

/// Output for SetConfig method.
///
/// Errors are reported through the bridge's D-Bus/gRPC error path.  A returned
/// value with `success: true` therefore means the configuration was applied and
/// the service reload was accepted.
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
    pub tools: Vec<CognitiveToolInfo>,
}

/// A discoverable tool exposed through the bridge-owned Cognitive registry.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CognitiveToolInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub namespace: String,
    /// JSON Schema accepted by this tool.  The generic `invoke_tool` door
    /// delegates validation to the tool, so discovery must expose the contract
    /// a caller needs to construct valid arguments.
    pub input_schema: serde_json::Value,
    /// Tool-owned schema revision, if the registry supplies one.
    pub schema_version: String,
    /// Runtime execution state: `live`, `mock`, or `disabled`.
    pub readiness: String,
    /// Human-readable reason when this is not a live tool.
    pub readiness_reason: Option<String>,
}

/// Output for RegisterTool method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RegisterToolOutput {
    pub success: bool,
    pub tool_name: String,
    pub target: String,
    pub persisted: bool,
}

/// Input for safe dynamic registration.  Registration creates only a
/// persisted declarative alias to an existing live tool; it never evaluates
/// caller-provided code, schema, provider credentials, or permissions.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RegisterToolInput {
    pub name: String,
    pub target: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Output for MemoryStore method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryStoreOutput {
    pub success: bool,
    pub id: String,
    pub namespace: String,
    pub key: String,
    pub semantic_mirrored: bool,
}

/// Output for MemoryRetrieve method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryRetrieveOutput {
    pub found: bool,
    pub id: Option<String>,
    pub namespace: String,
    pub key: String,
    pub value: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub access_count: Option<i64>,
    pub updated_at: Option<String>,
}

/// A memory entry returned from a bounded metadata query.  Value content is
/// intentionally omitted; callers use `memory_retrieve` for a point read.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryEntryInfo {
    pub id: String,
    pub namespace_id: String,
    pub key: String,
    pub tags: Vec<String>,
    pub access_count: i64,
    pub updated_at: String,
}

/// Output for MemoryQuery method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryQueryOutput {
    pub entries: Vec<MemoryEntryInfo>,
    pub count: usize,
}

/// Output for MemoryDelete method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryDeleteOutput {
    pub success: bool,
    pub namespace: String,
    pub key: String,
}

/// One registered memory namespace with its available ownership metadata.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryNamespaceInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub linked_task_id: Option<String>,
    pub linked_cron: Option<String>,
}

/// Output for MemoryListNamespaces method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemoryListNamespacesOutput {
    pub namespaces: Vec<MemoryNamespaceInfo>,
    pub count: usize,
}

/// One ranked code chunk from the RAG pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeSearchResult {
    pub retrieval_collection: String,
    pub score: f64,
    pub repo: String,
    pub file_path: String,
    pub language: String,
    pub file_type: String,
    pub symbols: Vec<String>,
    pub doc_comments: Vec<String>,
    pub imports: Vec<String>,
    pub tags: Vec<String>,
    pub is_test: bool,
    pub line_start: i64,
    pub line_end: i64,
    pub chunk_index: i64,
    pub total_chunks: i64,
    pub content: String,
}

/// Output for CodeSearch method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeSearchOutput {
    pub results: Vec<CodeSearchResult>,
    pub count: usize,
    pub collections: Vec<String>,
    pub retrieval_mode: String,
    pub rerank_enabled: bool,
    pub kiro_lsp_state_dir: String,
}

/// Output for CodeIndex method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeIndexOutput {
    pub success: bool,
    pub mode: String,
    pub collection: String,
    pub files_indexed: usize,
    pub chunks_created: usize,
    pub chunks_upserted: usize,
    pub chunks_skipped: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentUpsertInput {
    pub capability_id: String,
    pub category: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub status: Option<String>,
    pub schema_surface: Option<String>,
    pub required_capability: Option<String>,
    pub subid: Option<String>,
    pub dependencies: Option<Vec<String>>,
    pub tests: Option<Vec<String>>,
    pub live_verified: Option<bool>,
    pub deployed_commit: Option<String>,
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentUpsertOutput {
    pub capability_id: String,
    pub status: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentListInput {
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentListOutput {
    pub capabilities: Vec<serde_json::Value>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentCategoriesOutput {
    pub categories: Vec<DevelopmentCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentCategory {
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentSummaryInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentSummaryOutput {
    pub groups: Vec<serde_json::Value>,
    pub group_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentHistoryInput {
    pub capability_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentHistoryOutput {
    pub capability_id: String,
    pub history: Vec<serde_json::Value>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentVerificationInput {
    pub capability_id: String,
    pub status: Option<String>,
    pub live_verified: Option<bool>,
    pub commit: Option<String>,
    pub details: Option<String>,
    pub blocker: Option<String>,
    pub checks: Option<Vec<DevelopmentVerificationCheck>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentVerificationCheck {
    pub name: String,
    pub passed: bool,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DevelopmentVerificationOutput {
    pub capability_id: String,
    pub status: String,
    pub live_verified: bool,
    pub recorded_at: String,
}

/// Output for CodeContext method
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CodeContextOutput {
    pub results: Vec<CodeSearchResult>,
    pub count: usize,
    pub collections: Vec<String>,
    pub retrieval_mode: String,
    pub rerank_enabled: bool,
    pub kiro_lsp_state_dir: String,
    pub session_id: String,
    pub signals: serde_json::Value,
    pub retrieval_error: Option<String>,
}

/// Output for the provider-neutral grounded question method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AskQuestionOutput {
    pub answer: String,
    pub citations: Vec<Citation>,
    pub grounded: bool,
    pub conversation_id: String,
    pub namespace: String,
    /// Bounded, ephemeral session signals from the shared context engine.
    /// This is observable query state, not a mutable memory namespace.
    pub context_json: serde_json::Value,
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
    #[serde(default = "default_wg")]
    #[schemars(description = "WireGuard interface to read identity from", example = example_wg_interface(), extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.wg-interface@v1"))]
    pub wg_interface: String,
    #[serde(default = "default_true")]
    #[schemars(description = "Register on D-Bus as org.opdbus.CognitiveMcp", extend("x-oscal-subid" = "mut.software.plugin.cognitive-mcp.dbus-enabled@v1"))]
    pub dbus_enabled: bool,
    #[serde(default = "default_running")]
    #[schemars(description = "Whether the runit service is currently running", extend("readOnly" = true), extend("x-oscal-subid" = "obs.software.plugin.cognitive-mcp.running@v1"))]
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
    #[schemars(description = "Provider-neutral grounded question input for the configured notebook namespace", extend("readOnly" = true), extend("x-oscal-subid" = "obs.service.cognitive-mcp.question.ask@v1"))]
    pub ask_question_request: Option<AskQuestionInput>,
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

// Handler audit (all sealed schema methods below) — where each is actually backed:
// get_config              → cognitive_mcp.rs:current_config() (this file, StatePlugin)
// set_config              → cognitive_mcp.rs:apply_state() (this file, StatePlugin)
// get_health              → op-cognitive-mcp/grpc_service.rs:get_health (tonic RPC)
// list_tools              → op-cognitive-mcp/dbus_interface.rs:list_tools (D-Bus)
// register_tool           → op-cognitive-mcp/cognitive_tools.rs:RegisterToolTool
//                           (persisted allow-listed alias; never executable code)
// memory_store            → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_store
// memory_retrieve         → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_retrieve
// memory_query            → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_query
// memory_delete           → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_delete
// memory_list_namespaces  → op-cognitive-mcp/cognitive_tools.rs:MemoryTool::op_list_namespaces
// code_search             → op-cognitive-mcp/code_tools.rs:CodeSearchTool
// code_index              → op-cognitive-mcp/code_tools.rs:CodeIndexTool
// code_context            → op-cognitive-mcp/code_tools.rs:CodeContextTool
// ask_question            → op-cognitive-mcp/typed_tools.rs:TypedQueryTool (grounded local retrieval)
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
        method_decl_from_schemars_with_output::<NoArgs, GetConfigOutput>(
            "get_config",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.config.get@v1",
        ),
    );
    schema.methods.insert(
        "set_config".to_string(),
        method_decl_from_schemars_with_output::<SetConfigInput, SetConfigOutput>(
            "set_config",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.config.set@v1",
        ),
    );
    schema.methods.insert(
        "get_health".to_string(),
        method_decl_from_schemars_with_output::<NoArgs, GetHealthOutput>(
            "get_health",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.health@v1",
        ),
    );
    schema.methods.insert(
        "list_tools".to_string(),
        method_decl_from_schemars_with_output::<NoArgs, ListToolsOutput>(
            "list_tools",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.tool.list@v1",
        ),
    );
    schema.methods.insert(
        "register_tool".to_string(),
        method_decl_from_schemars_with_output::<RegisterToolInput, RegisterToolOutput>(
            "register_tool",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.tool.register@v1",
        ),
    );
    schema.methods.insert(
        "memory_store".to_string(),
        method_decl_from_schemars_with_output::<MemoryStoreInput, MemoryStoreOutput>(
            "memory_store",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.memory.store@v1",
        ),
    );
    schema.methods.insert(
        "memory_retrieve".to_string(),
        method_decl_from_schemars_with_output::<MemoryRetrieveInput, MemoryRetrieveOutput>(
            "memory_retrieve",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.memory.retrieve@v1",
        ),
    );
    schema.methods.insert(
        "memory_query".to_string(),
        method_decl_from_schemars_with_output::<MemoryQueryInput, MemoryQueryOutput>(
            "memory_query",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.memory.query@v1",
        ),
    );
    schema.methods.insert(
        "memory_delete".to_string(),
        method_decl_from_schemars_with_output::<MemoryDeleteInput, MemoryDeleteOutput>(
            "memory_delete",
            SideEffect::Mutation,
            true,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.memory.delete@v1",
        ),
    );
    schema.methods.insert(
        "memory_list_namespaces".to_string(),
        method_decl_from_schemars_with_output::<
            MemoryListNamespacesInput,
            MemoryListNamespacesOutput,
        >(
            "memory_list_namespaces",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.memory.namespace.list@v1",
        ),
    );
    schema.methods.insert(
        "code_search".to_string(),
        method_decl_from_schemars_with_output::<CodeSearchInput, CodeSearchOutput>(
            "code_search",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.code-rag.search@v1",
        ),
    );
    schema.methods.insert(
        "code_index".to_string(),
        method_decl_from_schemars_with_output::<CodeIndexInput, CodeIndexOutput>(
            "code_index",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.code-rag.index@v1",
        ),
    );
    schema.methods.insert(
        "code_context".to_string(),
        method_decl_from_schemars_with_output::<CodeContextInput, CodeContextOutput>(
            "code_context",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.code-context.get@v1",
        ),
    );
    schema.methods.insert(
        "ask_question".to_string(),
        method_decl_from_schemars_with_output::<AskQuestionInput, AskQuestionOutput>(
            "ask_question",
            SideEffect::Read,
            false,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.question.ask@v1",
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
    schema.methods.insert(
        "development_upsert".to_string(),
        method_decl_from_schemars_with_output::<DevelopmentUpsertInput, DevelopmentUpsertOutput>(
            "development_upsert",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.development.upsert@v1",
        ),
    );
    schema.methods.insert(
        "development_list".to_string(),
        method_decl_from_schemars_with_output::<DevelopmentListInput, DevelopmentListOutput>(
            "development_list",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.development.list@v1",
        ),
    );
    schema.methods.insert(
        "development_categories".to_string(),
        method_decl_from_schemars_with_output::<(), DevelopmentCategoriesOutput>(
            "development_categories",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.development.categories@v1",
        ),
    );
    schema.methods.insert(
        "development_summary".to_string(),
        method_decl_from_schemars_with_output::<DevelopmentSummaryInput, DevelopmentSummaryOutput>(
            "development_summary",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.development.summary@v1",
        ),
    );
    schema.methods.insert(
        "development_history".to_string(),
        method_decl_from_schemars_with_output::<DevelopmentHistoryInput, DevelopmentHistoryOutput>(
            "development_history",
            SideEffect::Read,
            true,
            "cognitive_mcp.read",
            "obs.service.cognitive-mcp.development.history@v1",
        ),
    );
    schema.methods.insert(
        "development_record_verification".to_string(),
        method_decl_from_schemars_with_output::<
            DevelopmentVerificationInput,
            DevelopmentVerificationOutput,
        >(
            "development_record_verification",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.development.verify@v1",
        ),
    );
    // Generic tool invocation. The 15 methods above are fixed at blob-seal time, but
    // the ToolRegistry is populated at runtime (406 tools today), so a per-tool method
    // is impossible without re-sealing on every registration. `invoke_tool` is the one
    // typed door to the whole registry: `tool_name` selects the tool and `arguments` is
    // passed verbatim for the tool itself to validate.
    schema.methods.insert(
        "invoke_tool".to_string(),
        method_decl_from_schemars_with_output::<InvokeToolInput, InvokeToolOutput>(
            "invoke_tool",
            SideEffect::Mutation,
            false,
            "cognitive_mcp.invoke",
            "mut.service.cognitive-mcp.tool.invoke@v1",
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

    #[test]
    fn development_methods_are_sealed_with_expected_contracts() {
        let schema = cognitive_mcp_schema();
        let expected = [
            (
                "development_upsert",
                SideEffect::Mutation,
                "cognitive_mcp.invoke",
                "mut.service.cognitive-mcp.development.upsert@v1",
            ),
            (
                "development_list",
                SideEffect::Read,
                "cognitive_mcp.read",
                "obs.service.cognitive-mcp.development.list@v1",
            ),
            (
                "development_categories",
                SideEffect::Read,
                "cognitive_mcp.read",
                "obs.service.cognitive-mcp.development.categories@v1",
            ),
            (
                "development_summary",
                SideEffect::Read,
                "cognitive_mcp.read",
                "obs.service.cognitive-mcp.development.summary@v1",
            ),
            (
                "development_history",
                SideEffect::Read,
                "cognitive_mcp.read",
                "obs.service.cognitive-mcp.development.history@v1",
            ),
            (
                "development_record_verification",
                SideEffect::Mutation,
                "cognitive_mcp.invoke",
                "mut.service.cognitive-mcp.development.verify@v1",
            ),
        ];
        for (name, effect, capability, subid) in expected {
            let method = &schema.methods[name];
            assert_eq!(method.side_effect, effect, "{name} side effect");
            assert_eq!(
                method.required_capability.as_deref(),
                Some(capability),
                "{name} capability"
            );
            assert_eq!(method.subid, subid, "{name} subid");
        }
    }

    #[test]
    fn grounded_question_contract_is_provider_neutral() {
        let schema = cognitive_mcp_schema();
        let method = schema
            .methods
            .get("ask_question")
            .expect("ask_question must remain in the Cognitive MCP schema");

        assert_eq!(method.side_effect, SideEffect::Read);
        assert_eq!(
            method.required_capability.as_deref(),
            Some("cognitive_mcp.read")
        );
        assert_eq!(method.subid, "obs.service.cognitive-mcp.question.ask@v1");
        assert!(
            !schema.methods.contains_key("gemini_query"),
            "Cognitive MCP must not expose a provider-specific question method"
        );
    }

    #[test]
    fn lifecycle_control_uses_the_canonical_runit_dbus_contract() {
        assert_eq!(RUNIT_SYSTEMCTL_SERVICE, "org.opdbus.v1.Runit.Systemctl");
        assert_eq!(
            RUNIT_SYSTEMCTL_PATH,
            "/org/opdbus/v1/plugins/runit/systemctl"
        );
        assert_eq!(RUNIT_SYSTEMCTL_INTERFACE, RUNIT_SYSTEMCTL_SERVICE);
    }

    #[test]
    fn set_config_is_a_complete_typed_mutation_contract() {
        let schema = cognitive_mcp_schema();
        let method = schema
            .methods
            .get("set_config")
            .expect("set_config must remain in the Cognitive MCP schema");
        let args = serde_json::to_value(&method.args).expect("serialize set_config args schema");
        let properties = args["properties"]
            .as_object()
            .expect("set_config args properties");
        let required = args["required"]
            .as_array()
            .expect("set_config required properties");

        assert!(properties.contains_key("wg_interface"));
        assert!(properties.contains_key("dbus_enabled"));
        assert!(required.iter().any(|field| field == "wg_interface"));
        assert!(required.iter().any(|field| field == "dbus_enabled"));
        assert_eq!(method.side_effect, SideEffect::Mutation);
        assert_eq!(
            method.required_capability.as_deref(),
            Some("cognitive_mcp.invoke")
        );
    }

    #[test]
    fn memory_and_code_methods_expose_the_arguments_their_live_tools_require() {
        let schema = cognitive_mcp_schema();
        let expected_fields = [
            ("memory_store", "namespace"),
            ("memory_retrieve", "key"),
            ("memory_query", "limit"),
            ("memory_delete", "key"),
            ("memory_list_namespaces", "namespace_kind"),
            ("code_search", "query"),
            ("code_index", "mode"),
            ("code_context", "session_id"),
        ];

        for (method_name, required_field) in expected_fields {
            let method = schema
                .methods
                .get(method_name)
                .unwrap_or_else(|| panic!("missing method {method_name}"));
            let args = serde_json::to_value(&method.args)
                .unwrap_or_else(|error| panic!("serialize {method_name} args: {error}"));
            let properties = args["properties"]
                .as_object()
                .unwrap_or_else(|| panic!("{method_name} must accept an object"));
            assert!(
                properties.contains_key(required_field),
                "{method_name} is missing its live tool argument {required_field}"
            );
            assert_ne!(
                args.get("type").and_then(JVal::as_str),
                Some("null"),
                "{method_name} must not expose the old unit/null request contract"
            );
        }
    }

    #[test]
    fn list_tools_output_exposes_each_tool_invocation_contract() {
        let schema = serde_json::to_value(schemars::schema_for!(ListToolsOutput))
            .expect("list_tools output schema");
        let definitions = schema
            .get("$defs")
            .or_else(|| schema.get("definitions"))
            .and_then(JVal::as_object)
            .expect("tool info definition");
        let fields = definitions["CognitiveToolInfo"]["properties"]
            .as_object()
            .expect("tool info properties");

        for field in [
            "name",
            "description",
            "category",
            "tags",
            "namespace",
            "input_schema",
            "schema_version",
            "readiness",
            "readiness_reason",
        ] {
            assert!(
                fields.contains_key(field),
                "missing tool catalog field '{field}'"
            );
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(CognitiveMcpPlugin::new()))
}
